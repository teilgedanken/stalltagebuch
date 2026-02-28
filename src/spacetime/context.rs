//! Dioxus context provider and reactive state for SpacetimeDB.
//!
//! Usage in the root `App` component:
//! ```rust
//! // Provide the context once at the root
//! use_context_provider(SpacetimeContext::disconnected);
//! // Connect when settings are available
//! let ctx = use_spacetime();
//! ctx.connect(url, database, token);
//! ```
//!
//! Child components read data through the exposed signals:
//! ```rust
//! let ctx = use_spacetime();
//! let quails = ctx.quails.read();
//! ```

use super::{
    client::{
        CreateEventArgs, CreateQuailArgs, SetQuailPhotoArgs, UpdateEventArgs, UpdateQuailArgs,
        UpsertEggRecordArgs,
    },
    types::{RemoteEggRecord, RemoteQuail, RemoteQuailEvent},
    SpacetimeClient,
};
use crate::error::AppError;
use dioxus::prelude::*;
use serde_json::Value;

// ─── State ────────────────────────────────────────────────────────────────────

/// Whether the client is connected, connecting, or disconnected.
#[derive(Clone, PartialEq, Debug)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// All reactive SpacetimeDB state shared across the component tree.
///
/// Stored as a Dioxus context so that any component can call `use_spacetime()`
/// to access it.
#[derive(Clone)]
pub struct SpacetimeContext {
    pub connection_state: Signal<ConnectionState>,
    pub quails: Signal<Vec<RemoteQuail>>,
    pub events: Signal<Vec<RemoteQuailEvent>>,
    pub egg_records: Signal<Vec<RemoteEggRecord>>,
    pub(crate) client: Signal<Option<SpacetimeClient>>,
}

impl SpacetimeContext {
    /// Create a new, disconnected context.
    pub fn disconnected() -> Self {
        Self {
            connection_state: Signal::new(ConnectionState::Disconnected),
            quails: Signal::new(vec![]),
            events: Signal::new(vec![]),
            egg_records: Signal::new(vec![]),
            client: Signal::new(None),
        }
    }

    /// Return the underlying client if connected.
    pub fn get_client(&self) -> Option<SpacetimeClient> {
        self.client.read().clone()
    }

    /// Connect to SpacetimeDB and load initial data.
    pub fn connect(&mut self, base_url: String, database: String, token: String) {
        let client = SpacetimeClient::new(base_url, database, token);
        let mut state = self.connection_state;
        let mut quails = self.quails;
        let mut events = self.events;
        let mut egg_records = self.egg_records;
        let mut client_sig = self.client;

        state.set(ConnectionState::Connecting);

        spawn(async move {
            match load_all_data(&client).await {
                Ok((q, e, er)) => {
                    quails.set(q);
                    events.set(e);
                    egg_records.set(er);
                    client_sig.set(Some(client));
                    state.set(ConnectionState::Connected);
                }
                Err(err) => {
                    log::error!("SpacetimeDB connect: {err}");
                    state.set(ConnectionState::Error(err.to_string()));
                }
            }
        });
    }

    /// Disconnect and clear all data.
    pub fn disconnect(&mut self) {
        self.client.set(None);
        self.quails.set(vec![]);
        self.events.set(vec![]);
        self.egg_records.set(vec![]);
        self.connection_state.set(ConnectionState::Disconnected);
    }

    /// Reload all table data from the server (e.g. after a mutation).
    pub fn refresh(&mut self) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut quails = self.quails;
        let mut events = self.events;
        let mut egg_records = self.egg_records;
        let mut state = self.connection_state;

        spawn(async move {
            match load_all_data(&client).await {
                Ok((q, e, er)) => {
                    quails.set(q);
                    events.set(e);
                    egg_records.set(er);
                }
                Err(err) => {
                    log::error!("SpacetimeDB refresh: {err}");
                    state.set(ConnectionState::Error(err.to_string()));
                }
            }
        });
    }

    // ── Reducer helpers ──────────────────────────────────────────────────────

    pub fn create_quail(&mut self, args: CreateQuailArgs) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            if let Err(e) = client.call_reducer("create_quail", &args).await {
                log::error!("create_quail: {e}");
            } else {
                ctx.refresh();
            }
        });
    }

    pub fn update_quail(&mut self, args: UpdateQuailArgs) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            if let Err(e) = client.call_reducer("update_quail", &args).await {
                log::error!("update_quail: {e}");
            } else {
                ctx.refresh();
            }
        });
    }

    pub fn delete_quail(&mut self, uuid: String) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            if let Err(e) = client.call_reducer("delete_quail", &uuid).await {
                log::error!("delete_quail: {e}");
            } else {
                ctx.refresh();
            }
        });
    }

    pub fn set_quail_photo(&mut self, quail_uuid: String, photo_uuid: Option<String>) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            let args = SetQuailPhotoArgs {
                quail_uuid,
                photo_uuid,
            };
            if let Err(e) = client.call_reducer("set_quail_photo", &args).await {
                log::error!("set_quail_photo: {e}");
            } else {
                ctx.refresh();
            }
        });
    }

    pub fn create_event(&mut self, args: CreateEventArgs) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            if let Err(e) = client.call_reducer("create_event", &args).await {
                log::error!("create_event: {e}");
            } else {
                ctx.refresh();
            }
        });
    }

    pub fn update_event(&mut self, args: UpdateEventArgs) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            if let Err(e) = client.call_reducer("update_event", &args).await {
                log::error!("update_event: {e}");
            } else {
                ctx.refresh();
            }
        });
    }

    pub fn delete_event(&mut self, uuid: String) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            if let Err(e) = client.call_reducer("delete_event", &uuid).await {
                log::error!("delete_event: {e}");
            } else {
                ctx.refresh();
            }
        });
    }

    pub fn upsert_egg_record(&mut self, args: UpsertEggRecordArgs) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            if let Err(e) = client.call_reducer("upsert_egg_record", &args).await {
                log::error!("upsert_egg_record: {e}");
            } else {
                ctx.refresh();
            }
        });
    }

    pub fn delete_egg_record(&mut self, uuid: String) {
        let Some(client) = self.get_client() else {
            return;
        };
        let mut ctx = self.clone();
        spawn(async move {
            if let Err(e) = client.call_reducer("delete_egg_record", &uuid).await {
                log::error!("delete_egg_record: {e}");
            } else {
                ctx.refresh();
            }
        });
    }
}

/// Convenience hook – reads the shared `SpacetimeContext` from Dioxus context.
pub fn use_spacetime() -> SpacetimeContext {
    use_context::<SpacetimeContext>()
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

async fn load_all_data(
    client: &SpacetimeClient,
) -> Result<
    (
        Vec<RemoteQuail>,
        Vec<RemoteQuailEvent>,
        Vec<RemoteEggRecord>,
    ),
    AppError,
> {
    let (quails, events, egg_records) = tokio::join!(
        fetch_quails(client),
        fetch_events(client),
        fetch_egg_records(client),
    );
    Ok((quails?, events?, egg_records?))
}

async fn fetch_quails(client: &SpacetimeClient) -> Result<Vec<RemoteQuail>, AppError> {
    let rows = client
        .sql("SELECT id, uuid, name, gender, ring_color, profile_photo, owner FROM quails")
        .await?;
    rows.into_iter().map(parse_quail).collect()
}

async fn fetch_events(client: &SpacetimeClient) -> Result<Vec<RemoteQuailEvent>, AppError> {
    let rows = client
        .sql("SELECT id, uuid, quail_uuid, event_type, event_date, notes, photos, owner FROM quail_events")
        .await?;
    rows.into_iter().map(parse_event).collect()
}

async fn fetch_egg_records(client: &SpacetimeClient) -> Result<Vec<RemoteEggRecord>, AppError> {
    let rows = client
        .sql("SELECT id, uuid, record_date, total_eggs, notes, owner FROM egg_records")
        .await?;
    rows.into_iter().map(parse_egg_record).collect()
}

// ─── Row parsers ──────────────────────────────────────────────────────────────

fn str_field(row: &Value, idx: usize) -> Result<String, AppError> {
    row.get(idx)
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| AppError::Other(format!("missing string field at index {idx}")))
}

fn opt_str_field(row: &Value, idx: usize) -> Option<String> {
    row.get(idx).and_then(|v| v.as_str()).map(str::to_owned)
}

fn u64_field(row: &Value, idx: usize) -> Result<u64, AppError> {
    row.get(idx)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| AppError::Other(format!("missing u64 field at index {idx}")))
}

fn i32_field(row: &Value, idx: usize) -> Result<i32, AppError> {
    row.get(idx)
        .and_then(|v| v.as_i64())
        .map(|n| n as i32)
        .ok_or_else(|| AppError::Other(format!("missing i32 field at index {idx}")))
}

fn parse_quail(row: Value) -> Result<RemoteQuail, AppError> {
    let ctx = "quails";
    Ok(RemoteQuail {
        id: u64_field(&row, 0).map_err(|e| AppError::Other(format!("[{ctx}.id] {e}")))?,
        uuid: str_field(&row, 1).map_err(|e| AppError::Other(format!("[{ctx}.uuid] {e}")))?,
        name: str_field(&row, 2).map_err(|e| AppError::Other(format!("[{ctx}.name] {e}")))?,
        gender: str_field(&row, 3).map_err(|e| AppError::Other(format!("[{ctx}.gender] {e}")))?,
        ring_color: opt_str_field(&row, 4),
        profile_photo: opt_str_field(&row, 5),
        owner: str_field(&row, 6).map_err(|e| AppError::Other(format!("[{ctx}.owner] {e}")))?,
    })
}

fn parse_event(row: Value) -> Result<RemoteQuailEvent, AppError> {
    let ctx = "quail_events";
    Ok(RemoteQuailEvent {
        id: u64_field(&row, 0).map_err(|e| AppError::Other(format!("[{ctx}.id] {e}")))?,
        uuid: str_field(&row, 1).map_err(|e| AppError::Other(format!("[{ctx}.uuid] {e}")))?,
        quail_uuid: str_field(&row, 2)
            .map_err(|e| AppError::Other(format!("[{ctx}.quail_uuid] {e}")))?,
        event_type: str_field(&row, 3)
            .map_err(|e| AppError::Other(format!("[{ctx}.event_type] {e}")))?,
        event_date: str_field(&row, 4)
            .map_err(|e| AppError::Other(format!("[{ctx}.event_date] {e}")))?,
        notes: opt_str_field(&row, 5),
        photos: opt_str_field(&row, 6),
        owner: str_field(&row, 7).map_err(|e| AppError::Other(format!("[{ctx}.owner] {e}")))?,
    })
}

fn parse_egg_record(row: Value) -> Result<RemoteEggRecord, AppError> {
    let ctx = "egg_records";
    Ok(RemoteEggRecord {
        id: u64_field(&row, 0).map_err(|e| AppError::Other(format!("[{ctx}.id] {e}")))?,
        uuid: str_field(&row, 1).map_err(|e| AppError::Other(format!("[{ctx}.uuid] {e}")))?,
        record_date: str_field(&row, 2)
            .map_err(|e| AppError::Other(format!("[{ctx}.record_date] {e}")))?,
        total_eggs: i32_field(&row, 3)
            .map_err(|e| AppError::Other(format!("[{ctx}.total_eggs] {e}")))?,
        notes: opt_str_field(&row, 4),
        owner: str_field(&row, 5).map_err(|e| AppError::Other(format!("[{ctx}.owner] {e}")))?,
    })
}
