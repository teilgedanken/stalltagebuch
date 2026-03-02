use crate::models::{Photo, PhotoGalleryConfig, PhotoResult, PhotoSize};
use crate::thumbnail::{ThumbnailError, rename_photo_with_uuid};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

/// Error type for photo gallery operations
#[derive(Debug)]
pub enum PhotoGalleryError {
    DatabaseError(rusqlite::Error),
    ThumbnailError(ThumbnailError),
    NotFound(String),
    IoError(std::io::Error),
    Other(String),
}

impl std::fmt::Display for PhotoGalleryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhotoGalleryError::DatabaseError(e) => write!(f, "Database error: {}", e),
            PhotoGalleryError::ThumbnailError(e) => write!(f, "Thumbnail error: {}", e),
            PhotoGalleryError::NotFound(msg) => write!(f, "Not found: {}", msg),
            PhotoGalleryError::IoError(e) => write!(f, "IO error: {}", e),
            PhotoGalleryError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PhotoGalleryError {}

impl From<rusqlite::Error> for PhotoGalleryError {
    fn from(err: rusqlite::Error) -> Self {
        PhotoGalleryError::DatabaseError(err)
    }
}

impl From<ThumbnailError> for PhotoGalleryError {
    fn from(err: ThumbnailError) -> Self {
        PhotoGalleryError::ThumbnailError(err)
    }
}

impl From<std::io::Error> for PhotoGalleryError {
    fn from(err: std::io::Error) -> Self {
        PhotoGalleryError::IoError(err)
    }
}

/// Photo Gallery Service
pub struct PhotoGalleryService {
    config: PhotoGalleryConfig,
}

impl PhotoGalleryService {
    /// Initialize the photo gallery service with configuration
    pub fn new(config: PhotoGalleryConfig) -> Self {
        Self { config }
    }

    /// Returns the absolute path to a photo (for UI display)
    pub fn get_absolute_photo_path(&self, relative_path: &str) -> String {
        if self.config.storage_path.is_empty() {
            relative_path.to_string()
        } else {
            format!(
                "{}/{}",
                self.config.storage_path.trim_end_matches('/'),
                relative_path
            )
        }
    }

    /// Return configured thumbnail sizes (small, medium)
    pub fn thumbnail_sizes(&self) -> (u32, u32) {
        (
            self.config.thumbnail_small_size,
            self.config.thumbnail_medium_size,
        )
    }

    /// Get profile photo for a quail
    ///
    /// First tries to get the explicitly set profile_photo from the quails table.
    /// If none is set, falls back to the first photo in the quail's collection.
    pub fn get_profile_photo(
        &self,
        conn: &Connection,
        quail_uuid: &Uuid,
    ) -> Result<Option<Photo>, PhotoGalleryError> {
        log::debug!(
            "get_profile_photo: Looking for profile photo of quail {}",
            quail_uuid
        );

        // First, try to get the explicitly set profile_photo
        let mut stmt = conn.prepare(
            "SELECT p.uuid, p.quail_id, p.event_id, COALESCE(p.relative_path, p.path) as rel_path, p.thumbnail_path,
                    p.thumbnail_small_path, p.thumbnail_medium_path, p.sync_status, p.sync_error, p.retry_count
             FROM photos p 
             JOIN quails q ON q.profile_photo = p.uuid 
             WHERE q.uuid = ?1",
        )?;

        let res = stmt
            .query_row(params![quail_uuid.to_string()], |row| {
                let uuid_str: String = row.get(0)?;
                let quail_id_str: Option<String> = row.get(1)?;
                let event_id_str: Option<String> = row.get(2)?;
                let relative_path: String = row.get(3)?;
                let relative_thumb: Option<String> = row.get(4)?;
                let thumbnail_small: Option<String> = row.get(5)?;
                let thumbnail_medium: Option<String> = row.get(6)?;
                let sync_status: Option<String> = row.get(7)?;
                let sync_error: Option<String> = row.get(8)?;
                let retry_count: Option<i32> = row.get(9)?;

                Ok(Photo {
                    uuid: Uuid::parse_str(&uuid_str).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    quail_id: quail_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                    collection_id: None,
                    event_id: event_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                    path: self.get_absolute_photo_path(&relative_path),
                    relative_path: Some(relative_path.clone()),
                    thumbnail_path: relative_thumb.map(|t| self.get_absolute_photo_path(&t)),
                    thumbnail_small_path: thumbnail_small.map(|t| self.get_absolute_photo_path(&t)),
                    thumbnail_medium_path: thumbnail_medium
                        .map(|t| self.get_absolute_photo_path(&t)),
                    sync_status,
                    sync_error,
                    retry_count,
                    created_at: None,
                })
            })
            .optional()?;

        if res.is_some() {
            log::debug!(
                "get_profile_photo: Found explicit profile_photo for quail {}",
                quail_uuid
            );
            return Ok(res);
        }

        // No explicit profile_photo set, try to get the first photo from the quail's collection
        log::debug!(
            "get_profile_photo: No explicit profile_photo, checking collection for quail {}",
            quail_uuid
        );

        let mut stmt = conn.prepare(
            "SELECT p.uuid, p.quail_id, p.event_id, COALESCE(p.relative_path, p.path) as rel_path, p.thumbnail_path,
                    p.thumbnail_small_path, p.thumbnail_medium_path, p.sync_status, p.sync_error, p.retry_count
             FROM photos p 
             JOIN quails q ON q.collection_id = p.collection_id
             WHERE q.uuid = ?1 AND p.deleted = 0
             ORDER BY p.created_at ASC
             LIMIT 1",
        )?;

        let fallback_res = stmt
            .query_row(params![quail_uuid.to_string()], |row| {
                let uuid_str: String = row.get(0)?;
                let quail_id_str: Option<String> = row.get(1)?;
                let event_id_str: Option<String> = row.get(2)?;
                let relative_path: String = row.get(3)?;
                let relative_thumb: Option<String> = row.get(4)?;
                let thumbnail_small: Option<String> = row.get(5)?;
                let thumbnail_medium: Option<String> = row.get(6)?;
                let sync_status: Option<String> = row.get(7)?;
                let sync_error: Option<String> = row.get(8)?;
                let retry_count: Option<i32> = row.get(9)?;

                Ok(Photo {
                    uuid: Uuid::parse_str(&uuid_str).map_err(|_| rusqlite::Error::InvalidQuery)?,
                    quail_id: quail_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                    collection_id: None,
                    event_id: event_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                    path: self.get_absolute_photo_path(&relative_path),
                    relative_path: Some(relative_path.clone()),
                    thumbnail_path: relative_thumb.map(|t| self.get_absolute_photo_path(&t)),
                    thumbnail_small_path: thumbnail_small.map(|t| self.get_absolute_photo_path(&t)),
                    thumbnail_medium_path: thumbnail_medium
                        .map(|t| self.get_absolute_photo_path(&t)),
                    sync_status,
                    sync_error,
                    retry_count,
                    created_at: None,
                })
            })
            .optional()?;

        if fallback_res.is_some() {
            log::debug!(
                "get_profile_photo: Found collection photo as fallback for quail {}",
                quail_uuid
            );
        } else {
            log::debug!(
                "get_profile_photo: No photos found at all for quail {}",
                quail_uuid
            );
        }

        Ok(fallback_res)
    }

    /// Delete a photo
    ///
    /// The caller is responsible for any additional operations like CRDT operation capture.
    pub async fn delete_photo(
        &self,
        conn: &Connection,
        photo_uuid: &Uuid,
    ) -> Result<(), PhotoGalleryError> {
        let rows = conn.execute(
            "DELETE FROM photos WHERE uuid = ?1",
            params![photo_uuid.to_string()],
        )?;

        if rows == 0 {
            return Err(PhotoGalleryError::NotFound("Photo not found".into()));
        }

        Ok(())
    }

    /// Get photo with local file check
    pub fn get_photo(
        &self,
        conn: &Connection,
        photo_uuid: &Uuid,
        size: PhotoSize,
    ) -> Result<PhotoResult, PhotoGalleryError> {
        // Query photo info from database
        let photo_info: Option<(String, Option<String>, Option<String>, Option<String>, Option<i32>)> = conn
            .query_row(
                "SELECT COALESCE(relative_path, path), thumbnail_small_path, thumbnail_medium_path, sync_status, retry_count
                 FROM photos 
                 WHERE uuid = ?1 AND deleted = 0",
                params![photo_uuid.to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?;

        let (relative_path, small_thumb, medium_thumb, sync_status, retry_count) = match photo_info
        {
            Some(info) => info,
            None => return Err(PhotoGalleryError::NotFound("Photo not found".into())),
        };

        // Determine which file to load based on size
        let file_path = match size {
            PhotoSize::Small => small_thumb.as_ref().unwrap_or(&relative_path),
            PhotoSize::Medium => medium_thumb.as_ref().unwrap_or(&relative_path),
            PhotoSize::Original => &relative_path,
        };

        let absolute_path = self.get_absolute_photo_path(file_path);

        // Check if file exists locally
        if std::path::Path::new(&absolute_path).exists() {
            match std::fs::read(&absolute_path) {
                Ok(bytes) => return Ok(PhotoResult::Available(bytes)),
                Err(e) => {
                    log::warn!("Failed to read file {}: {}", absolute_path, e);
                }
            }
        }

        // File doesn't exist locally - check sync status
        let status = sync_status.unwrap_or_else(|| "local_only".to_string());
        let retry_count = retry_count.unwrap_or(0);

        match status.as_str() {
            "downloading" => Ok(PhotoResult::Downloading),
            "download_failed" if retry_count >= 5 => Ok(PhotoResult::Failed(
                "Max retries reached".to_string(),
                retry_count,
            )),
            _ => Ok(PhotoResult::Failed(
                "Photo not available locally".to_string(),
                retry_count,
            )),
        }
    }

    // === Collection-Based API (New) ===

    /// Add a photo to a collection
    ///
    /// Returns the UUID of the created photo. The caller is responsible for
    /// any additional operations like CRDT operation capture.
    pub async fn add_photo_to_collection(
        &self,
        conn: &Connection,
        collection_id: &Uuid,
        path: String,
    ) -> Result<Uuid, PhotoGalleryError> {
        log::debug!("Adding photo to collection {}: {}", collection_id, path);

        // Rename photo and create multi-size thumbnails (in blocking thread)
        let (new_path, small_thumb, medium_thumb) = rename_photo_with_uuid(
            &path,
            self.config.thumbnail_small_size,
            self.config.thumbnail_medium_size,
        )
        .await?;

        log::debug!("Photo renamed to: {}", new_path);
        log::debug!("Small thumbnail: {}", small_thumb);
        log::debug!("Medium thumbnail: {}", medium_thumb);

        // Extract UUID from the renamed filename (e.g. "{uuid}.jpg") so that the
        // DB uuid and the on-disk filename share the same UUID – required for sync.
        let relative_path = std::path::Path::new(&new_path)
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| new_path.clone());

        let photo_uuid = std::path::Path::new(&relative_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);

        // Save to database with collection_id
        conn.execute(
            "INSERT INTO photos (uuid, collection_id, path, relative_path, thumbnail_path, thumbnail_small_path, thumbnail_medium_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                photo_uuid.to_string(),
                collection_id.to_string(),
                new_path,
                relative_path,
                small_thumb.clone(),
                small_thumb,
                medium_thumb,
            ],
        )?;

        log::debug!("Photo saved to database with UUID: {}", photo_uuid);

        Ok(photo_uuid)
    }

    /// List all photos in a collection
    pub fn list_collection_photos(
        &self,
        conn: &Connection,
        collection_id: &Uuid,
    ) -> Result<Vec<Photo>, PhotoGalleryError> {
        let mut stmt = conn.prepare(
            "SELECT uuid, collection_id, path, relative_path, thumbnail_path, thumbnail_small_path, 
                    thumbnail_medium_path, sync_status, created_at
             FROM photos
             WHERE collection_id = ?1 AND deleted = 0
             ORDER BY created_at DESC",
        )?;

        let photos = stmt
            .query_map(params![collection_id.to_string()], |row| {
                let uuid_str: String = row.get(0)?;
                let collection_id_str: Option<String> = row.get(1)?;
                let db_path: String = row.get(2)?;
                let relative_path: Option<String> = row.get(3)?;
                let thumbnail_path: Option<String> = row.get(4)?;
                let thumbnail_small_path: Option<String> = row.get(5)?;
                let thumbnail_medium_path: Option<String> = row.get(6)?;
                Ok((
                    uuid_str,
                    collection_id_str,
                    db_path,
                    relative_path,
                    thumbnail_path,
                    thumbnail_small_path,
                    thumbnail_medium_path,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(
                |(
                    uuid_str,
                    collection_id_str,
                    db_path,
                    relative_path,
                    thumbnail_path,
                    thumbnail_small_path,
                    thumbnail_medium_path,
                    sync_status,
                    created_at,
                )| {
                    // Normalise to absolute paths, matching other list/get methods.
                    let path = if let Some(ref rel) = relative_path {
                        self.get_absolute_photo_path(rel)
                    } else {
                        db_path
                    };
                    Photo {
                        uuid: Uuid::parse_str(&uuid_str).unwrap_or_else(|_| Uuid::new_v4()),
                        collection_id: collection_id_str
                            .as_deref()
                            .and_then(|s| Uuid::parse_str(s).ok()),
                        quail_id: None,
                        event_id: None,
                        path,
                        relative_path: relative_path.clone(),
                        thumbnail_path: thumbnail_path
                            .as_deref()
                            .map(|p| self.get_absolute_photo_path(p)),
                        thumbnail_small_path: thumbnail_small_path
                            .as_deref()
                            .map(|p| self.get_absolute_photo_path(p)),
                        thumbnail_medium_path: thumbnail_medium_path
                            .as_deref()
                            .map(|p| self.get_absolute_photo_path(p)),
                        sync_status,
                        sync_error: None,
                        retry_count: None,
                        created_at,
                    }
                },
            )
            .collect();

        Ok(photos)
    }
}
