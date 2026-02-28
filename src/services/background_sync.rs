use crate::database;
use crate::error::AppError;
use crate::services::{download_service, sync_service, upload_service};
use chrono::Utc;
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::watch;

/// Background sync configuration (reduced per requirement)
const SYNC_INTERVAL_SECONDS: u64 = 30; // 30 seconds
const RETRY_DELAY_SECONDS: u64 = 60; // 1 minute on error

/// Global flag to control background sync
static SYNC_ENABLED: AtomicBool = AtomicBool::new(false);
static NEXT_SYNC_AT: AtomicU64 = AtomicU64::new(0); // epoch ms of next planned sync
static SYNC_LOG: OnceLock<Arc<Mutex<Vec<SyncLogEntry>>>> = OnceLock::new();

/// Global progress channel for photo uploads: (current, total)
static UPLOAD_PROGRESS: OnceLock<watch::Sender<(usize, usize)>> = OnceLock::new();

/// In-memory session log entry (volatile – lost on app restart)
#[derive(Debug, Clone, PartialEq)]
pub struct SyncLogEntry {
    pub ts_ms: i64,
    pub operations_downloaded: usize,
    pub photos_uploaded: usize,
}

fn log_store() -> Arc<Mutex<Vec<SyncLogEntry>>> {
    SYNC_LOG
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}

fn append_log(entry: SyncLogEntry) {
    if let Ok(mut guard) = log_store().lock() {
        guard.push(entry);
        // Optional: cap size
        let len = guard.len();
        if len > 500 {
            let remove = len - 500;
            guard.drain(0..remove);
        }
    }
}

pub fn get_sync_log() -> Vec<SyncLogEntry> {
    if let Ok(guard) = log_store().lock() {
        guard.clone()
    } else {
        Vec::new()
    }
}

pub fn next_sync_eta_seconds() -> Option<u64> {
    if !SYNC_ENABLED.load(Ordering::SeqCst) {
        return None;
    }
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let target = NEXT_SYNC_AT.load(Ordering::SeqCst);
    if target == 0 || target <= now_ms {
        Some(0)
    } else {
        Some((target - now_ms) / 1000)
    }
}

pub fn sync_interval_seconds() -> u64 {
    SYNC_INTERVAL_SECONDS
}

/// Subscribe to upload progress updates (current, total)
pub fn subscribe_upload_progress() -> watch::Receiver<(usize, usize)> {
    UPLOAD_PROGRESS
        .get_or_init(|| {
            let (tx, _rx) = watch::channel((0, 0));
            tx
        })
        .subscribe()
}

/// Internal helper to update upload progress
fn set_upload_progress(current: usize, total: usize) {
    if let Some(tx) = UPLOAD_PROGRESS.get() {
        let _ = tx.send((current, total));
    } else {
        // Initialize if not yet done
        let tx = UPLOAD_PROGRESS.get_or_init(|| {
            let (tx, _rx) = watch::channel((current, total));
            tx
        });
        let _ = tx.send((current, total));
    }
}

/// Starts the background sync loop
///
/// This will continuously sync in the background at regular intervals.
/// Call `stop_background_sync()` to stop it.
pub fn start_background_sync() {
    if SYNC_ENABLED.swap(true, Ordering::SeqCst) {
        log::warn!("Background sync already running");
        return;
    }

    log::info!(
        "Starting background sync with {} second interval",
        SYNC_INTERVAL_SECONDS
    );

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        while SYNC_ENABLED.load(Ordering::SeqCst) {
            runtime.block_on(async {
                // Perform sync cycle
                match perform_sync_cycle().await {
                    Ok(stats) => {
                        log::info!("Background sync completed: {:?}", stats);
                    }
                    Err(e) => {
                        log::error!("Background sync error: {}", e);
                        // Wait shorter time before retry on error
                        // set next attempt ETA
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;
                        NEXT_SYNC_AT.store(now_ms + RETRY_DELAY_SECONDS * 1000, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_secs(RETRY_DELAY_SECONDS)).await;
                        return;
                    }
                }

                // Wait for next sync interval
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                NEXT_SYNC_AT.store(now_ms + SYNC_INTERVAL_SECONDS * 1000, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_secs(SYNC_INTERVAL_SECONDS)).await;
            });
        }

        log::info!("Background sync stopped");
    });
}

/// Stops the background sync loop
pub fn stop_background_sync() {
    if SYNC_ENABLED.swap(false, Ordering::SeqCst) {
        log::info!("Stopping background sync");
    }
}

/// Checks if background sync is running
pub fn is_background_sync_running() -> bool {
    SYNC_ENABLED.load(Ordering::SeqCst)
}

#[derive(Debug, Clone)]
pub struct SyncStats {
    pub operations_downloaded: usize,
    pub photos_uploaded: usize,
}

/// Performs one complete sync cycle: download remote changes first, then upload local changes
///
/// Download-first strategy ensures we get the latest remote state before uploading,
/// reducing conflicts and ensuring we're working with up-to-date data.
async fn perform_sync_cycle() -> Result<SyncStats, AppError> {
    let conn = database::init_database()?;

    // Check if sync is configured and enabled
    let settings = sync_service::load_sync_settings(&conn)?
        .ok_or_else(|| AppError::NotFound("Sync not configured".to_string()))?;

    if !settings.enabled {
        return Err(AppError::Validation("Sync disabled".to_string()));
    }

    // Phase 1: Download remote changes first (new multi-master sync)
    let ops_downloaded = download_service::download_and_merge_ops(&conn).await?;

    // Phase 2: Upload pending local data (only once, if initial upload not done yet)
    if !settings.initial_upload_done {
        upload_pending_local_data(&conn).await.unwrap_or_else(|e| {
            log::error!("Upload pending local data failed: {}", e);
        });
        // Mark as done
        sync_service::set_initial_upload_done(&conn).unwrap_or_else(|e| {
            log::error!("Failed to set initial_upload_done flag: {}", e);
        });
    }

    // Phase 3: Upload local photos that are missing remotely
    let photos_uploaded = upload_service::upload_photos_batch(&conn)
        .await
        .unwrap_or_else(|e| {
            log::error!("Photo upload failed: {}", e);
            0
        });

    let stats = SyncStats {
        operations_downloaded: ops_downloaded,
        photos_uploaded,
    };

    // Log entry for session log
    append_log(SyncLogEntry {
        ts_ms: Utc::now().timestamp_millis(),
        operations_downloaded: ops_downloaded,
        photos_uploaded,
    });

    Ok(stats)
}

/// Uploads all local data that hasn't been synchronized yet
///
/// This is useful when sync is configured after local data has been created.
/// It reads all local entities and creates CRDT operations for them.
/// Now with atomic validation: first uploads all photos, then operations.
async fn upload_pending_local_data(conn: &Connection) -> Result<(), AppError> {
    use crate::services::{crdt_service, upload_service};

    let device_id = upload_service::get_device_id(conn)?;

    // Phase 1: Upload all photos first (with progress tracking)
    log::info!("Phase 1: Uploading photos...");
    let total_photos = upload_service::count_pending_photos(conn)?;
    set_upload_progress(0, total_photos);

    let mut uploaded_photos = 0;
    loop {
        let uploaded = upload_service::upload_photos_batch(conn).await?;
        if uploaded == 0 {
            break; // No more photos to upload
        }
        uploaded_photos += uploaded;
        set_upload_progress(uploaded_photos, total_photos);
    }

    // Reset progress after completion
    set_upload_progress(0, 0);
    log::info!("Phase 1 complete: {} photos uploaded", uploaded_photos);

    // Phase 2: Upload metadata operations (atomically after all photos done)
    let mut all_ops = Vec::new();

    // Upload all quails
    let mut stmt = conn.prepare(
        "SELECT uuid, name, gender, ring_color, profile_photo FROM quails WHERE deleted = 0",
    )?;
    let quails = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for quail in quails {
        let (uuid, name, gender, ring_color, profile_photo) = quail?;

        all_ops.push(crdt_service::Operation::new(
            "quail".to_string(),
            uuid.clone(),
            device_id.clone(),
            crdt_service::CrdtOp::LwwSet {
                field: "name".to_string(),
                value: serde_json::Value::String(name),
            },
        ));

        all_ops.push(crdt_service::Operation::new(
            "quail".to_string(),
            uuid.clone(),
            device_id.clone(),
            crdt_service::CrdtOp::LwwSet {
                field: "gender".to_string(),
                value: serde_json::Value::String(gender),
            },
        ));

        if let Some(color) = ring_color {
            all_ops.push(crdt_service::Operation::new(
                "quail".to_string(),
                uuid.clone(),
                device_id.clone(),
                crdt_service::CrdtOp::LwwSet {
                    field: "ring_color".to_string(),
                    value: serde_json::Value::String(color),
                },
            ));
        }

        if let Some(photo) = profile_photo {
            all_ops.push(crdt_service::Operation::new(
                "quail".to_string(),
                uuid.clone(),
                device_id.clone(),
                crdt_service::CrdtOp::LwwSet {
                    field: "profile_photo".to_string(),
                    value: serde_json::Value::String(photo),
                },
            ));
        }
    }

    // Upload all events
    let mut stmt = conn.prepare(
        "SELECT uuid, quail_id, event_type, event_date, notes FROM quail_events WHERE deleted = 0",
    )?;
    let events = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for event in events {
        let (uuid, quail_id, event_type, event_date, notes) = event?;

        all_ops.push(crdt_service::Operation::new(
            "event".to_string(),
            uuid.clone(),
            device_id.clone(),
            crdt_service::CrdtOp::LwwSet {
                field: "quail_id".to_string(),
                value: serde_json::Value::String(quail_id),
            },
        ));

        all_ops.push(crdt_service::Operation::new(
            "event".to_string(),
            uuid.clone(),
            device_id.clone(),
            crdt_service::CrdtOp::LwwSet {
                field: "event_type".to_string(),
                value: serde_json::Value::String(event_type),
            },
        ));

        all_ops.push(crdt_service::Operation::new(
            "event".to_string(),
            uuid.clone(),
            device_id.clone(),
            crdt_service::CrdtOp::LwwSet {
                field: "event_date".to_string(),
                value: serde_json::Value::String(event_date),
            },
        ));

        if let Some(notes_text) = notes {
            all_ops.push(crdt_service::Operation::new(
                "event".to_string(),
                uuid.clone(),
                device_id.clone(),
                crdt_service::CrdtOp::LwwSet {
                    field: "notes".to_string(),
                    value: serde_json::Value::String(notes_text),
                },
            ));
        }
    }

    // Upload all photos
    let mut stmt = conn.prepare("SELECT uuid, quail_id, event_id, COALESCE(relative_path, path) as rel_path, thumbnail_path FROM photos WHERE deleted = 0")?;
    let photos = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    for photo in photos {
        let (uuid, quail_id, event_id, rel_path, thumbnail_path) = photo?;

        all_ops.push(crdt_service::Operation::new(
            "photo".to_string(),
            uuid.clone(),
            device_id.clone(),
            crdt_service::CrdtOp::LwwSet {
                field: "relative_path".to_string(),
                value: serde_json::Value::String(rel_path),
            },
        ));

        if let Some(qid) = quail_id {
            all_ops.push(crdt_service::Operation::new(
                "photo".to_string(),
                uuid.clone(),
                device_id.clone(),
                crdt_service::CrdtOp::LwwSet {
                    field: "quail_id".to_string(),
                    value: serde_json::Value::String(qid),
                },
            ));
        }

        if let Some(eid) = event_id {
            all_ops.push(crdt_service::Operation::new(
                "photo".to_string(),
                uuid.clone(),
                device_id.clone(),
                crdt_service::CrdtOp::LwwSet {
                    field: "event_id".to_string(),
                    value: serde_json::Value::String(eid),
                },
            ));
        }

        if let Some(thumb) = thumbnail_path {
            all_ops.push(crdt_service::Operation::new(
                "photo".to_string(),
                uuid.clone(),
                device_id.clone(),
                crdt_service::CrdtOp::LwwSet {
                    field: "relative_thumb".to_string(),
                    value: serde_json::Value::String(thumb),
                },
            ));
        }
    }

    // Upload all egg records
    let mut stmt =
        conn.prepare("SELECT uuid, record_date, total_eggs FROM egg_records WHERE deleted = 0")?;
    let eggs = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)?,
        ))
    })?;

    for egg in eggs {
        let (uuid, record_date, total_eggs) = egg?;

        all_ops.push(crdt_service::Operation::new(
            "egg".to_string(),
            uuid.clone(),
            device_id.clone(),
            crdt_service::CrdtOp::LwwSet {
                field: "record_date".to_string(),
                value: serde_json::Value::String(record_date),
            },
        ));

        all_ops.push(crdt_service::Operation::new(
            "egg".to_string(),
            uuid.clone(),
            device_id.clone(),
            crdt_service::CrdtOp::LwwSet {
                field: "total_eggs".to_string(),
                value: serde_json::Value::Number(total_eggs.into()),
            },
        ));
    }

    // Upload in batch
    if !all_ops.is_empty() {
        upload_service::upload_ops_batch(conn, all_ops).await?;
        log::info!("Uploaded pending local data successfully");
    }

    Ok(())
}

/// Triggers an immediate sync (in addition to scheduled background syncs)
pub async fn sync_now() -> Result<SyncStats, AppError> {
    perform_sync_cycle().await
}
