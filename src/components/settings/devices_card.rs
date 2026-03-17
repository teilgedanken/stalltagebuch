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
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "📱 Geräte" }

            if devices_vec.is_empty() {
                p { style: "margin: 0; color: #888; font-size: 14px;",
                    "Keine Geräte registriert"
                }
            } else {
                div { style: "display: flex; flex-direction: column; gap: 8px;",
                    {
                        devices_vec.iter().map(|device| {
                            let is_current = device.device_id == current_id;
                            let bg_color = if is_current { "#e3f2fd" } else { "#f5f5f5" };
                            let border_color = if is_current { "4px solid #0066cc" } else { "1px solid #ddd" };

                            rsx! {
                                div { style: "padding: 12px; border-radius: 6px; border-left: {border_color}; background: {bg_color}; display: flex; align-items: center; gap: 12px;",
                                    div { style: "flex: 1;",
                                        div { style: "display: flex; align-items: center; gap: 8px; margin-bottom: 6px;",
                                            span { style: "font-weight: 600; font-size: 14px;",
                                                "{device.name.as_ref().unwrap_or(&device.device_id)}"
                                            }
                                            if is_current {
                                                span { style: "background: #0066cc; color: white; padding: 2px 8px; border-radius: 12px; font-size: 11px; font-weight: 600;",
                                                    "Dieses Gerät"
                                                }
                                            }
                                        }
                                        p { style: "margin: 0; font-size: 12px; color: #666;",
                                            "Zuletzt gesehen: {format_timestamp(device.last_seen)}"
                                        }
                                        if device.first_seen != device.last_seen {
                                            p { style: "margin: 4px 0 0 0; font-size: 12px; color: #999;",
                                                "Registriert: {format_timestamp(device.first_seen)}"
                                            }
                                        }
                                    }
                                }
                            }
                        })
                    }
                }
            }
        }
    }
}
