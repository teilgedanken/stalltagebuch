//! Photo sync functionality using WebDAV
//!
//! This module provides photo upload and download capabilities via WebDAV,
//! typically used with Nextcloud.

#[cfg(feature = "sync")]
use crate::service::PhotoGalleryError;

/// Configuration for photo sync
#[derive(Debug, Clone)]
pub struct PhotoSyncConfig {
    pub server_url: String,
    pub username: String,
    pub app_password: String,
    pub remote_path: String,
}

#[cfg(feature = "sync")]
/// Photo sync service for WebDAV operations
pub struct PhotoSyncService {
    config: PhotoSyncConfig,
}

#[cfg(feature = "sync")]
impl PhotoSyncService {
    /// Create a new photo sync service
    pub fn new(config: PhotoSyncConfig) -> Self {
        Self { config }
    }

    /// Create a WebDAV client
    fn create_client(&self) -> Result<reqwest_dav::Client, PhotoGalleryError> {
        let webdav_url = format!(
            "{}/remote.php/dav/files/{}",
            self.config.server_url.trim_end_matches('/'),
            self.config.username
        );

        reqwest_dav::ClientBuilder::new()
            .set_host(webdav_url)
            .set_auth(reqwest_dav::Auth::Basic(
                self.config.username.clone(),
                self.config.app_password.clone(),
            ))
            .build()
            .map_err(|e| PhotoGalleryError::Other(format!("WebDAV client error: {:?}", e)))
    }

    /// Upload a photo to remote storage
    pub async fn upload_photo(
        &self,
        local_path: &str,
        relative_path: &str,
    ) -> Result<(), PhotoGalleryError> {
        let client = self.create_client()?;

        // Ensure photos directory exists
        let photos_dir = format!(
            "{}/sync/photos",
            self.config.remote_path.trim_end_matches('/')
        );

        // Try to create directory (ignore if already exists)
        let _ = self.ensure_directory(&client, &photos_dir).await;

        // Read local file
        let file_data = std::fs::read(local_path)?;

        // Upload to remote
        let remote_path = format!("{}/{}", photos_dir, relative_path);
        client
            .put(&remote_path, file_data)
            .await
            .map_err(|e| PhotoGalleryError::Other(format!("Upload failed: {:?}", e)))?;

        log::info!("Uploaded photo {} to {}", relative_path, remote_path);
        Ok(())
    }

    /// Download a photo from remote storage
    pub async fn download_photo(
        &self,
        relative_path: &str,
        local_path: &str,
    ) -> Result<(), PhotoGalleryError> {
        let client = self.create_client()?;

        let remote_path = format!(
            "{}/sync/photos/{}",
            self.config.remote_path.trim_end_matches('/'),
            relative_path
        );

        // Download file
        let response = client
            .get(&remote_path)
            .await
            .map_err(|e| PhotoGalleryError::Other(format!("Download failed: {:?}", e)))?;

        // Convert response to bytes
        let bytes = response.bytes().await.map_err(|e| {
            PhotoGalleryError::Other(format!("Failed to read response bytes: {}", e))
        })?;

        // Save to local storage
        std::fs::write(local_path, bytes)?;

        log::info!("Downloaded photo {} to {}", relative_path, local_path);
        Ok(())
    }

    /// Ensure a directory exists on the remote server
    async fn ensure_directory(
        &self,
        client: &reqwest_dav::Client,
        path: &str,
    ) -> Result<(), PhotoGalleryError> {
        // Split path and create each level
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        let mut current_path = String::new();

        for part in parts {
            if current_path.is_empty() {
                current_path = part.to_string();
            } else {
                current_path = format!("{}/{}", current_path, part);
            }

            // Try to create directory (ignore errors as it might already exist)
            if let Err(e) = client.mkcol(&current_path).await {
                log::debug!("MKCOL '{}' note: {:?}", current_path, e);
            }
        }

        Ok(())
    }

    /// Check if a photo exists on the remote server
    pub async fn photo_exists(&self, relative_path: &str) -> Result<bool, PhotoGalleryError> {
        let client = self.create_client()?;

        let remote_path = format!(
            "{}/sync/photos/{}",
            self.config.remote_path.trim_end_matches('/'),
            relative_path
        );

        // Try to get file properties
        match client
            .list(&remote_path, reqwest_dav::Depth::Number(0))
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Upload photo with retry logic
    pub async fn upload_photo_with_retry(
        &self,
        local_path: &str,
        relative_path: &str,
        max_retries: u32,
    ) -> Result<(), PhotoGalleryError> {
        let mut retries = 0;

        loop {
            match self.upload_photo(local_path, relative_path).await {
                Ok(()) => return Ok(()),
                Err(e) if retries < max_retries => {
                    retries += 1;
                    let backoff = calculate_backoff(retries);
                    log::warn!(
                        "Photo upload failed (attempt {}/{}): {}. Retrying in {}s...",
                        retries,
                        max_retries + 1,
                        e,
                        backoff
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
                }
                Err(e) => {
                    return Err(PhotoGalleryError::Other(format!(
                        "Upload failed after {} retries: {}",
                        max_retries + 1,
                        e
                    )));
                }
            }
        }
    }

    /// Download photo with retry logic
    pub async fn download_photo_with_retry(
        &self,
        relative_path: &str,
        local_path: &str,
        max_retries: u32,
    ) -> Result<(), PhotoGalleryError> {
        let mut retries = 0;

        loop {
            match self.download_photo(relative_path, local_path).await {
                Ok(()) => return Ok(()),
                Err(e) if retries < max_retries => {
                    retries += 1;
                    let backoff = calculate_backoff(retries);
                    log::warn!(
                        "Photo download failed (attempt {}/{}): {}. Retrying in {}s...",
                        retries,
                        max_retries + 1,
                        e,
                        backoff
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(backoff)).await;
                }
                Err(e) => {
                    return Err(PhotoGalleryError::Other(format!(
                        "Download failed after {} retries: {}",
                        max_retries + 1,
                        e
                    )));
                }
            }
        }
    }
}

#[cfg(feature = "sync")]
/// Calculate exponential backoff delay with jitter
fn calculate_backoff(retry: u32) -> u64 {
    use rand::Rng;

    let base_delay = 60 * (1 << (retry - 1).min(4)); // 60s, 120s, 240s, 480s, 960s
    let max_delay = base_delay.min(300); // Cap at 300s (5 minutes)
    let jitter = rand::rng().random_range(0..=max_delay);
    jitter
}

#[cfg(not(feature = "sync"))]
/// Stub when sync feature is disabled
pub struct PhotoSyncService;

#[cfg(not(feature = "sync"))]
impl PhotoSyncService {
    pub fn new(_config: PhotoSyncConfig) -> Self {
        Self
    }
}
