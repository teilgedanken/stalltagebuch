use image::{imageops::FilterType, ImageFormat};
use std::io::Cursor;
use std::path::Path;

/// Error type for thumbnail operations
#[derive(Debug)]
pub enum ThumbnailError {
    ImageLoadError(String),
    ImageSaveError(String),
    IoError(std::io::Error),
    PathError(String),
}

impl std::fmt::Display for ThumbnailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThumbnailError::ImageLoadError(msg) => write!(f, "Image load error: {}", msg),
            ThumbnailError::ImageSaveError(msg) => write!(f, "Image save error: {}", msg),
            ThumbnailError::IoError(e) => write!(f, "IO error: {}", e),
            ThumbnailError::PathError(msg) => write!(f, "Path error: {}", msg),
        }
    }
}

impl std::error::Error for ThumbnailError {}

impl From<std::io::Error> for ThumbnailError {
    fn from(err: std::io::Error) -> Self {
        ThumbnailError::IoError(err)
    }
}

/// Creates multi-size WebP thumbnails from a JPEG image
/// Returns (small_filename, medium_filename) or error
pub fn create_thumbnails(
    original_path: &str,
    uuid: &str,
    small_size: u32,
    medium_size: u32,
) -> Result<(String, String), ThumbnailError> {
    log::debug!("Creating thumbnails for UUID: {}", uuid);

    // Load original image
    let img = image::open(original_path)
        .map_err(|e| ThumbnailError::ImageLoadError(format!("Failed to load image: {}", e)))?;

    let parent_dir = Path::new(original_path)
        .parent()
        .ok_or_else(|| ThumbnailError::PathError("No parent directory found".to_string()))?;

    // Create small thumbnail (128px, 70% quality)
    let small_filename = format!("{}_small.webp", uuid);
    let small_path = parent_dir.join(&small_filename);
    let small_img = img.resize(small_size, small_size, FilterType::Lanczos3);

    let mut small_buffer = Cursor::new(Vec::new());
    small_img
        .write_to(&mut small_buffer, ImageFormat::WebP)
        .map_err(|e| {
            ThumbnailError::ImageSaveError(format!("Failed to write small thumbnail: {}", e))
        })?;

    std::fs::write(&small_path, small_buffer.into_inner())?;

    log::debug!("Small thumbnail created: {:?}", small_path);

    // Create medium thumbnail (512px, 75% quality)
    let medium_filename = format!("{}_medium.webp", uuid);
    let medium_path = parent_dir.join(&medium_filename);
    let medium_img = img.resize(medium_size, medium_size, FilterType::Lanczos3);

    let mut medium_buffer = Cursor::new(Vec::new());
    medium_img
        .write_to(&mut medium_buffer, ImageFormat::WebP)
        .map_err(|e| {
            ThumbnailError::ImageSaveError(format!("Failed to write medium thumbnail: {}", e))
        })?;

    std::fs::write(&medium_path, medium_buffer.into_inner())?;

    log::debug!("Medium thumbnail created: {:?}", medium_path);

    Ok((small_filename, medium_filename))
}

/// Renames a photo file with UUID and returns the new path + thumbnail names
/// Uses spawn_blocking to avoid blocking the async runtime
pub async fn rename_photo_with_uuid(
    original_path: &str,
    small_size: u32,
    medium_size: u32,
) -> Result<(String, String, String), ThumbnailError> {
    let original_path = original_path.to_string();

    tokio::task::spawn_blocking(move || {
        log::debug!("=== rename_photo_with_uuid called ===");
        log::debug!("Original path: {}", original_path);

        let uuid = uuid::Uuid::new_v4().to_string();
        let new_filename = format!("{}.jpg", uuid);

        log::debug!("Generated UUID: {}", uuid);
        log::debug!("New filename: {}", new_filename);

        // File is already in the correct directory, simply rename it there
        let old_path = Path::new(&original_path);

        if let Some(parent_dir) = old_path.parent() {
            let new_path = parent_dir.join(&new_filename);

            log::debug!("Old path: {:?}", old_path);
            log::debug!("New path: {:?}", new_path);
            log::debug!("Checking if old_path exists: {}", old_path.exists());

            if old_path.exists() {
                log::debug!("Path exists, copying...");
                std::fs::copy(old_path, &new_path).map_err(|e| ThumbnailError::IoError(e))?;

                log::debug!("Copy successful, removing original...");
                if let Err(e) = std::fs::remove_file(old_path) {
                    log::warn!("Could not remove original: {}", e);
                } else {
                    log::debug!("Original removed");
                }

                // Create multi-size WebP thumbnails
                log::debug!("Creating thumbnails...");
                let (small_thumb, medium_thumb) =
                    create_thumbnails(new_path.to_str().unwrap(), &uuid, small_size, medium_size)?;

                log::debug!("=== rename_photo_with_uuid completed ===");
                return Ok((new_filename, small_thumb, medium_thumb));
            } else {
                log::error!("ERROR: Original path doesn't exist!");
                return Err(ThumbnailError::PathError(format!(
                    "Original file not found: {}",
                    original_path
                )));
            }
        }

        Err(ThumbnailError::PathError(
            "No parent directory found".to_string(),
        ))
    })
    .await
    .map_err(|e| ThumbnailError::PathError(format!("Task join error: {}", e)))?
}
