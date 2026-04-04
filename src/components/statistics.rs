use crate::{Screen, spacetime};
use dioxus::prelude::*;
use dioxus_i18n::tid;

#[derive(Clone, Debug)]
struct EggStatistics {
    total_records: i32,
    total_eggs: i32,
    daily_average: f64,
    weekly_average: f64,
    monthly_average: f64,
    min_eggs: i32,
    max_eggs: i32,
    first_date: Option<String>,
    last_date: Option<String>,
}

#[component]
pub fn StatisticsScreen(on_navigate: EventHandler<Screen>) -> Element {
    let egg_records = spacetime::use_table_egg_records();
    spacetime::use_subscription(&["SELECT * FROM egg_records"]);

    let mut stats = use_signal(|| None::<EggStatistics>);
    let mut period_records = use_signal(|| Vec::<(String, i32)>::new());
    let mut error = use_signal(|| String::new());
    let mut selected_period = use_signal(|| "month".to_string());

    let mut load_statistics = move || {
        let today = chrono::Local::now().date_naive();
        let date_format = tid!("stats-date-format-short");
        let period_start = match selected_period().as_str() {
            "week" => Some(today - chrono::Duration::days(7)),
            "month" => Some(today - chrono::Duration::days(30)),
            "year" => Some(today - chrono::Duration::days(365)),
            _ => None,
        };

        let mut valid_records: Vec<(chrono::NaiveDate, i32)> = egg_records()
            .into_iter()
            .filter_map(|record| {
                let date = chrono::DateTime::from_timestamp(record.record_date, 0)
                    .map(|dt| dt.date_naive())?;
                Some((date, record.total_eggs))
            })
            .collect();

        if let Some(start) = period_start {
            valid_records.retain(|(date, _)| *date >= start && *date <= today);
        }

        valid_records.sort_by(|a, b| b.0.cmp(&a.0));

        if valid_records.is_empty() {
            stats.set(None);
            period_records.set(Vec::new());
            error.set(String::new());
            return;
        }

        let total_records = valid_records.len() as i32;
        let total_eggs: i32 = valid_records.iter().map(|(_, eggs)| *eggs).sum();
        let min_eggs = valid_records
            .iter()
            .map(|(_, eggs)| *eggs)
            .min()
            .unwrap_or(0);
        let max_eggs = valid_records
            .iter()
            .map(|(_, eggs)| *eggs)
            .max()
            .unwrap_or(0);
        let daily_average = total_eggs as f64 / total_records as f64;

        let weekly_window = valid_records.iter().take(7).collect::<Vec<_>>();
        let weekly_average = if weekly_window.is_empty() {
            0.0
        } else {
            weekly_window
                .iter()
                .map(|(_, eggs)| *eggs as f64)
                .sum::<f64>()
                / weekly_window.len() as f64
        };

        let monthly_window = valid_records.iter().take(30).collect::<Vec<_>>();
        let monthly_average = if monthly_window.is_empty() {
            0.0
        } else {
            monthly_window
                .iter()
                .map(|(_, eggs)| *eggs as f64)
                .sum::<f64>()
                / monthly_window.len() as f64
        };

        let first_date = valid_records
            .iter()
            .map(|(date, _)| *date)
            .min()
            .map(|date| format_display_date(date, &date_format));
        let last_date = valid_records
            .iter()
            .map(|(date, _)| *date)
            .max()
            .map(|date| format_display_date(date, &date_format));

        stats.set(Some(EggStatistics {
            total_records,
            total_eggs,
            daily_average,
            weekly_average,
            monthly_average,
            min_eggs,
            max_eggs,
            first_date,
            last_date,
        }));

        period_records.set(
            valid_records
                .iter()
                .map(|(date, eggs)| (format_display_date(*date, &date_format), *eggs))
                .collect(),
        );
        error.set(String::new());
    };

    // Load on mount and when period changes
    use_effect(move || {
        load_statistics();
    });

    rsx! {
        section { class: "section",
            div { class: "container",
                div { class: "mt-1",
                    button {
                        class: "button is-primary is-fullwidth mb-4 is-large",
                        onclick: move |_| on_navigate.call(Screen::EggTracking(None)),
                        span { class: "icon is-small", "➕ " }
                        span { {tid!("stats-add-entry")} }
                    }
                }

                div { class: "mb-5",
                    h1 { class: "title is-4 mb-2",
                        "📊 "
                        {tid!("stats-title")}
                    }

                    div { class: "buttons has-addons is-centered",
                        for (label , value) in [
                            (tid!("period-week"), "week"),
                            (tid!("period-month"), "month"),
                            (tid!("period-year"), "year"),
                            (tid!("period-all"), "all"),
                        ]
                        {
                            button {
                                class: if selected_period() == value { "button is-link is-small" } else { "button is-link is-light is-small" },
                                onclick: move |_| selected_period.set(value.to_string()),
                                "{label}"
                            }
                        }
                    }
                }

                if !error().is_empty() {
                    div { class: "notification is-danger is-light", "{error}" }
                }

                if let Some(s) = stats() {
                    div { class: "is-flex is-flex-direction-column",
                        div { class: "box",
                            div { class: "is-flex is-justify-content-space-between is-align-items-center mb-4",
                                h2 { class: "title is-7 mb-0",
                                    "📈 "
                                    {tid!("stats-overview")}
                                }
                                if let (Some(first), Some(last)) = (&s.first_date, &s.last_date) {
                                    span { class: "tag is-info is-light",
                                        "📅 "
                                        "{first} "
                                        {tid!("stats-until")}
                                        " {last}"
                                    }
                                }
                            }
                            div { class: "columns is-multiline is-mobile is-align-items-stretch",
                                StatCard {
                                    label: tid!("stats-total-records"),
                                    value: format!("{}", s.total_records),
                                    icon: "📋",
                                }

                                StatCard {
                                    label: tid!("stats-total-eggs"),
                                    value: format!("{}", s.total_eggs),
                                    icon: "🥚",
                                }
                                StatCard {
                                    label: tid!("stats-min"),
                                    value: format!("{}", s.min_eggs),
                                    icon: "⬇️",
                                }
                                StatCard {
                                    label: tid!("stats-max"),
                                    value: format!("{}", s.max_eggs),
                                    icon: "⬆️",
                                }
                            }
                        }

                        div { class: "box",
                            h2 { class: "title is-7 mb-4",
                                "📊 "
                                {tid!("stats-averages")}
                            }
                            div { class: "columns is-multiline is-mobile is-align-items-stretch",
                                StatCard {
                                    label: tid!("stats-daily-avg"),
                                    value: format!("{:.1}", s.daily_average),
                                    icon: "📅",
                                }
                                StatCard {
                                    label: tid!("stats-weekly-avg"),
                                    value: format!("{:.1}", s.weekly_average),
                                    icon: "📆",
                                }
                                StatCard {
                                    label: tid!("stats-monthly-avg"),
                                    value: format!("{:.1}", s.monthly_average),
                                    icon: "🗓️",
                                }
                            }
                        }

                        if !period_records().is_empty() {
                            div { class: "box",
                                h2 { class: "title is-6 mb-4",
                                    "🥚 "
                                    {tid!("egg-history-title")}
                                }
                                div { class: "table-container",
                                    table { class: "table is-bordered is-striped is-narrow is-hoverable is-fullwidth",
                                        thead {
                                            tr {
                                                th { {tid!("stats-period")} }
                                                th { class: "has-text-right",
                                                    {tid!("stats-total-eggs")}
                                                }
                                            }
                                        }
                                        tbody {
                                            for (date , eggs) in period_records().iter() {
                                                tr {
                                                    td { "{date}" }
                                                    td { class: "has-text-right has-text-weight-semibold",
                                                        "{eggs}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    div { class: "notification is-light has-text-centered", {tid!("stats-no-data")} }
                }
            }
        }
    }
}

fn format_display_date(date: chrono::NaiveDate, date_format: &str) -> String {
    date.format(date_format).to_string()
}

#[component]
fn StatCard(label: String, value: String, icon: String) -> Element {
    rsx! {
        div { class: "column is-4 is-6-mobile is-flex",
            div { class: "box has-background-light has-text-centered is-flex is-flex-direction-column is-justify-content-center is-flex-grow-1",
                div { class: "is-size-4 mb-1", "{icon}" }
                div { class: "title is-6 has-text-link mb-1", "{value}" }
                div { class: "is-size-7 has-text-grey", "{label}" }
            }
        }
    }
}
