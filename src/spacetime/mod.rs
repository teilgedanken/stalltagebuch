//! SpacetimeDB client integration for Stalltagebuch.
//!
//! This module re-exports the auto-generated Dioxus hooks and types from SpacetimeDB.
//! The bindings are generated using:
//! ```sh
//! spacetime generate --lang dioxus \
//!     --out-dir src/spacetime_module_bindings \
//!     --module-path stalltagebuch-server
//! ```
//!
//! ## Key hooks:
//! - [`use_spacetimedb_context_provider`] – Initialize at app root
//! - [`use_spacetimedb_context`] – Access the context in child components
//! - [`use_table_quails`], [`use_table_quail_events`], [`use_table_egg_records`] – Get table signals
//! - [`use_reducer_create_quail`], [`use_reducer_update_quail`], etc. – Call reducers
//!
//! ## Token Persistence
//! Use [`auth::load_saved_token`] and [`auth::use_persist_spacetime_token`] to handle
//! automatic token persistence across app restarts.

pub mod auth;

// Re-export auth utilities for token persistence
pub use auth::{load_saved_token, use_persist_spacetime_token};

// Re-export the generated module bindings
pub use crate::spacetime_module_bindings::dioxus::*;
pub use crate::spacetime_module_bindings::*;
