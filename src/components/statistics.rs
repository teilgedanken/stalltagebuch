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
    let mut trend = use_signal(|| Vec::<(String, i32)>::new());
    let mut error = use_signal(|| String::new());
    let mut selected_period = use_signal(|| "all".to_string());

    let mut load_statistics = move || {
        let today = chrono::Local::now().date_naive();
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
            trend.set(Vec::new());
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
            .map(|date| date.format("%Y-%m-%d").to_string());
        let last_date = valid_records
            .iter()
            .map(|(date, _)| *date)
            .max()
            .map(|date| date.format("%Y-%m-%d").to_string());

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

        trend.set(
            valid_records
                .iter()
                .take(30)
                .map(|(date, eggs)| (date.format("%Y-%m-%d").to_string(), *eggs))
                .collect(),
        );
        error.set(String::new());
    };

    // Load on mount and when period changes
    use_effect(move || {
        load_statistics();
    });

    rsx! {
        div { style: "padding: 16px; max-width: 800px; margin: 0 auto; min-height: 100vh; background: #f5f5f5;",

            // Header
            div { style: "margin-bottom: 20px; padding-top: 8px;",
                h1 { style: "color: #0066cc; margin: 0 0 16px 0; font-size: 24px; font-weight: 700;",
                    "📊 "
                    {tid!("stats-title")}
                }

                // Period filter
                div { style: "display: flex; gap: 8px; flex-wrap: wrap;",
                    for (label , value) in [
                        (tid!("period-all"), "all"),
                        (tid!("period-week"), "week"),
                        (tid!("period-month"), "month"),
                        (tid!("period-year"), "year"),
                    ]
                    {
                        button {
                            style: if selected_period() == value { "padding: 8px 16px; background: #0066cc; color: white; border-radius: 8px; font-weight: 600;" } else { "padding: 8px 16px; background: white; color: #0066cc; border: 1px solid #0066cc; border-radius: 8px;" },
                            onclick: move |_| selected_period.set(value.to_string()),
                            "{label}"
                        }
                    }
                }
            }

            // Error
            if !error().is_empty() {
                div { style: "padding: 12px; background: #ffebee; border-radius: 8px; color: #c62828; margin-bottom: 16px;",
                    "{error}"
                }
            }

            // Statistiken
            if let Some(s) = stats() {
                div { style: "display: flex; flex-direction: column; gap: 12px;",

                    // Overview card
                    div { class: "card",
                        h2 { style: "margin: 0 0 16px 0; font-size: 18px; color: #333;",
                            "📈 "
                            {tid!("stats-overview")}
                        }
                        div { style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px;",

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

                    // Averages card
                    div { class: "card",
                        h2 { style: "margin: 0 0 16px 0; font-size: 18px; color: #333;",
                            "📊 "
                            {tid!("stats-averages")}
                        }
                        div { style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 12px;",

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

                    // Date range info
                    if let (Some(first), Some(last)) = (&s.first_date, &s.last_date) {
                        div { class: "card", style: "background: #e3f2fd;",
                            p { style: "margin: 0; font-size: 14px; color: #1565c0;",
                                "📅 "
                                {tid!("stats-period")}
                                ": {first} "
                                {tid!("stats-until")}
                                " {last}"
                            }
                        }
                    }

                    // Trend (simple list of last 10 days)
                    if !trend().is_empty() {
                        div { class: "card",
                            h2 { style: "margin: 0 0 16px 0; font-size: 18px; color: #333;",
                                "📈 "
                                {tid!("stats-last-10-days")}
                            }
                            div { style: "display: flex; flex-direction: column; gap: 8px;",
                                for (date , eggs) in trend().iter().take(10) {
                                    div { style: "display: flex; justify-content: space-between; padding: 8px; background: #f8f9fa; border-radius: 6px;",
                                        span { style: "color: #666;", "{date}" }
                                        span { style: "font-weight: 600; color: #ff8c00;",
                                            "🥚 {eggs}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div {
                    class: "card",
                    style: "text-align: center; padding: 40px; color: #999;",
                    {tid!("stats-no-data")} // Empty state when no statistics data available
                }
            }

            // Navigation
            div { style: "margin-top: 20px;",
                button {
                    class: "btn-primary",
                    style: "width: 100%;",
                    onclick: move |_| on_navigate.call(Screen::EggTracking(None)),
                    "➕ "
                    {tid!("stats-add-entry")}
                }
            }
        }
    }
}

#[component]
fn StatCard(label: String, value: String, icon: String) -> Element {
    rsx! {
        div { style: "background: #f8f9fa; padding: 12px; border-radius: 8px; text-align: center;",
            div { style: "font-size: 24px; margin-bottom: 4px;", "{icon}" }
            div { style: "font-size: 20px; font-weight: 700; color: #0066cc; margin-bottom: 4px;",
                "{value}"
            }
            div { style: "font-size: 12px; color: #666;", "{label}" }
        }
    }
}
