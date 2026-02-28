//! Persistence of [`SpacetimeSettings`] to a JSON config file.
//!
//! The file is stored next to the existing SQLite database in the app's data
//! directory so that settings survive across app restarts.

use crate::error::AppError;
use crate::models::SpacetimeSettings;
use stalltagebuch_database::get_app_directory;

const SETTINGS_FILE: &str = "spacetime_settings.json";

fn settings_path() -> std::path::PathBuf {
    let dir = get_app_directory().unwrap_or_else(|| std::path::PathBuf::from("."));
    dir.join(SETTINGS_FILE)
}

/// Load settings from disk.  Returns `Default::default()` if the file does
/// not exist yet.
pub fn load_spacetime_settings() -> Result<SpacetimeSettings, AppError> {
    let path = settings_path();
    if !path.exists() {
        return Ok(SpacetimeSettings::default());
    }
    let data = std::fs::read_to_string(&path)?;
    let settings: SpacetimeSettings =
        serde_json::from_str(&data).map_err(|e| AppError::Other(e.to_string()))?;
    Ok(settings)
}

/// Persist settings to disk.
pub fn save_spacetime_settings(settings: &SpacetimeSettings) -> Result<(), AppError> {
    let path = settings_path();
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| AppError::Other(e.to_string()))?;
    std::fs::write(&path, json)?;
    Ok(())
}
