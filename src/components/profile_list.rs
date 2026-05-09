use super::profile_i18n::{format_age_years_months, gender_label};
use super::ring_color_picker::{RingColorPalette, RingColorTrigger};
use super::synced_photo::SyncedThumbnailImage;
use crate::Screen;
use crate::models::{EventType, Gender, Quail, RingColor, ring_color_filter_matches};
use crate::spacetime;
use chrono::{Local, NaiveDate};
use dioxus::core::const_format::concatcp;
use dioxus::prelude::*;
use dioxus_i18n::tid;
use spacetimedb_sdk::DbContext;

#[component]
pub fn ProfileListScreen(on_navigate: EventHandler<Screen>) -> Element {
    let mut search_filter = use_signal(String::new);
    let mut show_dead = use_signal(|| false);
    let mut first_ring_filter = use_signal(|| None::<RingColor>);
    let mut second_ring_filter = use_signal(|| None::<RingColor>);
    let mut active_ring_filter_slot = use_signal(|| None::<usize>);
    let mut gender_filter = use_signal(|| None::<Gender>);
    let mut show_gender_palette = use_signal(|| false);
    let quails = spacetime::use_table_quails();
    let events = spacetime::use_table_quail_events();
    let photos = spacetime::use_table_photos();
    let connection = spacetime::use_connection();

    spacetime::use_subscription(&[
        "SELECT * FROM quails",
        "SELECT * FROM quail_events",
        "SELECT * FROM photos",
    ]);

    use_effect(move || {
        if show_dead() {
            active_ring_filter_slot.set(None);
            show_gender_palette.set(false);
        }
    });

    let filtered_profiles = use_memo(move || {
        let owner = connection()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        let all_events = events().clone();
        let search = search_filter().to_lowercase();
        let dead_only = show_dead();
        let first_ring = first_ring_filter();
        let second_ring = second_ring_filter();
        let selected_gender = gender_filter();
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
            Option<i64>,
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
                if !dead_only
                    && selected_gender
                        .as_ref()
                        .is_some_and(|selected| local_quail.gender != *selected)
                {
                    continue;
                }

                if !dead_only
                    && !ring_color_filter_matches(
                        first_ring.as_ref(),
                        second_ring.as_ref(),
                        local_quail.ring_color_left.as_ref(),
                        local_quail.ring_color_right.as_ref(),
                    )
                {
                    continue;
                }

                let profile_photo_uuid =
                    local_quail.profile_photo.as_ref().map(|id| id.to_string());
                let profile_photo_path = profile_photo_uuid
                    .as_ref()
                    .and_then(|uuid| photo_paths_by_uuid.get(uuid).cloned());
                let age_display = born_dates_by_quail
                    .get(&remote_quail.uuid)
                    .map(|birth_date| format_age_years_months(*birth_date, today));
                let days_since_last_event = latest_event_date_for(&remote_quail.uuid, &all_events)
                    .map(|d| today.signed_duration_since(d).num_days());

                rows.push((
                    local_quail,
                    status,
                    profile_photo_uuid,
                    profile_photo_path,
                    age_display,
                    days_since_last_event,
                ));
            }
        }

        rows.sort_by(|a, b| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()));
        rows
    });

    let dead_only = show_dead();
    let active_palette_slot = active_ring_filter_slot();
    let first_ring = first_ring_filter();
    let second_ring = second_ring_filter();
    let selected_gender = gender_filter();
    let gender_palette_open = show_gender_palette();
    let current_palette_selection = match active_palette_slot {
        Some(0) => first_ring.clone(),
        Some(1) => second_ring.clone(),
        _ => None,
    };

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
                                class: if dead_only { "button is-link" } else { "button is-link is-light" },
                                onclick: move |_| {
                                    let next_dead_only = !show_dead();
                                    show_dead.set(next_dead_only);
                                    if next_dead_only {
                                        active_ring_filter_slot.set(None);
                                    }
                                },
                                span { class: "icon is-small", "🪦" }
                            }
                            RingColorTrigger {
                                field_label: tid!("field-ring-color"),
                                side_label: tid!("ring-color-side-left"),
                                selected: first_ring.clone(),
                                is_open: active_palette_slot == Some(0),
                                disabled: dead_only,
                                compact: true,
                                on_toggle: move |_| {
                                    show_gender_palette.set(false);
                                    active_ring_filter_slot
                                        .set(if active_ring_filter_slot() == Some(0) { None } else { Some(0) });
                                },
                            }
                            RingColorTrigger {
                                field_label: tid!("field-ring-color"),
                                side_label: tid!("ring-color-side-right"),
                                selected: second_ring.clone(),
                                is_open: active_palette_slot == Some(1),
                                disabled: dead_only,
                                compact: true,
                                on_toggle: move |_| {
                                    show_gender_palette.set(false);
                                    active_ring_filter_slot
                                        .set(if active_ring_filter_slot() == Some(1) { None } else { Some(1) });
                                },
                            }
                            button {
                                class: if gender_palette_open { "button is-link" } else { "button is-link is-light" },
                                style: "min-width: 2.5rem;",
                                disabled: dead_only,
                                title: gender_filter_button_title(selected_gender.as_ref()),
                                aria_label: gender_filter_button_title(selected_gender.as_ref()),
                                onclick: move |_| {
                                    active_ring_filter_slot.set(None);
                                    show_gender_palette.set(!show_gender_palette());
                                },
                                "{gender_filter_button_icon(selected_gender.as_ref())}"
                            }
                            button {
                                class: "button is-success",
                                onclick: move |_| on_navigate.call(Screen::AddProfile),
                                span { class: "icon is-small", "+" }
                            }
                        }
                    }
                }

                if !dead_only {
                    if gender_palette_open {
                        div {
                            class: "box p-2 mb-3",
                            style: "max-width: 16rem; margin-left: auto;",
                            div {
                                class: "buttons are-small mb-0",
                                style: "display: flex; flex-wrap: wrap; gap: 0.35rem;",
                                for option in [None, Some(Gender::Female), Some(Gender::Male), Some(Gender::Unknown)] {
                                    {
                                        let option_value = option.clone();
                                        let option_title = gender_filter_button_title(option_value.as_ref());
                                        let option_icon = gender_filter_button_icon(option_value.as_ref());

                                        rsx! {
                                            button {
                                                class: if selected_gender == option { "button is-link" } else { "button is-light" },
                                                title: option_title.clone(),
                                                aria_label: option_title,
                                                onclick: move |_| {
                                                    gender_filter.set(option_value.clone());
                                                    show_gender_palette.set(false);
                                                },
                                                "{option_icon}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(slot) = active_palette_slot {
                        RingColorPalette {
                            selected: current_palette_selection.clone(),
                            key_prefix: format!("profile-filter-{slot}"),
                            container_style: "max-width: 16rem; margin-left: auto; margin-bottom: 1rem;".to_string(),
                            on_select: move |value| {
                                match slot {
                                    0 => first_ring_filter.set(value),
                                    1 => second_ring_filter.set(value),
                                    _ => {}
                                }
                                active_ring_filter_slot.set(None);
                            },
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
                        for (
        profile,
        status,
        profile_photo_uuid,
        profile_photo_path,
        age_display,
        days_since_last_event,
    ) in filtered_profiles() {
                            ProfileCard {
                                key: "{profile.uuid}",
                                profile: profile.clone(),
                                profile_photo_path,
                                profile_photo_uuid,
                                age_display,
                                status,
                                days_since_last_event,
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
    days_since_last_event: Option<i64>,
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

    let border = border_color(&profile.gender, days_since_last_event);

    rsx! {
        div {
            class: "profile-card",
            style: "border: 3px solid {border};",
            onclick: move |_| on_click.call(()),
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
                            div { class: "profile-gender", "{gender_label(&profile.gender)}" }
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

fn latest_event_date_for(quail_uuid: &str, events: &[spacetime::QuailEvent]) -> Option<NaiveDate> {
    events
        .iter()
        .filter(|event| event.quail_uuid == quail_uuid)
        .max_by(|a, b| {
            a.event_date
                .cmp(&b.event_date)
                .then_with(|| a.uuid.cmp(&b.uuid))
        })
        .and_then(|event| NaiveDate::parse_from_str(&event.event_date, "%Y-%m-%d").ok())
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

fn border_color(gender: &Gender, days: Option<i64>) -> String {
    let t = (days.unwrap_or(100).clamp(0, 100) as f32) / 100.0;
    let (r0, g0, b0, r1, g1, b1) = match gender {
        Gender::Male => (37u8, 99u8, 235u8, 153u8, 27u8, 27u8),
        Gender::Female => (22u8, 163u8, 74u8, 234u8, 88u8, 12u8),
        Gender::Unknown => (236u8, 72u8, 153u8, 255u8, 255u8, 255u8),
    };
    let r = (r0 as f32 + (r1 as f32 - r0 as f32) * t).round() as u8;
    let g = (g0 as f32 + (g1 as f32 - g0 as f32) * t).round() as u8;
    let b = (b0 as f32 + (b1 as f32 - b0 as f32) * t).round() as u8;
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn split_overlay_bg(left: Option<&RingColor>, right: Option<&RingColor>) -> String {
    let left_bg = left
        .map(get_light_color_for)
        .unwrap_or("rgba(255, 255, 255, 0.0)");
    let right_bg = right
        .map(get_light_color_for)
        .unwrap_or("rgba(255, 255, 255, 0.0)");
    format!(
        "linear-gradient(to right, {} 0%, {} 50%, {} 50%, {} 100%)",
        left_bg, left_bg, right_bg, right_bg
    )
}

fn get_light_color_for(color: &RingColor) -> &'static str {
    const TRANSPARENCY: &str = "0.8"; // Adjust this value to make colors more or less transparent
    match color {
        RingColor::Rot => concatcp!("rgba(255, 200, 200, ", TRANSPARENCY, ")"),
        RingColor::Dunkelblau => concatcp!("rgba(200, 210, 245, ", TRANSPARENCY, ")"),
        RingColor::Hellblau => concatcp!("rgba(210, 230, 255, ", TRANSPARENCY, ")"),
        RingColor::Gruen => concatcp!("rgba(200, 255, 200, ", TRANSPARENCY, ")"),
        RingColor::Gelb => concatcp!("rgba(255, 255, 200, ", TRANSPARENCY, ")"),
        RingColor::Orange => concatcp!("rgba(255, 230, 200, ", TRANSPARENCY, ")"),
        RingColor::Lila => concatcp!("rgba(230, 200, 255, ", TRANSPARENCY, ")"),
        RingColor::Rosa => concatcp!("rgba(255, 200, 230, ", TRANSPARENCY, ")"),
        RingColor::Schwarz => concatcp!("rgba(220, 220, 220, ", TRANSPARENCY, ")"),
        RingColor::Weiss => concatcp!("rgba(255, 255, 255, ", TRANSPARENCY, ")"),
    }
}

fn gender_filter_button_icon(selected: Option<&Gender>) -> &'static str {
    match selected {
        Some(Gender::Female) => "♀️",
        Some(Gender::Male) => "♂️",
        Some(Gender::Unknown) => "🐣",
        None => "⚧️",
    }
}

fn gender_filter_button_title(selected: Option<&Gender>) -> String {
    selected
        .map(gender_label)
        .unwrap_or_else(|| tid!("period-all"))
}
