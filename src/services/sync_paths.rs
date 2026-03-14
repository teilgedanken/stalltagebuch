use std::path::PathBuf;

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
    fn test_export_storage_root() {
        let root = export_storage_root();
        assert!(
            root.to_string_lossy().contains("Stalltagebuch")
                || root.to_string_lossy().contains("exports")
        );
    }
}
