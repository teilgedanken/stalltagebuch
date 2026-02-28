use crate::error::AppError;
use rusqlite::Connection;

/// Liefert stabile device_id (erzeugt & speichert falls fehlend)
pub fn get_device_id(conn: &Connection) -> Result<String, AppError> {
    use crate::services::sync_service;
    if let Some(mut settings) = sync_service::load_sync_settings(conn)? {
        if let Some(id) = &settings.device_id {
            return Ok(id.clone());
        }
        let new_id = uuid::Uuid::new_v4().to_string();
        settings.device_id = Some(new_id.clone());
        sync_service::save_sync_settings(conn, &settings)?;
        Ok(new_id)
    } else {
        // Fallback: ephemeral ID (Settings noch nicht konfiguriert)
        Ok(uuid::Uuid::new_v4().to_string())
    }
}

/// Uploads a batch of operations to sync/ops/<device>/<YYYYMM>/<ULID>.ndjson
///
/// This is a minimal skeleton for the new multi-master sync.
/// If sync is not configured or disabled, this function returns Ok() without error.
pub async fn upload_ops_batch(
    conn: &Connection,
    ops: Vec<crate::services::crdt_service::Operation>,
) -> Result<(), AppError> {
    use crate::services::{sync_paths, sync_service};

    if ops.is_empty() {
        return Ok(());
    }

    // If sync is not configured, just skip upload (app works locally)
    let settings = match sync_service::load_sync_settings(conn)? {
        Some(s) => s,
        None => {
            log::debug!("Sync not configured, skipping operation upload");
            return Ok(());
        }
    };

    // If sync is disabled, skip upload
    if !settings.enabled {
        log::debug!("Sync disabled, skipping operation upload");
        return Ok(());
    }

    let device_id = get_device_id(conn)?;
    let year_month = sync_paths::current_year_month();
    let ulid = ulid::Ulid::new().to_string();

    // Build NDJSON content
    let mut ndjson_lines = Vec::new();
    for op in &ops {
        let line = serde_json::to_string(op)
            .map_err(|e| AppError::Other(format!("JSON serialize failed: {}", e)))?;
        ndjson_lines.push(line);
    }
    let ndjson_content = ndjson_lines.join("\n") + "\n";

    // Build remote path
    let ops_dir = sync_paths::ops_path(&device_id, &year_month);
    let filename = format!("{}.ndjson", ulid);
    let full_path = format!(
        "{}/{}/{}",
        settings.remote_path.trim_end_matches('/'),
        ops_dir,
        filename
    );

    // Create WebDAV client
    let webdav_url = format!(
        "{}/remote.php/dav/files/{}",
        settings.server_url.trim_end_matches('/'),
        settings.username
    );

    let client = reqwest_dav::ClientBuilder::new()
        .set_host(webdav_url)
        .set_auth(reqwest_dav::Auth::Basic(
            settings.username.clone(),
            settings.app_password.clone(),
        ))
        .build()
        .map_err(|e| AppError::Other(format!("WebDAV client error: {:?}", e)))?;

    // Create directories if needed (WebDAV cannot create nested collections in one call)
    let base = settings.remote_path.trim_end_matches('/');
    let sync_base = format!("{}/sync", base);
    let ops_base = format!("{}/ops", sync_base);
    let device_base = format!("{}/{}", ops_base, device_id);
    let month_base = format!("{}/{}", device_base, year_month);

    // Try to create each level; ignore errors like 405 Method Not Allowed or 409 Conflict
    for path in [&sync_base, &ops_base, &device_base, &month_base] {
        if let Err(e) = client.mkcol(path).await {
            // Best-effort: path may already exist; only log
            log::debug!("MKCOL '{}' note: {:?}", path, e);
        }
    }

    // Upload (atomic create via If-None-Match not directly supported, use put)
    client
        .put(&full_path, ndjson_content.into_bytes())
        .await
        .map_err(|e| AppError::Other(format!("Upload ops batch failed: {:?}", e)))?;

    log::info!(
        "Uploaded ops batch: {} operations to {}",
        ops.len(),
        full_path
    );

    Ok(())
}

/// Counts how many photos are pending upload (sync_status='local_only')
pub fn count_pending_photos(conn: &Connection) -> Result<usize, AppError> {
    use crate::services::sync_service;

    // Get sync settings to create upload service
    let settings = match sync_service::load_sync_settings(conn)? {
        Some(s) if s.enabled => s,
        _ => return Ok(0),
    };

    let upload_config = photo_gallery::PhotoUploadConfig {
        server_url: settings.server_url,
        username: settings.username,
        password: settings.app_password,
        remote_path: settings.remote_path,
        storage_path: get_storage_path(),
    };

    let upload_service = photo_gallery::PhotoUploadService::new(upload_config);
    upload_service
        .count_pending_photos(conn)
        .map_err(|e| AppError::Other(format!("Photo upload error: {}", e)))
}

/// Uploads binary photo files to sync/photos/ with the original image only
///
/// Only uploads photos with sync_status='local_only'. Thumbnails are no
/// longer uploaded — they are generated locally on the receiving side.
/// Uses JoinSet for parallel uploads (max 3 concurrent photos).
/// If sync is not configured or disabled, this function returns Ok(0) without error.
pub async fn upload_photos_batch(conn: &Connection) -> Result<usize, AppError> {
    use crate::services::sync_service;

    // If sync is not configured, just skip upload (app works locally)
    let settings = match sync_service::load_sync_settings(conn)? {
        Some(s) => s,
        None => {
            log::debug!("Sync not configured, skipping photo upload");
            return Ok(0);
        }
    };

    // If sync is disabled, skip upload
    if !settings.enabled {
        log::debug!("Sync disabled, skipping photo upload");
        return Ok(0);
    }

    // Use photo-gallery upload service
    let upload_config = photo_gallery::PhotoUploadConfig {
        server_url: settings.server_url,
        username: settings.username,
        password: settings.app_password,
        remote_path: settings.remote_path,
        storage_path: get_storage_path(),
    };

    let upload_service = photo_gallery::PhotoUploadService::new(upload_config);
    upload_service
        .upload_photos_batch(conn)
        .await
        .map_err(|e| AppError::Other(format!("Photo upload error: {}", e)))
}

/// Get the storage path based on platform
fn get_storage_path() -> String {
    #[cfg(target_os = "android")]
    {
        "/storage/emulated/0/Android/data/de.teilgedanken.stalltagebuch/files/photos".to_string()
    }

    #[cfg(not(target_os = "android"))]
    {
        "./photos".to_string()
    }
}
