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

/// Summary of a completed export
#[derive(Clone, Debug)]
pub struct ExportStats {
    pub path: PathBuf,
    pub quails: usize,
    pub events: usize,
    pub egg_records: usize,
    pub photos_meta: usize,
    pub photos_files_included: usize,
    pub photos_files_missing: usize,
    pub zip_size_bytes: u64,
}

pub fn build_export_metadata() -> Result<ExportMetadata, AppError> {
    let device_id = device_id_service::get_device_id()
        .map_err(|e| AppError::Other(format!("Failed to get device ID: {}", e)))?;
    let now = Local::now();
    Ok(ExportMetadata {
        format_version: 2,
        exported_at: now.to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        device_id,
    })
}

/// Export the provided data to a timestamped ZIP file in the export storage directory.
pub async fn export_to_zip(
    export: ExportData,
    include_photo_files: bool,
    mut progress_callback: impl FnMut(ExportProgress),
) -> Result<ExportStats, AppError> {
    progress_callback(ExportProgress::PackingZip);

    let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
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

    let mut photos_files_included = 0usize;
    let mut photos_files_missing = 0usize;

    if include_photo_files {
        progress_callback(ExportProgress::ReadingPhotos);

        for photo in &export.photos {
            let relative_path = photo.relative_path.trim_start_matches('/');
            if relative_path.is_empty() {
                continue;
            }

            let absolute_path = crate::services::photo_paths::relative_to_absolute(relative_path);
            if !absolute_path.exists() {
                log::warn!(
                    "Skipping missing photo file during export: {}",
                    absolute_path.display()
                );
                photos_files_missing += 1;
                continue;
            }

            let bytes = std::fs::read(&absolute_path).map_err(|e| {
                AppError::Other(format!(
                    "Failed to read photo file '{}' for export: {}",
                    absolute_path.display(),
                    e
                ))
            })?;

            let zip_entry_path = format!("photos/{}", relative_path);
            zip.start_file(zip_entry_path, zip::write::SimpleFileOptions::default())
                .map_err(|e| AppError::Other(format!("Failed to add photo to ZIP: {}", e)))?;
            zip.write_all(&bytes)
                .map_err(|e| AppError::Other(format!("Failed to write photo to ZIP: {}", e)))?;
            photos_files_included += 1;
        }
    }

    // Close ZIP file
    zip.finish()
        .map_err(|e| AppError::Other(format!("Failed to finalize ZIP: {}", e)))?;

    let zip_size_bytes = std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);

    progress_callback(ExportProgress::Complete);

    log::info!(
        "Export completed: {} ({} quails, {} events, {} egg records, {} photos, {} photo files, {} missing, {} bytes)",
        zip_path.display(),
        export.quails.len(),
        export.quail_events.len(),
        export.egg_records.len(),
        export.photos.len(),
        photos_files_included,
        photos_files_missing,
        zip_size_bytes
    );

    Ok(ExportStats {
        path: zip_path,
        quails: export.quails.len(),
        events: export.quail_events.len(),
        egg_records: export.egg_records.len(),
        photos_meta: export.photos.len(),
        photos_files_included,
        photos_files_missing,
        zip_size_bytes,
    })
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
