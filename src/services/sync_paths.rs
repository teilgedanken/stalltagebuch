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
///     ├── snapshots/        # materialized states
///     │   └── <collection>/
///     │       └── <YYYYMMDD>/
///     │           └── <ULID>.json
///     └── control/          # coordination (leases/markers)
///         └── <collection>/
///             ├── latest.json
///             └── compactor/<ULID>.lease
/// ```

/// Base sync directory under remote_path
pub const SYNC_BASE: &str = "sync";

/// Operations directory
pub const OPS_DIR: &str = "sync/ops";

/// Snapshots directory
pub const SNAPSHOTS_DIR: &str = "sync/snapshots";

/// Control directory for coordination
pub const CONTROL_DIR: &str = "sync/control";

/// Build ops path for a device and month
pub fn ops_path(device_id: &str, year_month: &str) -> String {
    format!("{}/{}/{}", OPS_DIR, device_id, year_month)
}

/// Build snapshot path for a collection and date
pub fn snapshot_path(collection: &str, year_month_day: &str) -> String {
    format!("{}/{}/{}", SNAPSHOTS_DIR, collection, year_month_day)
}

/// Build control path for a collection
pub fn control_path(collection: &str) -> String {
    format!("{}/{}", CONTROL_DIR, collection)
}

/// Build latest marker path for a collection
pub fn latest_marker_path(collection: &str) -> String {
    format!("{}/{}/latest.json", CONTROL_DIR, collection)
}

/// Build compactor lease path for a collection
pub fn compactor_lease_path(collection: &str, ulid: &str) -> String {
    format!("{}/{}/compactor/{}.lease", CONTROL_DIR, collection, ulid)
}

/// Get current year-month string (YYYYMM)
pub fn current_year_month() -> String {
    chrono::Utc::now().format("%Y%m").to_string()
}

/// Get current year-month-day string (YYYYMMDD)
pub fn current_year_month_day() -> String {
    chrono::Utc::now().format("%Y%m%d").to_string()
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
    fn test_snapshot_path() {
        let path = snapshot_path("quails", "20251112");
        assert_eq!(path, "sync/snapshots/quails/20251112");
    }

    #[test]
    fn test_control_path() {
        let path = control_path("quails");
        assert_eq!(path, "sync/control/quails");
    }

    #[test]
    fn test_latest_marker_path() {
        let path = latest_marker_path("quails");
        assert_eq!(path, "sync/control/quails/latest.json");
    }
}
