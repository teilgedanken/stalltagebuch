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
use serde::Serialize;

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

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn reducer_url(&self, reducer: &str) -> String {
        format!(
            "{}/v1/database/{}/call/{}",
            self.base_url.trim_end_matches('/'),
            self.database,
            reducer
        )
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Call a reducer with a JSON-serialisable argument.
    pub async fn call_reducer<A: Serialize>(
        &self,
        reducer: &str,
        args: &A,
    ) -> Result<(), AppError> {
        // SpacetimeDB HTTP expects reducer parameters under a top-level `args` key
        // for reducers declared as `fn reducer(ctx: &ReducerContext, args: SomeArgs)`.
        let args_json = serde_json::to_value(args)
            .map_err(|e| AppError::Other(format!("serialise args: {e}")))?;
        let body = serde_json::json!({ "args": args_json });

        let resp = self
            .http
            .post(self.reducer_url(reducer))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Other(format!("network error: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Other(format!(
                "reducer {reducer} failed ({status}): {text}"
            )));
        }
        Ok(())
    }

    /// Call a reducer with an already-shaped JSON body.
    ///
    /// Use this for reducers with non-`args` signatures (e.g. multiple positional
    /// arguments encoded as top-level keys).
    pub async fn call_reducer_raw(
        &self,
        reducer: &str,
        body: &serde_json::Value,
    ) -> Result<(), AppError> {
        let resp = self
            .http
            .post(self.reducer_url(reducer))
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|e| AppError::Other(format!("network error: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Other(format!(
                "reducer {reducer} failed ({status}): {text}"
            )));
        }
        Ok(())
    }
}
