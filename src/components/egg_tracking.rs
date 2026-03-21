use crate::{Screen, spacetime};
use chrono::Local;
use dioxus::prelude::*;
use dioxus_i18n::tid;

#[component]
pub fn EggTrackingScreen(date: Option<String>, on_navigate: EventHandler<Screen>) -> Element {
    let egg_records = spacetime::use_table_egg_records();
    let connection = spacetime::use_connection();
    let upsert_egg_record = spacetime::use_reducer_upsert_egg_record();
    let delete_egg_record = spacetime::use_reducer_delete_egg_record();

    spacetime::use_subscription(&["SELECT * FROM egg_records"]);

    let mut date_str = use_signal(|| {
        date.clone()
            .unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string())
    });
    let mut total_eggs = use_signal(|| String::new());
    let mut notes = use_signal(|| String::new());
    let mut error = use_signal(|| None::<String>);
    let mut success = use_signal(|| false);
    let mut existing_record_uuid = use_signal(|| None::<String>);

    // Sync form with record for the selected date
    use_effect(move || {
        let selected_date = date_str();
        let selected_timestamp = chrono::NaiveDate::parse_from_str(&selected_date, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc().timestamp());
        let selected_record = egg_records()
            .into_iter()
            .find(|record| Some(record.record_date) == selected_timestamp);

        if let Some(record) = selected_record {
            total_eggs.set(record.total_eggs.to_string());
            notes.set(record.notes.unwrap_or_default());
            existing_record_uuid.set(Some(record.uuid));
        } else {
            total_eggs.set(String::new());
            notes.set(String::new());
            existing_record_uuid.set(None);
        }
    });

    let mut handle_submit = move || {
        error.set(None);
        success.set(false);

        if connection().is_none() {
            error.set(Some(tid!("error-not-connected")));
            return;
        }

        // Validate eggs count
        let eggs_str = total_eggs();
        let eggs_trimmed = eggs_str.trim();
        if eggs_trimmed.is_empty() {
            error.set(Some(tid!("error-eggs-count-empty")));
            return;
        }

        let eggs_count = match eggs_trimmed.parse::<i32>() {
            Ok(n) if n >= 0 => n,
            Ok(_) => {
                error.set(Some(tid!("error-eggs-count-negative")));
                return;
            }
            Err(_) => {
                error.set(Some(tid!("error-eggs-count-invalid")));
                return;
            }
        };

        // Parse date
        let date_value = date_str();
        let date_trimmed = date_value.trim();
        let record_date = match chrono::NaiveDate::parse_from_str(date_trimmed, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                error.set(Some(tid!("error-date-format")));
                return;
            }
        };
        let record_timestamp = match record_date
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc().timestamp())
        {
            Some(ts) => ts,
            None => {
                error.set(Some(tid!("error-date-format")));
                return;
            }
        };

        // Notes
        let notes_value = notes();
        let notes_trimmed = notes_value.trim();
        let notes_opt = if notes_trimmed.is_empty() {
            None
        } else {
            Some(notes_trimmed.to_string())
        };

        let current_uuid = existing_record_uuid();
        let upsert_reducer = upsert_egg_record.clone();
        let delete_reducer = delete_egg_record.clone();
        let on_navigate_submit = on_navigate.clone();
        let device_id = crate::services::device_id_service::get_device_id()
            .unwrap_or_else(|_| "unknown-device".to_string());

        let record_uuid = current_uuid
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        spawn(async move {
            if eggs_count == 0 {
                if let Some(uuid) = current_uuid {
                    if let Err(err) = delete_reducer(uuid) {
                        error.set(Some(err.to_string()));
                        return;
                    }
                }
            } else {
                if let Err(err) = upsert_reducer(spacetime::UpsertEggRecordArgs {
                    uuid: record_uuid,
                    record_date: record_timestamp,
                    total_eggs: eggs_count,
                    notes: notes_opt,
                    device_id,
                }) {
                    error.set(Some(err.to_string()));
                    return;
                }
            }

            success.set(true);
            on_navigate_submit.call(Screen::EggHistory);
        });
    };

    rsx! {
        div { style: "padding: 16px; max-width: 600px; margin: 0 auto; min-height: 100vh; background: #f5f5f5;",

            // Header
            div { style: "display: flex; align-items: center; margin-bottom: 24px;",
                h1 { style: "color: #0066cc; font-size: 24px; font-weight: 700; margin: 0;",
                    "🥚 "
                    {tid!("egg-tracking-title")}
                }
            }

            // Error Message
            if let Some(err) = error() {
                div { style: "background: #fee; border: 1px solid #fcc; color: #c33; padding: 12px; margin-bottom: 16px; border-radius: 8px; font-size: 14px;",
                    "⚠️ "
                    {err}
                }
            }

            // Success Message
            if success() {
                div { style: "background: #efe; border: 1px solid #cfc; color: #3a3; padding: 12px; margin-bottom: 16px; border-radius: 8px; font-size: 14px;",
                    "✅ "
                    {tid!("egg-tracking-success")}
                }
            }

            // Status
            if existing_record_uuid().is_some() {
                div { style: "background: #e8f4f8; padding: 12px; margin-bottom: 16px; border-radius: 8px; border-left: 3px solid #0066cc; font-size: 14px; color: #333;",
                    "📝 "
                    {tid!("egg-tracking-exists-warning")}
                }
            }

            // Form
            div { class: "card",

                // Date Field
                div { style: "margin-bottom: 20px;",
                    label { style: "display: block; margin-bottom: 6px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("field-date-required")}
                    }
                    input {
                        r#type: "date",
                        class: "input",
                        value: "{date_str}",
                        oninput: move |e| date_str.set(e.value()),
                        autofocus: true,
                    }
                    p { style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                        {tid!("field-date-format-hint")}
                    }
                }

                // Total Eggs Field
                div { style: "margin-bottom: 20px;",
                    label { style: "display: block; margin-bottom: 6px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("field-eggs-count-required")}
                    }
                    input {
                        r#type: "number",
                        class: "input",
                        placeholder: tid!("field-eggs-count-placeholder"),
                        min: "0",
                        value: "{total_eggs}",
                        oninput: move |e| total_eggs.set(e.value()),
                    }
                }

                // Notes Field
                div { style: "margin-bottom: 20px;",
                    label { style: "display: block; margin-bottom: 6px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("field-notes")}
                    }
                    textarea {
                        class: "input",
                        style: "min-height: 80px; resize: vertical; font-family: inherit;",
                        placeholder: tid!("field-notes-placeholder"),
                        value: "{notes}",
                        oninput: move |e| notes.set(e.value()),
                    }
                }

                // Action Buttons
                div { style: "display: flex; gap: 12px; margin-top: 24px;",
                    button {
                        class: "btn-success",
                        style: "flex: 1; padding: 14px;",
                        onclick: move |_| handle_submit(),
                        "💾 "
                        if existing_record_uuid().is_some() {
                            {tid!("action-update")}
                        } else {
                            {tid!("action-save")}
                        }
                    }
                }
            }

            // Quick Links
            div { style: "margin-top: 16px; display: flex; gap: 12px;",
                button {
                    class: "btn-primary",
                    style: "flex: 1; padding: 12px;",
                    onclick: move |_| on_navigate.call(Screen::EggHistory),
                    "📋 "
                    {tid!("egg-tracking-show-history")}
                }
            }
        }
    }
}
