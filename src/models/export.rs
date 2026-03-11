use serde::{Deserialize, Serialize};

/// V2 Export format metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportMetadata {
    pub format_version: u32,
    pub exported_at: String, // ISO 8601 format
    pub app_version: String,
    pub device_id: String,
}

/// V2 Export container for all data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportData {
    pub metadata: ExportMetadata,
    pub devices: Vec<DeviceExport>,
    pub quails: Vec<QuailExport>,
    pub quail_events: Vec<QuailEventExport>,
    pub egg_records: Vec<EggRecordExport>,
    pub photos: Vec<PhotoExport>,
    pub photo_collections: Vec<PhotoCollectionExport>,
}

/// Device export (all fields from SpacetimeDB)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceExport {
    pub device_id: String,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub first_seen: i64,
    pub last_seen: i64,
    pub owner: String,
}

/// Quail export (all fields from SpacetimeDB)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuailExport {
    pub uuid: String,
    pub name: String,
    pub gender: String,
    pub ring_color: Option<String>,
    pub profile_photo: Option<String>,
    pub birthday: Option<String>,
    pub device_id: String,
    pub owner: String,
    pub created_at: i64,
}

/// Quail event export (all fields from SpacetimeDB)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuailEventExport {
    pub uuid: String,
    pub quail_uuid: String,
    pub event_type: String,
    pub event_date: String, // ISO 8601 format
    pub notes: Option<String>,
    pub photos: Option<String>,
    pub device_id: String,
    pub owner: String,
}

/// Egg record export (all fields from SpacetimeDB)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EggRecordExport {
    pub uuid: String,
    pub record_date: i64,
    pub total_eggs: i32,
    pub notes: Option<String>,
    pub device_id: String,
    pub owner: String,
}

/// Photo export (all fields from SpacetimeDB)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhotoExport {
    pub uuid: String,
    pub collection_uuid: String,
    pub relative_path: String,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub last_sync_attempt: Option<i64>,
    pub retry_count: i32,
    pub device_id: String,
    pub owner: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Photo collection export (all fields from SpacetimeDB)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhotoCollectionExport {
    pub uuid: String,
    pub quail_uuid: Option<String>,
    pub event_uuid: Option<String>,
    pub preview_photo_uuid: Option<String>,
    pub name: String,
    pub device_id: String,
    pub owner: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ExportData {
    pub fn new(metadata: ExportMetadata) -> Self {
        Self {
            metadata,
            devices: Vec::new(),
            quails: Vec::new(),
            quail_events: Vec::new(),
            egg_records: Vec::new(),
            photos: Vec::new(),
            photo_collections: Vec::new(),
        }
    }
}
