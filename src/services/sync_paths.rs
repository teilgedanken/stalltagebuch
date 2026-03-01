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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ops_path() {
        let path = ops_path("device-123", "202511");
        assert_eq!(path, "sync/ops/device-123/202511");
    }
}
