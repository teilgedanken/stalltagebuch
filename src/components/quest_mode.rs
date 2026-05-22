use crate::components::ring_color_picker::ring_color_option_label;
use crate::components::synced_photo::{SyncedPreviewImage, SyncedThumbnailImage};
use crate::models::{EventType, RingColor, ring_color_preview_bg, ring_color_select_bg};
use crate::{Screen, spacetime};
use dioxus::prelude::*;
use dioxus_i18n::tid;
use spacetimedb_sdk::DbContext;

fn quest_ring_label(color: Option<&RingColor>) -> String {
    color
        .map(ring_color_option_label)
        .unwrap_or_else(|| tid!("ring-color-none"))
}

fn quest_ring_button_style(color: Option<&RingColor>, is_left: bool) -> String {
    let background = color
        .map(|v| ring_color_select_bg(v.as_str()))
        .unwrap_or("#ffffff");
    let border_radius = if is_left {
        "0.75rem 0 0 0.75rem"
    } else {
        "0 0.75rem 0.75rem 0"
    };
    let border_right = if is_left { "border-right: none;" } else { "" };
    format!(
        "flex: 1 1 0; min-height: 2.55rem; display: flex; align-items: center; justify-content: center; padding: 0.45rem 0.65rem; background: {}; border: 1px solid #e5e5e5; {} border-radius: {}; box-shadow: none; font-size: 0.8rem;",
        background, border_right, border_radius
    )
}

fn quest_ring_swatch_style(color: Option<&RingColor>) -> String {
    let background = color
        .map(|v| ring_color_preview_bg(v.as_str()))
        .unwrap_or(
            "linear-gradient(135deg, #ffffff 0%, #ffffff 45%, #ececec 45%, #ececec 55%, #ffffff 55%, #ffffff 100%)",
        );
    let border = if color.is_some() {
        "1px solid rgba(0, 0, 0, 0.35)"
    } else {
        "1px dashed #bbb"
    };
    format!(
        "display: inline-block; width: 0.9rem; height: 0.9rem; border-radius: 999px; border: {}; background: {}; flex-shrink: 0;",
        border, background
    )
}

#[component]
fn QuestGalleryButton(
    quail_uuid: String,
    uploading: Signal<bool>,
    upload_error: Signal<String>,
    on_success: EventHandler<()>,
) -> Element {
    let create_photo_collection = spacetime::use_reducer_create_photo_collection();
    let create_photo = spacetime::use_reducer_create_photo();

    rsx! {
        button {
            class: "button is-info is-fullwidth",
            disabled: uploading(),
            onclick: move |_| {
                uploading.set(true);
                upload_error.set(String::new());
                let quail_id = quail_uuid.clone();
                let create_photo_collection = create_photo_collection.clone();
                let create_photo = create_photo.clone();
                spawn(async move {
                    #[cfg(not(target_os = "android"))]
                    {
                        upload_error.set(tid!("error-multiselect-android-only"));
                    }
                    #[cfg(target_os = "android")]
                    {
                        let device_id =
                            crate::services::device_id_service::get_device_id()
                            .unwrap_or_else(|_| "unknown-device".to_string());
                        match crate::camera::pick_images() {
                            Ok(paths) => {
                                if let Ok(quail_uuid) = uuid::Uuid::parse_str(&quail_id) {
                                    let collection_uuid = quail_uuid;
                                    if let Err(err) = create_photo_collection(spacetime::CreatePhotoCollectionArgs {
                                        uuid: collection_uuid.to_string(),
                                        quail_uuid: Some(quail_uuid.to_string()),
                                        event_uuid: None,
                                        name: format!(
                                            "Quail-{}",
                                            quail_uuid.to_string().chars().take(8).collect::<String>(),
                                        ),
                                        device_id: device_id.clone(),
                                    }) {
                                        upload_error
                                            .set(
                                                format!("{}: {}", tid!("error-selection-failed"), err),
                                            );
                                        uploading.set(false);
                                        return;
                                    }
                                    let mut any_ok = false;
                                    for picked_path in paths {
                                        let source = picked_path.to_string_lossy().to_string();
                                        match crate::services::photo_service::process_photo(source)
                                            .await
                                        {
                                            Ok((relative_original, _, _)) => {
                                                if let Some(photo_uuid) = std::path::Path::new(
                                                        &relative_original,
                                                    )
                                                    .file_stem()
                                                    .and_then(|s| s.to_str())
                                                {
                                                    if let Err(err) = create_photo(spacetime::CreatePhotoArgs {
                                                        uuid: photo_uuid.to_string(),
                                                        collection_uuid: collection_uuid.to_string(),
                                                        relative_path: relative_original,
                                                        device_id: device_id.clone(),
                                                    }) {
                                                        upload_error
                                                            .set(
                                                                format!("{}: {}", tid!("error-selection-failed"), err),
                                                            );
                                                    } else {
                                                        any_ok = true;
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                upload_error
                                                    .set(
                                                        format!("{}: {}", tid!("error-selection-failed"), err),
                                                    );
                                            }
                                        }
                                    }
                                    if any_ok {
                                        on_success.call(());
                                    }
                                }
                            }
                            Err(e) => {
                                upload_error
                                    .set(format!("{}: {}", tid!("error-selection-failed"), e));
                            }
                        }
                    }
                    uploading.set(false);
                });
            },
            if uploading() {
                "⏳"
            } else {
                "🖼️ "
                {tid!("action-gallery")}
            }
        }
    }
}

#[component]
fn QuestCameraButton(
    quail_uuid: String,
    uploading: Signal<bool>,
    upload_error: Signal<String>,
    on_success: EventHandler<()>,
) -> Element {
    let create_photo_collection = spacetime::use_reducer_create_photo_collection();
    let create_photo = spacetime::use_reducer_create_photo();

    rsx! {
        button {
            class: "button is-primary is-fullwidth",
            disabled: uploading(),
            onclick: move |_| {
                uploading.set(true);
                upload_error.set(String::new());
                let quail_id = quail_uuid.clone();
                let create_photo_collection = create_photo_collection.clone();
                let create_photo = create_photo.clone();
                spawn(async move {
                    #[cfg(not(target_os = "android"))]
                    {
                        upload_error.set(tid!("error-camera-android-only"));
                    }
                    #[cfg(target_os = "android")]
                    {
                        let device_id =
                            crate::services::device_id_service::get_device_id()
                            .unwrap_or_else(|_| "unknown-device".to_string());
                        match crate::camera::capture_photo() {
                            Ok(path) => {
                                let source = path.to_string_lossy().to_string();
                                if let Ok(quail_uuid) = uuid::Uuid::parse_str(&quail_id) {
                                    let collection_uuid = quail_uuid;
                                    if let Err(err) = create_photo_collection(spacetime::CreatePhotoCollectionArgs {
                                        uuid: collection_uuid.to_string(),
                                        quail_uuid: Some(quail_uuid.to_string()),
                                        event_uuid: None,
                                        name: format!(
                                            "Quail-{}",
                                            quail_uuid.to_string().chars().take(8).collect::<String>(),
                                        ),
                                        device_id: device_id.clone(),
                                    }) {
                                        upload_error
                                            .set(format!("{}: {}", tid!("error-capture-failed"), err));
                                        uploading.set(false);
                                        return;
                                    }
                                    match crate::services::photo_service::process_photo(source)
                                        .await
                                    {
                                        Ok((relative_original, _, _)) => {
                                            if let Some(photo_uuid) = std::path::Path::new(
                                                    &relative_original,
                                                )
                                                .file_stem()
                                                .and_then(|s| s.to_str())
                                            {
                                                if let Err(err) = create_photo(spacetime::CreatePhotoArgs {
                                                    uuid: photo_uuid.to_string(),
                                                    collection_uuid: collection_uuid.to_string(),
                                                    relative_path: relative_original,
                                                    device_id,
                                                }) {
                                                    upload_error
                                                        .set(format!("{}: {}", tid!("error-capture-failed"), err));
                                                } else {
                                                    on_success.call(());
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            upload_error
                                                .set(format!("{}: {}", tid!("error-capture-failed"), err));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                upload_error
                                    .set(format!("{}: {}", tid!("error-capture-failed"), e));
                            }
                        }
                    }
                    uploading.set(false);
                });
            },
            if uploading() {
                "⏳"
            } else {
                "📷 "
                {tid!("action-photo")}
            }
        }
    }
}

#[component]
pub fn QuestModeScreen(on_navigate: EventHandler<Screen>) -> Element {
    let quails = spacetime::use_table_quails();
    let quail_events = spacetime::use_table_quail_events();
    let photo_collections_table = spacetime::use_table_photo_collections();
    let photos_table = spacetime::use_table_photos();
    let connection = spacetime::use_connection();

    spacetime::use_subscription(&[
        "SELECT * FROM quails",
        "SELECT * FROM quail_events",
        "SELECT * FROM photo_collections",
        "SELECT * FROM photos",
    ]);

    let mut current_index = use_signal(|| 0usize);
    let uploading = use_signal(|| false);
    let mut upload_error = use_signal(String::new);

    // Ranked quails: most photo-needy first.
    // Score = seconds since the newest photo was taken. No photo at all = i64::MAX.
    let ranked_quails = use_memo(move || {
        let now_secs = chrono::Local::now().timestamp();
        let owner_opt = connection()
            .as_ref()
            .and_then(|c| c.try_identity())
            .map(|id| id.to_string());

        let cols = photo_collections_table();
        let phts = photos_table();
        let events = quail_events();

        let mut scored: Vec<(spacetime::Quail, i64)> = quails()
            .into_iter()
            .filter(|q| {
                // exclude dead quails
                let is_dead = events
                    .iter()
                    .filter(|e| e.quail_uuid == q.uuid)
                    .max_by(|a, b| {
                        a.event_date
                            .cmp(&b.event_date)
                            .then_with(|| a.uuid.cmp(&b.uuid))
                    })
                    .map(|e| EventType::from_str(&e.event_type).is_final())
                    .unwrap_or(false);
                !is_dead && owner_opt.as_ref().map(|o| q.owner == *o).unwrap_or(true)
            })
            .map(|q| {
                let col_ids: Vec<&str> = cols
                    .iter()
                    .filter(|c| c.quail_uuid.as_deref() == Some(q.uuid.as_str()))
                    .map(|c| c.uuid.as_str())
                    .collect();

                // Most recent photo timestamp, normalised to seconds
                let latest_secs = phts
                    .iter()
                    .filter(|p| col_ids.contains(&p.collection_uuid.as_str()))
                    .map(|p| {
                        if p.created_at > 1_000_000_000_000 {
                            p.created_at / 1000
                        } else {
                            p.created_at
                        }
                    })
                    .max();

                let score = match latest_secs {
                    None => i64::MAX,
                    Some(ts) => (now_secs - ts).max(0),
                };

                (q, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored
    });

    // Up to 3 most recent photos for the currently displayed quail
    let current_quail_photos = use_memo(move || {
        let ranked = ranked_quails();
        let idx = current_index();
        let Some((quail, _)) = ranked.get(idx) else {
            return vec![];
        };

        let cols = photo_collections_table();
        let phts = photos_table();

        let col_ids: Vec<&str> = cols
            .iter()
            .filter(|c| c.quail_uuid.as_deref() == Some(quail.uuid.as_str()))
            .map(|c| c.uuid.as_str())
            .collect();

        let mut entries: Vec<(String, String, i64)> = phts
            .iter()
            .filter(|p| col_ids.contains(&p.collection_uuid.as_str()))
            .map(|p| (p.uuid.clone(), p.relative_path.clone(), p.created_at))
            .collect();

        entries.sort_by(|a, b| b.2.cmp(&a.2));
        entries
            .into_iter()
            .take(3)
            .map(|(uuid, path, _)| (uuid, path))
            .collect::<Vec<_>>()
    });

    let ranked = ranked_quails();
    let total = ranked.len();
    let idx = current_index();
    let current_entry = ranked.into_iter().nth(idx);

    rsx! {
        section { class: "section pt-4 pb-3",
            div { class: "container is-max-desktop",

                // ── Header ──────────────────────────────────────────────────
                div { class: "level mb-4",
                    div { class: "level-left",
                        button {
                            class: "button is-light",
                            onclick: move |_| on_navigate.call(Screen::Home),
                            "← "
                            {tid!("action-back")}
                        }
                    }
                    div { class: "level-item",
                        h1 { class: "title is-4 mb-0",
                            "📸 "
                            {tid!("quest-title")}
                        }
                    }
                    div { class: "level-right",
                        if total > 0 && idx < total {
                            span { class: "tag is-light is-medium", "{idx + 1} / {total}" }
                        }
                    }
                }

                // ── Quest card ───────────────────────────────────────────────
                if let Some((quail, score)) = current_entry {
                    div { class: "box",

                        // Profile image (square)
                        div { style: "width: 100%; aspect-ratio: 1/1; background: #f0f0f0; border-radius: 12px; overflow: hidden; display: flex; align-items: center; justify-content: center; margin-bottom: 1rem;",
                            {
                                let profile_entry = quail
                                    .profile_photo
                                    .as_ref()
                                    .and_then(|uuid| {
                                        photos_table()
                                            .into_iter()
                                            .find(|p| &p.uuid == uuid)
                                            .map(|p| (uuid.clone(), p.relative_path))
                                    });
                                if let Some((uuid, path)) = profile_entry {
                                    rsx! {
                                        SyncedPreviewImage { photo_uuid: Some(uuid), relative_path: path, alt: quail.name.clone() }
                                    }
                                } else {
                                    rsx! {
                                        div { style: "font-size: 80px; color: #ccc;", "🐦" }
                                    }
                                }
                            }
                        }

                        // Name + staleness tag
                        div {
                            class: "is-flex is-align-items-center mb-3",
                            style: "gap: 0.75rem; flex-wrap: wrap;",
                            h2 {
                                class: "title is-4 mb-0",
                                style: "flex: 1 1 auto;",
                                "{quail.name}"
                            }
                            if score == i64::MAX {
                                span { class: "tag is-danger", {tid!("quest-no-photo")} }
                            } else {
                                {
                                    let days = score / 86400;
                                    let cls = if days > 30 {
                                        "tag is-warning"
                                    } else if days > 7 {
                                        "tag is-info is-light"
                                    } else {
                                        "tag is-success is-light"
                                    };
                                    rsx! {
                                        span { class: "{cls}", {tid!("quest-last-photo-days", days : days)} }
                                    }
                                }
                            }
                        }

                        // Ring color buttons (same style as profile detail)
                        {
                            let left_ring = quail
                                .ring_color_left
                                .as_ref()
                                .map(|v| RingColor::from_str(v));
                            let right_ring = quail
                                .ring_color_right
                                .as_ref()
                                .map(|v| RingColor::from_str(v));
                            rsx! {
                                div { class: "mb-3", style: "display: flex; width: 100%;",
                                    button {
                                        class: "button",
                                        style: quest_ring_button_style(left_ring.as_ref(), true),
                                        title: format!(
                                            "{}: {}",
                                            tid!("ring-color-side-left"),
                                            quest_ring_label(left_ring.as_ref()),
                                        ),
                                        span { style: "display: flex; align-items: center; justify-content: center; gap: 0.4rem; width: 100%; min-width: 0;",
                                            span { style: quest_ring_swatch_style(left_ring.as_ref()) }
                                            span {
                                                class: "is-size-7 has-text-grey-dark",
                                                style: "overflow: hidden; text-overflow: ellipsis;",
                                                {quest_ring_label(left_ring.as_ref())}
                                            }
                                        }
                                    }
                                    button {
                                        class: "button",
                                        style: quest_ring_button_style(right_ring.as_ref(), false),
                                        title: format!(
                                            "{}: {}",
                                            tid!("ring-color-side-right"),
                                            quest_ring_label(right_ring.as_ref()),
                                        ),
                                        span { style: "display: flex; align-items: center; justify-content: center; gap: 0.4rem; width: 100%; min-width: 0;",
                                            span { style: quest_ring_swatch_style(right_ring.as_ref()) }
                                            span {
                                                class: "is-size-7 has-text-grey-dark",
                                                style: "overflow: hidden; text-overflow: ellipsis;",
                                                {quest_ring_label(right_ring.as_ref())}
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Recent photo thumbnails (up to 3, newest first)
                        {
                            let thumbs = current_quail_photos();
                            if !thumbs.is_empty() {
                                rsx! {
                                    p { class: "is-size-7 has-text-grey mb-2", {tid!("quest-recent-photos")} }
                                    div { class: "columns is-mobile is-variable is-2 mb-3",
                                        for (uuid, path) in thumbs.iter() {
                                            div { class: "column is-one-third",
                                                div { style: "aspect-ratio: 1/1; overflow: hidden; border-radius: 8px; position: relative;",
                                                    SyncedThumbnailImage {
                                                        key: "{uuid}",
                                                        photo_uuid: Some(uuid.clone()),
                                                        relative_path: path.clone(),
                                                        fill: true,
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                rsx! {}
                            }
                        }

                        // Upload error
                        if !upload_error().is_empty() {
                            div { class: "notification is-danger is-light py-2 mb-3",
                                "⚠️ "
                                {upload_error()}
                            }
                        }

                        // ── Camera + Gallery buttons (side by side) ─────────
                        div { class: "columns is-mobile is-variable is-2 mb-2",
                            div { class: "column",
                                QuestGalleryButton {
                                    quail_uuid: quail.uuid.clone(),
                                    uploading,
                                    upload_error,
                                    on_success: move |_| current_index.with_mut(|i| *i += 1),
                                }
                            }
                            div { class: "column",
                                QuestCameraButton {
                                    quail_uuid: quail.uuid.clone(),
                                    uploading,
                                    upload_error,
                                    on_success: move |_| current_index.with_mut(|i| *i += 1),
                                }
                            }
                        }

                        // ── Skip button ──────────────────────────────────────
                        button {
                            class: "button is-light is-fullwidth mt-2",
                            disabled: uploading(),
                            onclick: move |_| {
                                upload_error.set(String::new());
                                current_index.with_mut(|i| *i += 1);
                            },
                            "⏭ "
                            {tid!("quest-skip")}
                        }
                    }
                } else {
                    // ── Empty / all-done state ────────────────────────────────
                    div { class: "notification is-success is-light has-text-centered py-6",
                        p { class: "title is-3 mb-3", "🎉" }
                        p { class: "is-size-5", {tid!("quest-no-candidates")} }
                        div { class: "mt-4",
                            button {
                                class: "button is-light",
                                onclick: move |_| on_navigate.call(Screen::Home),
                                "← "
                                {tid!("action-back")}
                            }
                        }
                    }
                }
            }
        }
    }
}
