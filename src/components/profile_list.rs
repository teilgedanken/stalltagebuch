use crate::database;
use crate::models::{EventType, Gender, Quail, RingColor};
use crate::spacetime;
use crate::Screen;
use dioxus::prelude::*;
use dioxus_i18n::t;
use photo_gallery::ThumbnailImage;
use spacetimedb_sdk::DbContext;

#[component]
pub fn ProfileListScreen(on_navigate: EventHandler<Screen>) -> Element {
    let mut search_filter = use_signal(|| String::new());
    // Toggle zeigt "nur Tote" an; Standard (false) zeigt Lebende + Markierte
    let mut show_dead = use_signal(|| false);
    let quails = spacetime::use_table_quails();
    let events = spacetime::use_table_quail_events();
    let connection = spacetime::use_connection();

    spacetime::use_subscription(&["SELECT * FROM quails", "SELECT * FROM quail_events"]);

    let filtered_profiles = use_memo(move || {
        let owner = connection
            .read()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        let all_events = events.read().clone();
        let search = search_filter().to_lowercase();
        let dead_only = show_dead();

        let mut rows = Vec::<(Quail, Option<EventType>)>::new();

        for remote_quail in quails.read().iter() {
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
                rows.push((local_quail, status));
            }
        }

        rows.sort_by(|a, b| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()));
        rows
    });

    rsx! {
        div { style: "padding: 16px; max-width: 600px; margin: 0 auto; min-height: 100vh; background: #f5f5f5;",

            // Header
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; padding-top: 8px;",
                h1 { style: "color: #0066cc; margin: 0; font-size: 24px; font-weight: 700;",
                    "🐦 "
                    {t!("profile-list-title")}
                }
                div { style: "display: flex; gap: 8px; align-items: center;",
                    // Toggle: nur tote anzeigen
                    button {
                        style: format!(
                            "padding: 8px 10px; font-size: 16px; border-radius: 8px; {}",
                            if show_dead() {
                                "background:#ffe6e6; color:#a00; border:1px solid #f5b5b5;"
                            } else {
                                "background:#f0f0f0; color:#666; border:1px solid #ddd;"
                            },
                        ),
                        onclick: move |_| {
                            show_dead.set(!show_dead());
                        },
                        // Tombstone Emoji
                        "🪦"
                    }
                    button {
                        class: "btn-success",
                        style: "padding: 10px 16px; font-size: 16px; font-weight: 500;",
                        onclick: move |_| on_navigate.call(Screen::AddProfile),
                        "+ "
                        {t!("action-new")} // New
                    }
                }
            }

            // Search & Filter
            div { style: "margin: 12px 0 16px;",
                input {
                    style: "width: 100%; padding: 14px 16px; font-size: 16px; border: 2px solid #e0e0e0; border-radius: 10px; background: white; margin-bottom: 12px;",
                    r#type: "text",
                    placeholder: "🔍 {t!(\"search-placeholder-name\")}",
                    value: "{search_filter}",
                    oninput: move |e| {
                        search_filter.set(e.value());
                    },
                }
            }

            // Profile Grid
            if filtered_profiles().is_empty() {
                div { style: "text-align: center; padding: 40px; color: #999;",
                    {t!("profile-list-empty")} // No profiles available
                }
            } else {
                div { class: "profile-grid",
                    for (profile , status) in filtered_profiles() {
                        ProfileCard {
                            profile: profile.clone(),
                            status: status.clone(),
                            on_click: move |_| {
                                on_navigate.call(Screen::ProfileDetail(profile.uuid.to_string()));
                            },
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
    status: Option<EventType>,
    on_click: EventHandler<()>,
) -> Element {
    let profile_uuid = profile.uuid;

    // image state / loading handled by ThumbnailImage component and resource

    // Load profile photo UUID and trigger background download; UI uses `ThumbnailImage`
    let profile_photo_uuid = use_resource(move || async move {
        log::debug!("##### Lade Profilbild UUID für {:?}", profile_uuid);
        if let Ok(conn) = database::init_database() {
            match crate::services::photo_service::get_profile_photo(&conn, &profile_uuid) {
                Ok(Some(photo)) => Some(photo.uuid),
                Ok(None) => {
                    log::debug!("Kein Profilbild in DB gefunden für UUID: {}", profile_uuid);
                    None
                }
                Err(e) => {
                    log::error!("Fehler beim Laden des Profilbilds: {}", e);
                    None
                }
            }
        } else {
            None
        }
    });

    // Convert ring color to light version for overlay background
    let overlay_bg = if let Some(ring_color) = &profile.ring_color {
        get_light_color_for(ring_color)
    } else {
        "rgba(255, 255, 255, 0.9)".to_string()
    };

    // Image Data URL (Base64)
    // let has_image = image_data().is_some(); // aktuell nicht genutzt

    rsx! {
        div { class: "profile-card", onclick: move |_| on_click.call(()),
            // Square Image Container
            div { class: "profile-image",
                match profile_photo_uuid() {
                    None => rsx! {
                        div {
                            class: "profile-image-placeholder",
                            style: "display: flex; align-items: center; justify-content: center; font-size: 24px;",
                            "⏳"
                        }
                    },
                    Some(Some(uuid)) => rsx! {
                        ThumbnailImage { photo_uuid: Some(uuid.clone()), alt: profile.name.clone() }
                    },
                    Some(None) => rsx! {
                        div { class: "profile-image-placeholder", "🐦" }
                    },
                }

                // Overlay with name and gender
                div {
                    class: "profile-overlay",
                    style: format!("background: {};", overlay_bg),
                    div { class: "profile-name", "{profile.name}" }
                    div { class: "profile-gender", "{profile.gender.display_name()}" }
                }

                // Status Overlay Emoji (top right corner)
                {
                    if let Some(status) = status {
                        match status {
                            EventType::Sick => rsx! {
                                div { style: "position: absolute; top: 8px; right: 8px; font-size: 32px; background: rgba(255,255,255,0.9); border-radius: 50%; width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px rgba(0,0,0,0.3);",
                                    "🤒"
                                }
                            },
                            EventType::MarkedForSlaughter => {
                                rsx! {
                                    div { style: "position: absolute; top: 8px; right: 8px; font-size: 32px; background: rgba(255,255,255,0.9); border-radius: 50%; width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px rgba(0,0,0,0.3);",
                                        "🥩"
                                    }
                                }
                            }
                            EventType::Died => rsx! {
                                div { style: "position: absolute; top: 8px; right: 8px; font-size: 32px; background: rgba(255,255,255,0.9); border-radius: 50%; width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px rgba(0,0,0,0.3);",
                                    "🪦"
                                }
                            },
                            EventType::Slaughtered => {
                                rsx! {
                                    div { style: "position: absolute; top: 8px; right: 8px; font-size: 32px; background: rgba(255,255,255,0.9); border-radius: 50%; width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px rgba(0,0,0,0.3);",
                                        "🥩"
                                    }
                                }
                            }
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
                .then_with(|| a.id.cmp(&b.id))
        })
        .map(|event| EventType::from_str(&event.event_type))
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
        ring_color: remote
            .ring_color
            .as_ref()
            .map(|value| RingColor::from_str(value)),
        profile_photo,
    })
}

/// Helper function to convert color names to light versions
fn get_light_color_for(color: &RingColor) -> String {
    match color {
        RingColor::Rot => "rgba(255, 200, 200, 0.9)".to_string(),
        RingColor::Dunkelblau => "rgba(200, 210, 245, 0.9)".to_string(),
        RingColor::Hellblau => "rgba(210, 230, 255, 0.9)".to_string(),
        RingColor::Gruen => "rgba(200, 255, 200, 0.9)".to_string(),
        RingColor::Gelb => "rgba(255, 255, 200, 0.9)".to_string(),
        RingColor::Orange => "rgba(255, 230, 200, 0.9)".to_string(),
        RingColor::Lila => "rgba(230, 200, 255, 0.9)".to_string(),
        RingColor::Rosa => "rgba(255, 200, 230, 0.9)".to_string(),
        RingColor::Schwarz => "rgba(220, 220, 220, 0.9)".to_string(),
        RingColor::Weiss => "rgba(255, 255, 255, 0.9)".to_string(),
    }
}
