use crate::error::AppError;
use crate::services::{download_service, upload_service};
use chrono::Utc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    // TODO: Check if sync is configured and enabled via SpacetimeDB settings table

    // Phase 1: Download remote changes first (new multi-master sync)
    let ops_downloaded = download_service::download_and_merge_ops().await?;

    // Phase 2: Upload pending local data
    upload_pending_local_data().await.unwrap_or_else(|e| {
        log::error!("Upload pending local data failed: {}", e);
    });

    // Phase 3: Upload local photos that are missing remotely
    let photos_uploaded = upload_service::upload_photos_batch()
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
///
/// NOTE: Stub implementation - full functionality will be implemented with SpacetimeDB
async fn upload_pending_local_data() -> Result<(), AppError> {
    // TODO: Implement real sync upload logic with SpacetimeDB
    log::debug!("Upload pending local data stub - SpacetimeDB sync not yet fully implemented");
    Ok(())
}

/// Triggers an immediate sync (in addition to scheduled background syncs)
pub async fn sync_now() -> Result<SyncStats, AppError> {
    perform_sync_cycle().await
}
