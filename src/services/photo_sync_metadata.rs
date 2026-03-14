use crate::dioxus_spacetime_module_bindings as direct_spacetime;
use crate::dioxus_spacetime_module_bindings::photos_table::PhotosTableAccess as _;
use crate::dioxus_spacetime_module_bindings::update_photo_sync_status_reducer::update_photo_sync_status as _;
use crate::error::AppError;
use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use spacetimedb_sdk::{DbContext as _, Table as _};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::oneshot;
use tokio::time::{Duration, timeout};

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
    connection: Arc<direct_spacetime::DbConnection>,
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
                    if existing.client.connection.is_active() {
                        return Ok(existing.client.clone());
                    }
                    *guard = None;
                }
            }
        }

        let fresh_client = Self::connect_new(runtime).await?;

        let mut guard = shared
            .lock()
            .map_err(|_| AppError::Other("shared metadata client lock poisoned".to_string()))?;
        if let Some(existing) = guard.as_ref() {
            if existing.key == key {
                if existing.client.connection.is_active() {
                    return Ok(existing.client.clone());
                }
                *guard = None;
            }
        }

        if let Some(old) = guard.replace(SharedMetadataClient {
            key,
            client: fresh_client.clone(),
        }) {
            let _ = old.client.connection.disconnect();
        }

        Ok(fresh_client)
    }

    async fn connect_new(runtime: &PhotoSyncRuntime) -> Result<Self, AppError> {
        let (init_tx, init_rx) = oneshot::channel::<Result<(), String>>();
        let init_tx = Arc::new(Mutex::new(Some(init_tx)));
        let cache_key = runtime_cache_key(runtime);

        let init_for_connect = Arc::clone(&init_tx);
        let init_for_disconnect = Arc::clone(&init_tx);
        let cache_key_for_disconnect = cache_key.clone();

        let connection = direct_spacetime::DbConnection::builder()
            .with_uri(&runtime.spacetime_base_url)
            .with_database_name(&runtime.spacetime_database)
            .with_token(Some(runtime.spacetime_token.clone()))
            .on_connect(move |conn, _identity, _token| {
                let init_for_applied = Arc::clone(&init_for_connect);
                let init_for_error = Arc::clone(&init_for_connect);

                conn.subscription_builder()
                    .on_applied(move |_ctx| complete_init(&init_for_applied, Ok(())))
                    .on_error(move |_ctx, err| complete_init(&init_for_error, Err(err.to_string())))
                    .subscribe(["SELECT * FROM photos".to_string()]);
            })
            .on_disconnect(move |_ctx, err| {
                invalidate_shared_client(&cache_key_for_disconnect);
                let message = err.map(|error| error.to_string()).unwrap_or_else(|| {
                    "spacetimedb disconnected before photos subscription applied".to_string()
                });
                complete_init(&init_for_disconnect, Err(message));
            })
            .build()
            .map_err(|e| AppError::Other(format!("failed to connect to SpacetimeDB: {e}")))?;

        let connection = Arc::new(connection);
        let _ = connection.run_threaded();

        match timeout(Duration::from_secs(10), init_rx).await {
            Ok(Ok(Ok(()))) => Ok(Self { connection }),
            Ok(Ok(Err(error))) => Err(AppError::Other(format!(
                "SpacetimeDB photos subscription failed: {error}"
            ))),
            Ok(Err(_)) => Err(AppError::Other(
                "SpacetimeDB photos subscription channel closed unexpectedly".to_string(),
            )),
            Err(_) => Err(AppError::Other(
                "Timed out waiting for SpacetimeDB photos subscription".to_string(),
            )),
        }
    }

    pub async fn query_pending_photos(
        &self,
        max_retries: i32,
    ) -> Result<Vec<PhotoSyncRow>, AppError> {
        Ok(self
            .connection
            .db
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

    pub async fn query_photo_by_uuid(&self, uuid: &str) -> Result<Option<PhotoSyncRow>, AppError> {
        Ok(self
            .connection
            .db
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
        let args = direct_spacetime::UpdatePhotoSyncStatusArgs {
            uuid: uuid.to_string(),
            sync_status: sync_status.to_string(),
            sync_error,
            last_sync_attempt: Some(chrono::Utc::now().timestamp()),
            retry_count,
        };

        let (tx, rx) = oneshot::channel::<Result<(), AppError>>();
        self.connection
            .reducers
            .update_photo_sync_status_then(args, move |_ctx, result| {
                let mapped = match result {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(message)) => Err(AppError::Other(format!(
                        "update_photo_sync_status reducer rejected request: {message}"
                    ))),
                    Err(error) => Err(AppError::Other(format!(
                        "update_photo_sync_status reducer failed internally: {error}"
                    ))),
                };
                let _ = tx.send(mapped);
            })
            .map_err(|e| {
                AppError::Other(format!(
                    "failed to dispatch update_photo_sync_status reducer: {e}"
                ))
            })?;

        match timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AppError::Other(
                "update_photo_sync_status callback channel closed unexpectedly".to_string(),
            )),
            Err(_) => Err(AppError::Other(
                "Timed out waiting for update_photo_sync_status reducer".to_string(),
            )),
        }
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

fn complete_init(
    sender: &Arc<Mutex<Option<oneshot::Sender<Result<(), String>>>>>,
    result: Result<(), String>,
) {
    if let Ok(mut guard) = sender.lock() {
        if let Some(tx) = guard.take() {
            let _ = tx.send(result);
        }
    }
}

fn photo_to_sync_row(photo: direct_spacetime::Photo) -> PhotoSyncRow {
    PhotoSyncRow {
        uuid: photo.uuid,
        relative_path: photo.relative_path,
        sync_status: photo.sync_status,
        retry_count: photo.retry_count,
    }
}
