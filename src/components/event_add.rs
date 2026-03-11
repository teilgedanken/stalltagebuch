use crate::{Screen, models::quail_event::EventType, spacetime};
use chrono::NaiveDate;
use dioxus::prelude::*;
use dioxus_i18n::t;

#[component]
pub fn EventAdd(
    quail_id: String,
    quail_name: String,
    on_navigate: EventHandler<Screen>,
) -> Element {
    let connection = spacetime::use_connection();
    let create_event_reducer = spacetime::use_reducer_create_event();

    let mut event_type = use_signal(|| EventType::Alive);
    let mut event_date = use_signal(|| {
        chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    });
    let mut notes = use_signal(|| String::new());
    let mut error_message = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);

    let quail_id_for_save = quail_id.clone();
    let on_save = move |_| {
        // Check if connected to Spacetime
        if connection().is_none() {
            error_message.set(Some(t!("error-not-connected")));
            return;
        }

        saving.set(true);
        error_message.set(None);

        let parsed_date = match NaiveDate::parse_from_str(&event_date(), "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => {
                error_message.set(Some(t!("error-invalid-date")));
                saving.set(false);
                return;
            }
        };

        let event_uuid = uuid::Uuid::new_v4().to_string();
        let notes_value = if notes().is_empty() {
            None
        } else {
            Some(notes())
        };

        let create_reducer = create_event_reducer.clone();
        let quail_id_clone = quail_id_for_save.clone();
        let on_navigate_save = on_navigate.clone();
        let event_type_value = event_type();
        let device_id = crate::services::device_id_service::get_device_id()
            .unwrap_or_else(|_| "unknown-device".to_string());

        spawn(async move {
            create_reducer(spacetime::CreateEventArgs {
                uuid: event_uuid,
                quail_uuid: quail_id_clone.clone(),
                event_type: event_type_value.as_str().to_string(),
                event_date: parsed_date.format("%Y-%m-%d").to_string(),
                notes: notes_value,
                photos: None,
                device_id,
            });

            saving.set(false);
            on_navigate_save.call(Screen::ProfileDetail(quail_id_clone));
        });
    };

    rsx! {
        div { class: "container", style: "padding: 20px;",

            h2 { {t!("event-add-title")} }
            p { style: "color: #666; margin-bottom: 20px;",
                {t!("event-add-for", name : quail_name.clone())}
            }

            if let Some(error) = error_message() {
                div {
                    class: "error-message",
                    style: "background-color: #fee; color: #c00; padding: 10px; margin-bottom: 20px; border-radius: 4px;",
                    "{error}"
                }
            }

            div { class: "form-group", style: "margin-bottom: 20px;",

                label { style: "display: block; margin-bottom: 8px; font-weight: bold;",
                    {t!("field-event-type")}
                }
                select {
                    value: "{event_type().as_str()}",
                    oninput: move |e| {
                        let value = e.value();
                        let et = EventType::from_str(value.as_str());
                        event_type.set(et);
                    },
                    style: "width: 100%; padding: 8px; border: 1px solid #ccc; border-radius: 4px;",
                    option { value: "born", {t!("event-type-born")} }
                    option { value: "alive", {t!("event-type-alive")} }
                    option { value: "sick", {t!("event-type-sick")} }
                    option { value: "healthy", {t!("event-type-healthy")} }
                    option { value: "marked_for_slaughter", {t!("event-type-marked")} }
                    option { value: "slaughtered", {t!("event-type-slaughtered")} }
                    option { value: "died", {t!("event-type-died")} }
                }
            }

            div { class: "form-group", style: "margin-bottom: 20px;",

                label { style: "display: block; margin-bottom: 8px; font-weight: bold;",
                    {t!("field-date")}
                }
                input {
                    r#type: "date",
                    value: "{event_date}",
                    oninput: move |e| event_date.set(e.value()),
                    style: "width: 100%; padding: 8px; border: 1px solid #ccc; border-radius: 4px;",
                }
            }

            div { class: "form-group", style: "margin-bottom: 20px;",

                label { style: "display: block; margin-bottom: 8px; font-weight: bold;",
                    {t!("field-notes-optional")}
                }
                textarea {
                    value: "{notes}",
                    oninput: move |e| notes.set(e.value()),
                    style: "width: 100%; padding: 8px; border: 1px solid #ccc; border-radius: 4px; min-height: 100px;",
                    placeholder: t!("placeholder-event-notes"),
                }
            }

            div { class: "button-group", style: "display: flex; gap: 10px;",

                button {
                    disabled: saving(),
                    onclick: on_save,
                    style: "flex: 1; padding: 12px; background-color: #4CAF50; color: white; border: none; border-radius: 4px; font-size: 16px; cursor: pointer;",
                    if saving() {
                        "⏳ "
                        {t!("action-saving")}
                    } else {
                        {t!("action-save")}
                    }
                }

                button {
                    disabled: saving(),
                    onclick: {
                        let quail_id_for_cancel = quail_id.clone();
                        move |_| on_navigate.call(Screen::ProfileDetail(quail_id_for_cancel.clone()))
                    },
                    style: "flex: 1; padding: 12px; background-color: #f44336; color: white; border: none; border-radius: 4px; font-size: 16px; cursor: pointer;",
                    {t!("action-cancel")}
                }
            }
        }
    }
}
