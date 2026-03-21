use crate::Screen;
use crate::spacetime::{self, ConnectionState, use_spacetimedb_context};
use dioxus::prelude::*;
use dioxus_i18n::tid;

#[component]
pub fn HomeScreen(on_navigate: EventHandler<Screen>) -> Element {
    let quails = spacetime::use_table_quails();
    spacetime::use_subscription(&["SELECT * FROM quails"]);

    let mut db_status = use_signal(|| Err(tid!("status-initializing")));
    let spacetimedb_ctx = use_spacetimedb_context();
    let connection_state = spacetimedb_ctx.state;

    // Initialize database on mount
    use_effect(move || {
        // Database initialization is now handled by SpacetimeDB context
        let count = quails().len();
        db_status.set(Ok(format!("✅ {}", tid!("status-db-ready", count: count))));
    });

    rsx! {
        div { style: "padding: 16px; max-width: 600px; margin: 0 auto; min-height: 100vh; background: #f5f5f5;",
            h1 { style: "color: #0066cc; text-align: center; margin-bottom: 24px; margin-top: 48px; font-size: 28px; font-weight: 700;",
                {format!("🥚 {}", tid!("app-title"))}
            }
            if let Err(db_status) = db_status() {
                // Status Card
                div { class: "card-header",
                    h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #333;",
                        "Status"
                    }
                    p { style: "font-size: 14px; color: #555; margin: 0;", "{db_status}" }
                }
            }
            // Quick Actions
            div { class: "card", style: "margin-bottom: 128px;",
                h2 { style: "margin: 0 0 16px 0; font-size: 18px; color: #333;", "Schnellzugriff" }
                div { style: "display: flex; flex-direction: column; gap: 12px;",
                    button {
                        class: "btn-primary",
                        style: "padding: 16px; font-size: 16px; display: flex; align-items: center; justify-content: center;",
                        onclick: move |_| on_navigate.call(Screen::ProfileList),
                        {format!("🐦 {}", tid!("profile-list-title"))}
                    }
                    button {
                        class: "btn-success",
                        style: "padding: 16px; font-size: 16px; display: flex; align-items: center; justify-content: center;",
                        onclick: move |_| on_navigate.call(Screen::EggTracking(None)),
                        {format!("🥚 {}", tid!("egg-tracking-title"))}
                    }
                    button {
                        style: "padding: 16px; font-size: 16px; background: #ff8c00; color: white; display: flex; align-items: center; justify-content: center;",
                        onclick: move |_| on_navigate.call(Screen::Statistics),
                        {format!("📊 {}", tid!("stats-title"))}
                    }
                }
            }
            // Settings button
            div { class: "card", style: "margin-bottom: 16px;",
                button {
                    class: "btn-secondary",
                    style: "width: 100%; padding: 16px; font-size: 16px; display: flex; align-items: center; justify-content: center;",
                    onclick: move |_| on_navigate.call(Screen::Settings),
                    {format!("⚙️ {}", tid!("settings-title"))}
                }
            }

            // Info Card
            div { style: "background: #f8f9fa; padding: 16px; margin: 16px 0; border-radius: 8px; border: 1px solid #e0e0e0;",
                h3 { style: "margin: 0 0 12px 0; font-size: 14px; color: #666; font-weight: 600;",
                    "ℹ️ System-Info"
                }
                p { style: "font-size: 12px; color: #666; margin: 4px 0;",
                    "OS: {std::env::consts::OS}"
                }
                p { style: "font-size: 12px; color: #666; margin: 4px 0;",
                    "Arch: {std::env::consts::ARCH}"
                }
                p { style: "font-size: 11px; color: #888; margin: 4px 0; word-break: break-all;",
                    {
                        match connection_state() {
                            ConnectionState::Disconnected => {
                                tid!("info-spacetimedb-disconnected").to_string()
                            }
                            ConnectionState::Connecting => tid!("info-spacetimedb-connecting").to_string(),
                            ConnectionState::Reconnecting { .. } => {
                                tid!("info-spacetimedb-connecting").to_string()
                            }
                            ConnectionState::Connected(_, _) => {
                                tid!("info-spacetimedb-connected").to_string()
                            }
                            ConnectionState::Error => tid!("info-spacetimedb-error").to_string(),
                        }
                    }
                }
            }
        }
    }
}
