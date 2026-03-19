use super::profile_photo_card::ProfilePhotoCard;
// image loading is handled by photo_gallery components (PreviewCollection / FullscreenCollection)
use crate::Screen;
use crate::models::{EventType, Gender};
use crate::spacetime;
use dioxus::prelude::*;
use dioxus_i18n::tid;
use photo_gallery::{CollectionFullscreen, ThumbnailImage};
use spacetimedb_sdk::DbContext;

/// Helper function to resolve photo path to absolute path
/// Handles both full paths (starting with /) and relative filenames
// resolve_photo_path and image_to_data_url logic is no longer needed here

#[component]
pub fn ProfileDetailScreen(quail_id: String, on_navigate: EventHandler<Screen>) -> Element {
    let quails = spacetime::use_table_quails();
    let all_events = spacetime::use_table_quail_events();
    let photo_collections_table = spacetime::use_table_photo_collections();
    let photos_table = spacetime::use_table_photos();

    let connection = spacetime::use_connection();

    spacetime::use_subscription(&[
        "SELECT * FROM quails",
        "SELECT * FROM quail_events",
        "SELECT * FROM photo_collections",
        "SELECT * FROM photos",
    ]);

    // Force refresh counter - incremented when returning from other screens
    let mut refresh_counter = use_signal(|| 0u32);

    let quail_id_for_profile_memo = quail_id.clone();
    let profile = use_memo(move || {
        // Depend on refresh_counter to force re-evaluation
        let _ = refresh_counter();

        let owner = connection()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        quails()
            .iter()
            .find(|quail| {
                quail.uuid == quail_id_for_profile_memo
                    && owner
                        .as_ref()
                        .map(|owner_value| &quail.owner == owner_value)
                        .unwrap_or(true)
            })
            .cloned()
    });

    let quail_id_for_events_memo = quail_id.clone();
    let events = use_memo(move || {
        let owner = connection()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        let mut rows: Vec<spacetime::QuailEvent> = all_events()
            .iter()
            .filter(|event| {
                event.quail_uuid == quail_id_for_events_memo
                    && owner
                        .as_ref()
                        .map(|owner_value| &event.owner == owner_value)
                        .unwrap_or(true)
            })
            .cloned()
            .collect();

        rows.sort_by(|a, b| {
            b.event_date
                .cmp(&a.event_date)
                .then_with(|| b.uuid.cmp(&a.uuid))
        });
        rows
    });

    let quail_id_for_photos_memo = quail_id.clone();
    let photo_collections = use_memo(move || {
        let owner = connection()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        let mut rows: Vec<spacetime::PhotoCollection> = photo_collections_table()
            .iter()
            .filter(|coll| {
                coll.quail_uuid.as_ref() == Some(&quail_id_for_photos_memo)
                    && owner
                        .as_ref()
                        .map(|owner_value| &coll.owner == owner_value)
                        .unwrap_or(true)
            })
            .cloned()
            .collect();

        rows.sort_by(|a, b| b.uuid.cmp(&a.uuid));
        rows
    });

    let photo_uuid_to_meta = use_memo(move || {
        let mut map = std::collections::HashMap::new();
        for photo in photos_table().iter() {
            map.insert(
                photo.uuid.clone(),
                (photo.relative_path.clone(), photo.created_at),
            );
        }
        map
    });

    let collection_to_photos = use_memo(move || {
        let mut map = std::collections::HashMap::new();
        for photo in photos_table().iter() {
            map.entry(photo.collection_uuid.clone())
                .or_insert_with(Vec::new)
                .push(photo.uuid.clone());
        }
        map
    });

    let filtered_photo_items = use_memo(move || {
        let collections = photo_collections();
        let photo_map = photo_uuid_to_meta();
        let coll_photos = collection_to_photos();

        let mut rows: Vec<(String, String, i64)> = vec![];
        for collection in collections {
            if let Some(photo_uuids) = coll_photos.get(&collection.uuid) {
                for uuid in photo_uuids {
                    if let Some((path, created_at)) = photo_map.get(uuid) {
                        rows.push((uuid.clone(), path.clone(), *created_at));
                    }
                }
            }
        }

        // Stable chronological order by creation date (oldest first).
        rows.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(&b.0)));
        rows
    });

    // Auto-refresh when quails table changes to pick up edits from profile_edit screen
    let quail_id_for_auto_refresh = quail_id.clone();
    use_effect(move || {
        // Just reading quails() creates a dependency, forcing this effect to run
        // whenever quails table updates
        if let Ok(uuid) = uuid::Uuid::parse_str(&quail_id_for_auto_refresh) {
            if let Some(_) = quails().iter().find(|q| q.uuid == uuid.to_string()) {
                // Increment refresh counter to force memo recalculation
                refresh_counter.with_mut(|c| *c = c.wrapping_add(1));
            }
        }
    });

    rsx! {
        div { style: "padding: 16px; max-width: 800px; margin: 0 auto;",
            // Header
            div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 24px;",
                button {
                    style: "padding: 8px 16px; background: #e0e0e0; color: #333; border-radius: 8px; font-size: 16px;",
                    onclick: move |_| on_navigate.call(Screen::ProfileList),
                    "← "
                    {tid!("action-back")}
                }
                h1 {
                    style: "margin: 0; font-size: 26px; color: #0066cc; font-weight: 700;",
                    {tid!("profile-detail-title")} // Profile
                }
            }

            if let Some(p) = profile() {
                div { style: "display: flex; flex-direction: column; gap: 24px;",
                    // Profilbild + Medienverwaltung (Galerie/Kamera + Vollbild)
                    ProfilePhotoCard {
                        quail_id: quail_id.clone(),
                        profile_photo: p.profile_photo.clone(),
                    }

                    // Basisinfos
                    div { style: "display: flex; flex-direction: column; gap: 12px;",
                        h2 { style: "margin:0; font-size: 28px; color:#333; font-weight:600;",
                            "{p.name}"
                        }
                        div { style: "display:flex; flex-wrap:wrap; gap:8px;",
                            span { style: "padding:6px 14px; background:#e8f4f8; border-radius:16px; font-size:13px; color:#0066cc;",
                                "ID {p.uuid.chars().take(8).collect::<String>()}"
                            }
                            span { style: "padding:6px 14px; background:#fff3e0; border-radius:16px; font-size:13px; color:#ff8c00;",
                                "{Gender::from_str(&p.gender).display_name()}"
                            }
                            // Status Badge basierend auf letztem Event
                            if let Some(latest_event) = events().first() {
                                match EventType::from_str(&latest_event.event_type) {
                                    EventType::Born => rsx! {
                                        span { style: "padding:6px 14px; background:#e0ffe6; border-radius:16px; font-size:13px; color:#228833;",
                                            "🐣 "
                                            {tid!("status-born")}
                                        }
                                    },
                                    EventType::Alive => rsx! {
                                        span { style: "padding:6px 14px; background:#e0ffe6; border-radius:16px; font-size:13px; color:#228833;",
                                            "✅ "
                                            {tid!("status-alive")}
                                        }
                                    },
                                    EventType::Sick => rsx! {
                                        span { style: "padding:6px 14px; background:#ffe0e0; border-radius:16px; font-size:13px; color:#cc3333;",
                                            "🤒 "
                                            {tid!("status-sick")}
                                        }
                                    },
                                    EventType::Healthy => rsx! {
                                        span { style: "padding:6px 14px; background:#e0ffe6; border-radius:16px; font-size:13px; color:#228833;",
                                            "💪 "
                                            {tid!("status-healthy")}
                                        }
                                    },
                                    EventType::MarkedForSlaughter => {
                                        rsx! {
                                            span { style: "padding:6px 14px; background:#fff3e0; border-radius:16px; font-size:13px; color:#ff8800;",
                                                "🥩 "
                                                {tid!("status-marked")}
                                            }
                                        }
                                    }
                                    EventType::Slaughtered => rsx! {
                                        span { style: "padding:6px 14px; background:#f0f0f0; border-radius:16px; font-size:13px; color:#666;",
                                            "🥩 "
                                            {tid!("status-slaughtered")}
                                        }
                                    },
                                    EventType::Died => rsx! {
                                        span { style: "padding:6px 14px; background:#f0f0f0; border-radius:16px; font-size:13px; color:#666;",
                                            "🪦 "
                                            {tid!("status-died")}
                                        }
                                    },
                                }
                            }
                        }
                    }
                    // Detail Grid
                    div { style: "display:grid; gap:16px;",
                        div { style: "padding:14px; background:#f5f5f5; border-radius:8px;",
                            div { style: "font-size:11px; color:#666; font-weight:600; margin-bottom:4px;",
                                "UUID"
                            }
                            div { style: "font-size:11px; color:#999; word-break:break-all; font-family:monospace;",
                                "{p.uuid}"
                            }
                        }
                    }

                    // Photo Gallery Grid
                    {
                        let photo_items = filtered_photo_items();
                        if !photo_items.is_empty() {
                            let mut show_fullscreen = use_signal(|| false);
                            let mut selected_photos = use_signal(Vec::<String>::new);
                            let mut selected_index = use_signal(|| 0usize);
                            let photo_refresh = use_signal(|| 0u32);
                            let all_paths: Vec<String> = photo_items
                                .iter()
                                .map(|(_, path, _)| path.clone())
                                .collect();

                            rsx! {
                                div { style: "margin-top:24px;",
                                    div { style: "display:flex; align-items:center; margin-bottom:12px;",
                                        h3 { style: "margin:0; font-size:18px; color:#333; font-weight:600;",
                                            "📸 "
                                            {tid!("photos-title")} // Fotos
                                        }
                                    }

                                    div { style: "display:grid; grid-template-columns:repeat(auto-fill, minmax(128px, 1fr)); gap:12px;",
                                        for (idx, (photo_uuid, photo_path, _created_at)) in photo_items.iter().enumerate() {
                                            div {
                                                key: "{photo_uuid}-{photo_refresh()}",
                                                style: "cursor: pointer; position: relative;",
                                                onclick: {
                                                    let all_paths_click = all_paths.clone();
                                                    move |_| {
                                                        if !all_paths_click.is_empty() {
                                                            selected_photos.set(all_paths_click.clone());
                                                            selected_index.set(idx);
                                                            show_fullscreen.set(true);
                                                        }
                                                    }
                                                },
                                                ThumbnailImage {
                                                    relative_path: photo_path.clone(),
                                                }
                                            }
                                        }
                                    }

                                    if show_fullscreen() && !selected_photos().is_empty() {
                                        CollectionFullscreen {
                                            photo_paths: selected_photos(),
                                            initial_index: selected_index(),
                                            on_close: move |_| show_fullscreen.set(false),
                                        }
                                    }
                                }
                            }
                        } else {
                            rsx! {
                                div {}
                            }
                        }
                    }

                    // Events Timeline
                    div { style: "margin-top:24px;",
                        div { style: "display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;",
                            h3 { style: "margin:0; font-size:18px; color:#333; font-weight:600;",
                                "📅 "
                                {tid!("events-timeline-title")}
                            }
                            button {
                                style: "padding:8px 16px; background:#0066cc; color:white; border-radius:8px; font-size:14px; font-weight:500;",
                                onclick: move |_| {
                                    if let Some(p) = profile() {
                                        on_navigate
                                            .call(Screen::EventAdd {
                                                quail_id: p.uuid.to_string(),
                                                quail_name: p.name.clone(),
                                            });
                                    }
                                },
                                "+ "
                                {tid!("action-add-event")} // Add event
                            }
                        }

                        if events().is_empty() {
                            div {
                                style: "padding:24px; text-align:center; background:#f5f5f5; border-radius:8px; color:#999;",
                                {tid!("events-empty")} // No events available
                            }
                        } else {
                            div { style: "display:flex; flex-direction:column; gap:12px;",
                                for event in events() {
                                    div {
                                        key: "{event.uuid}",
                                        style: "padding:14px; background:white; border:1px solid #e0e0e0; border-radius:8px; cursor:pointer;",
                                        onclick: {
                                            let quail_id_for_event = quail_id.clone();
                                            move |_| {
                                                on_navigate
                                                    .call(Screen::EventEdit {
                                                        event_id: event.uuid.to_string(),
                                                        quail_id: quail_id_for_event.clone(),
                                                    });
                                            }
                                        },
                                        div { style: "display:flex; gap:10px; align-items:center; margin-bottom:8px;",
                                            span { style: "font-size:20px;",
                                                match EventType::from_str(&event.event_type) {
                                                    EventType::Born => "🐣",
                                                    EventType::Alive => "✅",
                                                    EventType::Sick => "🤒",
                                                    EventType::Healthy => "💪",
                                                    EventType::MarkedForSlaughter => "🥩",
                                                    EventType::Slaughtered => "🥩",
                                                    EventType::Died => "🪦",
                                                }
                                            }
                                            div {
                                                div { style: "font-size:14px; font-weight:600; color:#333;",
                                                    "{EventType::from_str(&event.event_type).display_name()}"
                                                }
                                                div { style: "font-size:12px; color:#666;",
                                                    {format_event_date(&event.event_date)}
                                                }
                                            }
                                        }
                                        if let Some(notes) = &event.notes {
                                            div { style: "font-size:13px; color:#555; line-height:1.4; white-space:pre-wrap;",
                                                "{notes}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Bearbeiten Button
                    button {
                        class: "btn-primary",
                        style: "width:100%; padding:14px; font-size:16px; font-weight:600; margin-top:24px;",
                        onclick: {
                            let quail_id_for_edit = quail_id.clone();
                            move |_| on_navigate.call(Screen::ProfileEdit(quail_id_for_edit.clone()))
                        },
                        "✏️ "
                        {tid!("action-edit")}
                    }
                }
            } else {
                div { style: "padding:48px; text-align:center;",
                    div { style: "font-size:48px; margin-bottom:16px;", "⏳" }
                    div { style: "color:#666;", {tid!("loading-profile")} } // Loading profile...
                }
            }
        }
    }
}
fn format_event_date(value: &str) -> String {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|date| date.format("%d.%m.%Y").to_string())
        .unwrap_or_else(|_| value.to_string())
}
