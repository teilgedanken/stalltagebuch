use crate::{Screen, models::EggRecord, spacetime};
use dioxus::prelude::*;
use dioxus_i18n::tid;

#[component]
pub fn EggHistoryScreen(on_navigate: EventHandler<Screen>) -> Element {
    let egg_records = spacetime::use_table_egg_records();
    spacetime::use_subscription(&["SELECT * FROM egg_records"]);

    let records = use_memo(move || {
        let mut mapped: Vec<EggRecord> = egg_records()
            .into_iter()
            .filter_map(|record| {
                let uuid = uuid::Uuid::parse_str(&record.uuid).ok()?;
                let record_date = chrono::DateTime::from_timestamp(record.record_date, 0)
                    .map(|dt| dt.date_naive())?;

                Some(EggRecord {
                    uuid,
                    record_date,
                    total_eggs: record.total_eggs,
                    notes: record.notes,
                })
            })
            .collect();

        mapped.sort_by(|a, b| b.record_date.cmp(&a.record_date));
        mapped
    });

    let status_message = format!("✅ {}", tid!("egg-history-loaded", count: records().len()));

    rsx! {
        section { class: "section pt-4 pb-3",
            div { class: "container is-max-tablet",
                div { class: "level mb-4",
                    div { class: "level-left",
                        h1 { class: "title is-4 mb-0",
                            "📋 "
                            {tid!("egg-history-title")}
                        }
                    }
                    div { class: "level-right",
                        button {
                            class: "button is-success",
                            onclick: move |_| on_navigate.call(Screen::EggTracking(None)),
                            "+ "
                            {tid!("action-new")}
                        }
                    }
                }

                if !status_message.is_empty() {
                    div { class: "notification is-info is-light",
                        "{status_message}"
                    }
                }

                if records().is_empty() {
                    div { class: "notification is-light has-text-centered",
                        {tid!("egg-history-empty")}
                    }
                } else {
                    for record in records().iter() {
                        EggRecordCard {
                            record: record.clone(),
                            on_edit: move |date| on_navigate.call(Screen::EggTracking(Some(date))),
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn EggRecordCard(record: EggRecord, on_edit: EventHandler<String>) -> Element {
    let date_str = record.record_date.format("%Y-%m-%d").to_string();
    let display_date = record.record_date.format("%d.%m.%Y").to_string();
    use chrono::Datelike;
    let weekday_num = record.record_date.weekday().num_days_from_monday();
    let weekday = match weekday_num {
        0 => tid!("weekday-mon"),
        1 => tid!("weekday-tue"),
        2 => tid!("weekday-wed"),
        3 => tid!("weekday-thu"),
        4 => tid!("weekday-fri"),
        5 => tid!("weekday-sat"),
        6 => tid!("weekday-sun"),
        _ => String::new(),
    };

    rsx! {
        div {
            class: "box",
            style: "cursor: pointer;",
            onclick: move |_| on_edit.call(date_str.clone()),

            div { class: "is-flex is-justify-content-space-between is-align-items-flex-start",
                div { class: "mr-3", style: "flex: 1; min-width: 0;",
                    h3 { class: "title is-6 mb-2",
                        "📅 {display_date} ({weekday})"
                    }

                    div { class: "tags mb-2",
                        span { class: "tag is-warning is-light is-medium",
                            "🥚 "
                            {tid!("egg-history-eggs-count", count : record.total_eggs)}
                        }
                    }

                    if let Some(notes) = &record.notes {
                        if !notes.trim().is_empty() {
                            div { class: "content is-small",
                                p {
                                    strong { "💬 " }
                                    "{notes}"
                                }
                            }
                        }
                    }
                }

                span { class: "tag is-light", "✏️" }
            }
        }
    }
}
