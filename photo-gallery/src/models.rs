use serde::{Deserialize, Serialize};

/// Represents a collection of photos in SpacetimeDB
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhotoCollection {
    /// Unique identifier (UUID string)
    pub uuid: String,
    /// ID assigned by SpacetimeDB
    pub id: u64,
    /// UUID of the quail this collection belongs to (if any)
    pub quail_uuid: Option<String>,
    /// UUID of the event this collection belongs to (if any)
    pub event_uuid: Option<String>,
    /// UUID of the preview photo for this collection
    pub preview_photo_uuid: Option<String>,
    /// Name of the collection
    pub name: String,
    /// Owner identifier
    pub owner: String,
}

/// Represents a photo in SpacetimeDB with metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Photo {
    /// Unique identifier (UUID string)
    pub uuid: String,
    /// ID assigned by SpacetimeDB
    pub id: u64,
    /// Collection UUID this photo belongs to
    pub collection_uuid: String,
    /// Relative path to the photo file
    pub relative_path: String,
    /// Sync status: 'local_only', 'uploading', 'synced', 'download_pending', etc.
    pub sync_status: String,
    /// Error message if sync failed
    pub sync_error: Option<String>,
    /// Timestamp of last sync attempt
    pub last_sync_attempt: Option<i64>,
    /// Retry count for failed operations
    pub retry_count: i32,
    /// Owner identifier
    pub owner: String,
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
