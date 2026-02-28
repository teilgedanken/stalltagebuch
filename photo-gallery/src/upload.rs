//! Photo upload functionality for syncing with WebDAV servers
//!
//! This module handles uploading photos to remote storage (e.g., Nextcloud)
//! using WebDAV protocol. It supports batch uploads with concurrency control
//! and automatic retry logic.

use rusqlite::Connection;
use std::sync::Arc;

/// Configuration for photo upload sync
#[derive(Debug, Clone)]
pub struct PhotoUploadConfig {
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub remote_path: String,
    pub storage_path: String,
}

/// Result type for upload operations
pub type UploadResult<T> = Result<T, UploadError>;

/// Errors that can occur during photo upload
#[derive(Debug)]
pub enum UploadError {
    DatabaseError(rusqlite::Error),
    WebDavError(String),
    IoError(std::io::Error),
    Other(String),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::DatabaseError(e) => write!(f, "Database error: {}", e),
            UploadError::WebDavError(e) => write!(f, "WebDAV error: {}", e),
            UploadError::IoError(e) => write!(f, "IO error: {}", e),
            UploadError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for UploadError {}

impl From<rusqlite::Error> for UploadError {
    fn from(err: rusqlite::Error) -> Self {
        UploadError::DatabaseError(err)
    }
}

impl From<std::io::Error> for UploadError {
    fn from(err: std::io::Error) -> Self {
        UploadError::IoError(err)
    }
}

/// Service for managing photo uploads
pub struct PhotoUploadService {
    config: PhotoUploadConfig,
}

impl PhotoUploadService {
    /// Create a new photo upload service
    pub fn new(config: PhotoUploadConfig) -> Self {
        Self { config }
    }

    /// Counts how many photos are pending upload (sync_status='local_only')
    pub fn count_pending_photos(&self, conn: &Connection) -> UploadResult<usize> {
        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM photos 
             WHERE deleted = 0 AND (sync_status = 'local_only' OR sync_status IS NULL)",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Uploads binary photo files to sync/photos/ with the original image only
    ///
    /// Only uploads photos with sync_status='local_only'. Thumbnails are no
    /// longer uploaded — they are generated locally on the receiving side.
    /// Uses JoinSet for parallel uploads (max 3 concurrent photos).
    pub async fn upload_photos_batch(&self, conn: &Connection) -> UploadResult<usize> {
        use tokio::task::JoinSet;

        // Create WebDAV client
        let webdav_url = format!(
            "{}/remote.php/dav/files/{}",
            self.config.server_url.trim_end_matches('/'),
            self.config.username
        );

        let client = Arc::new(
            reqwest_dav::ClientBuilder::new()
                .set_host(webdav_url)
                .set_auth(reqwest_dav::Auth::Basic(
                    self.config.username.clone(),
                    self.config.password.clone(),
                ))
                .build()
                .map_err(|e| UploadError::WebDavError(format!("WebDAV client error: {:?}", e)))?,
        );

        let base = self.config.remote_path.trim_end_matches('/');
        let sync_base = format!("{}/sync", base);
        let photos_dir = format!("{}/photos", sync_base);

        // Create photos directory if needed
        if let Err(e) = client.mkcol(&sync_base).await {
            log::debug!("MKCOL sync note: {:?}", e);
        }
        if let Err(e) = client.mkcol(&photos_dir).await {
            log::debug!("MKCOL photos note: {:?}", e);
        }

        // List existing remote photos
        let remote_photos = self.list_remote_photos(&client, &photos_dir).await?;

        // Get local photos that need upload (sync_status='local_only' or NULL)
        let mut stmt = conn.prepare(
            "SELECT uuid, COALESCE(relative_path, path) as rel_path, thumbnail_small_path, thumbnail_medium_path
             FROM photos 
             WHERE deleted = 0 AND (sync_status = 'local_only' OR sync_status IS NULL)",
        )?;

        let rows: Vec<(String, String, Option<String>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let total_photos = rows.len();
        log::info!("Found {} photos to upload", total_photos);

        if total_photos == 0 {
            return Ok(0);
        }

        let mut join_set = JoinSet::new();
        let mut uploaded_count = 0;

        for (uuid, rel_path, _small_thumb, _medium_thumb) in rows {
            let client_clone = client.clone();
            let photos_dir_clone = photos_dir.clone();
            let remote_photos_clone = remote_photos.clone();
            let storage_path = self.config.storage_path.clone();

            // Limit concurrent uploads to 3
            while join_set.len() >= 3 {
                if let Some(result) = join_set.join_next().await {
                    match result {
                        Ok(Ok((uuid_done, success))) => {
                            if success {
                                uploaded_count += 1;
                                // Update sync_status to 'synced'
                                let _ = conn.execute(
                                    "UPDATE photos SET sync_status = 'synced', retry_count = 0, sync_error = NULL WHERE uuid = ?1",
                                    rusqlite::params![uuid_done],
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }

            join_set.spawn(async move {
                Self::upload_single_photo(
                    uuid,
                    rel_path,
                    client_clone,
                    photos_dir_clone,
                    remote_photos_clone,
                    storage_path,
                )
                .await
            });
        }

        // Wait for remaining uploads
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok((uuid_done, success))) => {
                    if success {
                        uploaded_count += 1;
                        // Update sync_status to 'synced'
                        let _ = conn.execute(
                            "UPDATE photos SET sync_status = 'synced', retry_count = 0, sync_error = NULL WHERE uuid = ?1",
                            rusqlite::params![uuid_done],
                        );
                    }
                }
                _ => {}
            }
        }

        log::info!("Uploaded {} of {} photos", uploaded_count, total_photos);
        Ok(uploaded_count)
    }

    /// Uploads a single photo
    async fn upload_single_photo(
        uuid: String,
        rel_path: String,
        client: Arc<reqwest_dav::Client>,
        photos_dir: String,
        remote_photos: Vec<String>,
        storage_path: String,
    ) -> UploadResult<(String, bool)> {
        let photo_name = format!("{}.jpg", uuid);

        // Skip if already uploaded
        if remote_photos.contains(&photo_name) {
            log::debug!("Photo {} already exists remotely", uuid);
            return Ok((uuid, true));
        }

        // Construct absolute path
        let abs_path = if storage_path.is_empty() {
            rel_path.clone()
        } else {
            format!("{}/{}", storage_path.trim_end_matches('/'), rel_path)
        };

        let file_path = std::path::Path::new(&abs_path);

        if !file_path.exists() {
            let error_msg = format!("Photo file not found locally: {}", abs_path);
            log::warn!("{}", error_msg);
            return Ok((uuid, false));
        }

        // Read and upload original
        match std::fs::read(file_path) {
            Ok(data) => {
                let remote_path = format!("{}/{}", photos_dir, photo_name);
                if let Err(e) = client.put(&remote_path, data).await {
                    let error_msg = format!("Failed to upload original: {:?}", e);
                    log::error!("Photo {}: {}", uuid, error_msg);
                    return Ok((uuid, false));
                }
                log::info!("Uploaded original photo: {}", photo_name);
            }
            Err(e) => {
                let error_msg = format!("Failed to read photo: {:?}", e);
                log::error!("{}: {}", abs_path, error_msg);
                return Ok((uuid, false));
            }
        }

        // NOTE: Thumbnails are intentionally *not* uploaded anymore.
        // The receiving side should generate thumbnails locally from the
        // downloaded original image to avoid redundant uploads and save
        // remote storage / bandwidth.

        Ok((uuid, true))
    }

    /// Lists existing photo files in sync/photos/ directory
    async fn list_remote_photos(
        &self,
        client: &reqwest_dav::Client,
        photos_dir: &str,
    ) -> UploadResult<Vec<String>> {
        let list = match client.list(photos_dir, reqwest_dav::Depth::Number(1)).await {
            Ok(l) => l,
            Err(_) => return Ok(Vec::new()), // Directory doesn't exist yet
        };

        let mut names = Vec::new();
        for item in list {
            if let reqwest_dav::list_cmd::ListEntity::File(file) = item {
                if let Some(name) = file.href.split('/').last() {
                    if name.ends_with(".jpg") || name.ends_with(".webp") {
                        names.push(name.to_string());
                    }
                }
            }
        }

        Ok(names)
    }
}
