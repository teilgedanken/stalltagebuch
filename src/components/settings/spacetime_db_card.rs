use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use crate::spacetime::{ConnectionState, use_spacetimedb_context};
use dioxus::prelude::*;
use dioxus_i18n::tid;

const SPACETIME_RECONNECT_EDIT_THRESHOLD: u32 = 3;

// ─── SpacetimeDB settings card ────────────────────────────────────────────────

/// Card component that displays connection status and allows configuring
/// SpacetimeDB connection settings.
/// The save action notifies the app root so the Spacetime session can be
/// re-initialized immediately with the new credentials.
#[component]
pub fn SpacetimeDbCard(on_spacetime_settings_saved: EventHandler<SpacetimeSettings>) -> Element {
    let ctx = use_spacetimedb_context();

    // Load persisted settings on mount.
    let saved = spacetime_settings_service::load_spacetime_settings().unwrap_or_default();
    let mut server_url = use_signal_sync(|| saved.server_url.clone());
    let mut database_name = use_signal_sync(|| saved.database_name.clone());
    let mut token = use_signal_sync(|| saved.token.clone());
    let mut is_logged_in = use_signal_sync(|| saved.is_spacetime_configured());
    let mut show_details = use_signal_sync(|| false);
    let mut status_msg = use_signal(|| String::new());

    let conn_state = ctx.state;
    let connection = ctx.connection;
    let quails = ctx.tables.quails;
    let events = ctx.tables.quail_events;
    let egg_records = ctx.tables.egg_records;

    use_effect(move || {
        if let ConnectionState::Reconnecting { attempt, .. } = conn_state() {
            if attempt >= SPACETIME_RECONNECT_EDIT_THRESHOLD && is_logged_in() {
                is_logged_in.set(false);
                show_details.set(false);
                status_msg.set(format!(
                    "⚠️ {}",
                    tid!("spacetime-card-connection-retry-failed", attempts: attempt)
                ));
            }
        }
    });

    let connection_label = move || {
        let state = conn_state();
        let has_connection = connection().is_some();

        if !has_connection && matches!(state, ConnectionState::Connected(_, _)) {
            return tid!("spacetime-card-connection-disconnected").to_string();
        }

        match state {
            ConnectionState::Disconnected => {
                tid!("spacetime-card-connection-disconnected").to_string()
            }
            ConnectionState::Connecting => tid!("spacetime-card-connection-connecting").to_string(),
            ConnectionState::Reconnecting { .. } => {
                tid!("spacetime-card-connection-connecting").to_string()
            }
            ConnectionState::Connected(_, _) => {
                tid!("spacetime-card-connection-connected").to_string()
            }
            ConnectionState::Error => tid!("spacetime-card-connection-error").to_string(),
        }
    };

    let connection_icon = move || {
        let state = conn_state();
        let has_connection = connection().is_some();

        if !has_connection && matches!(state, ConnectionState::Connected(_, _)) {
            return "⚪";
        }

        match state {
            ConnectionState::Connected(_, _) => "✅",
            ConnectionState::Connecting | ConnectionState::Reconnecting { .. } => "🟡",
            ConnectionState::Error => "🔴",
            ConnectionState::Disconnected => "⚪",
        }
    };

    let status_button_class = move || {
        let state = conn_state();
        let has_connection = connection().is_some();

        if !has_connection && matches!(state, ConnectionState::Connected(_, _)) {
            return "button is-fullwidth is-dark";
        }

        match state {
            ConnectionState::Connected(_, _) => "is-success",
            ConnectionState::Connecting | ConnectionState::Reconnecting { .. } => "is-warning",
            ConnectionState::Error => "is-danger",
            ConnectionState::Disconnected => "is-dark",
        }
    };

    let details_panel_class = move || {
        let state = conn_state();
        let has_connection = connection().is_some();

        if !has_connection && matches!(state, ConnectionState::Connected(_, _)) {
            return "notification is-light";
        }

        match state {
            ConnectionState::Connected(_, _) => "notification is-success is-light",
            ConnectionState::Connecting | ConnectionState::Reconnecting { .. } => {
                "notification is-warning is-light"
            }
            ConnectionState::Error => "notification is-danger is-light",
            ConnectionState::Disconnected => "notification is-light",
        }
    };

    let save_and_connect = move |_| {
        let url = server_url().trim().to_string();
        let db = database_name().trim().to_string();
        let tok = token().trim().to_string();

        if url.is_empty() || db.is_empty() || tok.is_empty() {
            status_msg.set(format!("⚠️ {}", tid!("spacetime-card-fill-required")));
            return;
        }

        let settings = SpacetimeSettings {
            server_url: url.clone(),
            database_name: db.clone(),
            token: tok.clone(),
            ..spacetime_settings_service::load_spacetime_settings().unwrap_or_default()
        };
        match spacetime_settings_service::save_spacetime_settings(&settings) {
            Ok(()) => {
                status_msg.set(format!("✅ {}", tid!("spacetime-card-saved")));
                is_logged_in.set(true);
                show_details.set(false);
                on_spacetime_settings_saved.call(settings);
            }
            Err(e) => {
                status_msg.set(format!(
                    "❌ {}",
                    tid!("spacetime-card-save-failed", error: e.to_string())
                ));
                return;
            }
        }
    };

    let open_login_form = move |_| {
        is_logged_in.set(false);
        show_details.set(false);
    };

    let toggle_details = move |_| {
        if matches!(conn_state(), ConnectionState::Connected(_, _)) {
            show_details.set(!show_details());
    }
    };

    rsx! {
        div { class: "box mb-4",
            h2 { class: "title is-5 mb-4", "🗄️ {tid!(\"spacetime-card-title\")}" }

            div { class: "field has-addons mt-4 is-fullwidth",
                p { class: "control is-expanded",
                    button {
                        class: "{status_button_class()} button is-fullwidth is-small is-light",
                        onclick: toggle_details,
                        div { class: "is-flex is-align-items-center is-justify-content-space-between is-fullwidth",
                            span { "{connection_icon()} {connection_label()}" }
                        }
                    }
                }
                if show_details() {
                    p { class: "control",
                        button {
                            class: "button is-warning is-light is-small",
                            onclick: open_login_form,
                            "✏️ {tid!(\"spacetime-card-edit-credentials\")}"
                        }
                    }
                }
                if matches!(conn_state(), ConnectionState::Connected(_, _)) {p { class: "control",
                    button {
                        class: "{status_button_class()} button is-small is-light",
                        onclick: toggle_details,
                        
                            span {
                            if show_details(){
                                "▼"
                            } else {
                                "▶"
                            }}
                        }
                    }
                }
            }

            if !is_logged_in() {
                div { class: "field",
                    label { class: "label", {tid!("spacetime-card-server-label")} }
                    div { class: "control",
                        input {
                            class: "input",
                            r#type: "url",
                            value: "{server_url}",
                            oninput: move |e| server_url.set(e.value()),
                            placeholder: "https://testnet.spacetimedb.com",
                        }
                    }
                    p { class: "help", {tid!("spacetime-card-server-hint")} }
                }

                div { class: "field",
                    label { class: "label", {tid!("spacetime-card-database-label")} }
                    div { class: "control",
                        input {
                            class: "input",
                            r#type: "text",
                            value: "{database_name}",
                            oninput: move |e| database_name.set(e.value()),
                            placeholder: "stalltagebuch",
                        }
                    }
                    p { class: "help", {tid!("spacetime-card-database-hint")} }
                }

                div { class: "field",
                    label { class: "label", {tid!("spacetime-card-token-label")} }
                    div { class: "control",
                        input {
                            class: "input",
                            r#type: "text",
                            value: "{token}",
                            oninput: move |e| token.set(e.value()),
                            placeholder: "Token aus spacetime login",
                        }
                    }
                    p { class: "help", {tid!("spacetime-card-token-hint")} }
                }

                div { class: "field",
                    div { class: "control",
                        button {
                            class: "button is-link is-fullwidth",
                            onclick: save_and_connect,
                            disabled: server_url().trim().is_empty() || database_name().trim().is_empty() || token().trim().is_empty(),
                            "🔐 {tid!(\"spacetime-card-login-button\")}"
                        }
                    }
                }
            } else {

                if show_details() {
                    div { class: "{details_panel_class()} mt-3",
                        p { class: "has-text-weight-semibold mb-2", {tid!("spacetime-card-connection-info")} }
                        p { class: "mb-1", {tid!("spacetime-card-server-value", value: server_url())} }
                        p { class: "mb-1", {tid!("spacetime-card-database-value", value: database_name())} }
                        p { class: "mb-3", {tid!("spacetime-card-status-value", value: connection_label())} }

                        p { class: "has-text-weight-semibold mb-2", {tid!("spacetime-card-live-stats")} }
                        p { class: "mb-1", {tid!("spacetime-card-stats-quails", count: quails().len())} }
                        p { class: "mb-1", {tid!("spacetime-card-stats-events", count: events().len())} }
                        p { {tid!("spacetime-card-stats-egg-records", count: egg_records().len())} }
                    }
                }
            }

            if !status_msg().is_empty() {
                div { class: "notification is-light mt-4",
                    "{status_msg}"
                }
            }
        }
    }
}
