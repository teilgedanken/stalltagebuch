//! Authentication token persistence for SpacetimeDB.
//!
//! This module handles saving and restoring the client identity token
//! across app restarts, ensuring users maintain the same SpacetimeDB identity.

use crate::services::device_id_service;
use crate::services::spacetime_settings_service;
use crate::spacetime_module_bindings::dioxus::{ConnectionState, use_spacetimedb_context};
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
        if let ConnectionState::Connected(_identity, ref token) = connection_state().clone() {
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

/// A hook that automatically registers the device with SpacetimeDB when connected.
///
/// This should be called once in the root App component. It will:
/// 1. Get the device ID (ANDROID_ID on Android)
/// 2. Call the register_device reducer when connected
/// 3. Update the device's last_seen timestamp on each connection
///
/// The device registration happens every time the app connects, ensuring the
/// last_seen timestamp is kept up to date.
#[must_use = "this hook must be called in a component to enable device registration"]
pub fn use_register_device() {
    let ctx = use_spacetimedb_context();
    let connection_state = ctx.state.clone();
    let register_device = crate::spacetime_module_bindings::dioxus::use_reducer_register_device();
    let devices = crate::spacetime_module_bindings::dioxus::use_table_devices();

    use_effect(move || {
        // Read the connection state to make this effect reactive
        let state = connection_state().clone();
        log::info!(
            "Device registration: connection state changed to {:?}",
            state
        );

        // When the connection state changes to Connected, register the device
        if let ConnectionState::Connected(_identity, _token) = state {
            log::info!("Device registration: connected, starting registration");
            let register_device_fn = register_device.clone();

            // Get the device ID before spawning async
            let device_id = match device_id_service::get_device_id() {
                Ok(id) => {
                    log::info!("Device registration: got device ID: {}", id);
                    id
                }
                Err(e) => {
                    log::error!("Device registration: failed to get device ID: {}", e);
                    return;
                }
            };

            // Check if device already has a name in the devices table
            let devices_vec = devices().clone();
            let existing_device = devices_vec.iter().find(|d| d.device_id == device_id);
            let has_existing_name = existing_device.and_then(|d| d.name.as_ref()).is_some();

            log::info!(
                "Device registration: checking existing device - has_name={}",
                has_existing_name
            );

            // Get the device model name only if no existing name
            let device_name = if has_existing_name {
                log::info!(
                    "Device registration: device {} already has a name, skipping model name",
                    device_id
                );
                None
            } else {
                log::info!("Device registration: getting device model for new device");
                match device_id_service::get_device_model() {
                    Ok(model) => {
                        log::info!("Device registration: got device model: {}", model);
                        Some(model)
                    }
                    Err(e) => {
                        log::warn!("Device registration: failed to get device model: {}", e);
                        None
                    }
                }
            };

            spawn(async move {
                // Register the device (creates or updates last_seen on reconnect).
                log::info!(
                    "Device registration: calling register_device reducer for device {}",
                    device_id
                );
                register_device_fn(
                    crate::spacetime_module_bindings::register_device_args_type::RegisterDeviceArgs {
                        device_id: device_id.clone(),
                        name: device_name,
                        comment: None,
                    },
                );
                log::info!(
                    "Device registration: register_device reducer called for device {}",
                    device_id
                );
            });
        } else {
            log::info!("Device registration: not connected, skipping registration");
        }
    });
}
