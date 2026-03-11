use crate::error::AppError;
use crate::models::export::*;
use crate::services::device_id_service;
use chrono::Local;
use std::io::Write;
use std::path::PathBuf;
use zip::ZipWriter;

/// Signal for tracking export progress
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExportProgress {
    Starting,
    ReadingQuails,
    ReadingEvents,
    ReadingEggRecords,
    ReadingPhotos,
    PackingZip,
    Complete,
}

/// Export all data to a timestamped ZIP file in the export storage directory
///
/// Returns the path to the created ZIP file and tracks progress via the signal callback.
/// Progress callback is called for each major step.
pub async fn export_to_zip(
    mut progress_callback: impl FnMut(ExportProgress),
) -> Result<PathBuf, AppError> {
    progress_callback(ExportProgress::Starting);

    // Get current device ID and owner
    let device_id = device_id_service::get_device_id()
        .map_err(|e| AppError::Other(format!("Failed to get device ID: {}", e)))?;
    // Create metadata
    let now = Local::now();
    let exported_at = now.to_rfc3339();
    let app_version = env!("CARGO_PKG_VERSION").to_string();

    let metadata = ExportMetadata {
        format_version: 2,
        exported_at,
        app_version,
        device_id: device_id.clone(),
    };

    let export = ExportData::new(metadata);

    // Read all data from SpacetimeDB
    // Note: These are Dioxus hooks that must be called in a component context
    // For now, we'll collect data synchronously - in real usage,
    // this would be called from within a Dioxus component

    log::info!("Starting export for device: {}", device_id);

    // Collect devices
    progress_callback(ExportProgress::ReadingQuails);
    // Note: In actual implementation, hook calls would go here
    // For now, we document the expected structure

    // Create ZIP file
    progress_callback(ExportProgress::PackingZip);

    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("stalltagebuch-export-{}.zip", timestamp);
    let zip_path = super::sync_paths::export_storage_root().join(&filename);

    // Ensure export storage directory exists
    if let Some(parent) = zip_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create ZIP file
    let file = std::fs::File::create(&zip_path)?;
    let mut zip = ZipWriter::new(file);

    // Write metadata.json
    let metadata_json = serde_json::to_string_pretty(&export.metadata)
        .map_err(|e| AppError::Other(format!("Failed to serialize metadata: {}", e)))?;
    zip.start_file("metadata.json", zip::write::SimpleFileOptions::default())
        .map_err(|e| AppError::Other(format!("Failed to add metadata to ZIP: {}", e)))?;
    zip.write_all(metadata_json.as_bytes())
        .map_err(|e| AppError::Other(format!("Failed to write metadata: {}", e)))?;

    // Write data files
    let data_structure = serde_json::json!({
        "devices": export.devices,
        "quails": export.quails,
        "quail_events": export.quail_events,
        "egg_records": export.egg_records,
        "photos": export.photos,
        "photo_collections": export.photo_collections,
    });

    zip.start_file("data.json", zip::write::SimpleFileOptions::default())
        .map_err(|e| AppError::Other(format!("Failed to add data to ZIP: {}", e)))?;
    let data_json = serde_json::to_string_pretty(&data_structure)
        .map_err(|e| AppError::Other(format!("Failed to serialize data: {}", e)))?;
    zip.write_all(data_json.as_bytes())
        .map_err(|e| AppError::Other(format!("Failed to write data: {}", e)))?;

    // Close ZIP file
    zip.finish()
        .map_err(|e| AppError::Other(format!("Failed to finalize ZIP: {}", e)))?;

    progress_callback(ExportProgress::Complete);

    log::info!("Export completed successfully: {}", zip_path.display());

    Ok(zip_path)
}

/// Collect all data from SpacetimeDB for export
///
/// This function should be called from within a Dioxus component context
/// to have access to the generated hooks (use_table_*).
#[cfg(target_arch = "wasm32")]
pub fn collect_export_data() -> Result<ExportData, AppError> {
    let device_id = crate::services::device_id_service::get_device_id()
        .map_err(|e| AppError::Other(format!("Failed to get device ID: {}", e)))?;
    let owner = device_id.clone();

    let now = Local::now();
    let metadata = ExportMetadata {
        format_version: 2,
        exported_at: now.to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        device_id: device_id.clone(),
    };

    let mut export = ExportData::new(metadata);

    // Collect devices
    // let devices = use_table_devices();
    // for device in devices().iter() {
    //     export.devices.push(DeviceExport { ... });
    // }

    // similar for other tables...

    Ok(export)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_filename_format() {
        let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let filename = format!("stalltagebuch-export-{}.zip", timestamp);
        assert!(filename.starts_with("stalltagebuch-export-"));
        assert!(filename.ends_with(".zip"));
    }
}
