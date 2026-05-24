use serde::{Deserialize, Serialize};

/// V1 Quail format (using DateTime strings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuailV1 {
    pub uuid: String,
    pub name: String,
    pub gender: String,
    pub ring_color: Option<String>,
    pub profile_photo: Option<String>,
    pub created_at: String, // DateTime string
    pub updated_at: String, // DateTime string
    pub deleted: i32,
    pub rev: i64,
    pub logical_clock: i64,
}

/// V1 Event format (using DateTime strings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuailEventV1 {
    pub uuid: String,
    pub quail_id: String,
    pub event_type: String,
    pub event_date: String, // Date string (e.g. "2025-11-22")
    pub notes: Option<String>,
    pub created_at: String, // DateTime string
    pub updated_at: String, // DateTime string
    pub deleted: i32,
    pub rev: i64,
    pub logical_clock: i64,
}

/// V1 Egg record format (using DateTime strings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EggRecordV1 {
    pub uuid: String,
    pub record_date: String, // Date string (e.g. "2025-11-21")
    pub total_eggs: i32,
    pub notes: Option<String>,
    pub created_at: String, // DateTime string
    pub updated_at: String, // DateTime string
    pub deleted: i32,
    pub rev: i64,
    pub logical_clock: i64,
}

/// V1 Photo format (using DateTime strings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoV1 {
    pub uuid: String,
    pub quail_id: Option<String>,
    pub event_id: Option<String>,
    pub relative_path: String,
    pub path: String,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub last_sync_attempt: Option<i64>,
    pub retry_count: i32,
    pub thumbnail_path: Option<String>,
    pub thumbnail_small_path: Option<String>,
    pub thumbnail_medium_path: Option<String>,
    pub created_at: String, // DateTime string
    pub updated_at: String, // DateTime string
    pub deleted: i32,
    pub rev: i64,
    pub logical_clock: i64,
}

/// V1 Export container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportContainerV1 {
    pub quails: Vec<QuailV1>,
    pub events: Vec<QuailEventV1>,
    pub egg_records: Vec<EggRecordV1>,
    pub photos: Vec<PhotoV1>,
}

/// Helper to parse DateTime strings from V1 format (e.g. "2025-11-22 16:34:53")
/// Returns Unix timestamp in milliseconds
pub fn parse_v1_datetime(dt_str: &str) -> Result<i64, String> {
    let dt = chrono::NaiveDateTime::parse_from_str(dt_str, "%Y-%m-%d %H:%M:%S")
        .map_err(|e| format!("Failed to parse datetime '{}': {}", dt_str, e))?;
    Ok(dt.and_utc().timestamp_millis())
}

/// Helper to parse V1 date strings (e.g. "2025-11-22")
/// Returns Unix timestamp of midnight UTC in seconds
pub fn parse_v1_date(date_str: &str) -> Result<i64, String> {
    let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| format!("Failed to parse date '{}': {}", date_str, e))?;
    let dt = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    Ok(dt.timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_v1_datetime() {
        let dt_str = "2025-11-22 16:34:53";
        let ts = parse_v1_datetime(dt_str);
        assert!(ts.is_ok());
    }

    #[test]
    fn test_parse_v1_date() {
        let date_str = "2025-11-22";
        let ts = parse_v1_date(date_str);
        assert!(ts.is_ok());
    }
}
