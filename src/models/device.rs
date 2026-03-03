//! Device model for tracking devices that connect to the system.

use serde::{Deserialize, Serialize};

/// A device that has connected to the database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// Unique device identifier (e.g., ANDROID_ID on Android).
    pub device_id: String,
    /// Sequential ID assigned by SpacetimeDB.
    pub id: u64,
    /// User-friendly device name (e.g., "Mein Handy", "Tablet").
    pub name: Option<String>,
    /// Optional comment/description for this device.
    pub comment: Option<String>,
    /// Unix timestamp (seconds) when device first connected.
    pub first_seen: i64,
    /// Unix timestamp (seconds) when device last connected.
    pub last_seen: i64,
    /// The SpacetimeDB identity of the user who owns this device.
    pub owner: String,
}

impl Device {
    /// Create a new device with the given ID.
    pub fn new(device_id: String, owner: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Self {
            device_id,
            id: 0,
            name: None,
            comment: None,
            first_seen: now,
            last_seen: now,
            owner,
        }
    }

    /// Create a device with a custom name.
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    /// Create a device with a comment.
    pub fn with_comment(mut self, comment: String) -> Self {
        self.comment = Some(comment);
        self
    }

    /// Get a human-readable name for the device (name or device_id if no name is set).
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.device_id)
    }
}
