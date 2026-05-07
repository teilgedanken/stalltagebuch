use crate::error::AppError;
use crate::image_processing::{CropRect, crop_image};
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

/// Crop a photo with versioning and then regenerate thumbnails
///
/// # Arguments
/// * `path` - Path to the photo to crop (relative path like "uuid.jpg" or "uuid-v1.jpg")
/// * `crop_rect` - Crop bounds with normalized coordinates (0.0-1.0)
/// * `photo_uuid` - The UUID of the photo (without extension)
/// * `current_version` - Current version number (0 for original, 1+ for crops)
///
/// # Returns
/// Result with (new_relative_path, small_thumb_path, medium_thumb_path, new_version) of the cropped photo
pub async fn crop_and_process_photo(
    path: String,
    crop_rect: CropRect,
    photo_uuid: String,
    current_version: i32,
) -> Result<(String, String, String, i32), AppError> {
    // Convert relative path to absolute if needed
    let absolute_path = if path.starts_with('/') {
        path.clone()
    } else {
        let storage_path = get_storage_path();
        std::path::PathBuf::from(&storage_path)
            .join(&path)
            .to_string_lossy()
            .to_string()
    };

    log::debug!("Cropping photo at absolute path: {}", absolute_path);

    let photo_path = std::path::PathBuf::from(&absolute_path);

    // Determine the new version number for the crop
    let new_version = current_version + 1;

    // Create the versioned cropped filename
    let ext = photo_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let versioned_filename = format!("{}-v{}.{}", photo_uuid, new_version, ext);

    let temp_dir = photo_path.parent().ok_or_else(|| {
        AppError::ImageProcessing("Cannot get parent directory of photo".to_string())
    })?;
    let cropped_path = temp_dir.join(&versioned_filename);

    // Perform the crop
    crop_image(&photo_path, &cropped_path, crop_rect).map_err(|e| {
        // Clean up if crop fails
        let _ = std::fs::remove_file(&cropped_path);
        e
    })?;

    // Extract the relative path by removing the storage prefix
    let storage_path = get_storage_path();
    let new_relative_path = cropped_path
        .to_string_lossy()
        .strip_prefix(&format!("{}/", storage_path))
        .unwrap_or(versioned_filename.as_str())
        .to_string();

    log::debug!(
        "Crop successful: new version {}, saved as {}",
        new_version,
        new_relative_path
    );

    // Regenerate thumbnails for the new cropped version
    let service = init_photo_service();
    let (small_size, medium_size) = service.thumbnail_sizes();

    // Use the versioned file stem (e.g. "uuid-v1") as the thumbnail key so that
    // get_photo_file_path("uuid-v1.jpg", Small) finds "uuid-v1_128.webp" correctly.
    let versioned_stem = cropped_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&versioned_filename)
        .to_string();

    let cropped_path_clone = cropped_path.to_string_lossy().to_string();
    let thumbnail_result = tokio::task::spawn_blocking(move || {
        photo_gallery::thumbnail::create_thumbnails(
            &cropped_path_clone,
            &versioned_stem,
            small_size,
            medium_size,
        )
    })
    .await
    .map_err(|e| AppError::Other(format!("Thumbnail generation task failed: {}", e)))?
    .map_err(|e| AppError::ImageProcessing(format!("Failed to regenerate thumbnails: {}", e)))?;

    Ok((
        new_relative_path,
        thumbnail_result.0,
        thumbnail_result.1,
        new_version,
    ))
}
