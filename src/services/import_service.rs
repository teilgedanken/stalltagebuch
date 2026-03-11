use crate::error::AppError;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use zip::ZipArchive;

/// Signal for tracking import progress
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImportProgress {
    Starting,
    ReadingZip,
    DetectingFormat,
    ImportingData,
    ImportingPhotos,
    Complete,
}

/// Import data from a ZIP file, detecting format version automatically
///
/// Supports both v1 (legacy SQLite export) and v2 (SpacetimeDB export) formats.
/// Progress callback is called for each major step.
pub async fn import_from_zip(
    zip_path: &Path,
    mut progress_callback: impl FnMut(String),
) -> Result<(usize, usize), AppError> {
    progress_callback("Starting import...".to_string());

    // Open ZIP file
    progress_callback("Reading ZIP file...".to_string());
    let file = File::open(zip_path)
        .map_err(|e| AppError::Other(format!("Failed to open ZIP file: {}", e)))?;

    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| AppError::Other(format!("Failed to read ZIP archive: {}", e)))?;

    // Detect format version
    progress_callback("Detecting format version...".to_string());

    // Try to read metadata.json to determine format version
    let format_version = detect_format_version(&mut archive)?;

    log::info!("Detected export format version: {}", format_version);

    // Re-open archive for actual import (since we consumed it during detection)
    let file = File::open(zip_path)
        .map_err(|e| AppError::Other(format!("Failed to re-open ZIP file: {}", e)))?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| AppError::Other(format!("Failed to re-read ZIP archive: {}", e)))?;

    // Import based on format version
    let (item_count, photo_count) = match format_version {
        1 => {
            log::info!("Importing v1 format...");
            progress_callback("Importing v1 format data...".to_string());
            crate::services::import_v1_service::import_v1_from_zip(
                &mut archive,
                &mut progress_callback,
            )
            .await?
        }
        2 => {
            log::info!("Importing v2 format...");
            progress_callback("Importing v2 format data...".to_string());
            import_v2_from_zip(&mut archive, &mut progress_callback).await?
        }
        _ => {
            return Err(AppError::Other(format!(
                "Unsupported export format version: {}",
                format_version
            )));
        }
    };

    progress_callback("Import complete!".to_string());

    Ok((item_count, photo_count))
}

/// Detect the format version of a ZIP archive
fn detect_format_version(
    archive: &mut ZipArchive<impl std::io::Read + std::io::Seek>,
) -> Result<u32, AppError> {
    // Try to find metadata.json
    let mut file = archive
        .by_name("metadata.json")
        .map_err(|_| AppError::Other("No metadata.json found in archive".to_string()))?;

    let mut content = String::new();
    std::io::Read::read_to_string(&mut file, &mut content)
        .map_err(|e| AppError::Other(format!("Failed to read metadata.json: {}", e)))?;

    // Parse as JSON to determine format
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Other(format!("Failed to parse metadata.json: {}", e)))?;

    let format_version = value
        .get("format_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::Other("Invalid metadata: format_version not found".to_string()))?;

    Ok(format_version as u32)
}

/// Import v2 format data (placeholder for now)
async fn import_v2_from_zip(
    _archive: &mut ZipArchive<impl std::io::Read + std::io::Seek>,
    _progress_callback: &mut impl FnMut(String),
) -> Result<(usize, usize), AppError> {
    // TODO: Implement v2 import
    // - Read data.json
    // - Validate format
    // - Insert into SpacetimeDB via reducers
    // - Handle photos from photos/ directory
    // - Handle conflicts (duplicate UUIDs)

    log::warn!("V2 import not yet implemented");
    Err(AppError::Other("V2 import not yet implemented".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection() {
        // This will be tested with actual ZIP files
    }
}
