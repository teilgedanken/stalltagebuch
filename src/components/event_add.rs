use crate::{Screen, models::quail_event::EventType, spacetime};
use chrono::NaiveDate;
use dioxus::prelude::*;
use dioxus_i18n::tid;

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
            error_message.set(Some(tid!("error-not-connected")));
            return;
        }

        saving.set(true);
        error_message.set(None);

        let parsed_date = match NaiveDate::parse_from_str(&event_date(), "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => {
                error_message.set(Some(tid!("error-invalid-date")));
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
            if let Err(err) = create_reducer(spacetime::CreateEventArgs {
                uuid: event_uuid,
                quail_uuid: quail_id_clone.clone(),
                event_type: event_type_value.as_str().to_string(),
                event_date: parsed_date.format("%Y-%m-%d").to_string(),
                notes: notes_value,
                photos: None,
                device_id,
            }) {
                error_message.set(Some(err.to_string()));
                saving.set(false);
                return;
            }

            saving.set(false);
            on_navigate_save.call(Screen::ProfileDetail(quail_id_clone));
        });
    };

    rsx! {
        section { class: "section pt-4 pb-3",
            div { class: "container is-max-tablet",
                div { class: "box",
                    h2 { class: "title is-4", {tid!("event-add-title")} }
                    p { class: "subtitle is-6", {tid!("event-add-for", name : quail_name.clone())} }

                    if let Some(error) = error_message() {
                        div { class: "notification is-danger is-light", "{error}" }
                    }

                    div { class: "field",
                        label { class: "label", {tid!("field-event-type")} }
                        div { class: "control",
                            div { class: "select is-fullwidth",
                                select {
                                    value: "{event_type().as_str()}",
                                    oninput: move |e| {
                                        let value = e.value();
                                        let et = EventType::from_str(value.as_str());
                                        event_type.set(et);
                                    },
                                    option { value: "born", {tid!("event-type-born")} }
                                    option { value: "alive", {tid!("event-type-alive")} }
                                    option { value: "sick", {tid!("event-type-sick")} }
                                    option { value: "healthy", {tid!("event-type-healthy")} }
                                    option { value: "marked_for_slaughter", {tid!("event-type-marked")} }
                                    option { value: "slaughtered", {tid!("event-type-slaughtered")} }
                                    option { value: "died", {tid!("event-type-died")} }
                                }
                            }
                        }
                    }

                    div { class: "field",
                        label { class: "label", {tid!("field-date")} }
                        div { class: "control",
                            input {
                                r#type: "date",
                                class: "input",
                                value: "{event_date}",
                                oninput: move |e| event_date.set(e.value()),
                            }
                        }
                    }

                    div { class: "field",
                        label { class: "label", {tid!("field-notes-optional")} }
                        div { class: "control",
                            textarea {
                                class: "textarea",
                                value: "{notes}",
                                oninput: move |e| notes.set(e.value()),
                                placeholder: tid!("placeholder-event-notes"),
                            }
                        }
                    }

                    div { class: "field has-addons mt-4",
                        p { class: "control is-expanded",
                            button {
                                class: "button is-primary is-fullwidth",
                                disabled: saving(),
                                onclick: on_save,
                                if saving() {
                                    "⏳ "
                                    {tid!("action-saving")}
                                } else {
                                    {tid!("action-save")}
                                }
                            }
                        }

                        p { class: "control is-expanded",
                            button {
                                class: "button is-light is-fullwidth",
                                disabled: saving(),
                                onclick: {
                                    let quail_id_for_cancel = quail_id.clone();
                                    move |_| on_navigate.call(Screen::ProfileDetail(quail_id_for_cancel.clone()))
                                },
                                {tid!("action-cancel")}
                            }
                        }
                    }
                }
            }
        }
    }
}
