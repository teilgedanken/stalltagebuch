use super::profile_i18n::{format_age_years_months, gender_label};
use super::profile_photo_card::ProfilePhotoCard;
use super::synced_photo::{SyncedCollectionFullscreen, SyncedThumbnailImage};
// image loading is handled by photo_gallery components (PreviewCollection / FullscreenCollection)
use crate::Screen;
use crate::models::{EventType, Gender};
use crate::spacetime;
use chrono::{Local, NaiveDate};
use dioxus::prelude::*;
use dioxus_i18n::tid;
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

    let age_display = use_memo(move || {
        let today = Local::now().date_naive();
        age_display_from_events(&events(), today)
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
        section { class: "section pt-4 pb-3",
            div { class: "container is-max-desktop",
                div { class: "level mb-4",
                    div { class: "level-left",
                        button {
                            class: "button is-light",
                            onclick: move |_| on_navigate.call(Screen::ProfileList),
                            "← "
                            {tid!("action-back")}
                        }
                    }
                    div { class: "level-item",
                        h1 { class: "title is-4 mb-0", {tid!("profile-detail-title")} }
                    }
                    div { class: "level-right" }
                }

                if let Some(p) = profile() {
                    div {
                        class: "is-flex is-flex-direction-column",
                        style: "gap: 24px;",
                        ProfilePhotoCard {
                            quail_id: quail_id.clone(),
                            profile_photo: p.profile_photo.clone(),
                        }

                        div { class: "box",
                            h2 { class: "title is-3 mb-3", "{p.name}" }

                            div { class: "tags mb-3",
                                span { class: "tag is-info is-light",
                                    "ID {p.uuid.chars().take(8).collect::<String>()}"
                                }
                                span { class: "tag is-warning is-light",
                                    "{gender_label(&Gender::from_str(&p.gender))}"
                                }
                                if let Some(latest_event) = events().first() {
                                    match EventType::from_str(&latest_event.event_type) {
                                        EventType::Born => rsx! {
                                            span { class: "tag is-success is-light",
                                                "🐣 "
                                                {tid!("status-born")}
                                            }
                                        },
                                        EventType::Alive => rsx! {
                                            span { class: "tag is-success is-light",
                                                "✅ "
                                                {tid!("status-alive")}
                                            }
                                        },
                                        EventType::Sick => rsx! {
                                            span { class: "tag is-danger is-light",
                                                "🤒 "
                                                {tid!("status-sick")}
                                            }
                                        },
                                        EventType::Healthy => rsx! {
                                            span { class: "tag is-success is-light",
                                                "💪 "
                                                {tid!("status-healthy")}
                                            }
                                        },
                                        EventType::MarkedForSlaughter => rsx! {
                                            span { class: "tag is-warning is-light",
                                                "🥩 "
                                                {tid!("status-marked")}
                                            }
                                        },
                                        EventType::Slaughtered => rsx! {
                                            span { class: "tag is-dark is-light",
                                                "🥩 "
                                                {tid!("status-slaughtered")}
                                            }
                                        },
                                        EventType::Died => rsx! {
                                            span { class: "tag is-dark is-light",
                                                "🪦 "
                                                {tid!("status-died")}
                                            }
                                        },
                                    }
                                }
                            }

                            div { class: "columns is-mobile is-multiline",
                                if let Some(age) = age_display() {
                                    div { class: "column is-half",
                                        article { class: "message is-light",
                                            div { class: "message-body",
                                                p { class: "is-size-7 has-text-grey mb-1",
                                                    {tid!("field-age")}
                                                }
                                                p { class: "has-text-weight-semibold",
                                                    "{age}"
                                                }
                                            }
                                        }
                                    }
                                }

                                div { class: "column is-half",
                                    article { class: "message is-light",
                                        div { class: "message-body",
                                            p { class: "is-size-7 has-text-grey mb-1",
                                                "UUID"
                                            }
                                            p {
                                                class: "is-size-7 has-text-grey",
                                                style: "word-break: break-all;",
                                                "{p.uuid}"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        {
                            let photo_items = filtered_photo_items();
                            if !photo_items.is_empty() {
                                let mut show_fullscreen = use_signal(|| false);
                                let mut selected_photo_items = use_signal(Vec::<(String, String)>::new);
                                let mut selected_index = use_signal(|| 0usize);
                                let all_photo_items: Vec<(String, String)> = photo_items
                                    .iter()
                                    .map(|(uuid, path, _)| (uuid.clone(), path.clone()))
                                    .collect();

                                rsx! {
                                    h3 { class: "title is-5 mb-0",
                                        "📸 "
                                        {tid!("photos-title")}
                                    }

                                    div { class: "grid is-col-min-5 is-gap-0",
                                        for (idx , (photo_uuid , photo_path , _created_at)) in photo_items.iter().enumerate() {
                                            div {
                                                key: "{photo_uuid}",
                                                class: "cell is-clickable",
                                                onclick: {
                                                    let all_photo_items_click = all_photo_items.clone();
                                                    move |_| {
                                                        if !all_photo_items_click.is_empty() {
                                                            selected_photo_items.set(all_photo_items_click.clone());
                                                            selected_index.set(idx);
                                                            show_fullscreen.set(true);
                                                        }
                                                    }
                                                },
                                                SyncedThumbnailImage {
                                                    photo_uuid: Some(photo_uuid.clone()),
                                                    relative_path: photo_path.clone(),
                                                }
                                            }
                                        }
                                    }

                                    if show_fullscreen() && !selected_photo_items().is_empty() {
                                        SyncedCollectionFullscreen {
                                            photo_items: selected_photo_items(),
                                            initial_index: selected_index(),
                                            on_close: move |_| show_fullscreen.set(false),
                                        }
                                    }
                                }
                            } else {
                                rsx! {
                                    div {}
                                }
                            }
                        }

                        div { class: "box",
                            div { class: "level mb-3",
                                div { class: "level-left",
                                    h3 { class: "title is-5 mb-0",
                                        "📅 "
                                        {tid!("events-timeline-title")}
                                    }
                                }
                                div { class: "level-right",
                                    button {
                                        class: "button is-primary is-small",
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
                                        {tid!("action-add-event")}
                                    }
                                }
                            }

                            if events().is_empty() {
                                div { class: "notification is-light has-text-centered",
                                    {tid!("events-empty")}
                                }
                            } else {
                                div {
                                    class: "is-flex is-flex-direction-column",
                                    style: "gap: 12px;",
                                    for event in events() {
                                        div {
                                            key: "{event.uuid}",
                                            class: "box",
                                            style: "cursor: pointer; margin-bottom: 0;",
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
                                            div {
                                                class: "is-flex is-align-items-center mb-2",
                                                style: "gap: 10px;",
                                                span {
                                                    class: "tag is-light",
                                                    style: "font-size: 18px;",
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
                                                    p { class: "has-text-weight-semibold mb-0",
                                                        "{EventType::from_str(&event.event_type).display_name()}"
                                                    }
                                                    p { class: "is-size-7 has-text-grey mb-0",
                                                        {format_event_date(&event.event_date)}
                                                    }
                                                }
                                            }
                                            if let Some(notes) = &event.notes {
                                                p {
                                                    class: "is-size-7",
                                                    style: "white-space: pre-wrap;",
                                                    "{notes}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        button {
                            class: "button is-primary is-fullwidth is-medium",
                            onclick: {
                                let quail_id_for_edit = quail_id.clone();
                                move |_| on_navigate.call(Screen::ProfileEdit(quail_id_for_edit.clone()))
                            },
                            "✏️ "
                            {tid!("action-edit")}
                        }
                    }
                } else {
                    div { class: "notification is-light has-text-centered",
                        "⏳ "
                        {tid!("loading-profile")}
                    }
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

fn age_display_from_events(events: &[spacetime::QuailEvent], today: NaiveDate) -> Option<String> {
    let birth_date = events
        .iter()
        .filter(|event| EventType::from_str(&event.event_type) == EventType::Born)
        .filter_map(|event| NaiveDate::parse_from_str(&event.event_date, "%Y-%m-%d").ok())
        .min()?;

    Some(format_age_years_months(birth_date, today))
}
