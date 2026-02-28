//! SpacetimeDB client integration for Stalltagebuch.
//!
//! This module provides:
//! - [`SpacetimeClient`] – thin async wrapper around the SpacetimeDB HTTP API.
//! - [`SpacetimeContext`] – Dioxus context that holds reactive signals for
//!   the three main tables (quails, events, egg records) and exposes
//!   reducer call helpers.
//!
//! ## Workflow
//! 1. User enters the SpacetimeDB server URL, database name, and auth token
//!    in the Settings screen.
//! 2. `SpacetimeContext::connect` is called, which fetches the initial table
//!    data via SQL queries and stores it in signals.
//! 3. UI components read those signals; mutations call reducer helpers which
//!    POST to the HTTP reducer endpoint and then refresh the relevant signal.
//!
//! ## Generating typed bindings
//! Once the Dioxus bindings generator is merged into the `spacetime` CLI
//! (see <https://github.com/enaut/SpacetimeDB/pull/4>), run:
//! ```sh
//! spacetime generate --lang dioxus \
//!     --out-dir src/spacetime/module_bindings \
//!     --project-path stalltagebuch-server
//! ```
//! The generated hooks will replace the hand-written reactive layer in this
//! module, giving fine-grained per-row reactivity.

pub mod client;
pub mod context;
pub mod types;

pub use client::SpacetimeClient;
pub use context::{use_spacetime, SpacetimeContext};
pub use types::{RemoteEggRecord, RemoteQuail, RemoteQuailEvent};
