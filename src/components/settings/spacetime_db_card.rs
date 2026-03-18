use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use crate::spacetime::{ConnectionState, use_spacetimedb_context};
use dioxus::prelude::*;
use dioxus_i18n::tid;

// ─── SpacetimeDB settings card ────────────────────────────────────────────────

/// Card component that displays connection status and allows configuring
/// SpacetimeDB connection settings.
/// Note: Full integration with the generated SDK hooks is in progress.
#[component]
pub fn SpacetimeDbCard() -> Element {
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
    let quails = ctx.tables.quails;
    let events = ctx.tables.quail_events;
    let egg_records = ctx.tables.egg_records;

    let connection_label = move || match conn_state() {
        ConnectionState::Disconnected => tid!("spacetime-card-connection-disconnected").to_string(),
        ConnectionState::Connecting => tid!("spacetime-card-connection-connecting").to_string(),
        ConnectionState::Connected(_, _) => tid!("spacetime-card-connection-connected").to_string(),
        ConnectionState::Error => tid!("spacetime-card-connection-error").to_string(),
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
                // TODO: Trigger reconnection with new settings via generated SDK hooks
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
        show_details.set(!show_details());
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 16px 0; font-size: 18px; color: #333;", "🗄️ {tid!(\"spacetime-card-title\")}" }

            if !is_logged_in() {
                div { style: "margin-bottom: 16px;",
                    label {
                        style: "display: block; margin-bottom: 4px; font-weight: 600; font-size: 14px;",
                        {tid!("spacetime-card-server-label")}
                    }
                    input {
                        r#type: "url",
                        value: "{server_url}",
                        oninput: move |e| server_url.set(e.value()),
                        placeholder: "https://testnet.spacetimedb.com",
                        style: "width: 100%; padding: 10px; font-size: 16px; border: 1px solid #ccc; border-radius: 4px;",
                    }
                    p {
                        style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                        {tid!("spacetime-card-server-hint")}
                    }
                }

                div { style: "margin-bottom: 16px;",
                    label {
                        style: "display: block; margin-bottom: 4px; font-weight: 600; font-size: 14px;",
                        {tid!("spacetime-card-database-label")}
                    }
                    input {
                        r#type: "text",
                        value: "{database_name}",
                        oninput: move |e| database_name.set(e.value()),
                        placeholder: "stalltagebuch",
                        style: "width: 100%; padding: 10px; font-size: 16px; border: 1px solid #ccc; border-radius: 4px;",
                    }
                    p {
                        style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                        {tid!("spacetime-card-database-hint")}
                    }
                }

                div { style: "margin-bottom: 16px;",
                    label {
                        style: "display: block; margin-bottom: 4px; font-weight: 600; font-size: 14px;",
                        {tid!("spacetime-card-token-label")}
                    }
                    input {
                        r#type: "text",
                        value: "{token}",
                        oninput: move |e| token.set(e.value()),
                        placeholder: "Token aus spacetime login",
                        style: "width: 100%; padding: 10px; font-size: 16px; border: 1px solid #ccc; border-radius: 4px;",
                    }
                    p {
                        style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                        {tid!("spacetime-card-token-hint")}
                    }
                }

                button {
                    class: "btn-primary",
                    onclick: save_and_connect,
                    disabled: server_url().trim().is_empty() || database_name().trim().is_empty() || token().trim().is_empty(),
                    "🔐 {tid!(\"spacetime-card-login-button\")}"
                }
            } else {
                button {
                    onclick: toggle_details,
                    style: "width: 100%; padding: 16px; border: none; border-radius: 8px; font-size: 16px; font-weight: 700; background: #28a745; color: #fff; cursor: pointer; text-align: left;",
                    div { style: "display: flex; align-items: center; justify-content: space-between; gap: 12px;",
                        span { "✅ {tid!(\"spacetime-card-connected-button\")}" }
                        span { style: "font-size: 13px; opacity: 0.95;",
                            {if show_details() {
                                tid!("spacetime-card-hide-details")
                            } else {
                                tid!("spacetime-card-show-details")
                            }}
                        }
                    }
                }

                if show_details() {
                    div { style: "margin-top: 12px; padding: 12px; background: #d4edda; border-radius: 4px; color: #155724;",
                        p { style: "margin: 0 0 8px 0; font-weight: 600;", {tid!("spacetime-card-connection-info")} }
                        p { style: "margin: 0 0 4px 0; font-size: 14px;", {tid!("spacetime-card-server-value", value: server_url())} }
                        p { style: "margin: 0 0 4px 0; font-size: 14px;", {tid!("spacetime-card-database-value", value: database_name())} }
                        p { style: "margin: 0 0 12px 0; font-size: 14px;", {tid!("spacetime-card-status-value", value: connection_label())} }

                        p { style: "margin: 0 0 8px 0; font-weight: 600;", {tid!("spacetime-card-live-stats")} }
                        p { style: "margin: 0 0 4px 0; font-size: 14px;", {tid!("spacetime-card-stats-quails", count: quails().len())} }
                        p { style: "margin: 0 0 4px 0; font-size: 14px;", {tid!("spacetime-card-stats-events", count: events().len())} }
                        p { style: "margin: 0; font-size: 14px;", {tid!("spacetime-card-stats-egg-records", count: egg_records().len())} }

                        button {
                            class: "btn-primary",
                            style: "margin-top: 12px;",
                            onclick: open_login_form,
                            "🔄 {tid!(\"spacetime-card-edit-credentials\")}"
                        }
                    }
                }
            }

            if !status_msg().is_empty() {
                p { style: "margin: 12px 0 0 0; font-size: 13px; color: #555;", "{status_msg}" }
            }
        }
    }
}
