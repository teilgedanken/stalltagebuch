use crate::error::AppError;
use photo_gallery::{PhotoGalleryConfig, PhotoGalleryService, PhotoSize};
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
pub fn convert_photo_error(e: photo_gallery::PhotoGalleryError) -> AppError {
    match e {
        photo_gallery::PhotoGalleryError::NotFound(msg) => AppError::NotFound(msg),
        photo_gallery::PhotoGalleryError::IoError(e) => AppError::Filesystem(e),
        photo_gallery::PhotoGalleryError::ThumbnailError(e) => {
            AppError::Other(format!("Thumbnail error: {}", e))
        }
        photo_gallery::PhotoGalleryError::Other(msg) => AppError::Other(msg),
    }
}

/// Process a new photo: rename and create thumbnails
/// Returns (new_path, small_thumb_path, medium_thumb_path)
pub async fn process_photo(path: String) -> Result<(String, String, String), AppError> {
    let service = init_photo_service();
    service
        .process_photo(path)
        .await
        .map_err(convert_photo_error)
}

/// Get photo file path for a given size
pub fn get_photo_file_path(relative_path: &str, size: PhotoSize) -> Option<String> {
    let service = init_photo_service();
    service.get_photo_file_path(relative_path, size)
}
