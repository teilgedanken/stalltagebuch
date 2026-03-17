use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use crate::spacetime::{ConnectionState, use_spacetimedb_context};
use dioxus::prelude::*;

// ─── SpacetimeDB settings card ────────────────────────────────────────────────

/// Card component that displays connection status and allows configuring
/// SpacetimeDB connection settings.
/// Note: Full integration with the generated SDK hooks is in progress.
#[component]
pub fn SpacetimeDbCard() -> Element {
    let ctx = use_spacetimedb_context();

    // Load persisted settings on mount.
    let saved = spacetime_settings_service::load_spacetime_settings().unwrap_or_default();
    let mut server_url = use_signal(|| saved.server_url.clone());
    let mut database_name = use_signal(|| saved.database_name.clone());
    let mut token = use_signal(|| saved.token.clone());
    let mut status_msg = use_signal(|| String::new());

    let conn_state = ctx.state;

    let save_and_connect = move |_| {
        let url = server_url();
        let db = database_name();
        let tok = token();

        if url.is_empty() || db.is_empty() {
            status_msg.set("⚠️ Please fill in server URL, database name".to_string());
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
                status_msg.set("✅ Settings saved.".to_string());
                // TODO: Trigger reconnection with new settings via generated SDK hooks
            }
            Err(e) => {
                status_msg.set(format!("❌ Save failed: {e}"));
                return;
            }
        }
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "🗄️ SpacetimeDB" }

            // Connection state indicator
            {
                match conn_state() {
                    ConnectionState::Disconnected => rsx! {
                        p { style: "margin: 0 0 10px 0; color: #888;", "Not connected" }
                    },
                    ConnectionState::Connecting => rsx! {
                        p { style: "margin: 0 0 10px 0; color: #f57c00;", "⏳ Connecting…" }
                    },
                    ConnectionState::Connected(_, _) => rsx! {
                        p { style: "margin: 0 0 10px 0; color: #2e7d32;", "✅ Connected" }
                    },
                    ConnectionState::Error => rsx! {
                        p { style: "margin: 0 0 10px 0; color: #c62828;", "❌ Error connecting" }
                    },
                }
            }

            // Server URL
            label { style: "display: block; font-size: 13px; color: #555; margin-bottom: 4px;",
                "Server URL (e.g. https://testnet.spacetimedb.com)"
            }
            input {
                r#type: "text",
                value: "{server_url}",
                style: "width: 100%; box-sizing: border-box; padding: 8px; border: 1px solid #ddd; border-radius: 6px; font-size: 14px; margin-bottom: 10px;",
                oninput: move |e| server_url.set(e.value()),
            }

            // Database name
            label { style: "display: block; font-size: 13px; color: #555; margin-bottom: 4px;",
                "Database name (e.g. stalltagebuch)"
            }
            input {
                r#type: "text",
                value: "{database_name}",
                style: "width: 100%; box-sizing: border-box; padding: 8px; border: 1px solid #ddd; border-radius: 6px; font-size: 14px; margin-bottom: 10px;",
                oninput: move |e| database_name.set(e.value()),
            }

            // Auth token
            label { style: "display: block; font-size: 13px; color: #555; margin-bottom: 4px;",
                "Auth token (from spacetime login)"
            }
            input {
                r#type: "text",
                value: "{token}",
                style: "width: 100%; box-sizing: border-box; padding: 8px; border: 1px solid #ddd; border-radius: 6px; font-size: 14px; margin-bottom: 12px;",
                oninput: move |e| token.set(e.value()),
            }

            div { style: "display: flex; gap: 8px;",
                button {
                    class: "btn-primary",
                    style: "flex: 1;",
                    onclick: save_and_connect,
                    "💾 Save Settings"
                }
            }

            if !status_msg().is_empty() {
                p { style: "margin: 8px 0 0 0; font-size: 13px; color: #555;", "{status_msg}" }
            }

            details { style: "margin-top: 12px;",
                summary { style: "cursor: pointer; font-size: 13px; color: #0066cc;",
                    "ℹ️ How to deploy the server module"
                }
                div { style: "margin-top: 8px; font-size: 13px; color: #555; line-height: 1.6;",
                    p {
                        "1. Install the SpacetimeDB CLI from "
                        a {
                            href: "https://spacetimedb.com/install",
                            target: "_blank",
                            "spacetimedb.com/install"
                        }
                        " (verify the script before running)."
                    }
                    p {
                        "2. From the project root: "
                        code {
                            "spacetime publish stalltagebuch-server --project-path stalltagebuch-server"
                        }
                    }
                    p {
                        "3. Get your token: "
                        code { "spacetime login" }
                        " (or use an identity token from the web console)."
                    }
                    p {
                        "Dioxus bindings are auto-generated with: "
                        code {
                            "spacetime generate --lang dioxus --out-dir src/spacetime_module_bindings --module-path stalltagebuch-server"
                        }
                    }
                }
            }
        }
    }
}
