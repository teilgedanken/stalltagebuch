use crate::models::{PhotoGalleryConfig, PhotoResult, PhotoSize};
use crate::thumbnail::{ThumbnailError, rename_photo_with_uuid};

/// Error type for photo gallery operations
#[derive(Debug)]
pub enum PhotoGalleryError {
    ThumbnailError(ThumbnailError),
    NotFound(String),
    IoError(std::io::Error),
    Other(String),
}

impl std::fmt::Display for PhotoGalleryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhotoGalleryError::ThumbnailError(e) => write!(f, "Thumbnail error: {}", e),
            PhotoGalleryError::NotFound(msg) => write!(f, "Not found: {}", msg),
            PhotoGalleryError::IoError(e) => write!(f, "IO error: {}", e),
            PhotoGalleryError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PhotoGalleryError {}

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
///
/// This service provides file system utilities for photo management.
/// Database operations are handled by the main app through SpacetimeDB bindings.
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

    /// Process a photo: rename with UUID and generate thumbnails
    ///
    /// Returns (new_path, small_thumbnail_path, medium_thumbnail_path)
    /// The caller is responsible for saving this metadata to SpacetimeDB.
    pub async fn process_photo(
        &self,
        path: String,
    ) -> Result<(String, String, String), PhotoGalleryError> {
        log::debug!("Processing photo: {}", path);

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

        Ok((new_path, small_thumb, medium_thumb))
    }

    /// Get photo with local file check
    ///
    /// Checks if the file exists locally, returns PhotoResult indicating availability.
    pub fn get_photo(
        &self,
        relative_path: &str,
        size: PhotoSize,
    ) -> Result<PhotoResult, PhotoGalleryError> {
        // Determine which file to load based on size
        let file_path = match size {
            PhotoSize::Small => {
                let mut thumb_path = std::path::PathBuf::from(relative_path);
                if let Some(name) = thumb_path.file_stem() {
                    thumb_path.set_file_name(format!("{}_128.webp", name.to_string_lossy()));
                }
                thumb_path.to_string_lossy().to_string()
            }
            PhotoSize::Medium => {
                let mut thumb_path = std::path::PathBuf::from(relative_path);
                if let Some(name) = thumb_path.file_stem() {
                    thumb_path.set_file_name(format!("{}_512.webp", name.to_string_lossy()));
                }
                thumb_path.to_string_lossy().to_string()
            }
            PhotoSize::Original => relative_path.to_string(),
        };

        let absolute_path = self.get_absolute_photo_path(&file_path);

        // Check if file exists locally
        if std::path::Path::new(&absolute_path).exists() {
            match std::fs::read(&absolute_path) {
                Ok(bytes) => return Ok(PhotoResult::Available(bytes)),
                Err(e) => {
                    log::warn!("Failed to read file {}: {}", absolute_path, e);
                }
            }
        }

        // File doesn't exist locally
        Ok(PhotoResult::Failed(
            "Photo not available locally".to_string(),
            0,
        ))
    }

    /// Get photo file for display
    ///
    /// Returns the path to the photo file that exists locally.
    pub fn get_photo_file_path(&self, relative_path: &str, size: PhotoSize) -> Option<String> {
        // Determine which file to check based on size
        let file_path = match size {
            PhotoSize::Small => {
                let mut thumb_path = std::path::PathBuf::from(relative_path);
                if let Some(name) = thumb_path.file_stem() {
                    thumb_path.set_file_name(format!("{}_128.webp", name.to_string_lossy()));
                }
                thumb_path.to_string_lossy().to_string()
            }
            PhotoSize::Medium => {
                let mut thumb_path = std::path::PathBuf::from(relative_path);
                if let Some(name) = thumb_path.file_stem() {
                    thumb_path.set_file_name(format!("{}_512.webp", name.to_string_lossy()));
                }
                thumb_path.to_string_lossy().to_string()
            }
            PhotoSize::Original => relative_path.to_string(),
        };

        let absolute_path = self.get_absolute_photo_path(&file_path);

        // Check if file exists locally
        if std::path::Path::new(&absolute_path).exists() {
            Some(absolute_path)
        } else {
            None
        }
    }
}
