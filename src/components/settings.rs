mod backup_card;
mod devices_card;
mod network_check_card;
mod nextcloud_card;
mod spacetime_db_card;

use crate::Screen;
use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use dioxus::prelude::*;
use dioxus_i18n::tid;

use backup_card::BackupCard;
use devices_card::DevicesCard;
use network_check_card::NetworkCheckCard;
use nextcloud_card::NextcloudCard;
use spacetime_db_card::SpacetimeDbCard;

#[component]
pub fn SettingsScreen(
    on_navigate: EventHandler<Screen>,
    on_spacetime_settings_saved: EventHandler<SpacetimeSettings>,
) -> Element {
    let mut status_message = use_signal_sync(|| String::new());
    let mut is_nextcloud_configured = use_signal_sync(|| {
        spacetime_settings_service::load_spacetime_settings()
            .map(|s| s.is_nextcloud_configured())
            .unwrap_or(false)
    });

    rsx! {
        section { class: "section pt-4 pb-3",
            div { class: "container is-max-tablet",
                div { class: "level mb-4",
                    div { class: "level-left",
                        button {
                            class: "button is-light",
                            onclick: move |_| on_navigate.call(Screen::Home),
                            "← "
                            {tid!("action-back")}
                        }
                    }
                    div { class: "level-item",
                        h1 { class: "title is-4 mb-0",
                            "⚙️ "
                            {tid!("settings-title")}
                        }
                    }
                    div { class: "level-right" }
                }

                if !status_message().is_empty() {
                    div { class: "notification is-info is-light", "{status_message}" }
                }

                NetworkCheckCard {}
                SpacetimeDbCard { on_spacetime_settings_saved }
                NextcloudCard {
                    on_status_message: move |message| status_message.set(message),
                    on_nextcloud_config_changed: move |configured| { is_nextcloud_configured.set(configured) },
                }
                DevicesCard {}
                BackupCard {
                    on_status_message: move |message| status_message.set(message),
                    is_nextcloud_configured: is_nextcloud_configured(),
                }
            }
        }
    }
}
