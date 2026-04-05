use super::synced_photo::SyncedThumbnailImage;
use crate::Screen;
use crate::models::{EventType, Gender, Quail, RingColor};
use crate::spacetime;
use chrono::{Datelike, Local, NaiveDate};
use dioxus::prelude::*;
use dioxus_i18n::tid;
use spacetimedb_sdk::DbContext;

#[component]
pub fn ProfileListScreen(on_navigate: EventHandler<Screen>) -> Element {
    let mut search_filter = use_signal(String::new);
    let mut show_dead = use_signal(|| false);
    let quails = spacetime::use_table_quails();
    let events = spacetime::use_table_quail_events();
    let photos = spacetime::use_table_photos();
    let connection = spacetime::use_connection();

    spacetime::use_subscription(&[
        "SELECT * FROM quails",
        "SELECT * FROM quail_events",
        "SELECT * FROM photos",
    ]);

    let filtered_profiles = use_memo(move || {
        let owner = connection()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        let all_events = events().clone();
        let search = search_filter().to_lowercase();
        let dead_only = show_dead();
        let today = Local::now().date_naive();

        let born_dates_by_quail = earliest_born_dates_by_quail(&all_events);

        let photo_paths_by_uuid: std::collections::HashMap<String, String> = photos()
            .iter()
            .map(|photo| (photo.uuid.clone(), photo.relative_path.clone()))
            .collect();

        let mut rows = Vec::<(
            Quail,
            Option<EventType>,
            Option<String>,
            Option<String>,
            Option<String>,
        )>::new();

        for remote_quail in quails().iter() {
            if let Some(owner_value) = owner.as_ref() {
                if &remote_quail.owner != owner_value {
                    continue;
                }
            }

            if !search.is_empty() && !remote_quail.name.to_lowercase().contains(&search) {
                continue;
            }

            let status = latest_status_for(&remote_quail.uuid, &all_events);
            if dead_only {
                if !matches!(status, Some(EventType::Died | EventType::Slaughtered)) {
                    continue;
                }
            } else if matches!(status, Some(EventType::Died | EventType::Slaughtered)) {
                continue;
            }

            if let Some(local_quail) = to_local_quail(remote_quail) {
                let profile_photo_uuid =
                    local_quail.profile_photo.as_ref().map(|id| id.to_string());
                let profile_photo_path = profile_photo_uuid
                    .as_ref()
                    .and_then(|uuid| photo_paths_by_uuid.get(uuid).cloned());
                let age_display = born_dates_by_quail
                    .get(&remote_quail.uuid)
                    .map(|birth_date| format_age_years_months(*birth_date, today));

                rows.push((
                    local_quail,
                    status,
                    profile_photo_uuid,
                    profile_photo_path,
                    age_display,
                ));
            }
        }

        rows.sort_by(|a, b| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()));
        rows
    });

    rsx! {
        section { class: "section pt-4 pb-3",
            div { class: "container is-max-tablet",
                div { class: "level is-mobile mb-4",
                    div { class: "level-left",
                        h1 { class: "title is-5 mb-0", {tid!("profile-list-title")} }
                    }
                    div { class: "level-right",
                        div { class: "buttons has-addons mb-0",
                            button {
                                class: if show_dead() { "button is-link" } else { "button is-link is-light" },
                                onclick: move |_| show_dead.set(!show_dead()),
                                span { class: "icon is-small", "🪦" }
                            }
                            button {
                                class: "button is-success",
                                onclick: move |_| on_navigate.call(Screen::AddProfile),
                                span { class: "icon is-small", "+ " }
                                span { {tid!("action-new")} }
                            }
                        }
                    }
                }

                div { class: "field mb-4",
                    p { class: "control has-icons-left",
                        input {
                            class: "input",
                            r#type: "text",
                            placeholder: tid!("search-placeholder-name"),
                            value: "{search_filter}",
                            oninput: move |e| search_filter.set(e.value()),
                        }
                        span { class: "icon is-small is-left", "🔍" }
                    }
                }

                if filtered_profiles().is_empty() {
                    div { class: "notification is-light has-text-centered",
                        {tid!("profile-list-empty")}
                    }
                } else {
                    div { class: "profile-grid",
                        for (profile , status , profile_photo_uuid , profile_photo_path , age_display) in filtered_profiles() {
                            ProfileCard {
                                key: "{profile.uuid}",
                                profile: profile.clone(),
                                profile_photo_path,
                                profile_photo_uuid,
                                age_display,
                                status,
                                on_click: move |_| on_navigate.call(Screen::ProfileDetail(profile.uuid.to_string())),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn ProfileCard(
    profile: Quail,
    profile_photo_path: Option<String>,
    profile_photo_uuid: Option<String>,
    age_display: Option<String>,
    status: Option<EventType>,
    on_click: EventHandler<()>,
) -> Element {
    // Subscribe to photos table to dynamically find photo path
    let photos_table = spacetime::use_table_photos();

    // Clone UUID for use in memo
    let photo_uuid_for_lookup = profile_photo_uuid.clone();

    let overlay_bg = split_overlay_bg(
        profile.ring_color_left.as_ref(),
        profile.ring_color_right.as_ref(),
    );

    // Try to get photo path from the provided path, or dynamically fetch from photos table
    let effective_photo_path = use_memo(move || {
        profile_photo_path.clone().or_else(|| {
            // If no path provided, dynamically look it up from photos table
            if let Some(uuid) = &photo_uuid_for_lookup {
                photos_table()
                    .iter()
                    .find(|p| p.uuid == *uuid)
                    .map(|p| p.relative_path.clone())
            } else {
                None
            }
        })
    });

    rsx! {
        div { class: "profile-card", onclick: move |_| on_click.call(()),
            div { class: "profile-image",
                if let Some(photo_path) = effective_photo_path() {
                    SyncedThumbnailImage {
                        photo_uuid: profile_photo_uuid.clone(),
                        relative_path: photo_path,
                        alt: profile.name.clone(),
                        fill: true,
                    }
                } else {
                    div { class: "profile-image-placeholder", "🐦" }
                }

                div {
                    class: "profile-overlay",
                    style: format!("background: {};", overlay_bg),
                    div {
                        class: "is-flex is-align-items-flex-end is-justify-content-space-between",
                        style: "gap: 3px;",
                        div {
                            div { class: "profile-name", "{profile.name}" }
                            div { class: "profile-gender", "{profile.gender.display_name()}" }
                        }
                        if let Some(age) = age_display {
                            span {
                                class: "tag is-light is-small",
                                style: "margin-left: auto; white-space: nowrap;",
                                "{age}"
                            }
                        }
                    }
                }

                {
                    if let Some(status) = status {
                        match status {
                            EventType::Sick => rsx! {
                                span {
                                    class: "tag is-danger is-light",
                                    style: "position: absolute; top: 8px; right: 8px; font-size: 22px;",
                                    "🤒"
                                }
                            },
                            EventType::MarkedForSlaughter | EventType::Slaughtered => {
                                rsx! {
                                    span {
                                        class: "tag is-warning is-light",
                                        style: "position: absolute; top: 8px; right: 8px; font-size: 22px;",
                                        "🥩"
                                    }
                                }
                            }
                            EventType::Died => rsx! {
                                span {
                                    class: "tag is-dark is-light",
                                    style: "position: absolute; top: 8px; right: 8px; font-size: 22px;",
                                    "🪦"
                                }
                            },
                            _ => rsx! {},
                        }
                    } else {
                        rsx! {}
                    }
                }
            }
        }
    }
}

fn latest_status_for(quail_uuid: &str, events: &[spacetime::QuailEvent]) -> Option<EventType> {
    events
        .iter()
        .filter(|event| event.quail_uuid == quail_uuid)
        .max_by(|a, b| {
            a.event_date
                .cmp(&b.event_date)
                .then_with(|| a.uuid.cmp(&b.uuid))
        })
        .map(|event| EventType::from_str(&event.event_type))
}

fn earliest_born_dates_by_quail(
    events: &[spacetime::QuailEvent],
) -> std::collections::HashMap<String, NaiveDate> {
    let mut map = std::collections::HashMap::<String, NaiveDate>::new();

    for event in events {
        if EventType::from_str(&event.event_type) != EventType::Born {
            continue;
        }

        let Ok(date) = NaiveDate::parse_from_str(&event.event_date, "%Y-%m-%d") else {
            continue;
        };

        map.entry(event.quail_uuid.clone())
            .and_modify(|current| {
                if date < *current {
                    *current = date;
                }
            })
            .or_insert(date);
    }

    map
}

fn format_age_years_months(birth_date: NaiveDate, today: NaiveDate) -> String {
    if birth_date > today {
        return "0 Monate".to_string();
    }

    let mut total_months = (today.year() - birth_date.year()) * 12
        + (today.month() as i32 - birth_date.month() as i32);

    if today.day() < birth_date.day() {
        total_months -= 1;
    }

    if total_months < 0 {
        total_months = 0;
    }

    let years = total_months / 12;
    let months = total_months % 12;

    if years > 0 {
        let years_label = if years == 1 { "Jahr" } else { "Jahre" };
        if months > 0 {
            let months_label = if months == 1 { "Monat" } else { "Monate" };
            format!("{} {} {} {}", years, years_label, months, months_label)
        } else {
            format!("{} {}", years, years_label)
        }
    } else {
        let months_label = if months == 1 { "Monat" } else { "Monate" };
        format!("{} {}", months, months_label)
    }
}

fn to_local_quail(remote: &spacetime::Quail) -> Option<Quail> {
    let uuid = uuid::Uuid::parse_str(&remote.uuid).ok()?;
    let profile_photo = remote
        .profile_photo
        .as_ref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok());

    Some(Quail {
        uuid,
        name: remote.name.clone(),
        gender: Gender::from_str(&remote.gender),
        ring_color_left: remote
            .ring_color_left
            .as_ref()
            .map(|value| RingColor::from_str(value)),
        ring_color_right: remote
            .ring_color_right
            .as_ref()
            .map(|value| RingColor::from_str(value)),
        profile_photo,
    })
}

fn split_overlay_bg(left: Option<&RingColor>, right: Option<&RingColor>) -> String {
    let left_bg = left
        .map(get_light_color_for)
        .unwrap_or("rgba(255, 255, 255, 0.9)");
    let right_bg = right
        .map(get_light_color_for)
        .unwrap_or("rgba(255, 255, 255, 0.9)");
    format!(
        "linear-gradient(to right, {} 0%, {} 50%, {} 50%, {} 100%)",
        left_bg, left_bg, right_bg, right_bg
    )
}

fn get_light_color_for(color: &RingColor) -> &'static str {
    match color {
        RingColor::Rot => "rgba(255, 200, 200, 0.9)",
        RingColor::Dunkelblau => "rgba(200, 210, 245, 0.9)",
        RingColor::Hellblau => "rgba(210, 230, 255, 0.9)",
        RingColor::Gruen => "rgba(200, 255, 200, 0.9)",
        RingColor::Gelb => "rgba(255, 255, 200, 0.9)",
        RingColor::Orange => "rgba(255, 230, 200, 0.9)",
        RingColor::Lila => "rgba(230, 200, 255, 0.9)",
        RingColor::Rosa => "rgba(255, 200, 230, 0.9)",
        RingColor::Schwarz => "rgba(220, 220, 220, 0.9)",
        RingColor::Weiss => "rgba(255, 255, 255, 0.9)",
    }
}
