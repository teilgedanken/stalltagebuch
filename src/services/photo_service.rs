use crate::error::AppError;
use photo_gallery::{
    create_thumbnails, Photo, PhotoGalleryConfig, PhotoGalleryService, PhotoResult, PhotoSize,
    PhotoSyncConfig, PhotoSyncService,
};
use rusqlite::{Connection, OptionalExtension};
use std::sync::OnceLock;
use uuid::Uuid;

// Global photo gallery service
static PHOTO_SERVICE: OnceLock<PhotoGalleryService> = OnceLock::new();

/// Initialize the photo gallery service
pub fn init_photo_service() -> &'static PhotoGalleryService {
    PHOTO_SERVICE.get_or_init(|| {
        let config = PhotoGalleryConfig {
            storage_path: get_storage_path(),
            enable_thumbnails: true,
            thumbnail_small_size: 128,
            thumbnail_medium_size: 512,
        };
        PhotoGalleryService::new(config)
    })
}

/// Get the storage path based on platform
pub fn get_storage_path() -> String {
    #[cfg(target_os = "android")]
    {
        "/storage/emulated/0/Android/data/de.teilgedanken.stalltagebuch/files/photos".to_string()
    }

    #[cfg(not(target_os = "android"))]
    {
        "./photos".to_string()
    }
}

/// Returns the absolute path to a photo (for UI display)
pub fn get_absolute_photo_path(relative_path: &str) -> String {
    init_photo_service().get_absolute_photo_path(relative_path)
}

/// Adapter function to convert PhotoGalleryError to AppError
fn convert_error(e: photo_gallery::PhotoGalleryError) -> AppError {
    match e {
        photo_gallery::PhotoGalleryError::DatabaseError(e) => AppError::Database(e),
        photo_gallery::PhotoGalleryError::NotFound(msg) => AppError::NotFound(msg),
        // Map IO-related errors from the photo_gallery crate to the app-wide Filesystem error
        photo_gallery::PhotoGalleryError::IoError(e) => AppError::Filesystem(e),
        photo_gallery::PhotoGalleryError::ThumbnailError(e) => {
            AppError::Other(format!("Thumbnail error: {}", e))
        }
        photo_gallery::PhotoGalleryError::Other(msg) => AppError::Other(msg),
    }
}

pub async fn add_quail_photo(
    conn: &Connection,
    quail_id: Uuid,
    path: String,
    _thumbnail_path: Option<String>,
) -> Result<Uuid, AppError> {
    log::debug!("=== add_quail_photo called ===");
    log::debug!("Quail ID: {}, Path: {}", quail_id, path);

    let service = init_photo_service();

    // Ensure the quail exists before attaching a photo.
    let exists: i32 = conn.query_row(
        "SELECT COUNT(*) FROM quails WHERE uuid = ?1 AND deleted = 0",
        rusqlite::params![quail_id.to_string()],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::NotFound("Wachtel nicht gefunden".into()));
    }

    // Add photo using photo-gallery service
    let photo_uuid = service
        .add_photo_to_collection(conn, &quail_id, path)
        .await
        .map_err(convert_error)?;

    // Get the relative path for operation capture
    let relative_path: String = conn.query_row(
        "SELECT relative_path FROM photos WHERE uuid = ?1",
        rusqlite::params![photo_uuid.to_string()],
        |row| row.get(0),
    )?;

    let thumbnail_small: Option<String> = conn.query_row(
        "SELECT thumbnail_small_path FROM photos WHERE uuid = ?1",
        rusqlite::params![photo_uuid.to_string()],
        |row| row.get(0),
    )?;

    // Capture CRDT operation after photo is created
    crate::services::operation_capture::capture_photo_create(
        conn,
        &photo_uuid.to_string(),
        Some(&quail_id.to_string()),
        None,
        &relative_path,
        thumbnail_small.as_deref(),
    )
    .await?;

    Ok(photo_uuid)
}

pub async fn add_event_photo(
    conn: &Connection,
    event_id: Uuid,
    path: String,
    _thumbnail_path: Option<String>,
) -> Result<Uuid, AppError> {
    let service = init_photo_service();

    // Ensure the event exists before attaching a photo.
    let exists: i32 = conn.query_row(
        "SELECT COUNT(*) FROM quail_events WHERE uuid = ?1 AND deleted = 0",
        rusqlite::params![event_id.to_string()],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::NotFound("Event nicht gefunden".into()));
    }

    // Add photo using photo-gallery service
    let photo_uuid = service
        .add_photo_to_collection(conn, &event_id, path)
        .await
        .map_err(convert_error)?;

    // Get the relative path for operation capture
    let relative_path: String = conn.query_row(
        "SELECT relative_path FROM photos WHERE uuid = ?1",
        rusqlite::params![photo_uuid.to_string()],
        |row| row.get(0),
    )?;

    let thumbnail_small: Option<String> = conn.query_row(
        "SELECT thumbnail_small_path FROM photos WHERE uuid = ?1",
        rusqlite::params![photo_uuid.to_string()],
        |row| row.get(0),
    )?;

    // Capture CRDT operation after photo is created
    crate::services::operation_capture::capture_photo_create(
        conn,
        &photo_uuid.to_string(),
        None,
        Some(&event_id.to_string()),
        &relative_path,
        thumbnail_small.as_deref(),
    )
    .await?;

    Ok(photo_uuid)
}

pub fn get_profile_photo(conn: &Connection, quail_uuid: &Uuid) -> Result<Option<Photo>, AppError> {
    let service = init_photo_service();
    service
        .get_profile_photo(conn, quail_uuid)
        .map_err(convert_error)
}

pub async fn set_profile_photo(
    conn: &Connection,
    quail_uuid: &Uuid,
    photo_uuid: &Uuid,
) -> Result<(), AppError> {
    use rusqlite::{params, OptionalExtension};

    // Verify photo belongs to quail
    let photo_quail: Option<String> = conn
        .query_row(
            "SELECT quail_id FROM photos WHERE uuid = ?1",
            params![photo_uuid.to_string()],
            |row| row.get(0),
        )
        .optional()?;

    match photo_quail {
        Some(qid) if qid == quail_uuid.to_string() => {
            // Set profile_photo FK
            let rows = conn.execute(
                "UPDATE quails SET profile_photo = ?1 WHERE uuid = ?2",
                params![photo_uuid.to_string(), quail_uuid.to_string()],
            )?;
            if rows == 0 {
                return Err(AppError::NotFound("Wachtel nicht gefunden".into()));
            }

            // Capture CRDT operation for the update
            crate::services::operation_capture::capture_quail_update(
                conn,
                &quail_uuid.to_string(),
                "profile_photo",
                serde_json::Value::String(photo_uuid.to_string()),
            )
            .await?;

            Ok(())
        }
        Some(_) => Err(AppError::NotFound("Foto gehört nicht zur Wachtel".into())),
        None => Err(AppError::NotFound("Foto nicht gefunden".into())),
    }
}

pub async fn delete_photo(conn: &Connection, photo_uuid: &Uuid) -> Result<(), AppError> {
    let service = init_photo_service();

    // Delete photo using photo-gallery service
    service
        .delete_photo(conn, photo_uuid)
        .await
        .map_err(convert_error)?;

    // Capture CRDT deletion after photo is deleted
    crate::services::operation_capture::capture_photo_delete(conn, &photo_uuid.to_string()).await?;

    Ok(())
}

/// Get photo with on-demand download capability
/// Returns Available(bytes), Downloading, or Failed(error, retry_count)
pub async fn get_photo_with_download(
    conn: &Connection,
    photo_uuid: &Uuid,
    size: PhotoSize,
) -> Result<PhotoResult, AppError> {
    let service = init_photo_service();

    // First check if photo is available locally
    match service
        .get_photo(conn, photo_uuid, size)
        .map_err(convert_error)?
    {
        PhotoResult::Available(bytes) => return Ok(PhotoResult::Available(bytes)),
        PhotoResult::Downloading => return Ok(PhotoResult::Downloading),
        PhotoResult::Failed(_, retry_count) => {
            // File doesn't exist locally, try to download if sync is configured
            if retry_count < 5 {
                return spawn_photo_download(conn, photo_uuid, retry_count).await;
            } else {
                return Ok(PhotoResult::Failed(
                    "Maximale Anzahl an Versuchen erreicht".to_string(),
                    retry_count,
                ));
            }
        }
    }
}

/// Spawns a background task to download a photo
async fn spawn_photo_download(
    conn: &Connection,
    photo_uuid: &Uuid,
    retry_count: i32,
) -> Result<PhotoResult, AppError> {
    use rusqlite::params;

    // Get photo path
    let relative_path: String = conn.query_row(
        "SELECT COALESCE(relative_path, path) FROM photos WHERE uuid = ?1",
        params![photo_uuid.to_string()],
        |row| row.get(0),
    )?;

    // Update status to downloading
    conn.execute(
        "UPDATE photos SET sync_status = 'downloading', last_sync_attempt = ?1 WHERE uuid = ?2",
        params![
            chrono::Utc::now().timestamp_millis(),
            photo_uuid.to_string()
        ],
    )?;

    let photo_uuid_clone = *photo_uuid;
    let relative_path_clone = relative_path.clone();
    let retry_count_clone = retry_count;

    // Spawn download task
    tokio::spawn(async move {
        // Calculate backoff with jitter
        if retry_count_clone > 0 {
            let base_delay = 60 * (1 << (retry_count_clone - 1).min(4));
            let max_delay = base_delay.min(300);
            let jitter = rand::random::<u64>() % (max_delay + 1);

            log::debug!(
                "Photo download retry {} for {}: waiting {}s",
                retry_count_clone,
                photo_uuid_clone,
                jitter
            );

            tokio::time::sleep(std::time::Duration::from_secs(jitter)).await;
        }

        // Perform actual download
        match download_photo_from_remote(&photo_uuid_clone, &relative_path_clone).await {
            Ok(()) => {
                log::info!("Successfully downloaded photo: {}", photo_uuid_clone);
                // Update status to synced
                if let Ok(conn) = crate::database::init_database() {
                    let _ = conn.execute(
                        "UPDATE photos SET sync_status = 'synced', retry_count = 0, sync_error = NULL WHERE uuid = ?1",
                        params![photo_uuid_clone.to_string()],
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to download photo {}: {}", photo_uuid_clone, e);
                // Update status to failed
                if let Ok(conn) = crate::database::init_database() {
                    let _ = conn.execute(
                        "UPDATE photos SET sync_status = 'download_failed', retry_count = ?1, sync_error = ?2 WHERE uuid = ?3",
                        params![retry_count_clone + 1, e.to_string(), photo_uuid_clone.to_string()],
                    );
                }
            }
        }
    });

    Ok(PhotoResult::Downloading)
}

/// Downloads a photo from remote storage
async fn download_photo_from_remote(
    photo_uuid: &Uuid,
    relative_path: &str,
) -> Result<(), AppError> {
    // Load sync settings
    let conn = crate::database::init_database()?;
    let settings = crate::services::sync_service::load_sync_settings(&conn)?
        .ok_or_else(|| AppError::Other("Sync nicht konfiguriert".to_string()))?;

    let sync_config = PhotoSyncConfig {
        server_url: settings.server_url,
        username: settings.username,
        app_password: settings.app_password,
        remote_path: settings.remote_path,
    };

    let sync_service = PhotoSyncService::new(sync_config);
    let local_path = get_absolute_photo_path(relative_path);

    sync_service
        .download_photo(relative_path, &local_path)
        .await
        .map_err(convert_error)?;

    log::info!("Downloaded photo {} to {}", photo_uuid, local_path);

    // After downloading the original, create local thumbnails
    let uuid_stem = std::path::Path::new(relative_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| AppError::Other("Invalid relative_path for downloaded photo".to_string()))?
        .to_string();

    // Use configured sizes from the gallery service and run in blocking thread
    let svc = init_photo_service();
    let (small_size, medium_size) = svc.thumbnail_sizes();

    let local_clone = local_path.clone();
    let uuid_clone = uuid_stem.clone();

    match tokio::task::spawn_blocking(move || {
        create_thumbnails(&local_clone, &uuid_clone, small_size, medium_size)
    })
    .await
    {
        Ok(Ok((small_name, medium_name))) => {
            use rusqlite::params;

            // Persist thumbnail filenames in DB (relative paths)
            let _ = conn.execute(
                "UPDATE photos SET thumbnail_small_path = ?1, thumbnail_medium_path = ?2 WHERE uuid = ?3",
                params![small_name, medium_name, photo_uuid.to_string()],
            );
        }
        Ok(Err(e)) => {
            log::warn!("Failed to create thumbnails for {}: {}", relative_path, e);
        }
        Err(e) => {
            log::warn!("Thumbnail task join error for {}: {}", relative_path, e);
        }
    }
    Ok(())
}

/// Retry all failed downloads that haven't exceeded max retries
pub async fn retry_failed_downloads(conn: &Connection) -> Result<usize, AppError> {
    let mut stmt = conn.prepare(
        "SELECT uuid, COALESCE(relative_path, path), retry_count
         FROM photos 
         WHERE sync_status = 'download_failed' AND retry_count < 5 AND deleted = 0",
    )?;

    let photos: Vec<(Uuid, String, i32)> = stmt
        .query_map([], |row| {
            let uuid_str: String = row.get(0)?;
            Ok((
                Uuid::parse_str(&uuid_str).map_err(|_| rusqlite::Error::InvalidQuery)?,
                row.get(1)?,
                row.get(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let count = photos.len();
    log::info!("Retrying {} failed photo downloads", count);

    for (uuid, _relative_path, retry_count) in photos {
        let _ = spawn_photo_download(conn, &uuid, retry_count).await;
    }

    Ok(count)
}

/// Cleanup orphaned photos (photos without valid quail_id or event_id references)
pub async fn cleanup_orphaned_photos(conn: &Connection) -> Result<usize, AppError> {
    use rusqlite::params;

    // Find orphaned photos
    let mut stmt = conn.prepare(
        "SELECT uuid, COALESCE(relative_path, path), thumbnail_small_path, thumbnail_medium_path
         FROM photos 
         WHERE deleted = 0 AND (
            (quail_id IS NOT NULL AND quail_id NOT IN (SELECT uuid FROM quails WHERE deleted = 0))
            OR
            (event_id IS NOT NULL AND event_id NOT IN (SELECT uuid FROM quail_events WHERE deleted = 0))
         )",
    )?;

    let orphaned: Vec<(Uuid, String, Option<String>, Option<String>)> = stmt
        .query_map([], |row| {
            let uuid_str: String = row.get(0)?;
            Ok((
                Uuid::parse_str(&uuid_str).map_err(|_| rusqlite::Error::InvalidQuery)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let count = orphaned.len();
    log::info!("Found {} orphaned photos to clean up", count);

    for (uuid, relative_path, small_thumb, medium_thumb) in orphaned {
        // Delete physical files
        let _ = std::fs::remove_file(get_absolute_photo_path(&relative_path));
        if let Some(small) = small_thumb {
            let _ = std::fs::remove_file(get_absolute_photo_path(&small));
        }
        if let Some(medium) = medium_thumb {
            let _ = std::fs::remove_file(get_absolute_photo_path(&medium));
        }

        // Mark as deleted in database
        conn.execute(
            "UPDATE photos SET deleted = 1 WHERE uuid = ?1",
            params![uuid.to_string()],
        )?;
    }

    Ok(count)
}

// === Collection-Based API (New) ===

/// Get or create a photo collection for a quail
pub fn get_or_create_quail_collection(
    _conn: &Connection,
    quail_id: &Uuid,
) -> Result<Uuid, AppError> {
    // The quail UUID itself IS the collection ID in the photo-gallery system
    Ok(*quail_id)
}

/// Get or create a photo collection for an event
pub fn get_or_create_event_collection(
    _conn: &Connection,
    event_id: &Uuid,
) -> Result<Uuid, AppError> {
    // The event UUID itself IS the collection ID in the photo-gallery system
    Ok(*event_id)
}

/// Add photo to a collection (new collection-based API)
pub async fn add_photo_to_collection(
    conn: &Connection,
    collection_id: &Uuid,
    path: String,
) -> Result<Uuid, AppError> {
    log::debug!("Adding photo to collection {}: {}", collection_id, path);

    let service = init_photo_service();

    // Add photo using photo-gallery service
    let photo_uuid = service
        .add_photo_to_collection(conn, collection_id, path)
        .await
        .map_err(convert_error)?;

    // Get the relative path for operation capture
    let relative_path: String = conn.query_row(
        "SELECT relative_path FROM photos WHERE uuid = ?1",
        rusqlite::params![photo_uuid.to_string()],
        |row| row.get(0),
    )?;

    let thumbnail_small: Option<String> = conn.query_row(
        "SELECT thumbnail_small_path FROM photos WHERE uuid = ?1",
        rusqlite::params![photo_uuid.to_string()],
        |row| row.get(0),
    )?;

    // Capture CRDT operation after photo is created
    crate::services::operation_capture::capture_photo_create(
        conn,
        &photo_uuid.to_string(),
        None, // No direct quail_id
        None, // No direct event_id
        &relative_path,
        thumbnail_small.as_deref(),
    )
    .await?;

    Ok(photo_uuid)
}

/// List all photos in a collection
pub fn list_collection_photos(
    conn: &Connection,
    collection_id: &Uuid,
) -> Result<Vec<Photo>, AppError> {
    let service = init_photo_service();
    service
        .list_collection_photos(conn, collection_id)
        .map_err(convert_error)
}

/// Get the collection for a quail (if exists)
pub fn get_quail_collection(conn: &Connection, quail_id: &Uuid) -> Result<Option<Uuid>, AppError> {
    use rusqlite::params;

    let collection_id_str: Option<String> = conn
        .query_row(
            "SELECT collection_id FROM quails WHERE uuid = ?1",
            params![quail_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;

    match collection_id_str {
        Some(id_str) => {
            Ok(Some(Uuid::parse_str(&id_str).map_err(|e| {
                AppError::Other(format!("Invalid collection UUID: {}", e))
            })?))
        }
        None => Ok(None),
    }
}

/// Get the collection for an event (if exists)
pub fn get_event_collection(conn: &Connection, event_id: &Uuid) -> Result<Option<Uuid>, AppError> {
    use rusqlite::params;

    let collection_id_str: Option<String> = conn
        .query_row(
            "SELECT collection_id FROM quail_events WHERE uuid = ?1",
            params![event_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;

    match collection_id_str {
        Some(id_str) => {
            Ok(Some(Uuid::parse_str(&id_str).map_err(|e| {
                AppError::Other(format!("Invalid collection UUID: {}", e))
            })?))
        }
        None => Ok(None),
    }
}
