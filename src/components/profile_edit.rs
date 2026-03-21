use crate::{
    Screen,
    models::{Gender, Quail, RingColor},
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
    let connection = spacetime::use_connection();
    let update_quail_reducer = spacetime::use_reducer_update_quail();
    let delete_quail_reducer = spacetime::use_reducer_delete_quail();
    let delete_photo = spacetime::use_reducer_delete_photo();

    // Subscribe to quails and photos tables
    spacetime::use_subscription(&[
        "SELECT * FROM quails",
        "SELECT * FROM photo_collections",
        "SELECT * FROM photos",
    ]);

    // Local state signals
    let mut profile = use_signal(|| None::<Quail>);
    let mut name = use_signal(|| String::new());
    let mut gender = use_signal(|| "unknown".to_string());
    let mut ring_color = use_signal(|| String::new());
    let mut birthday = use_signal(|| None::<String>);
    let mut photos = use_signal(|| Vec::<Photo>::new());
    let mut selected_profile_photo_id = use_signal(|| None::<String>);
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
                    ring_color: if let Some(rc) = &quail.ring_color {
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
                if let Some(rc) = &local_quail.ring_color {
                    ring_color.set(rc.as_str().to_string());
                }
                if let Some(profile_uuid) = local_quail.profile_photo {
                    selected_profile_photo_id.set(Some(profile_uuid.to_string()));
                }
                birthday.set(quail.birthday.clone());
                profile.set(Some(local_quail));

                // Load photos for this quail from SpacetimeDB
                let quail_uuid_str = quail.uuid.clone();
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
        // Check if connected to Spacetime
        if connection().is_none() {
            error.set(tid!("error-not-connected"));
            return;
        }

        if name().trim().is_empty() {
            error.set(tid!("error-name-required")); // Name is required
            return;
        }

        saving.set(true);

        if let Some(updated_profile) = profile() {
            let updated_name = name().trim().to_string();
            let updated_gender = match gender().as_str() {
                "male" => "male".to_string(),
                "female" => "female".to_string(),
                _ => "unknown".to_string(),
            };

            // Ring Color
            let ring_color_value = ring_color();
            let ring_color_trimmed = ring_color_value.trim();
            let updated_ring_color = if ring_color_trimmed.is_empty() {
                "".to_string()
            } else {
                ring_color_trimmed.to_string()
            };

            let quail_uuid = updated_profile.uuid.to_string();
            let selected_photo = selected_profile_photo_id();
            let update_reducer = update_quail_reducer.clone();
            let on_navigate_submit = on_navigate.clone();
            let quail_id_clone = quail_id_for_submit.clone();

            spawn(async move {
                // Create reducer call for update
                if let Err(err) = update_reducer(spacetime::UpdateQuailArgs {
                    uuid: quail_uuid.clone(),
                    name: updated_name,
                    gender: updated_gender,
                    ring_color: if updated_ring_color.is_empty() {
                        None
                    } else {
                        Some(updated_ring_color)
                    },
                    profile_photo: selected_photo,
                    birthday: birthday(),
                }) {
                    error.set(err.to_string());
                    saving.set(false);
                    return;
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
        div { style: "padding: 16px; max-width: 600px; margin: 0 auto; min-height: 100vh; background: #f5f5f5;",

            // Header
            div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 20px; padding-top: 8px;",
                button {
                    style: "padding: 8px 12px; background: #e0e0e0; color: #666; font-size: 20px; border-radius: 8px;",
                    onclick: move |_| on_navigate.call(Screen::ProfileDetail(quail_id_for_back.clone())),
                    "←"
                }
                h1 { style: "color: #0066cc; margin: 0; font-size: 24px; font-weight: 700; flex: 1;",
                    "✏️ "
                    {tid!("profile-edit-title")}
                }
            }

            // Success Message
            if success() {
                div { style: "padding: 12px 16px; background: #d4edda; border-radius: 8px; color: #155724; font-size: 14px; margin-bottom: 16px; border-left: 3px solid #28a745;",
                    "✓ "
                    {tid!("success-profile-updated")}
                }
            }

            // Error Message
            if !error().is_empty() {
                div { style: "padding: 12px 16px; background: #ffe6e6; border-radius: 8px; color: #cc0000; font-size: 14px; margin-bottom: 16px; border-left: 3px solid #cc0000;",
                    "⚠️ "
                    {error}
                }
            }

            // Form
            div { class: "card",

                // Name Field
                div { style: "margin-bottom: 20px;",
                    label {
                        style: "display: block; margin-bottom: 8px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("field-name-required")} // Name *
                    }
                    input {
                        style: "width: 100%; padding: 14px 16px; font-size: 16px; border: 2px solid #e0e0e0; border-radius: 8px; background: white;",
                        r#type: "text",
                        placeholder: "{tid!(\"field-name-placeholder\")}", // e.g. Hen 1
                        value: "{name}",
                        oninput: move |e| name.set(e.value()),
                        autofocus: true,
                    }
                }

                // Gender Field
                div { style: "margin-bottom: 20px;",
                    label {
                        style: "display: block; margin-bottom: 8px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("field-gender")} // Gender
                    }
                    select {
                        style: "width: 100%; padding: 14px 16px; font-size: 16px; border: 2px solid #e0e0e0; border-radius: 8px; background: white;",
                        value: "{gender}",
                        onchange: move |e| gender.set(e.value()),
                        option { value: "unknown", {tid!("gender-unknown")} } // Unknown
                        option { value: "female", {tid!("gender-female")} } // Female
                        option { value: "male", {tid!("gender-male")} } // Male
                    }
                }

                // Ring Color Field
                div { style: "margin-bottom: 20px;",
                    label {
                        style: "display: block; margin-bottom: 8px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("field-ring-color")} // Ring color
                    }
                    select {
                        style: "width: 100%; padding: 14px 16px; font-size: 16px; border: 2px solid #e0e0e0; border-radius: 8px; background: white;",
                        value: "{ring_color}",
                        onchange: move |e| ring_color.set(e.value()),
                        option { value: "", {tid!("ring-color-none")} } // None
                        option { value: "lila", {tid!("ring-color-purple")} } // Purple
                        option { value: "rosa", {tid!("ring-color-pink")} } // Pink
                        option { value: "hellblau", {tid!("ring-color-light-blue")} } // Light blue
                        option { value: "dunkelblau", {tid!("ring-color-dark-blue")} } // Dark blue
                        option { value: "rot", {tid!("ring-color-red")} } // Red
                        option { value: "orange", {tid!("ring-color-orange")} } // Orange
                        option { value: "weiss", {tid!("ring-color-white")} } // White
                        option { value: "gelb", {tid!("ring-color-yellow")} } // Yellow
                        option { value: "schwarz", {tid!("ring-color-black")} } // Black
                        option { value: "gruen", {tid!("ring-color-green")} } // Green
                    }
                }

                div { style: "padding: 12px; background: #e3f2fd; border-radius: 8px; color: #0066cc; font-size: 13px; margin-bottom: 20px;",
                    "ℹ️ "
                    {tid!("info-photos-detail-view")}
                }

                // Photo Gallery with Profile Selection
                div { style: "margin-bottom: 24px;",
                    label {
                        style: "display: block; margin-bottom: 8px; font-weight: 600; color: #333; font-size: 14px;",
                        {format!("{} ({})", tid!("field-photos"), photos().len())} // Photos count
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
                                        photo_service.get_photo_file_path(
                                            &photo.relative_path,
                                            PhotoSize::Medium,
                                        )
                                    })
                                    .or_else(|| {
                                        photo_service.get_photo_file_path(
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
                                    // Delete photo via SpacetimeDB reducer
                                    if let Err(err) = delete_photo_fn(photo_id) {
                                        error.set(err.to_string());
                                    }
                                    // Photo list will auto-update via subscriptions
                                },
                                on_select: move |photo_id: String| {
                                    selected_profile_photo_id.set(Some(photo_id));
                                },
                            }
                        }
                    }

                    if !photos().is_empty() {
                        div {
                            style: "margin-top: 12px; padding: 10px; background: #f9f9f9; border-radius: 6px; font-size: 12px; color: #666;",
                            {tid!("info-tap-photo-to-mark")} // Tap a photo to mark it as profile photo.
                        }
                    }
                }

                // Buttons
                div { style: "display: flex; gap: 12px;",
                    button {
                        class: "btn-success",
                        style: "flex: 1; padding: 14px; font-size: 16px; font-weight: 600;",
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
                    button {
                        style: "flex: 1; padding: 14px; background: #e0e0e0; color: #666; font-size: 16px; font-weight: 600;",
                        disabled: saving(),
                        onclick: move |_| on_navigate.call(Screen::ProfileDetail(quail_id_for_cancel.clone())),
                        "✕ "
                        {tid!("action-cancel")}
                    }
                }

                // Delete Section
                div { style: "margin-top: 32px; padding-top: 24px; border-top: 2px solid #f0f0f0;",
                    if show_delete_confirm() {
                        div {
                            div { style: "margin-bottom: 16px; padding: 12px; background: #fff3cd; border-radius: 8px; color: #856404;",
                                "⚠️ "
                                {tid!("confirm-delete-quail")}
                            }
                            div { style: "display: flex; gap: 12px;",
                                button {
                                    class: "btn-danger",
                                    style: "flex: 1; padding: 14px; font-size: 16px; font-weight: 600;",
                                    onclick: move |_| handle_delete(),
                                    "🗑️ "
                                    {tid!("action-delete-permanently")}
                                }
                                button {
                                    style: "flex: 1; padding: 14px; background: #e0e0e0; color: #666; font-size: 16px; font-weight: 600;",
                                    onclick: move |_| show_delete_confirm.set(false),
                                    {tid!("action-cancel")} // Cancel
                                }
                            }
                        }
                    } else {
                        button {
                            style: "width: 100%; padding: 12px; background: #ffe6e6; color: #cc0000; font-size: 14px; font-weight: 600; border: 1px solid #ffcccc; border-radius: 8px;",
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
