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
        section { class: "section pt-5 pb-3",
            div { class: "container is-max-tablet",
                h1 { class: "title is-3 has-text-centered mb-5",
                    {format!("🥚 {}", tid!("app-title"))}
                }

                if let Err(db_status) = db_status() {
                    div { class: "notification is-warning is-light",
                        strong { "Status: " }
                        "{db_status}"
                    }
                }

                div { class: "box",
                    h2 { class: "title is-5", "Schnellzugriff" }
                    div { class: "buttons are-medium",
                        button {
                            class: "button is-primary is-fullwidth",
                            onclick: move |_| on_navigate.call(Screen::ProfileList),
                            {format!("🐦 {}", tid!("profile-list-title"))}
                        }
                        button {
                            class: "button is-success is-fullwidth",
                            onclick: move |_| on_navigate.call(Screen::EggTracking(None)),
                            {format!("🥚 {}", tid!("egg-tracking-title"))}
                        }
                        button {
                            class: "button is-warning is-fullwidth",
                            onclick: move |_| on_navigate.call(Screen::Statistics),
                            {format!("📊 {}", tid!("stats-title"))}
                        }
                    }
                }

                div { class: "box mt-4",
                    button {
                        class: "button is-light is-fullwidth",
                        onclick: move |_| on_navigate.call(Screen::Settings),
                        {format!("⚙️ {}", tid!("settings-title"))}
                    }
                }

                article { class: "message is-info is-light mt-4",
                    div { class: "message-header", "ℹ️ System-Info" }
                    div { class: "message-body",
                        p {
                            strong { "OS: " }
                            "{std::env::consts::OS}"
                        }
                        p {
                            strong { "Arch: " }
                            "{std::env::consts::ARCH}"
                        }
                        p {
                            {
                                match connection_state() {
                                    ConnectionState::Disconnected => {
                                        tid!("info-spacetimedb-disconnected").to_string()
                                    }
                                    ConnectionState::Connecting => {
                                        tid!("info-spacetimedb-connecting").to_string()
                                    }
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
    }
}
