use chrono::{Local, TimeZone};
use dioxus::prelude::*;

// ─── Devices card ─────────────────────────────────────────────────────────────

/// Card component that displays all registered devices and highlights the current one.
#[component]
pub fn DevicesCard() -> Element {
    // Subscribe to the devices table
    crate::spacetime::use_subscription(&["SELECT * FROM devices"]);

    let devices = crate::dioxus_spacetime_module_bindings::dioxus::use_table_devices();
    let mut current_device_id = use_signal(|| String::new());

    // Get current device ID on mount
    use_effect(
        move || match crate::services::device_id_service::get_device_id() {
            Ok(id) => {
                current_device_id.set(id);
            }
            Err(e) => {
                log::error!("Failed to get current device ID: {}", e);
            }
        },
    );

    let devices_vec = devices().clone();
    let current_id = current_device_id();

    // Format timestamp from i64 (seconds) to readable string
    let format_timestamp = |seconds: i64| -> String {
        if let Some(dt) = Local.timestamp_opt(seconds, 0).single() {
            dt.format("%d.%m.%Y %H:%M:%S").to_string()
        } else {
            "Unknown".to_string()
        }
    };

    rsx! {
        div { class: "box",
            h2 { class: "title is-5", "📱 Geräte" }

            if devices_vec.is_empty() {
                p { class: "has-text-grey",
                    "Keine Geräte registriert"
                }
            } else {
                div { class: "is-flex is-flex-direction-column", style: "gap: 8px;",
                    for device in devices_vec.iter() {
                        {
                            let is_current = device.device_id == current_id;
                            let bg_class = if is_current {
                                "notification is-info is-light"
                            } else {
                                "notification is-light"
                            };

                            rsx! {
                                div { class: "{bg_class}",
                                    div { class: "is-flex is-align-items-center is-justify-content-space-between mb-1",
                                        p { class: "mb-0 has-text-weight-semibold",
                                            "{device.name.as_ref().unwrap_or(&device.device_id)}"
                                        }
                                        if is_current {
                                            span { class: "tag is-info", "Dieses Gerät" }
                                        }
                                    }
                                    p { class: "mb-0 is-size-7 has-text-grey",
                                        "Zuletzt gesehen: {format_timestamp(device.last_seen)}"
                                    }
                                    if device.first_seen != device.last_seen {
                                        p { class: "mb-0 mt-1 is-size-7 has-text-grey-light",
                                            "Registriert: {format_timestamp(device.first_seen)}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
