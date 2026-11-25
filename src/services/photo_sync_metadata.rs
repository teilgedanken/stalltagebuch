use crate::dioxus_spacetime_module_bindings as direct_spacetime;
use crate::dioxus_spacetime_module_bindings::photos_table::PhotosTableAccess as _;
use crate::error::AppError;
use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use crate::spacetime::client::SpacetimeClient;
use spacetimedb_sdk::Table as _;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct SharedMetadataClient {
    key: String,
    client: SpacetimePhotoMetadataClient,
}

static SHARED_METADATA_CLIENT: OnceLock<Mutex<Option<SharedMetadataClient>>> = OnceLock::new();

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
    client: SpacetimeClient,
}

impl SpacetimePhotoMetadataClient {
    pub async fn new(runtime: &PhotoSyncRuntime) -> Result<Self, AppError> {
        let key = runtime_cache_key(runtime);
        let shared = shared_client_store();

        {
            let mut guard = shared
                .lock()
                .map_err(|_| AppError::Other("shared metadata client lock poisoned".to_string()))?;
            if let Some(existing) = guard.as_ref() {
                if existing.key == key {
                    if existing.client.client.is_active() {
                        return Ok(existing.client.clone());
                    }
                    *guard = None;
                }
            }
        }

        let cache_key_for_disconnect = key.clone();
        let client = SpacetimeClient::connect_with_subscriptions(
            &runtime.spacetime_base_url,
            &runtime.spacetime_database,
            &runtime.spacetime_token,
            &["SELECT * FROM photos"],
            move |_err| invalidate_shared_client(&cache_key_for_disconnect),
        )
        .await?;
        let fresh_client = Self { client };

        let mut guard = shared
            .lock()
            .map_err(|_| AppError::Other("shared metadata client lock poisoned".to_string()))?;
        if let Some(existing) = guard.as_ref() {
            if existing.key == key {
                if existing.client.client.is_active() {
                    return Ok(existing.client.clone());
                }
                *guard = None;
            }
        }

        if let Some(old) = guard.replace(SharedMetadataClient {
            key,
            client: fresh_client.clone(),
        }) {
            old.client.client.disconnect();
        }

        Ok(fresh_client)
    }

    pub async fn query_pending_photos(
        &self,
        max_retries: i32,
    ) -> Result<Vec<PhotoSyncRow>, AppError> {
        Ok(self
            .client
            .db()
            .photos()
            .iter()
            .filter(|photo| {
                matches!(
                    photo.sync_status.as_str(),
                    "local_only" | "pending" | "error"
                ) && photo.retry_count < max_retries
            })
            .map(photo_to_sync_row)
            .collect())
    }

    pub async fn query_all_photos(&self) -> Result<Vec<PhotoSyncRow>, AppError> {
        Ok(self
            .client
            .db()
            .photos()
            .iter()
            .map(photo_to_sync_row)
            .collect())
    }

    pub async fn query_photo_by_uuid(&self, uuid: &str) -> Result<Option<PhotoSyncRow>, AppError> {
        Ok(self
            .client
            .db()
            .photos()
            .uuid()
            .find(&uuid.to_string())
            .map(photo_to_sync_row))
    }

    pub async fn update_photo_sync_status(
        &self,
        uuid: &str,
        sync_status: &str,
        sync_error: Option<String>,
        retry_count: i32,
    ) -> Result<(), AppError> {
        self.client
            .update_photo_sync_status(direct_spacetime::UpdatePhotoSyncStatusArgs {
                uuid: uuid.to_string(),
                sync_status: sync_status.to_string(),
                sync_error,
                last_sync_attempt: Some(chrono::Utc::now().timestamp()),
                retry_count,
            })
            .await
    }
}

fn shared_client_store() -> &'static Mutex<Option<SharedMetadataClient>> {
    SHARED_METADATA_CLIENT.get_or_init(|| Mutex::new(None))
}

fn invalidate_shared_client(cache_key: &str) {
    let shared = shared_client_store();
    if let Ok(mut guard) = shared.lock() {
        if guard.as_ref().is_some_and(|entry| entry.key == cache_key) {
            *guard = None;
        }
    }
}

fn runtime_cache_key(runtime: &PhotoSyncRuntime) -> String {
    format!(
        "{}|{}|{}",
        runtime.spacetime_base_url, runtime.spacetime_database, runtime.spacetime_token
    )
}

fn photo_to_sync_row(photo: direct_spacetime::Photo) -> PhotoSyncRow {
    PhotoSyncRow {
        uuid: photo.uuid,
        relative_path: photo.relative_path,
        retry_count: photo.retry_count,
    }
}
