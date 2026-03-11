use std::path::PathBuf;

/// Sync path constants for new multi-master sync layout
///
/// Directory structure:
/// ```
/// Stalltagebuch/
/// └── sync/
///     ├── ops/              # append-only operations
///     │   └── <device-id>/
///     │       └── <YYYYMM>/
///     │           └── <ULID>.ndjson
/// ```

/// Operations directory
pub const OPS_DIR: &str = "sync/ops";

/// Build ops path for a device and month
pub fn ops_path(device_id: &str, year_month: &str) -> String {
    format!("{}/{}/{}", OPS_DIR, device_id, year_month)
}

/// Get current year-month string (YYYYMM)
pub fn current_year_month() -> String {
    chrono::Utc::now().format("%Y%m").to_string()
}

/// Get export storage root directory for local backups
///
/// On Android: `/storage/emulated/0/Documents/Stalltagebuch/`
/// On desktop/test: `./exports/`
pub fn export_storage_root() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        PathBuf::from("/storage/emulated/0/Documents/Stalltagebuch")
    }

    #[cfg(not(target_os = "android"))]
    {
        PathBuf::from("./exports")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ops_path() {
        let path = ops_path("device-123", "202511");
        assert_eq!(path, "sync/ops/device-123/202511");
    }

    #[test]
    fn test_export_storage_root() {
        let root = export_storage_root();
        assert!(
            root.to_string_lossy().contains("Stalltagebuch")
                || root.to_string_lossy().contains("exports")
        );
    }
}
