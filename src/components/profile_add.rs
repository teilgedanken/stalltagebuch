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
    let mut birthday = use_signal(|| String::new());
    let mut ring_color_left = use_signal(|| String::new());
    let mut ring_color_right = use_signal(|| String::new());
    let mut photo_path = use_signal(|| None::<PathBuf>);
    let mut uploading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut success = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let create_quail = spacetime::use_reducer_create_quail();
    let create_event = spacetime::use_reducer_create_event();
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

        let ring_color_left_value = normalize_ring_color_selection(&ring_color_left());
        let ring_color_right_value = normalize_ring_color_selection(&ring_color_right());
        let birthday_value = normalize_optional_date_input(&birthday());

        let device_id = crate::services::device_id_service::get_device_id()
            .unwrap_or_else(|_| "unknown-device".to_string());

        // Capture photo_path BEFORE moving into async
        let selected_photo_path = photo_path();
        let photo_name_for_display = name_trimmed.clone();

        if let Err(err) = create_quail(spacetime::CreateQuailArgs {
            uuid: quail_uuid.clone(),
            name: name_trimmed.to_string(),
            gender: gender_value.as_str().to_string(),
            ring_color_left: ring_color_left_value,
            ring_color_right: ring_color_right_value,
            profile_photo: None,
            device_id: device_id.clone(),
        }) {
            error.set(Some(err.to_string()));
            saving.set(false);
            return;
        }

        let birthday_event_date = birthday_value.clone();
        let on_navigate_submit = on_navigate.clone();
        let create_event_reducer = create_event.clone();
        let create_photo_collection_reducer = create_photo_collection.clone();
        let create_photo_reducer = create_photo.clone();
        let set_quail_photo_reducer = set_quail_photo.clone();

        spawn(async move {
            if let Some(event_date) = birthday_event_date {
                if let Err(err) = create_event_reducer(spacetime::CreateEventArgs {
                    uuid: uuid::Uuid::new_v4().to_string(),
                    quail_uuid: quail_uuid.clone(),
                    event_type: "born".to_string(),
                    event_date,
                    notes: None,
                    photos: None,
                    device_id: device_id.clone(),
                }) {
                    error.set(Some(err.to_string()));
                    saving.set(false);
                    return;
                }
            }

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
        section { class: "section pt-4 pb-4",
            div { class: "container is-max-tablet",
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
                        h1 { class: "title is-4 mb-0",
                            {tid!("profile-add-title")}
                        }
                    }
                    div { class: "level-right" }
                }

                if let Some(err) = error() {
                    div { class: "notification is-danger is-light",
                        "⚠️ {err}"
                    }
                }

                if success() {
                    div { class: "notification is-success is-light",
                        "✅ "
                        {tid!("profile-created-success")}
                    }
                }

                div { class: "box",
                    div { class: "field",
                        label { class: "label", {tid!("profile-name-label")} }
                        div { class: "control",
                            input {
                                r#type: "text",
                                class: "input",
                                placeholder: tid!("profile-name-placeholder"),
                                value: "{name}",
                                oninput: move |e| name.set(e.value()),
                                autofocus: true,
                            }
                        }
                    }

                    div { class: "field",
                        label { class: "label", {tid!("profile-gender-label")} }
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
                        label { class: "label", {tid!("profile-ring-color-label")} }
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
                                                option { value: "hellblau", {tid!("ring-color-light-blue")} }
                                                option { value: "dunkelblau", {tid!("ring-color-dark-blue")} }
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
                                            style: format!("width: 2rem; height: 2rem; border: 1px solid #bbb; background: {};", ring_color_preview_bg(&ring_color_left())),
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
                                                option { value: "hellblau", {tid!("ring-color-light-blue")} }
                                                option { value: "dunkelblau", {tid!("ring-color-dark-blue")} }
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
                                            style: format!("width: 2rem; height: 2rem; border: 1px solid #bbb; background: {};", ring_color_preview_bg(&ring_color_right())),
                                        }
                                    }
                                }
                            }
                        }
                    }

                    article { class: "message is-info is-light",
                        div { class: "message-body",
                            "ℹ️ "
                            {tid!("profile-add-info")}
                        }
                    }

                    div { class: "field",
                        label { class: "label", {tid!("profile-photo-label")} }

                        if let Some(path) = photo_path() {
                            div { class: "notification is-light",
                                div { class: "is-flex is-align-items-center is-justify-content-space-between",
                                    div {
                                        strong { "📷 " }
                                        {tid!("photo-selected")}
                                        p { class: "help", {path.file_name().and_then(|n| n.to_str()).unwrap_or("Unbekannt")} }
                                    }
                                    button {
                                        class: "button is-small is-light",
                                        onclick: move |_| photo_path.set(None),
                                        "🗑️"
                                    }
                                }
                            }
                        } else {
                            div {
                                class: "notification is-light has-text-centered",
                                style: "border: 2px dashed #ccc;",
                                {tid!("photo-none-selected")}
                            }
                        }

                        div { class: "field is-grouped mt-3",
                            p { class: "control is-expanded",
                                button {
                                    class: "button is-light is-fullwidth",
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
                            }

                            p { class: "control is-expanded",
                                button {
                                    class: "button is-light is-fullwidth",
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
                    }

                    div { class: "field is-grouped mt-5",
                        p { class: "control is-expanded",
                            button {
                                class: "button is-primary is-fullwidth",
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
                        }
                        p { class: "control is-expanded",
                            button {
                                class: "button is-light is-fullwidth",
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
    }
}

fn normalize_ring_color_selection(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(RingColor::from_str(trimmed).as_str().to_string())
    }
}

fn normalize_optional_date_input(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn ring_color_preview_bg(value: &str) -> &'static str {
    match value.trim() {
        "rot" => "#ef5350",
        "dunkelblau" => "#5c6bc0",
        "hellblau" => "#64b5f6",
        "gruen" => "#66bb6a",
        "gelb" => "#fff176",
        "orange" => "#ffb74d",
        "lila" => "#ba68c8",
        "rosa" => "#f48fb1",
        "schwarz" => "#616161",
        "weiss" => "#f5f5f5",
        _ => "#ffffff",
    }
}

fn ring_color_select_bg(value: &str) -> &'static str {
    match value.trim() {
        "rot" => "#ffebee",
        "dunkelblau" => "#e8eaf6",
        "hellblau" => "#e3f2fd",
        "gruen" => "#e8f5e9",
        "gelb" => "#fffde7",
        "orange" => "#fff3e0",
        "lila" => "#f3e5f5",
        "rosa" => "#fce4ec",
        "schwarz" => "#f5f5f5",
        "weiss" => "#ffffff",
        _ => "#ffffff",
    }
}
