// Export/Import service for full local backup

use crate::error::AppError;
use crate::services::photo_service::get_absolute_photo_path;
use base64::Engine as _;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImportMode {
    MergePreferImport,
}

#[derive(Serialize, Deserialize)]
struct ExportMetadata {
    format_version: u32,
    exported_at: String,
    app_version: String,
}

#[derive(Serialize, Deserialize)]
struct ExportQuails {
    quails: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct ExportEvents {
    events: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct ExportEggRecords {
    egg_records: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct ExportPhotos {
    photos: Vec<serde_json::Value>,
}


fn ensure_parent_dir(path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            AppError::Other(format!("Fehler beim Erstellen des Verzeichnisses: {}", e))
        })?;
    }
    Ok(())
}

fn get_export_base_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        PathBuf::from(
            "/storage/emulated/0/Android/data/de.teilgedanken.stalltagebuch/files/exports",
        )
    }

    #[cfg(not(target_os = "android"))]
    {
        PathBuf::from("./exports")
    }
}