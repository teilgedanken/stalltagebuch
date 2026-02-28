use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a collection of photos
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhotoCollection {
    pub uuid: Uuid,
    pub preview_photo_uuid: Option<Uuid>,
    pub name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Represents a photo with metadata
/// Supports both old schema (quail_id/event_id) and new schema (collection_id)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Photo {
    pub uuid: Uuid,
    // Old schema (deprecated, for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quail_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<Uuid>,
    // New schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_id: Option<Uuid>,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
    pub thumbnail_path: Option<String>,
    pub thumbnail_small_path: Option<String>,
    pub thumbnail_medium_path: Option<String>,
    pub sync_status: Option<String>,
    pub sync_error: Option<String>,
    pub retry_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Size variants for photo retrieval
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoSize {
    Small,    // 128px WebP for lists
    Medium,   // 512px WebP for detail views
    Original, // Full size JPG
}

/// Result of photo retrieval operation
#[derive(Debug, Clone, PartialEq)]
pub enum PhotoResult {
    Available(Vec<u8>),
    Downloading,
    Failed(String, i32), // (error message, retry_count)
}

/// Configuration for photo gallery initialization
#[derive(Debug, Clone)]
pub struct PhotoGalleryConfig {
    /// Base directory for photo storage
    pub storage_path: String,
    /// Database connection (will be passed as reference)
    pub enable_thumbnails: bool,
    /// Thumbnail sizes configuration
    pub thumbnail_small_size: u32,
    pub thumbnail_medium_size: u32,
}

impl Default for PhotoGalleryConfig {
    fn default() -> Self {
        Self {
            storage_path: String::new(),
            enable_thumbnails: true,
            thumbnail_small_size: 256,
            thumbnail_medium_size: 512,
        }
    }
}
