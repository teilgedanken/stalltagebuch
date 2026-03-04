use crate::error::AppError;
use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct PhotoSyncRuntime {
    pub spacetime_base_url: String,
    pub spacetime_database: String,
    pub spacetime_token: String,
    pub nextcloud_server_url: String,
    pub nextcloud_username: String,
    pub nextcloud_app_password: String,
    pub nextcloud_remote_path: String,
}

#[derive(Clone, Debug)]
pub struct PhotoSyncRow {
    pub uuid: String,
    pub relative_path: String,
    pub sync_status: String,
    pub retry_count: i32,
}

pub fn load_runtime() -> Result<PhotoSyncRuntime, AppError> {
    let settings = spacetime_settings_service::load_spacetime_settings()?;
    validate_settings(settings)
}

fn validate_settings(settings: SpacetimeSettings) -> Result<PhotoSyncRuntime, AppError> {
    if !settings.is_spacetime_configured() {
        return Err(AppError::Validation(
            "SpacetimeDB settings are not configured".to_string(),
        ));
    }

    if !settings.is_nextcloud_configured() || settings.nextcloud_remote_path.trim().is_empty() {
        return Err(AppError::Validation(
            "Nextcloud sync settings are not configured".to_string(),
        ));
    }

    Ok(PhotoSyncRuntime {
        spacetime_base_url: settings.server_url,
        spacetime_database: settings.database_name,
        spacetime_token: settings.token,
        nextcloud_server_url: settings.nextcloud_url,
        nextcloud_username: settings.nextcloud_username,
        nextcloud_app_password: settings.nextcloud_app_password,
        nextcloud_remote_path: settings.nextcloud_remote_path,
    })
}

#[derive(Clone)]
pub struct SpacetimePhotoMetadataClient {
    http: reqwest::Client,
    base_url: String,
    database: String,
    token: String,
}

impl SpacetimePhotoMetadataClient {
    pub fn new(runtime: &PhotoSyncRuntime) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| AppError::Other(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            http,
            base_url: runtime.spacetime_base_url.clone(),
            database: runtime.spacetime_database.clone(),
            token: runtime.spacetime_token.clone(),
        })
    }

    pub async fn query_pending_photos(
        &self,
        max_retries: i32,
    ) -> Result<Vec<PhotoSyncRow>, AppError> {
        let sql = format!(
            "SELECT uuid, relative_path, sync_status, retry_count \
             FROM photos \
             WHERE sync_status IN ('local_only', 'pending', 'error') \
               AND retry_count < {max_retries}"
        );
        self.query_rows(&sql).await
    }

    pub async fn query_photo_by_uuid(&self, uuid: &str) -> Result<Option<PhotoSyncRow>, AppError> {
        let escaped_uuid = uuid.replace('\'', "''");
        let sql = format!(
            "SELECT uuid, relative_path, sync_status, retry_count \
             FROM photos \
             WHERE uuid = '{escaped_uuid}' \
             LIMIT 1"
        );
        let mut rows = self.query_rows(&sql).await?;
        Ok(rows.pop())
    }

    pub async fn update_photo_sync_status(
        &self,
        uuid: &str,
        sync_status: &str,
        sync_error: Option<String>,
        retry_count: i32,
    ) -> Result<(), AppError> {
        #[derive(Serialize)]
        struct UpdatePhotoSyncStatusArgs {
            uuid: String,
            sync_status: String,
            sync_error: Option<String>,
            last_sync_attempt: Option<i64>,
            retry_count: i32,
        }

        let args = UpdatePhotoSyncStatusArgs {
            uuid: uuid.to_string(),
            sync_status: sync_status.to_string(),
            sync_error,
            last_sync_attempt: Some(chrono::Utc::now().timestamp()),
            retry_count,
        };

        self.call_reducer("update_photo_sync_status", &args).await
    }

    async fn query_rows(&self, sql: &str) -> Result<Vec<PhotoSyncRow>, AppError> {
        #[derive(Serialize)]
        struct SqlBody<'a> {
            sql: &'a str,
        }

        let url = format!(
            "{}/v1/database/{}/sql",
            self.base_url.trim_end_matches('/'),
            self.database
        );

        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(&SqlBody { sql })
            .send()
            .await
            .map_err(|e| AppError::Other(format!("spacetimedb sql request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Other(format!(
                "spacetimedb sql failed ({status}): {body}"
            )));
        }

        let payload: Value = resp
            .json()
            .await
            .map_err(|e| AppError::Other(format!("spacetimedb sql parse failed: {e}")))?;

        let rows = extract_rows(&payload)?;
        rows.into_iter().map(parse_photo_row).collect()
    }

    async fn call_reducer<A: Serialize>(&self, reducer: &str, args: &A) -> Result<(), AppError> {
        let url = format!(
            "{}/v1/database/{}/call/{}",
            self.base_url.trim_end_matches('/'),
            self.database,
            reducer
        );

        let resp = self
            .http
            .post(url)
            .bearer_auth(&self.token)
            .json(args)
            .send()
            .await
            .map_err(|e| AppError::Other(format!("spacetimedb reducer call failed: {e}")))?;

        if resp.status().is_success() {
            return Ok(());
        }

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        Err(AppError::Other(format!(
            "reducer {reducer} failed ({status}): {body}"
        )))
    }
}

fn extract_rows(payload: &Value) -> Result<Vec<Value>, AppError> {
    if let Some(rows) = payload.as_array() {
        return Ok(rows.clone());
    }

    if let Some(rows) = payload.get("rows").and_then(Value::as_array) {
        return Ok(rows.clone());
    }

    Err(AppError::Other(
        "invalid spacetimedb sql response format".to_string(),
    ))
}

fn parse_photo_row(row: Value) -> Result<PhotoSyncRow, AppError> {
    let values = row
        .as_array()
        .ok_or_else(|| AppError::Other("spacetimedb row is not an array".to_string()))?;

    let uuid = values
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Other("missing uuid in photo row".to_string()))?
        .to_string();

    let relative_path = values
        .get(1)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Other("missing relative_path in photo row".to_string()))?
        .to_string();

    let sync_status = values
        .get(2)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Other("missing sync_status in photo row".to_string()))?
        .to_string();

    let retry_count = values
        .get(3)
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::Other("missing retry_count in photo row".to_string()))?
        as i32;

    Ok(PhotoSyncRow {
        uuid,
        relative_path,
        sync_status,
        retry_count,
    })
}
