//! Typed, awaitable SpacetimeDB client for non-UI code (e.g. the ZIP importers
//! and background photo sync metadata service).
//!
//! Components use the generated Dioxus hooks.  Plain async services have no
//! component context, so this wraps a [`DbConnection`] and exposes the
//! generated reducers and table access as `async` methods that resolve once the
//! server has executed (or rejected) the reducer or applied subscriptions.

use crate::dioxus_spacetime_module_bindings::{
    CreateEventArgs, CreatePhotoArgs, CreatePhotoCollectionArgs, CreateQuailArgs, DbConnection,
    ReducerEventContext, RegisterDeviceArgs, RemoteReducers, RemoteTables, UpdateEventArgs,
    UpdatePhotoCollectionArgs, UpdatePhotoSyncStatusArgs, UpdateQuailArgs, UpsertEggRecordArgs,
    create_event_reducer::create_event as _,
    create_photo_collection_reducer::create_photo_collection as _,
    create_photo_reducer::create_photo as _, create_quail_reducer::create_quail as _,
    register_device_reducer::register_device as _, set_quail_photo_reducer::set_quail_photo as _,
    update_event_reducer::update_event as _,
    update_photo_collection_reducer::update_photo_collection as _,
    update_photo_sync_status_reducer::update_photo_sync_status as _,
    update_quail_reducer::update_quail as _, upsert_egg_record_reducer::upsert_egg_record as _,
};
use crate::error::AppError;
use spacetimedb_sdk::DbContext as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

const TIMEOUT: Duration = Duration::from_secs(30);

type ReducerResult = Result<Result<(), String>, spacetimedb_sdk::__codegen::InternalError>;
type ReducerCallback = Box<dyn FnOnce(&ReducerEventContext, ReducerResult) + Send>;

struct ConnectionDropGuard(DbConnection);

impl Drop for ConnectionDropGuard {
    fn drop(&mut self) {
        let _ = self.0.disconnect();
    }
}

/// Connection to one SpacetimeDB database. Cheap to clone (`Arc`-backed) and disconnects
/// automatically when the last handle is dropped.
#[derive(Clone)]
pub struct SpacetimeClient {
    inner: Arc<ConnectionDropGuard>,
}

impl SpacetimeClient {
    /// Connect and wait until the connection is established.
    pub async fn connect(base_url: &str, database: &str, token: &str) -> Result<Self, AppError> {
        Self::connect_with_subscriptions(base_url, database, token, &[], |_| {}).await
    }

    /// Connect, optionally subscribe to `queries` (waiting until applied), and invoke `on_disconnect`
    /// when the connection closes.
    pub async fn connect_with_subscriptions(
        base_url: &str,
        database: &str,
        token: &str,
        queries: &[&str],
        on_disconnect: impl FnOnce(Option<String>) + Send + 'static,
    ) -> Result<Self, AppError> {
        let (tx, rx) = oneshot::channel::<Result<(), String>>();
        let tx = Arc::new(Mutex::new(Some(tx)));
        let finish = |tx: &Arc<Mutex<Option<oneshot::Sender<Result<(), String>>>>>,
                      result: Result<(), String>| {
            if let Some(tx) = tx.lock().ok().and_then(|mut g| g.take()) {
                let _ = tx.send(result);
            }
        };

        let owned_queries: Vec<String> = queries.iter().map(|q| (*q).to_string()).collect();
        let (on_ok, on_err, on_disc) = (tx.clone(), tx.clone(), tx);
        let connection = DbConnection::builder()
            .with_uri(base_url)
            .with_database_name(database)
            .with_token(Some(token.to_string()))
            .on_connect(move |conn, _identity, _token| {
                if owned_queries.is_empty() {
                    finish(&on_ok, Ok(()));
                } else {
                    let on_applied = on_ok.clone();
                    let on_sub_err = on_ok;
                    conn.subscription_builder()
                        .on_applied(move |_ctx| finish(&on_applied, Ok(())))
                        .on_error(move |_ctx, err| finish(&on_sub_err, Err(err.to_string())))
                        .subscribe(owned_queries);
                }
            })
            .on_connect_error(move |_ctx, err| finish(&on_err, Err(err.to_string())))
            .on_disconnect(move |_ctx, err| {
                let err_str = err.map(|e| e.to_string());
                on_disconnect(err_str.clone());
                let msg = err_str.unwrap_or_else(|| "disconnected".to_string());
                finish(&on_disc, Err(msg));
            })
            .build()
            .map_err(|e| AppError::Other(format!("failed to connect to SpacetimeDB: {e}")))?;
        let _ = connection.run_threaded();

        match timeout(TIMEOUT, rx).await {
            Ok(Ok(Ok(()))) => Ok(Self {
                inner: Arc::new(ConnectionDropGuard(connection)),
            }),
            Ok(Ok(Err(e))) => Err(AppError::Other(format!(
                "SpacetimeDB connection failed: {e}"
            ))),
            Ok(Err(_)) => Err(AppError::Other(
                "SpacetimeDB connection channel closed".into(),
            )),
            Err(_) => Err(AppError::Other(
                "Timed out connecting to SpacetimeDB".into(),
            )),
        }
    }

    /// Whether the underlying WebSocket connection is still active.
    pub fn is_active(&self) -> bool {
        self.inner.0.is_active()
    }

    /// Explicitly disconnect the underlying connection.
    pub fn disconnect(&self) {
        let _ = self.inner.0.disconnect();
    }

    /// Access the local client table cache populated by subscriptions.
    pub fn db(&self) -> &RemoteTables {
        &self.inner.0.db
    }

    /// Invoke a reducer and await its outcome.
    async fn call(
        &self,
        name: &str,
        invoke: impl FnOnce(&RemoteReducers, ReducerCallback) -> spacetimedb_sdk::Result<()>,
    ) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel::<ReducerResult>();
        invoke(
            &self.inner.0.reducers,
            Box::new(move |_ctx, result| {
                let _ = tx.send(result);
            }),
        )
        .map_err(|e| AppError::Other(format!("failed to dispatch reducer {name}: {e}")))?;

        match timeout(TIMEOUT, rx).await {
            Ok(Ok(Ok(Ok(())))) => Ok(()),
            Ok(Ok(Ok(Err(msg)))) => Err(AppError::Other(format!("reducer {name} failed: {msg}"))),
            Ok(Ok(Err(e))) => Err(AppError::Other(format!(
                "reducer {name} failed internally: {e}"
            ))),
            Ok(Err(_)) => Err(AppError::Other(format!("reducer {name}: callback dropped"))),
            Err(_) => Err(AppError::Other(format!(
                "Timed out waiting for reducer {name}"
            ))),
        }
    }

    pub async fn create_quail(&self, args: CreateQuailArgs) -> Result<(), AppError> {
        self.call("create_quail", |r, cb| r.create_quail_then(args, cb))
            .await
    }

    pub async fn update_quail(&self, args: UpdateQuailArgs) -> Result<(), AppError> {
        self.call("update_quail", |r, cb| r.update_quail_then(args, cb))
            .await
    }

    pub async fn create_event(&self, args: CreateEventArgs) -> Result<(), AppError> {
        self.call("create_event", |r, cb| r.create_event_then(args, cb))
            .await
    }

    pub async fn update_event(&self, args: UpdateEventArgs) -> Result<(), AppError> {
        self.call("update_event", |r, cb| r.update_event_then(args, cb))
            .await
    }

    pub async fn upsert_egg_record(&self, args: UpsertEggRecordArgs) -> Result<(), AppError> {
        self.call("upsert_egg_record", |r, cb| {
            r.upsert_egg_record_then(args, cb)
        })
        .await
    }

    pub async fn register_device(&self, args: RegisterDeviceArgs) -> Result<(), AppError> {
        self.call("register_device", |r, cb| r.register_device_then(args, cb))
            .await
    }

    pub async fn create_photo_collection(
        &self,
        args: CreatePhotoCollectionArgs,
    ) -> Result<(), AppError> {
        self.call("create_photo_collection", |r, cb| {
            r.create_photo_collection_then(args, cb)
        })
        .await
    }

    pub async fn update_photo_collection(
        &self,
        args: UpdatePhotoCollectionArgs,
    ) -> Result<(), AppError> {
        self.call("update_photo_collection", |r, cb| {
            r.update_photo_collection_then(args, cb)
        })
        .await
    }

    pub async fn create_photo(&self, args: CreatePhotoArgs) -> Result<(), AppError> {
        self.call("create_photo", |r, cb| r.create_photo_then(args, cb))
            .await
    }

    pub async fn update_photo_sync_status(
        &self,
        args: UpdatePhotoSyncStatusArgs,
    ) -> Result<(), AppError> {
        self.call("update_photo_sync_status", |r, cb| {
            r.update_photo_sync_status_then(args, cb)
        })
        .await
    }

    pub async fn set_quail_photo(
        &self,
        quail_uuid: String,
        photo_uuid: Option<String>,
    ) -> Result<(), AppError> {
        self.call("set_quail_photo", |r, cb| {
            r.set_quail_photo_then(quail_uuid, photo_uuid, cb)
        })
        .await
    }
}
