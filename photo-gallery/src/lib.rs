//! # Photo Gallery
//!
//! A reusable photo gallery management library with thumbnail generation and storage.
//!
//! This crate provides cross-platform photo management functionality, including:
//! - Photo storage and retrieval
//! - Automatic thumbnail generation (WebP format)
//! - Database integration with SQLite
//! - Support for multiple photo sizes (small, medium, original)
//!
//! ## Platform Separation
//!
//! This crate focuses on cross-platform photo logic. Platform-specific code
//! (e.g., Android JNI camera integration) should remain in the application crate.
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use photo_gallery::{PhotoGalleryService, PhotoGalleryConfig};
//!
//! let config = PhotoGalleryConfig {
//!     storage_path: "/path/to/photos".to_string(),
//!     enable_thumbnails: true,
//!     thumbnail_small_size: 128,
//!     thumbnail_medium_size: 512,
//! };
//!
//! let service = PhotoGalleryService::new(config);
//! ```

pub mod models;
pub mod schema;
pub mod service;
pub mod thumbnail;

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "sync")]
pub mod upload;

#[cfg(feature = "sync")]
pub mod download;

#[cfg(feature = "components")]
pub mod components;

pub mod picker;

pub use models::{Photo, PhotoCollection, PhotoGalleryConfig, PhotoResult, PhotoSize};
pub use schema::{init_photo_schema, migrate_existing_photos_to_collections};
pub use service::{PhotoGalleryError, PhotoGalleryService};
pub use thumbnail::{create_thumbnails, rename_photo_with_uuid, ThumbnailError};

#[cfg(feature = "sync")]
pub use sync::{PhotoSyncConfig, PhotoSyncService};

#[cfg(feature = "sync")]
pub use upload::{PhotoUploadConfig, PhotoUploadService, UploadError, UploadResult};

#[cfg(feature = "sync")]
pub use download::{DownloadError, DownloadResult, PhotoDownloadConfig, PhotoDownloadService};

#[cfg(feature = "components")]
pub use components::{
    get_collection_photos, get_collection_preview_path, get_collection_preview_uuid,
    get_photo_path, CollectionFullscreen, FullscreenImage, PhotoGalleryContext, PreviewCollection,
    PreviewImage, ThumbnailCollection, ThumbnailImage,
};

pub use picker::{
    capture_photo, has_camera_permission, pick_image, pick_images, AndroidPickerConfig, PickerError,
};
