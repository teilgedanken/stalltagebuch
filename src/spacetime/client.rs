//! HTTP client for the SpacetimeDB HTTP API.
//!
//! SpacetimeDB exposes a REST-like HTTP interface alongside its WebSocket
//! subscription API.  This module uses that HTTP interface so that the app
//! can work without the generated WebSocket bindings (which require
//! `spacetime generate`).
//!
//! Endpoint reference (SpacetimeDB v2):
//! - `POST /v1/database/{name}/call/{reducer}` – call a reducer
//! - `POST /v1/database/{name}/sql`             – execute a SQL query

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Thin async wrapper around the SpacetimeDB HTTP API.
///
/// Clone-cheap: the inner `reqwest::Client` is already `Arc`-wrapped.
#[derive(Clone, Debug)]
pub struct SpacetimeClient {
    http: reqwest::Client,
    /// e.g. `https://testnet.spacetimedb.com`
    pub base_url: String,
    /// Name or identity of the deployed database, e.g. `stalltagebuch`
    pub database: String,
    /// Bearer token obtained from `spacetime login` or the Stalltagebuch
    /// settings screen.
    pub token: String,
}

/// A single row returned by a SQL query.
pub type SqlRow = Value;

/// Result of a SQL query (`POST /v1/database/{db}/sql`).
#[derive(Debug, Deserialize)]
pub struct SqlResult {
    pub schema: Option<Value>,
    pub rows: Vec<SqlRow>,
}

impl SpacetimeClient {
    /// Build a new client.  All fields are required.
    pub fn new(
        base_url: impl Into<String>,
        database: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect(
                "Failed to build reqwest HTTP client. \
                 This usually indicates a TLS configuration problem \
                 (e.g. missing root certificates).",
            );
        Self {
            http,
            base_url: base_url.into(),
            database: database.into(),
            token: token.into(),
        }
    }

    /// Return `true` if this client looks correctly configured (non-empty fields).
    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.database.is_empty() && !self.token.is_empty()
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn reducer_url(&self, reducer: &str) -> String {
        format!(
            "{}/v1/database/{}/call/{}",
            self.base_url.trim_end_matches('/'),
            self.database,
            reducer
        )
    }

    fn sql_url(&self) -> String {
        format!(
            "{}/v1/database/{}/sql",
            self.base_url.trim_end_matches('/'),
            self.database
        )
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Call a reducer with a JSON-serialisable argument.
    pub async fn call_reducer<A: Serialize>(
        &self,
        reducer: &str,
        args: &A,
    ) -> Result<(), AppError> {
        let body = serde_json::to_value(args)
            .map_err(|e| AppError::Other(format!("serialise args: {e}")))?;

        let resp = self
            .http
            .post(self.reducer_url(reducer))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Other(format!(
                "reducer {reducer} failed ({status}): {text}"
            )));
        }
        Ok(())
    }

    /// Execute a SQL query and return the raw JSON rows.
    pub async fn sql(&self, query: &str) -> Result<Vec<SqlRow>, AppError> {
        #[derive(Serialize)]
        struct SqlBody<'a> {
            sql: &'a str,
        }

        let resp = self
            .http
            .post(self.sql_url())
            .bearer_auth(&self.token)
            .json(&SqlBody { sql: query })
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Other(format!(
                "SQL query failed ({status}): {text}"
            )));
        }

        // SpacetimeDB returns either a direct array or {"rows": [...]}
        let json: Value = resp
            .json()
            .await
            .map_err(|e| AppError::Other(format!("parse SQL response: {e}")))?;

        let rows = if let Some(arr) = json.as_array() {
            arr.clone()
        } else if let Some(arr) = json.get("rows").and_then(|v| v.as_array()) {
            arr.clone()
        } else {
            vec![]
        };

        Ok(rows)
    }

    /// Verify the connection by executing a simple query.
    pub async fn ping(&self) -> Result<(), AppError> {
        self.sql("SELECT 1").await?;
        Ok(())
    }
}

// ── Reducer argument types ─────────────────────────────────────────────────────
// These mirror the `*Args` types in the server module and are used to build
// the JSON body for reducer calls.

#[derive(Serialize)]
pub struct CreateQuailArgs {
    pub uuid: String,
    pub name: String,
    pub gender: String,
    pub ring_color: Option<String>,
    pub profile_photo: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateQuailArgs {
    pub uuid: String,
    pub name: String,
    pub gender: String,
    pub ring_color: Option<String>,
    pub profile_photo: Option<String>,
}

#[derive(Serialize)]
pub struct CreateEventArgs {
    pub uuid: String,
    pub quail_uuid: String,
    pub event_type: String,
    pub event_date: String,
    pub notes: Option<String>,
    pub photos: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateEventArgs {
    pub uuid: String,
    pub event_type: String,
    pub event_date: String,
    pub notes: Option<String>,
    pub photos: Option<String>,
}

#[derive(Serialize)]
pub struct UpsertEggRecordArgs {
    pub uuid: String,
    pub record_date: String,
    pub total_eggs: i32,
    pub notes: Option<String>,
}

#[derive(Serialize)]
pub struct SetQuailPhotoArgs {
    pub quail_uuid: String,
    pub photo_uuid: Option<String>,
}
