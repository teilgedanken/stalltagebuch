use crate::{
    Screen,
    models::{
        Gender, Quail, RingColor, normalize_ring_color_code, ring_color_combination_conflicts,
        ring_color_preview_bg, ring_color_select_bg,
    },
    spacetime,
};
use dioxus::prelude::*;
use dioxus_gallery_components::{Gallery, GalleryConfig, GalleryItem};
use dioxus_i18n::tid;
use photo_gallery::{PhotoGalleryConfig, PhotoGalleryService, PhotoSize};
use spacetime::Photo;

#[component]
pub fn ProfileEditScreen(quail_id: String, on_navigate: EventHandler<Screen>) -> Element {
    // Spacetime subscriptions and data
    let quails = spacetime::use_table_quails();
    let photos_table = spacetime::use_table_photos();
    let photo_collections_table = spacetime::use_table_photo_collections();
    let quail_events_table = spacetime::use_table_quail_events();
    let connection = spacetime::use_connection();
    let update_quail_reducer = spacetime::use_reducer_update_quail();
    let create_event_reducer = spacetime::use_reducer_create_event();
    let update_event_reducer = spacetime::use_reducer_update_event();
    let delete_quail_reducer = spacetime::use_reducer_delete_quail();
    let delete_photo = spacetime::use_reducer_delete_photo();

    // Subscribe to quails and photos tables
    spacetime::use_subscription(&[
        "SELECT * FROM quails",
        "SELECT * FROM quail_events",
        "SELECT * FROM photo_collections",
        "SELECT * FROM photos",
    ]);

    // Local state signals
    let mut profile = use_signal(|| None::<Quail>);
    let mut name = use_signal(|| String::new());
    let mut gender = use_signal(|| "unknown".to_string());
    let mut birthday = use_signal(|| String::new());
    let mut ring_color_left = use_signal(|| String::new());
    let mut ring_color_right = use_signal(|| String::new());
    let mut photos = use_signal(|| Vec::<Photo>::new());
    let mut selected_profile_photo_id = use_signal(|| None::<String>);
    let mut existing_birthday_event_id = use_signal(|| None::<String>);
    let mut show_delete_confirm = use_signal(|| false);
    let mut error = use_signal(|| String::new());
    let mut success = use_signal(|| false);
    let mut saving = use_signal(|| false);

    // Load profile and photos from Spacetime table
    let quail_id_for_load = quail_id.clone();
    use_effect(move || {
        if let Ok(uuid) = uuid::Uuid::parse_str(&quail_id_for_load) {
            // Find quail in Spacetime table
            if let Some(quail) = quails().into_iter().find(|q| q.uuid == uuid.to_string()) {
                // Convert spacetime quail to local model
                let local_quail = Quail {
                    uuid: uuid::Uuid::parse_str(&quail.uuid).unwrap_or_else(|_| uuid::Uuid::nil()),
                    name: quail.name.clone(),
                    gender: match quail.gender.as_str() {
                        "male" => Gender::Male,
                        "female" => Gender::Female,
                        _ => Gender::Unknown,
                    },
                    ring_color_left: if let Some(rc) = &quail.ring_color_left {
                        Some(RingColor::from_str(rc))
                    } else {
                        None
                    },
                    ring_color_right: if let Some(rc) = &quail.ring_color_right {
                        Some(RingColor::from_str(rc))
                    } else {
                        None
                    },
                    profile_photo: quail
                        .profile_photo
                        .as_ref()
                        .and_then(|uuid_str| uuid::Uuid::parse_str(uuid_str).ok()),
                };

                name.set(local_quail.name.clone());
                gender.set(local_quail.gender.as_str().to_string());
                if let Some(rc) = &local_quail.ring_color_left {
                    ring_color_left.set(rc.as_str().to_string());
                }
                if let Some(rc) = &local_quail.ring_color_right {
                    ring_color_right.set(rc.as_str().to_string());
                }
                if let Some(profile_uuid) = local_quail.profile_photo {
                    selected_profile_photo_id.set(Some(profile_uuid.to_string()));
                }

                let quail_uuid_str = quail.uuid.clone();

                // Find existing birth event, if present
                if let Some(born_event) = quail_events_table()
                    .into_iter()
                    .find(|e| e.quail_uuid == quail_uuid_str && e.event_type == "born")
                {
                    birthday.set(born_event.event_date.clone());
                    existing_birthday_event_id.set(Some(born_event.uuid.clone()));
                } else {
                    birthday.set(String::new());
                    existing_birthday_event_id.set(None);
                }

                profile.set(Some(local_quail));

                // Load photos for this quail from SpacetimeDB
                let collection_ids: std::collections::HashSet<String> = photo_collections_table()
                    .iter()
                    .filter(|c| c.quail_uuid.as_ref() == Some(&quail_uuid_str))
                    .map(|c| c.uuid.clone())
                    .collect();

                let photo_list: Vec<Photo> = photos_table()
                    .iter()
                    .filter(|p| collection_ids.contains(&p.collection_uuid))
                    .cloned()
                    .collect();
                photos.set(photo_list);
            }
        }
    });

    let quail_id_for_submit = quail_id.clone();
    let mut handle_submit = move || {
        error.set(String::new());
        success.set(false);

        // Check if connected to Spacetime
        if connection().is_none() {
            error.set(tid!("error-not-connected"));
            return;
        }

        if name().trim().is_empty() {
            error.set(tid!("error-name-required")); // Name is required
            return;
        }

        let updated_ring_color_left = normalize_ring_color_selection(&ring_color_left());
        let updated_ring_color_right = normalize_ring_color_selection(&ring_color_right());

        if let Some(updated_profile) = profile() {
            let quail_uuid = updated_profile.uuid.to_string();

            if quails().iter().any(|quail| {
                quail.uuid != quail_uuid
                    && ring_color_combination_conflicts(
                        updated_ring_color_left.as_deref(),
                        updated_ring_color_right.as_deref(),
                        quail.ring_color_left.as_deref(),
                        quail.ring_color_right.as_deref(),
                    )
            }) {
                error.set(tid!("error-ring-color-combination-not-unique"));
                return;
            }

            saving.set(true);

            let device_id = crate::services::device_id_service::get_device_id()
                .unwrap_or_else(|_| "unknown-device".to_string());
            let updated_name = name().trim().to_string();
            let updated_gender = match gender().as_str() {
                "male" => "male".to_string(),
                "female" => "female".to_string(),
                _ => "unknown".to_string(),
            };
            let updated_birthday = normalize_optional_date_input(&birthday());
            let existing_event_id = existing_birthday_event_id();
            let selected_photo = selected_profile_photo_id();
            let update_reducer = update_quail_reducer.clone();
            let create_event_reducer = create_event_reducer.clone();
            let update_event_reducer = update_event_reducer.clone();
            let on_navigate_submit = on_navigate.clone();
            let quail_id_clone = quail_id_for_submit.clone();

            spawn(async move {
                // Create reducer call for update
                if let Err(err) = update_reducer(spacetime::UpdateQuailArgs {
                    uuid: quail_uuid.clone(),
                    name: updated_name,
                    gender: updated_gender,
                    ring_color_left: updated_ring_color_left,
                    ring_color_right: updated_ring_color_right,
                    profile_photo: selected_photo,
                }) {
                    error.set(err.to_string());
                    saving.set(false);
                    return;
                }

                if let Some(event_date_inner) = updated_birthday {
                    let event_date_owned = event_date_inner;
                    if let Some(event_uuid) = existing_event_id {
                        if let Err(err) = update_event_reducer(spacetime::UpdateEventArgs {
                            uuid: event_uuid.clone(),
                            event_type: "born".to_string(),
                            event_date: event_date_owned.clone(),
                            notes: None,
                            photos: None,
                        }) {
                            log::warn!("Failed to update birth event for {}: {}", quail_uuid, err);
                        }
                    } else if let Err(err) = create_event_reducer(spacetime::CreateEventArgs {
                        uuid: uuid::Uuid::new_v4().to_string(),
                        quail_uuid: quail_uuid.clone(),
                        event_type: "born".to_string(),
                        event_date: event_date_owned.clone(),
                        notes: None,
                        photos: None,
                        device_id: device_id.clone(),
                    }) {
                        log::warn!("Failed to create birth event for {}: {}", quail_uuid, err);
                    }
                }

                success.set(true);
                saving.set(false);
                // Navigate back after update
                on_navigate_submit.call(Screen::ProfileDetail(quail_id_clone));
            });
        }
    };

    let quail_id_for_delete = quail_id.clone();
    let mut handle_delete = move || {
        // Check if connected to Spacetime
        if connection().is_none() {
            error.set(tid!("error-not-connected"));
            return;
        }

        let quail_id_clone = quail_id_for_delete.clone();
        let delete_reducer_call = delete_quail_reducer.clone();
        let on_navigate_delete = on_navigate.clone();

        spawn(async move {
            if let Ok(uuid) = uuid::Uuid::parse_str(&quail_id_clone) {
                if let Err(err) = delete_reducer_call(uuid.to_string()) {
                    error.set(err.to_string());
                    return;
                }

                on_navigate_delete.call(Screen::ProfileList);
            }
        });
    };

    let quail_id_for_back = quail_id.clone();
    let quail_id_for_cancel = quail_id.clone();

    rsx! {
        section { class: "section pt-4 pb-3",
            div { class: "container is-max-tablet",
                div { class: "level mb-4",
                    div { class: "level-left",
                        button {
                            class: "button is-light",
                            onclick: move |_| on_navigate.call(Screen::ProfileDetail(quail_id_for_back.clone())),
                            "← "
                            {tid!("action-back")}
                        }
                    }
                    div { class: "level-item",
                        h1 { class: "title is-4 mb-0",
                            "✏️ "
                            {tid!("profile-edit-title")}
                        }
                    }
                    div { class: "level-right" }
                }

                if success() {
                    div { class: "notification is-success is-light",
                        "✓ "
                        {tid!("success-profile-updated")}
                    }
                }

                if !error().is_empty() {
                    div { class: "notification is-danger is-light",
                        "⚠️ "
                        {error()}
                    }
                }

                div { class: "box",
                    div { class: "field",
                        label { class: "label", {tid!("field-name-required")} }
                        div { class: "control",
                            input {
                                class: "input",
                                r#type: "text",
                                placeholder: tid!("field-name-placeholder"),
                                value: "{name}",
                                oninput: move |e| name.set(e.value()),
                                autofocus: true,
                            }
                        }
                    }

                    div { class: "field",
                        label { class: "label", {tid!("field-gender")} }
                        div { class: "control",
                            div { class: "select is-fullwidth",
                                select {
                                    value: "{gender}",
                                    onchange: move |e| gender.set(e.value()),
                                    option { value: "unknown", {tid!("gender-unknown")} }
                                    option { value: "female", {tid!("gender-female")} }
                                    option { value: "male", {tid!("gender-male")} }
                                }
                            }
                        }
                    }

                    div { class: "field",
                        label { class: "label", {tid!("field-date")} }
                        div { class: "control",
                            input {
                                r#type: "date",
                                class: "input",
                                value: "{birthday}",
                                oninput: move |e| birthday.set(e.value()),
                            }
                        }
                    }

                    div { class: "field",
                        label { class: "label", {tid!("field-ring-color")} }
                        div { class: "columns is-mobile",
                            div { class: "column",
                                p { class: "help mb-2", {tid!("ring-color-side-left")} }
                                div { class: "field has-addons",
                                    p { class: "control is-expanded",
                                        span { class: "select is-fullwidth",
                                            select {
                                                style: format!("background: {};", ring_color_select_bg(&ring_color_left())),
                                                value: "{ring_color_left}",
                                                onchange: move |e| ring_color_left.set(e.value()),
                                                option { value: "", {tid!("ring-color-none")} }
                                                option { value: "lila", {tid!("ring-color-purple")} }
                                                option { value: "rosa", {tid!("ring-color-pink")} }
                                                option { value: "hellblau",
                                                    {tid!("ring-color-light-blue")}
                                                }
                                                option { value: "dunkelblau",
                                                    {tid!("ring-color-dark-blue")}
                                                }
                                                option { value: "rot", {tid!("ring-color-red")} }
                                                option { value: "orange", {tid!("ring-color-orange")} }
                                                option { value: "weiss", {tid!("ring-color-white")} }
                                                option { value: "gelb", {tid!("ring-color-yellow")} }
                                                option { value: "schwarz", {tid!("ring-color-black")} }
                                                option { value: "gruen", {tid!("ring-color-green")} }
                                            }
                                        }
                                    }
                                    p { class: "control",
                                        span {
                                            class: "tag",
                                            style: format!(
                                                "width: 2rem; height: 2rem; border: 1px solid #bbb; background: {};",
                                                ring_color_preview_bg(&ring_color_left()),
                                            ),
                                        }
                                    }
                                }
                            }

                            div { class: "column",
                                p { class: "help mb-2", {tid!("ring-color-side-right")} }
                                div { class: "field has-addons",
                                    p { class: "control is-expanded",
                                        span { class: "select is-fullwidth",
                                            select {
                                                style: format!("background: {};", ring_color_select_bg(&ring_color_right())),
                                                value: "{ring_color_right}",
                                                onchange: move |e| ring_color_right.set(e.value()),
                                                option { value: "", {tid!("ring-color-none")} }
                                                option { value: "lila", {tid!("ring-color-purple")} }
                                                option { value: "rosa", {tid!("ring-color-pink")} }
                                                option { value: "hellblau",
                                                    {tid!("ring-color-light-blue")}
                                                }
                                                option { value: "dunkelblau",
                                                    {tid!("ring-color-dark-blue")}
                                                }
                                                option { value: "rot", {tid!("ring-color-red")} }
                                                option { value: "orange", {tid!("ring-color-orange")} }
                                                option { value: "weiss", {tid!("ring-color-white")} }
                                                option { value: "gelb", {tid!("ring-color-yellow")} }
                                                option { value: "schwarz", {tid!("ring-color-black")} }
                                                option { value: "gruen", {tid!("ring-color-green")} }
                                            }
                                        }
                                    }
                                    p { class: "control",
                                        span {
                                            class: "tag",
                                            style: format!(
                                                "width: 2rem; height: 2rem; border: 1px solid #bbb; background: {};",
                                                ring_color_preview_bg(&ring_color_right()),
                                            ),
                                        }
                                    }
                                }
                            }
                        }
                    }

                    article { class: "message is-info is-light",
                        div { class: "message-body",
                            "ℹ️ "
                            {tid!("info-photos-detail-view")}
                        }
                    }

                    div { class: "field mt-4",
                        label { class: "label",
                            {format!("{} ({})", tid!("field-photos"), photos().len())}
                        }

                        {
                            let photo_config = PhotoGalleryConfig {
                                storage_path: crate::services::photo_service::get_storage_path(),
                                enable_thumbnails: true,
                                thumbnail_small_size: 256,
                                thumbnail_medium_size: 512,
                            };
                            let photo_service = PhotoGalleryService::new(photo_config);
                            let gallery_items: Vec<GalleryItem> = photos()
                                .iter()
                                .filter_map(|photo| {
                                    let thumb_or_original = photo_service
                                        .get_photo_file_path(&photo.relative_path, PhotoSize::Small)
                                        .or_else(|| {
                                            photo_service
                                                .get_photo_file_path(&photo.relative_path, PhotoSize::Medium)
                                        })
                                        .or_else(|| {
                                            photo_service
                                                .get_photo_file_path(
                                                    &photo.relative_path,
                                                    PhotoSize::Original,
                                                )
                                        });
                                    let abs_path = thumb_or_original?;
                                    match crate::image_processing::image_path_to_data_url(&abs_path) {
                                        Ok(data_url) => {
                                            Some(GalleryItem {
                                                id: photo.uuid.clone(),
                                                data_url,
                                                caption: None,
                                            })
                                        }
                                        Err(_) => None,
                                    }
                                })
                                .collect();
                            let gallery_key = format!(
                                "{}:{}:{}",
                                quail_id,
                                selected_profile_photo_id().as_deref().unwrap_or("none"),
                                gallery_items
                                    .iter()
                                    .map(|item| item.id.as_str())
                                    .collect::<Vec<_>>()
                                    .join(","),
                            );
                            let gallery_config = GalleryConfig {
                                allow_delete: true,
                                allow_select: true,
                                selected_id: selected_profile_photo_id(),
                            };
                            rsx! {
                                Gallery {
                                    key: "{gallery_key}",
                                    items: gallery_items,
                                    config: gallery_config,
                                    on_delete: move |photo_id: String| {
                                        let delete_photo_fn = delete_photo.clone();
                                        if let Err(err) = delete_photo_fn(photo_id) {
                                            error.set(err.to_string());
                                        }
                                    },
                                    on_select: move |photo_id: String| {
                                        selected_profile_photo_id.set(Some(photo_id));
                                    },
                                }
                            }
                        }

                        if !photos().is_empty() {
                            p { class: "help mt-2", {tid!("info-tap-photo-to-mark")} }
                        }
                    }

                    div { class: "field has-addons mt-5",
                        p { class: "control is-expanded",
                            button {
                                class: "button is-success is-fullwidth",
                                disabled: saving(),
                                onclick: move |_| handle_submit(),
                                if saving() {
                                    "⏳ "
                                    {tid!("action-saving")}
                                } else {
                                    "✓ "
                                    {tid!("action-save")}
                                }
                            }
                        }
                        p { class: "control is-expanded",
                            button {
                                class: "button is-light is-fullwidth",
                                disabled: saving(),
                                onclick: move |_| on_navigate.call(Screen::ProfileDetail(quail_id_for_cancel.clone())),
                                "✕ "
                                {tid!("action-cancel")}
                            }
                        }
                    }

                    div {
                        class: "mt-5 pt-4",
                        style: "border-top: 1px solid var(--bulma-border);",
                        if show_delete_confirm() {
                            div {
                                div { class: "notification is-warning is-light",
                                    "⚠️ "
                                    {tid!("confirm-delete-quail")}
                                }
                                div { class: "field has-addons",
                                    p { class: "control is-expanded",
                                        button {
                                            class: "button is-danger is-small is-fullwidth",
                                            onclick: move |_| handle_delete(),
                                            "🗑️ "
                                            {tid!("action-delete-permanently")}
                                        }
                                    }
                                    p { class: "control is-expanded",
                                        button {
                                            class: "button is-light is-small is-fullwidth",
                                            onclick: move |_| show_delete_confirm.set(false),
                                            {tid!("action-cancel")}
                                        }
                                    }
                                }
                            }
                        } else {
                            button {
                                class: "button is-danger is-light is-fullwidth",
                                onclick: move |_| show_delete_confirm.set(true),
                                "🗑️ "
                                {tid!("action-delete-quail")}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn normalize_ring_color_selection(value: &str) -> Option<String> {
    normalize_ring_color_code(value)
}

fn normalize_optional_date_input(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
