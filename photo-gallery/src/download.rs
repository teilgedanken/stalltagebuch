//! Photo download functionality for syncing with WebDAV servers
//!
//! This module handles downloading photos from remote storage (e.g., Nextcloud)
//! using WebDAV protocol.

use rusqlite::Connection;

/// Configuration for photo download sync
#[derive(Debug, Clone)]
pub struct PhotoDownloadConfig {
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub remote_path: String,
    pub storage_path: String,
}

/// Result type for download operations
pub type DownloadResult<T> = Result<T, DownloadError>;

/// Errors that can occur during photo download
#[derive(Debug)]
pub enum DownloadError {
    DatabaseError(rusqlite::Error),
    WebDavError(String),
    IoError(std::io::Error),
    Other(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::DatabaseError(e) => write!(f, "Database error: {}", e),
            DownloadError::WebDavError(e) => write!(f, "WebDAV error: {}", e),
            DownloadError::IoError(e) => write!(f, "IO error: {}", e),
            DownloadError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<rusqlite::Error> for DownloadError {
    fn from(err: rusqlite::Error) -> Self {
        DownloadError::DatabaseError(err)
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(err: std::io::Error) -> Self {
        DownloadError::IoError(err)
    }
}

/// Service for managing photo downloads
pub struct PhotoDownloadService {
    config: PhotoDownloadConfig,
}

impl PhotoDownloadService {
    /// Create a new photo download service
    pub fn new(config: PhotoDownloadConfig) -> Self {
        Self { config }
    }

    /// Downloads missing photo files based on relative_path and marks them as synchronized
    pub async fn download_missing_photos(&self, conn: &Connection) -> DownloadResult<usize> {
        // Create WebDAV client
        let webdav_url = format!(
            "{}/remote.php/dav/files/{}",
            self.config.server_url.trim_end_matches('/'),
            self.config.username
        );

        let client = reqwest_dav::ClientBuilder::new()
            .set_host(webdav_url)
            .set_auth(reqwest_dav::Auth::Basic(
                self.config.username.clone(),
                self.config.password.clone(),
            ))
            .build()
            .map_err(|e| DownloadError::WebDavError(format!("WebDAV client error: {:?}", e)))?;

        let mut downloaded = 0usize;

        // Get all photos with relative_path or path
        let mut stmt = conn.prepare(
            "SELECT uuid, COALESCE(relative_path, path) AS rel
             FROM photos
             WHERE deleted = 0 AND (relative_path IS NOT NULL OR path IS NOT NULL)",
        )?;

        let rows = stmt.query_map([], |row| {
            let _uuid: String = row.get(0)?;
            let rel: String = row.get(1)?;
            Ok(rel)
        })?;

        for row in rows {
            let rel = row.map_err(|e| DownloadError::Other(format!("Row error: {:?}", e)))?;
            if rel.trim().is_empty() {
                continue;
            }

            let abs = self.get_absolute_photo_path(&rel);
            let abs_path = std::path::Path::new(&abs);
            if abs_path.exists() {
                continue;
            }

            // Remote path: sync/photos/<uuid>.jpg (flat structure)
            let photo_filename = if rel.contains('/') {
                // Extract filename if rel_path is nested
                rel.split('/').last().unwrap_or(&rel)
            } else {
                &rel
            };

            let remote_base = self.config.remote_path.trim_end_matches('/');
            let remote_path = format!("{}/sync/photos/{}", remote_base, photo_filename);
            log::info!("Downloading missing photo {} -> {}", remote_path, abs);

            // Attempt download
            match client.get(&remote_path).await {
                Ok(resp) => match resp.bytes().await {
                    Ok(bytes) => {
                        if let Some(parent) = abs_path.parent() {
                            if !parent.exists() {
                                if let Err(e) = std::fs::create_dir_all(parent) {
                                    log::error!(
                                        "Failed to create photo directory {}: {:?}",
                                        parent.display(),
                                        e
                                    );
                                    continue;
                                }
                            }
                        }
                        if let Err(e) = std::fs::write(&abs_path, &bytes) {
                            log::error!("Failed to save photo {}: {:?}", abs_path.display(), e);
                            continue;
                        }
                        downloaded += 1;
                    }
                    Err(e) => {
                        log::warn!("Failed to read bytes for {}: {:?}", remote_path, e);
                    }
                },
                Err(e) => {
                    log::debug!("Photo not found remotely {}: {:?}", remote_path, e);
                }
            }
        }

        log::info!("Downloaded {} missing photos", downloaded);
        Ok(downloaded)
    }

    /// Returns the absolute path to a photo
    fn get_absolute_photo_path(&self, relative_path: &str) -> String {
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
}
