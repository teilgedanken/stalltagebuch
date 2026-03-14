use crate::error::AppError;
use crate::services::upload_service;
use chrono::Utc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

static SYNC_LOG: OnceLock<Arc<Mutex<Vec<SyncLogEntry>>>> = OnceLock::new();
static SYNC_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Global progress channel for photo uploads: (current, total)
static UPLOAD_PROGRESS: OnceLock<watch::Sender<(usize, usize)>> = OnceLock::new();

/// In-memory session log entry (volatile – lost on app restart)
#[derive(Debug, Clone, PartialEq)]
pub struct SyncLogEntry {
    pub ts_ms: i64,
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

#[derive(Debug, Clone)]
pub struct SyncStats {
    pub photos_uploaded: usize,
}

/// Performs one complete photo sync cycle to Nextcloud.
async fn perform_sync_cycle() -> Result<SyncStats, AppError> {
    // Upload local photos that are missing remotely.
    let photos_uploaded = upload_service::upload_photos_batch_with_progress(|current, total| {
        set_upload_progress(current, total);
    })
    .await
    .unwrap_or_else(|e| {
        if matches!(e, AppError::Validation(_)) {
            log::debug!("Photo upload skipped: {}", e);
        } else {
            log::error!("Photo upload failed: {}", e);
        }
        0
    });
    set_upload_progress(0, 0);

    let stats = SyncStats { photos_uploaded };

    // Log entry for session log
    append_log(SyncLogEntry {
        ts_ms: Utc::now().timestamp_millis(),
        photos_uploaded,
    });

    Ok(stats)
}

/// Triggers an immediate photo sync.
pub async fn sync_now() -> Result<SyncStats, AppError> {
    if SYNC_IN_PROGRESS.swap(true, Ordering::SeqCst) {
        return Err(AppError::Validation("sync is already running".to_string()));
    }

    let result = perform_sync_cycle().await;
    SYNC_IN_PROGRESS.store(false, Ordering::SeqCst);
    result
}
