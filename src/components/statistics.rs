use crate::{Screen, spacetime};
use dioxus::prelude::*;
use dioxus_i18n::tid;
use spacetimedb_sdk::DbContext;

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

#[derive(Clone, Debug, Default)]
struct PopulationStatistics {
    total_quails: i32,
    female_quails: i32,
    male_quails: i32,
    unknown_gender_quails: i32,
    slaughtered_quails: i32,
    hen_to_rooster_ratio: String,
}

#[component]
pub fn StatisticsScreen(on_navigate: EventHandler<Screen>) -> Element {
    let egg_records = spacetime::use_table_egg_records();
    let quails = spacetime::use_table_quails();
    let quail_events = spacetime::use_table_quail_events();
    let connection = spacetime::use_connection();
    spacetime::use_subscription(&[
        "SELECT * FROM egg_records",
        "SELECT * FROM quails",
        "SELECT * FROM quail_events",
    ]);

    let mut stats = use_signal(|| None::<EggStatistics>);
    let mut period_records = use_signal(|| Vec::<(String, String, i32)>::new());
    let mut population_stats = use_signal(PopulationStatistics::default);
    let mut error = use_signal(|| String::new());
    let mut selected_period = use_signal(|| "month".to_string());
    let on_navigate_add = on_navigate.clone();

    let mut load_statistics = move || {
        let owner = connection()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        population_stats.set(calculate_population_stats(
            &quails(),
            &quail_events(),
            owner.as_ref(),
        ));

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
                .map(|(date, eggs)| {
                    (
                        format_display_date(*date, &date_format),
                        date.format("%Y-%m-%d").to_string(),
                        *eggs,
                    )
                })
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
                        onclick: move |_| on_navigate_add.call(Screen::EggTracking(None)),
                        span { class: "icon is-small", "+" }
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
                                            for (display_date , iso_date , eggs) in period_records().iter() {
                                                tr {
                                                    class: "is-clickable",
                                                    style: "cursor: pointer;",
                                                    onclick: {
                                                        let edit_date = iso_date.clone();
                                                        let on_navigate_row = on_navigate.clone();
                                                        move |_| on_navigate_row.call(Screen::EggTracking(Some(edit_date.clone())))
                                                    },
                                                    td { "{display_date}" }
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

                        div { class: "box",
                            h2 { class: "title is-7 mb-4", {tid!("stats-population-title")} }
                            div { class: "columns is-multiline is-mobile is-align-items-stretch",
                                StatCard {
                                    label: tid!("stats-population-total-quails"),
                                    value: format!("{}", population_stats().total_quails),
                                    icon: "⚧️",
                                }
                                StatCard {
                                    label: tid!("stats-population-hens"),
                                    value: format!("{}", population_stats().female_quails),
                                    icon: "🐦",
                                }
                                StatCard {
                                    label: tid!("stats-population-roosters"),
                                    value: format!("{}", population_stats().male_quails),
                                    icon: "🐓",
                                }
                                StatCard {
                                    label: format!("{} {}", tid!("field-gender"), tid!("gender-unknown")),
                                    value: format!("{}", population_stats().unknown_gender_quails),
                                    icon: "🐥",
                                }
                                StatCard {
                                    label: tid!("event-type-slaughtered"),
                                    value: format!("{}", population_stats().slaughtered_quails),
                                    icon: "🥩",
                                }
                                StatCard {
                                    label: tid!("stats-population-ratio"),
                                    value: population_stats().hen_to_rooster_ratio,
                                    icon: "⚖️",
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

fn calculate_population_stats(
    quails: &[spacetime::Quail],
    events: &[spacetime::QuailEvent],
    owner: Option<&String>,
) -> PopulationStatistics {
    let mut total_quails = 0;
    let mut female_quails = 0;
    let mut male_quails = 0;
    let mut unknown_gender_quails = 0;
    let mut slaughtered_quails = 0;

    for quail in quails {
        if let Some(owner_value) = owner {
            if &quail.owner != owner_value {
                continue;
            }
        }

        let latest_event_type = latest_event_type_for_quail(&quail.uuid, events);

        if matches!(latest_event_type, Some("slaughtered" | "geschlachtet")) {
            slaughtered_quails += 1;
            continue;
        }

        if matches!(latest_event_type, Some("died" | "gestorben")) {
            continue;
        }

        total_quails += 1;

        match quail.gender.as_str() {
            "female" | "weiblich" => female_quails += 1,
            "male" | "maennlich" | "männlich" => male_quails += 1,
            _ => unknown_gender_quails += 1,
        }
    }

    PopulationStatistics {
        total_quails,
        female_quails,
        male_quails,
        unknown_gender_quails,
        slaughtered_quails,
        hen_to_rooster_ratio: format_ratio(female_quails, male_quails),
    }
}

fn latest_event_type_for_quail<'a>(
    quail_uuid: &str,
    events: &'a [spacetime::QuailEvent],
) -> Option<&'a str> {
    events
        .iter()
        .filter(|event| event.quail_uuid == quail_uuid)
        .max_by(|a, b| {
            a.event_date
                .cmp(&b.event_date)
                .then_with(|| a.uuid.cmp(&b.uuid))
        })
        .map(|event| event.event_type.as_str())
}

fn format_ratio(hens: i32, roosters: i32) -> String {
    if hens == 0 && roosters == 0 {
        return "0:0".to_string();
    }
    if roosters == 0 {
        return format!("{}:0", hens);
    }

    let divisor = gcd(hens, roosters);
    format!("{}:{}", hens / divisor, roosters / divisor)
}

fn gcd(mut a: i32, mut b: i32) -> i32 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a.abs().max(1)
}

#[component]
fn StatCard(label: String, value: String, icon: String) -> Element {
    rsx! {
        div { class: "column is-4 is-6-mobile is-flex p-1",
            div { class: "box has-background-light has-text-centered is-flex is-flex-direction-column is-justify-content-center is-flex-grow-1 p-2 mb-0",
                div { class: "is-size-5 mb-0", "{icon}" }
                div { class: "title is-6 has-text-link mb-0 mt-1", "{value}" }
                div { class: "is-size-7 has-text-grey mt-1 mb-0", "{label}" }
            }
        }
    }
}
