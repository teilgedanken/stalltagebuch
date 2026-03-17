mod backup_card;
mod devices_card;
mod network_check_card;
mod nextcloud_card;
mod spacetime_db_card;

use crate::Screen;
use dioxus::prelude::*;
use dioxus_i18n::tid;

use backup_card::BackupCard;
use devices_card::DevicesCard;
use network_check_card::NetworkCheckCard;
use nextcloud_card::NextcloudCard;
use spacetime_db_card::SpacetimeDbCard;

#[component]
pub fn SettingsScreen(on_navigate: EventHandler<Screen>) -> Element {
    let mut status_message = use_signal_sync(|| String::new());

    rsx! {
        div { style: "padding: 16px; max-width: 600px; margin: 0 auto;",
            div { style: "display: flex; align-items: center; margin-bottom: 24px;",
                button {
                    class: "btn-back",
                    onclick: move |_| on_navigate.call(Screen::Home),
                    "← "
                    {tid!("action-back")}
                }
                h1 { style: "flex: 1; text-align: center; margin: 0; font-size: 24px; color: #0066cc;",
                    "⚙️ "
                    {tid!("settings-title")}
                }
                div { style: "width: 80px;" }
            }

            if !status_message().is_empty() {
                div { style: "padding: 12px; margin-bottom: 16px; background: #f0f0f0; border-radius: 8px; border-left: 4px solid #0066cc;",
                    "{status_message}"
                }
            }

            NetworkCheckCard {}
            SpacetimeDbCard {}
            DevicesCard {}
            BackupCard {
                on_status_message: move |message| status_message.set(message),
            }
            NextcloudCard {
                on_status_message: move |message| status_message.set(message),
            }
        }
    }
}
