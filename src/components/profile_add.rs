use crate::{
    Screen,
    models::{Gender, RingColor},
    spacetime,
};
use dioxus::prelude::*;
use dioxus_i18n::tid;
use std::path::PathBuf;

#[component]
pub fn AddProfileScreen(on_navigate: EventHandler<Screen>) -> Element {
    let mut name = use_signal(|| String::new());
    let mut gender = use_signal(|| "unknown".to_string());
    let mut ring_color = use_signal(|| String::new());
    let mut photo_path = use_signal(|| None::<PathBuf>);
    let mut uploading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut success = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let create_quail = spacetime::use_reducer_create_quail();
    let create_photo_collection = spacetime::use_reducer_create_photo_collection();
    let create_photo = spacetime::use_reducer_create_photo();
    let set_quail_photo = spacetime::use_reducer_set_quail_photo();
    let connection = spacetime::use_connection();

    let mut handle_submit = move || {
        error.set(None);
        success.set(false);

        let name_trimmed = name().trim().to_string();
        if name_trimmed.is_empty() {
            error.set(Some(tid!("error-name-required"))); // Name cannot be empty
            return;
        }

        if connection().is_none() {
            error.set(Some(tid!("error", error: "SpacetimeDB not connected")));
            return;
        }

        saving.set(true);
        let quail_uuid = uuid::Uuid::new_v4().to_string();
        let gender_value = match gender().as_str() {
            "male" => Gender::Male,
            "female" => Gender::Female,
            _ => Gender::Unknown,
        };

        let ring_color_value = ring_color();
        let ring_color_trimmed = ring_color_value.trim();
        let ring_color_value = if ring_color_trimmed.is_empty() {
            None
        } else {
            Some(RingColor::from_str(ring_color_trimmed).as_str().to_string())
        };

        let device_id = crate::services::device_id_service::get_device_id()
            .unwrap_or_else(|_| "unknown-device".to_string());

        // Capture photo_path BEFORE moving into async
        let selected_photo_path = photo_path();
        let photo_name_for_display = name_trimmed.clone();

        if let Err(err) = create_quail(spacetime::CreateQuailArgs {
            uuid: quail_uuid.clone(),
            name: name_trimmed.to_string(),
            gender: gender_value.as_str().to_string(),
            ring_color: ring_color_value,
            profile_photo: None,
            birthday: None,
            device_id: device_id.clone(),
        }) {
            error.set(Some(err.to_string()));
            saving.set(false);
            return;
        }

        let on_navigate_submit = on_navigate.clone();
        let create_photo_collection_reducer = create_photo_collection.clone();
        let create_photo_reducer = create_photo.clone();
        let set_quail_photo_reducer = set_quail_photo.clone();

        spawn(async move {
            if let Some(path) = selected_photo_path {
                let collection_uuid = quail_uuid.clone();

                // Create photo collection in Spacetime
                if let Err(err) =
                    create_photo_collection_reducer(spacetime::CreatePhotoCollectionArgs {
                        uuid: collection_uuid.clone(),
                        quail_uuid: Some(quail_uuid.clone()),
                        event_uuid: None,
                        name: format!("Fotos für {}", photo_name_for_display),
                        device_id: device_id.clone(),
                    })
                {
                    log::warn!(
                        "Failed to create photo collection for new quail {}: {}",
                        quail_uuid,
                        err
                    );
                } else {
                    // Process the selected photo
                    let source = path.to_string_lossy().to_string();
                    match crate::services::photo_service::process_photo(source).await {
                        Ok((relative_original, _, _)) => {
                            if let Some(photo_uuid) = std::path::Path::new(&relative_original)
                                .file_stem()
                                .and_then(|s| s.to_str())
                            {
                                // Create photo record in Spacetime
                                if let Err(err) = create_photo_reducer(spacetime::CreatePhotoArgs {
                                    uuid: photo_uuid.to_string(),
                                    collection_uuid: collection_uuid.clone(),
                                    relative_path: relative_original.clone(),
                                    device_id: device_id.clone(),
                                }) {
                                    log::warn!(
                                        "Failed to create photo {} for new quail {}: {}",
                                        photo_uuid,
                                        quail_uuid,
                                        err
                                    );
                                } else if let Err(err) = set_quail_photo_reducer(
                                    quail_uuid.to_string(),
                                    Some(photo_uuid.to_string()),
                                ) {
                                    // The profile already exists; keep the flow successful and log the partial failure.
                                    log::warn!(
                                        "Failed to set profile photo {} for new quail {}: {}",
                                        photo_uuid,
                                        quail_uuid,
                                        err
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            log::warn!("Failed to process photo in AddProfileScreen: {}", err);
                        }
                    }
                }
            }

            success.set(true);
            saving.set(false);
            on_navigate_submit.call(Screen::ProfileList);
        });
    };

    rsx! {
        div { style: "padding: 16px; max-width: 600px; margin: 0 auto; min-height: 100vh; background: #f5f5f5;",

            div { style: "display: flex; align-items: center; margin-bottom: 24px;",
                button {
                    class: "btn-secondary",
                    style: "margin-right: 12px; padding: 8px 16px;",
                    onclick: move |_| on_navigate.call(Screen::ProfileList),
                    "← "
                    {tid!("action-back")}
                }
                h1 {
                    style: "color: #0066cc; font-size: 24px; font-weight: 700; margin: 0;",
                    {tid!("profile-add-title")} // New profile page title
                }
            }

            if let Some(err) = error() {
                div { style: "background: #fee; border: 1px solid #fcc; color: #c33; padding: 12px; margin-bottom: 16px; border-radius: 8px; font-size: 14px;",
                    "⚠️ {err}"
                }
            }

            if success() {
                div { style: "background: #efe; border: 1px solid #cfc; color: #3a3; padding: 12px; margin-bottom: 16px; border-radius: 8px; font-size: 14px;",
                    "✅ "
                    {tid!("profile-created-success")}
                }
            }

            div { class: "card",

                div { style: "margin-bottom: 20px;",
                    label {
                        style: "display: block; margin-bottom: 6px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("profile-name-label")} // Name field label with required marker
                    }
                    input {
                        r#type: "text",
                        class: "input",
                        placeholder: tid!("profile-name-placeholder"), // Example name placeholder
                        value: "{name}",
                        oninput: move |e| name.set(e.value()),
                        autofocus: true,
                    }
                }

                div { style: "margin-bottom: 20px;",
                    label {
                        style: "display: block; margin-bottom: 6px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("profile-gender-label")} // Gender field label
                    }
                    select {
                        class: "input",
                        value: "{gender}",
                        onchange: move |e| gender.set(e.value()),
                        option { value: "unknown", {tid!("gender-unknown")} } // Unknown gender option
                        option { value: "female", {tid!("gender-female")} } // Female gender option
                        option { value: "male", {tid!("gender-male")} } // Male gender option
                    }
                }

                div { style: "margin-bottom: 20px;",
                    label {
                        style: "display: block; margin-bottom: 6px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("profile-ring-color-label")} // Ring color field label
                    }
                    select {
                        class: "input",
                        value: "{ring_color}",
                        onchange: move |e| ring_color.set(e.value()),
                        option { value: "", {tid!("ring-color-none")} } // No ring color option
                        option { value: "lila", {tid!("ring-color-purple")} } // Purple ring color
                        option { value: "rosa", {tid!("ring-color-pink")} } // Pink ring color
                        option { value: "hellblau", {tid!("ring-color-light-blue")} } // Light blue ring color
                        option { value: "dunkelblau", {tid!("ring-color-dark-blue")} } // Dark blue ring color
                        option { value: "rot", {tid!("ring-color-red")} } // Red ring color
                        option { value: "orange", {tid!("ring-color-orange")} } // Orange ring color
                        option { value: "weiss", {tid!("ring-color-white")} } // White ring color
                        option { value: "gelb", {tid!("ring-color-yellow")} } // Yellow ring color
                        option { value: "schwarz", {tid!("ring-color-black")} } // Black ring color
                        option { value: "gruen", {tid!("ring-color-green")} } // Green ring color
                    }
                }

                div { style: "padding: 12px; background: #e3f2fd; border-radius: 8px; color: #0066cc; font-size: 13px; margin-bottom: 20px;",
                    "ℹ️ "
                    {tid!("profile-add-info")}
                }

                div { style: "margin-bottom: 20px;",
                    label {
                        style: "display: block; margin-bottom: 6px; font-weight: 600; color: #333; font-size: 14px;",
                        {tid!("profile-photo-label")} // Photo field label
                    }

                    div { style: "margin-bottom: 12px;",
                        if let Some(path) = photo_path() {
                            div { style: "display: flex; align-items: center; gap: 12px; padding: 12px; background: #f0f0f0; border-radius: 8px;",
                                div { style: "width: 60px; height: 60px; background: #ddd; border-radius: 8px; display: flex; align-items: center; justify-content: center; font-size: 32px;",
                                    "📷"
                                }
                                div { style: "flex: 1;",
                                    div {
                                        style: "font-size: 14px; font-weight: 600; color: #333;",
                                        {tid!("photo-selected")} // Photo selected status message
                                    }
                                    div { style: "font-size: 12px; color: #666; word-break: break-all;",
                                        "{path.file_name().and_then(|n| n.to_str()).unwrap_or(\"Unbekannt\")}"
                                    }
                                }
                                button {
                                    class: "btn-secondary",
                                    style: "padding: 6px 12px; font-size: 12px;",
                                    onclick: move |_| photo_path.set(None),
                                    "🗑️"
                                }
                            }
                        } else {
                            div {
                                style: "width: 100%; height: 120px; border: 2px dashed #ccc; border-radius: 8px; display: flex; align-items: center; justify-content: center; color: #999; font-size: 14px;",
                                {tid!("photo-none-selected")} // No photo selected message
                            }
                        }
                    }

                    div { style: "display: flex; gap: 8px;",
                        button {
                            class: "btn-secondary",
                            style: "flex: 1; padding: 10px; font-size: 14px;",
                            disabled: uploading(),
                            onclick: move |_| {
                                uploading.set(true);
                                error.set(None);
                                spawn(async move {
                                    #[cfg(target_os = "android")]
                                    {
                                        match crate::camera::pick_image() {
                                            Ok(path) => photo_path.set(Some(path)),
                                            Err(e) => error.set(Some(format!("{}: {}", tid!("error"), e))),
                                        }
                                    }
                                    #[cfg(not(target_os = "android"))]
                                    {
                                        error.set(Some(tid!("error-android-only")));
                                    }
                                    uploading.set(false);
                                });
                            },
                            if uploading() {
                                "⏳ "
                                {tid!("action-loading")}
                            } else {
                                "🖼️ "
                                {tid!("action-gallery")}
                            }
                        }
                        button {
                            class: "btn-secondary",
                            style: "flex: 1; padding: 10px; font-size: 14px;",
                            disabled: uploading(),
                            onclick: move |_| {
                                uploading.set(true);
                                error.set(None);
                                spawn(async move {
                                    #[cfg(target_os = "android")]
                                    {
                                        match crate::camera::capture_photo() {
                                            Ok(path) => photo_path.set(Some(path)),
                                            Err(e) => error.set(Some(format!("{}: {}", tid!("error"), e))),
                                        }
                                    }
                                    #[cfg(not(target_os = "android"))]
                                    {
                                        error.set(Some(tid!("error-android-only")));
                                    }
                                    uploading.set(false);
                                });
                            },
                            if uploading() {
                                "⏳ "
                                {tid!("action-loading")}
                            } else {
                                "📷 "
                                {tid!("action-camera")}
                            }
                        }
                    }
                }

                div { style: "display: flex; gap: 12px; margin-top: 24px;",
                    button {
                        class: "btn-primary",
                        style: "flex: 1; padding: 14px;",
                        disabled: saving(),
                        onclick: move |_| handle_submit(),
                        if saving() {
                            "⏳ "
                            {tid!("action-saving")}
                        } else {
                            "💾 "
                            {tid!("action-save")}
                        }
                    }
                    button {
                        class: "btn-secondary",
                        style: "flex: 1; padding: 14px;",
                        disabled: saving(),
                        onclick: move |_| on_navigate.call(Screen::ProfileList),
                        "❌ "
                        {tid!("action-cancel")}
                    }
                }
            }
        }
    }
}
