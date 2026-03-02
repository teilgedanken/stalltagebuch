//! # Photo Gallery
//!
//! A reusable photo gallery management library with thumbnail generation and storage.
//!
//! This crate provides cross-platform photo management functionality, including:
//! - Photo storage and retrieval
//! - Automatic thumbnail generation (WebP format)
//! - Support for multiple photo sizes (small, medium, original)
//!
//! ## Platform Separation
//!
//! This crate focuses primarily on cross-platform photo logic (storage, thumbnails,
//! and UI components). The only platform-specific boundary currently exposed
//! from this crate is the Android photo picker/camera bridge, which lives in the
//! [`picker`] module and is consumed by the application crate via FFI/JNI.
//!
//! Database operations and sync are handled by the parent application.
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
//! // Process photos and thumbnails
//! let (new_path, small_thumb, medium_thumb) = 
//!     service.process_photo("/tmp/photo.jpg".to_string()).await?;
//! ```

pub mod models;
pub mod service;
pub mod thumbnail;

#[cfg(feature = "components")]
pub mod components;

pub mod picker;

pub use models::{Photo, PhotoCollection, PhotoGalleryConfig, PhotoResult, PhotoSize};
pub use service::{PhotoGalleryError, PhotoGalleryService};
pub use thumbnail::{ThumbnailError, create_thumbnails, rename_photo_with_uuid};

#[cfg(feature = "components")]
pub use components::{
    CollectionFullscreen, FullscreenImage, PhotoGalleryContext, PreviewCollection, PreviewImage,
    ThumbnailCollection, ThumbnailImage,
};

pub use picker::{
    AndroidPickerConfig, PickerError, capture_photo, has_camera_permission, pick_image, pick_images,
};
