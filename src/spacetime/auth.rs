//! Authentication token persistence for SpacetimeDB.
//!
//! This module handles saving and restoring the client identity token
//! across app restarts, ensuring users maintain the same SpacetimeDB identity.

use crate::services::spacetime_settings_service;
use crate::spacetime_module_bindings::dioxus::{use_spacetimedb_context, ConnectionState};
use dioxus::prelude::*;

/// Load the saved SpacetimeDB authentication token from persistent storage.
///
/// Returns `None` if no token has been saved yet or if loading fails.
pub fn load_saved_token() -> Option<String> {
    spacetime_settings_service::load_spacetime_settings()
        .ok()
        .and_then(|s| {
            if s.token.is_empty() {
                None
            } else {
                Some(s.token)
            }
        })
}

/// A hook that watches the SpacetimeDB connection state and automatically
/// persists the authentication token whenever a successful connection is established.
///
/// This ensures that the token is saved after each connection, allowing the app
/// to reuse the same identity across restarts.
#[must_use = "this hook must be called in a component to enable token persistence"]
pub fn use_persist_spacetime_token() {
    let ctx = use_spacetimedb_context();
    let connection_state = ctx.state;

    use_effect(move || {
        // When the connection state changes to Connected, extract and save the token
        if let ConnectionState::Connected(_identity, ref token) = *connection_state.read() {
            // Save the token to persistent settings
            if let Ok(mut settings) = spacetime_settings_service::load_spacetime_settings() {
                settings.token = token.clone();
                if let Err(e) = spacetime_settings_service::save_spacetime_settings(&settings) {
                    log::error!("Failed to save SpacetimeDB token: {}", e);
                } else {
                    log::info!("SpacetimeDB token persisted successfully");
                }
            }
        }
    });
}
