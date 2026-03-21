use crate::error::AppError;
use photo_gallery::{PhotoGalleryConfig, PhotoGalleryService};
use std::path::PathBuf;
use std::sync::OnceLock;

// Global photo gallery service
static PHOTO_SERVICE: OnceLock<PhotoGalleryService> = OnceLock::new();

/// Initialize the photo gallery service
pub fn init_photo_service() -> &'static PhotoGalleryService {
    PHOTO_SERVICE.get_or_init(|| {
        let config = PhotoGalleryConfig {
            storage_path: get_storage_path(),
            enable_thumbnails: true,
            thumbnail_small_size: 400,
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
    let storage_root = PathBuf::from(get_storage_path());
    std::fs::create_dir_all(&storage_root)?;

    let source = PathBuf::from(&path);

    // Picker returns files from cacheDir. Move/copy them into the managed photo
    // storage first so UI loaders (which resolve relative paths against photos dir)
    // can actually find originals and thumbnails.
    let process_input = if source.parent() == Some(storage_root.as_path()) {
        path
    } else {
        let ext = source
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| !e.is_empty())
            .unwrap_or("jpg");
        let temp_name = format!("import_{}.{}", uuid::Uuid::new_v4(), ext);
        let temp_target = storage_root.join(temp_name);

        std::fs::copy(&source, &temp_target)?;
        temp_target.to_string_lossy().to_string()
    };

    service
        .process_photo(process_input)
        .await
        .map_err(convert_photo_error)
}
