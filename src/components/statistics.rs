use crate::{Screen, spacetime};
use dioxus::prelude::*;
use dioxus_i18n::tid;
use spacetimedb_sdk::DbContext;

#[derive(Clone, Debug)]
struct EggStatistics {
    total_records: i32,
    total_eggs: f64,
    daily_average: f64,
    weekly_average: Option<f64>,
    monthly_average: Option<f64>,
    min_eggs: f64,
    max_eggs: f64,
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
    let mut chart_data =
        use_signal(|| (Vec::<f32>::new(), Vec::<f32>::new(), Vec::<String>::new()));
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
        let date_format = tid!("stats-date-format-extra-short");
        let period_start = match selected_period().as_str() {
            "week" => Some(today - chrono::Duration::days(6)),
            "month" => Some(today - chrono::Duration::days(29)),
            "year" => Some(today - chrono::Duration::days(364)),
            _ => None,
        };

        let mut all_records: Vec<(chrono::NaiveDate, i32)> = egg_records()
            .into_iter()
            .filter_map(|record| {
                let date = chrono::DateTime::from_timestamp(record.record_date, 0)
                    .map(|dt| dt.date_naive())?;
                (date <= today).then_some((date, record.total_eggs))
            })
            .collect();
        all_records.sort_by_key(|(date, _)| *date);

        if all_records.is_empty() {
            stats.set(None);
            period_records.set(Vec::new());
            chart_data.set((Vec::new(), Vec::new(), Vec::new()));
            error.set(String::new());
            return;
        }

        let distributed_daily = distribute_egg_records(&all_records);
        let selected_daily: Vec<(chrono::NaiveDate, f64)> = distributed_daily
            .iter()
            .filter(|(date, _)| period_start.map_or(true, |start| **date >= start))
            .map(|(date, eggs)| (*date, *eggs))
            .collect();

        let selected_records: Vec<(chrono::NaiveDate, i32)> = all_records
            .iter()
            .copied()
            .filter(|(date, _)| period_start.map_or(true, |start| *date >= start))
            .collect();

        if selected_daily.is_empty() {
            stats.set(None);
            period_records.set(
                selected_records
                    .iter()
                    .rev()
                    .map(|(date, eggs)| {
                        (
                            format_display_date(*date, &date_format),
                            date.format("%Y-%m-%d").to_string(),
                            *eggs,
                        )
                    })
                    .collect(),
            );
            chart_data.set((Vec::new(), Vec::new(), Vec::new()));
            error.set(String::new());
            return;
        }

        let total_records = selected_records.len() as i32;
        let total_eggs: f64 = selected_daily.iter().map(|(_, eggs)| *eggs).sum();
        let min_eggs = selected_daily
            .iter()
            .map(|(_, eggs)| *eggs)
            .fold(f64::INFINITY, f64::min);
        let max_eggs = selected_daily
            .iter()
            .map(|(_, eggs)| *eggs)
            .fold(f64::NEG_INFINITY, f64::max);
        let daily_average = total_eggs / selected_daily.len() as f64;

        let weekly_average =
            average_in_recent_days(&selected_daily, today - chrono::Duration::days(6));
        let monthly_average =
            average_in_recent_days(&selected_daily, today - chrono::Duration::days(29));

        let first_date = selected_daily
            .first()
            .map(|(date, _)| format_display_date(*date, &date_format));
        let last_date = selected_daily
            .last()
            .map(|(date, _)| format_display_date(*date, &date_format));

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
            selected_records
                .iter()
                .rev()
                .map(|(date, eggs)| {
                    (
                        format_display_date(*date, &date_format),
                        date.format("%Y-%m-%d").to_string(),
                        *eggs,
                    )
                })
                .collect(),
        );

        let daily_dates: Vec<chrono::NaiveDate> =
            selected_daily.iter().map(|(date, _)| *date).collect();
        let raw_series: Vec<f32> = selected_daily
            .iter()
            .map(|(_, eggs)| *eggs as f32)
            .collect();

        // Centered 3-day moving average over estimated calendar-day values.
        let n = raw_series.len();
        let ma_series: Vec<f32> = (0..n)
            .map(|i| {
                if n == 1 {
                    raw_series[0]
                } else if i == 0 {
                    (raw_series[0] + raw_series[1]) / 2.0
                } else if i == n - 1 {
                    (raw_series[n - 2] + raw_series[n - 1]) / 2.0
                } else {
                    (raw_series[i - 1] + raw_series[i] + raw_series[i + 1]) / 3.0
                }
            })
            .collect();

        // At most ~8 visible x-axis labels so the axis stays readable on mobile
        let label_step = (n / 8).max(1);
        let chart_labels: Vec<String> = daily_dates
            .iter()
            .enumerate()
            .map(|(i, date)| {
                if i % label_step == 0 {
                    format_display_date(*date, &date_format)
                } else {
                    String::new()
                }
            })
            .collect();

        chart_data.set((raw_series, ma_series, chart_labels));
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
                        for (label, value) in [
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
                                    value: format!("{:.1}", s.total_eggs),
                                    icon: "🥚",
                                }
                                StatCard {
                                    label: tid!("stats-min"),
                                    value: format!("{:.1}", s.min_eggs),
                                    icon: "⬇️",
                                }
                                StatCard {
                                    label: tid!("stats-max"),
                                    value: format!("{:.1}", s.max_eggs),
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
                                    value: s
                                        .weekly_average
                                        .map(|average| format!("{average:.1}"))
                                        .unwrap_or_else(|| "—".to_string()),
                                    icon: "📆",
                                }
                                StatCard {
                                    label: tid!("stats-monthly-avg"),
                                    value: s
                                        .monthly_average
                                        .map(|average| format!("{average:.1}"))
                                        .unwrap_or_else(|| "—".to_string()),
                                    icon: "🗓️",
                                }
                            }
                        }

                        if !chart_data().0.is_empty() {
                            div { class: "box",
                                h2 { class: "title is-6 mb-2",
                                    "📈 "
                                    {tid!("stats-egg-chart-title")}
                                }
                                EggHistoryChart {
                                    moving_avg: chart_data().1,
                                    labels: chart_data().2,
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
                                            for (display_date, iso_date, eggs) in period_records().iter() {
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

fn distribute_egg_records(
    records: &[(chrono::NaiveDate, i32)],
) -> std::collections::BTreeMap<chrono::NaiveDate, f64> {
    let mut daily_values = std::collections::BTreeMap::new();
    let Some((first_date, first_eggs)) = records.first() else {
        return daily_values;
    };

    daily_values.insert(*first_date, *first_eggs as f64);
    let mut previous_date = *first_date;

    for (date, eggs) in records.iter().skip(1) {
        let elapsed_days = (*date - previous_date).num_days();
        if elapsed_days <= 0 {
            *daily_values.entry(*date).or_insert(0.0) += *eggs as f64;
        } else {
            let eggs_per_day = *eggs as f64 / elapsed_days as f64;
            for day_offset in 1..=elapsed_days {
                let day = previous_date + chrono::Duration::days(day_offset);
                *daily_values.entry(day).or_insert(0.0) += eggs_per_day;
            }
        }
        previous_date = *date;
    }

    daily_values
}

fn average_in_recent_days(
    daily_values: &[(chrono::NaiveDate, f64)],
    start_date: chrono::NaiveDate,
) -> Option<f64> {
    let recent_values: Vec<f64> = daily_values
        .iter()
        .filter(|(date, _)| *date >= start_date)
        .map(|(_, eggs)| *eggs)
        .collect();

    (!recent_values.is_empty())
        .then(|| recent_values.iter().sum::<f64>() / recent_values.len() as f64)
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

#[component]
fn EggHistoryChart(moving_avg: Vec<f32>, labels: Vec<String>) -> Element {
    const WIDTH: f32 = 600.0;
    const HEIGHT: f32 = 320.0;
    const LEFT: f32 = 48.0;
    const RIGHT: f32 = 590.0;
    const TOP: f32 = 16.0;
    const BOTTOM: f32 = 276.0;

    let ma_label = tid!("stats-chart-ma").to_string();
    let max_value = moving_avg.iter().copied().fold(0.0_f32, f32::max).max(1.0);
    let plot_height = BOTTOM - TOP;
    let plot_width = RIGHT - LEFT;
    let points: Vec<(f32, f32)> = moving_avg
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = if moving_avg.len() == 1 {
                LEFT + plot_width / 2.0
            } else {
                LEFT + index as f32 / (moving_avg.len() - 1) as f32 * plot_width
            };
            let y = BOTTOM - value / max_value * plot_height;
            (x, y)
        })
        .collect();
    let line_points = points
        .iter()
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .collect::<Vec<_>>()
        .join(" ");
    let y_ticks: Vec<(f32, String)> = (0..=4)
        .map(|tick| {
            let fraction = tick as f32 / 4.0;
            (
                BOTTOM - fraction * plot_height,
                format!("{:.1}", fraction * max_value),
            )
        })
        .collect();
    let x_labels: Vec<(f32, String)> = labels
        .iter()
        .enumerate()
        .filter(|(_, label)| !label.is_empty())
        .map(|(index, label)| {
            let x = if labels.len() == 1 {
                LEFT + plot_width / 2.0
            } else {
                LEFT + index as f32 / (labels.len() - 1) as f32 * plot_width
            };
            (x, label.clone())
        })
        .collect();

    rsx! {
        div {
            div { class: "is-flex is-align-items-center mb-2",
                span {
                    style: "display:inline-block;width:1.5rem;border-top:2px solid var(--bulma-link);margin-right:0.5rem;",
                }
                span { class: "is-size-7 has-text-grey", "{ma_label}" }
            }
            svg {
                view_box: "0 0 {WIDTH} {HEIGHT}",
                width: "100%",
                height: "auto",
                role: "img",
                style: "display:block;width:100%;height:auto;",
                for (y, value) in y_ticks {
                    line {
                        x1: "{LEFT}",
                        y1: "{y:.1}",
                        x2: "{RIGHT}",
                        y2: "{y:.1}",
                        stroke: "var(--bulma-border)",
                        stroke_width: "1",
                    }
                    text {
                        x: "{LEFT - 8.0}",
                        y: "{y + 4.0:.1}",
                        text_anchor: "end",
                        fill: "var(--bulma-text-weak)",
                        font_size: "12",
                        "{value}"
                    }
                }
                polyline {
                    points: "{line_points}",
                    fill: "none",
                    stroke: "var(--bulma-link)",
                    stroke_width: "3",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                }
                for (x, y) in points {
                    circle {
                        cx: "{x:.1}",
                        cy: "{y:.1}",
                        r: "2.5",
                        fill: "var(--bulma-link)",
                    }
                }
                for (x, label) in x_labels {
                    text {
                        x: "{x:.1}",
                        y: "{HEIGHT - 6.0}",
                        text_anchor: "middle",
                        fill: "var(--bulma-text-weak)",
                        font_size: "12",
                        "{label}"
                    }
                }
            }
        }
    }
}
