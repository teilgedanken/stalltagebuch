use crate::database;
use crate::models::{Quail, RingColor};
use crate::services;
use crate::Screen;
use dioxus::prelude::*;
use dioxus_i18n::t;
use photo_gallery::ThumbnailImage;

#[component]
pub fn ProfileListScreen(on_navigate: EventHandler<Screen>) -> Element {
    let mut profiles = use_signal(|| Vec::<Quail>::new());
    let mut search_filter = use_signal(|| String::new());
    // Toggle zeigt "nur Tote" an; Standard (false) zeigt Lebende + Markierte
    let mut show_dead = use_signal(|| false);

    // Load profiles
    let mut load_profiles = move || match database::init_database() {
        Ok(conn) => {
            let search_value = search_filter();
            let filter = if search_value.is_empty() {
                None
            } else {
                Some(search_value.as_str())
            };

            match services::profile_service::list_profiles_with_status(&conn, filter, !show_dead())
            {
                Ok(list) => profiles.set(list),
                Err(e) => log::error!("{}: {}", t!("error-load-profiles-failed"), e), // Failed to load profiles
            }
        }
        Err(e) => log::error!("DB-Fehler: {}", e),
    };

    // Load on mount
    use_effect(move || {
        load_profiles();
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
                            load_profiles();
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
                        load_profiles();
                    },
                }
            }

            // Profile Grid
            if profiles().is_empty() {
                div { style: "text-align: center; padding: 40px; color: #999;",
                    {t!("profile-list-empty")} // No profiles available
                }
            } else {
                div { class: "profile-grid",
                    for profile in profiles() {
                        ProfileCard {
                            profile: profile.clone(),
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
pub fn ProfileCard(profile: Quail, on_click: EventHandler<()>) -> Element {
    let profile_uuid = profile.uuid;

    // image state / loading handled by ThumbnailImage component and resource

    // Load profile photo UUID and trigger background download; UI uses `ThumbnailImage`
    let profile_photo_uuid = use_resource(move || async move {
        log::debug!("##### Lade Profilbild UUID für {:?}", profile_uuid);
        if let Ok(conn) = database::init_database() {
            match services::photo_service::get_profile_photo(&conn, &profile_uuid) {
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

    // Load current status from events
    let mut current_status = use_signal(|| None::<crate::models::EventType>);
    let profile_uuid_for_effect = profile.uuid;
    use_effect(move || {
        if let Ok(conn) = database::init_database() {
            if let Ok(status) = services::profile_service::get_profile_current_status(
                &conn,
                &profile_uuid_for_effect,
            ) {
                current_status.set(status);
            }
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
                    if let Some(status) = current_status() {
                        match status {
                            crate::models::EventType::Sick => rsx! {
                                div { style: "position: absolute; top: 8px; right: 8px; font-size: 32px; background: rgba(255,255,255,0.9); border-radius: 50%; width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px rgba(0,0,0,0.3);",
                                    "🤒"
                                }
                            },
                            crate::models::EventType::MarkedForSlaughter => {
                                rsx! {
                                    div { style: "position: absolute; top: 8px; right: 8px; font-size: 32px; background: rgba(255,255,255,0.9); border-radius: 50%; width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px rgba(0,0,0,0.3);",
                                        "🥩"
                                    }
                                }
                            }
                            crate::models::EventType::Died => rsx! {
                                div { style: "position: absolute; top: 8px; right: 8px; font-size: 32px; background: rgba(255,255,255,0.9); border-radius: 50%; width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; box-shadow: 0 2px 8px rgba(0,0,0,0.3);",
                                    "🪦"
                                }
                            },
                            crate::models::EventType::Slaughtered => {
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
