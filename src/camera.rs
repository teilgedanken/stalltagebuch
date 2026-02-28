// This module is now a thin wrapper around photo-gallery's picker functionality
// to maintain backward compatibility with existing code.

use crate::error::AppError;
use std::path::PathBuf;

// Re-export photo-gallery picker functionality
pub use photo_gallery::picker::{AndroidPickerConfig, PickerError};

// Helper function to convert PickerError to AppError
fn picker_error_to_app_error(e: photo_gallery::picker::PickerError) -> AppError {
    match e {
        photo_gallery::picker::PickerError::PermissionDenied(msg) => {
            AppError::PermissionDenied(msg)
        }
        photo_gallery::picker::PickerError::Timeout(msg) => AppError::PermissionDenied(msg),
        photo_gallery::picker::PickerError::Cancelled(msg) => AppError::PermissionDenied(msg),
        photo_gallery::picker::PickerError::PlatformNotSupported(msg) => {
            AppError::PermissionDenied(msg)
        }
        photo_gallery::picker::PickerError::Other(msg) => AppError::PermissionDenied(msg),
    }
}

pub fn pick_image() -> Result<PathBuf, AppError> {
    photo_gallery::picker::pick_image().map_err(picker_error_to_app_error)
}

pub fn pick_images() -> Result<Vec<PathBuf>, AppError> {
    photo_gallery::picker::pick_images().map_err(picker_error_to_app_error)
}

pub fn capture_photo() -> Result<PathBuf, AppError> {
    photo_gallery::picker::capture_photo().map_err(picker_error_to_app_error)
}

#[allow(dead_code)]
pub fn has_camera_permission() -> Result<bool, AppError> {
    photo_gallery::picker::has_camera_permission().map_err(picker_error_to_app_error)
}
