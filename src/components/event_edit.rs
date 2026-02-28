use crate::{
    database,
    models::{EventType, QuailEvent},
    services::{event_service, photo_service},
    Screen,
};
use base64::Engine;
use chrono::NaiveDate;
use dioxus::prelude::*;
use dioxus_gallery_components::{Gallery, GalleryConfig, GalleryItem};
use dioxus_i18n::t;
use photo_gallery::{Photo, PhotoResult, PhotoSize};

/// Helper component to load and display event photos using Gallery
#[component]
fn EventPhotoGallery(event_id: String, photos: Signal<Vec<Photo>>) -> Element {
    // Load all photo data asynchronously
    let photo_list = photos();
    let mut loaded_photos = use_signal(|| Vec::<(String, String)>::new());

    // Trigger loading for all photos
    use_effect(move || {
        let photo_list = photos();
        spawn(async move {
            let mut loaded = Vec::new();
            for photo in photo_list {
                let photo_uuid = photo.uuid.to_string();
                if let Ok(conn) = database::init_database() {
                    if let Ok(uuid) = uuid::Uuid::parse_str(&photo_uuid) {
                        match photo_service::get_photo_with_download(&conn, &uuid, PhotoSize::Small)
                            .await
                        {
                            Ok(PhotoResult::Available(bytes)) => {
                                let data_url = format!(
                                    "data:image/webp;base64,{}",
                                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                                );
                                loaded.push((photo_uuid, data_url));
                            }
                            Ok(PhotoResult::Downloading) => {
                                log::debug!("Photo {} still downloading", photo_uuid);
                            }
                            Ok(PhotoResult::Failed(err, retry_count)) => {
                                log::warn!(
                                    "Photo {} download failed: {} (retry count: {})",
                                    photo_uuid,
                                    err,
                                    retry_count
                                );
                            }
                            Err(e) => {
                                log::error!("Failed to load photo {}: {}", photo_uuid, e);
                            }
                        }
                    } else {
                        log::error!("Invalid photo UUID: {}", photo_uuid);
                    }
                } else {
                    log::error!("Failed to initialize database");
                }
            }
            loaded_photos.set(loaded);
        });
    });

    let gallery_items: Vec<GalleryItem> = loaded_photos()
        .iter()
        .map(|(id, data_url)| GalleryItem {
            id: id.clone(),
            data_url: data_url.clone(),
            caption: None,
        })
        .collect();

    let gallery_config = GalleryConfig {
        allow_delete: true,
        allow_select: false,
        selected_id: None,
    };

    rsx! {
        if gallery_items.is_empty() && !photo_list.is_empty() {
            // Loading state
            div { style: "padding: 24px; text-align: center; color: #666;", "⏳ Loading photos..." }
        } else {
            Gallery {
                items: gallery_items,
                config: gallery_config,
                on_delete: move |photo_id: String| {
                    let event_id_clone = event_id.clone();
                    spawn(async move {
                        if let Ok(conn) = database::init_database() {
                            if let Ok(uuid) = uuid::Uuid::parse_str(&photo_id) {
                                match photo_service::delete_photo(&conn, &uuid).await {
                                    Ok(_) => {
                                        log::info!("Successfully deleted photo {}", photo_id);
                                    }
                                    Err(e) => {
                                        log::error!("Failed to delete photo {}: {}", photo_id, e);
                                    }
                                }
                            }
                            if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                                let photo_list = match crate::services::photo_service::get_event_collection(
                                    &conn,
                                    &e_uuid,
                                ) {
                                    Ok(Some(collection_id)) => {
                                        crate::services::photo_service::list_collection_photos(
                                                &conn,
                                                &collection_id,
                                            )
                                            .ok()
                                    }
                                    _ => None,
                                };
                                if let Some(list) = photo_list {
                                    photos.set(list);
                                }
                            }
                        }
                    });
                },
            }
        }
    }
}

#[component]
pub fn EventEditScreen(
    event_id: String,
    quail_id: String,
    on_navigate: EventHandler<Screen>,
) -> Element {
    let mut event = use_signal(|| None::<QuailEvent>);
    let mut event_type = use_signal(|| EventType::Alive);
    let mut event_date_str = use_signal(|| {
        chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    });
    let mut notes = use_signal(|| String::new());
    let mut photos = use_signal(|| Vec::<Photo>::new());
    let mut error = use_signal(|| String::new());
    let mut success = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let saving = use_signal(|| false);

    #[cfg(target_os = "android")]
    let event_id_for_gallery = event_id.clone();
    #[cfg(target_os = "android")]
    let event_id_for_camera = event_id.clone();

    // Retry failed downloads beim Mount
    use_effect(move || {
        spawn(async move {
            if let Ok(conn) = database::init_database() {
                if let Err(e) = crate::services::photo_service::retry_failed_downloads(&conn).await
                {
                    log::warn!("Failed to retry photo downloads: {}", e);
                }
            }
        });
    });

    // Load event + photos
    let event_id_for_load = event_id.clone();
    use_effect(move || {
        if let Ok(conn) = database::init_database() {
            if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_for_load) {
                match event_service::get_event_by_id(&conn, &e_uuid) {
                    Ok(Some(e)) => {
                        event.set(Some(e.clone()));
                        event_type.set(e.event_type.clone());
                        event_date_str.set(e.event_date.format("%Y-%m-%d").to_string());
                        notes.set(e.notes.unwrap_or_default());
                    }
                    Ok(None) => error.set(t!("event-not-found")),
                    Err(e) => error.set(t!("error-loading", error: e.to_string())),
                }
                // Load photos using collection-based API
                let photo_list =
                    match crate::services::photo_service::get_event_collection(&conn, &e_uuid) {
                        Ok(Some(collection_id)) => {
                            crate::services::photo_service::list_collection_photos(
                                &conn,
                                &collection_id,
                            )
                            .ok()
                        }
                        _ => None,
                    };

                if let Some(list) = photo_list {
                    photos.set(list);
                } else {
                    log::error!("Fehler beim Laden der Event-Fotos");
                }
            }
        }
    });

    // Save handler
    let event_id_for_save = event_id.clone();
    let quail_id_for_save = quail_id.clone();
    let mut saving_signal = saving.clone();
    let mut handle_save = move || {
        saving_signal.set(true);
        if event_date_str().is_empty() {
            error.set(t!("error-empty-date"));
            saving_signal.set(false);
            return;
        }
        let parsed_date = match NaiveDate::parse_from_str(&event_date_str(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                error.set(t!("error-invalid-date"));
                saving_signal.set(false);
                return;
            }
        };
        let event_id_clone = event_id_for_save.clone();
        let quail_id_clone = quail_id_for_save.clone();
        let event_type_val = event_type();
        let notes_val = if notes().is_empty() {
            None
        } else {
            Some(notes())
        };
        let mut saving_signal = saving_signal.clone();
        spawn(async move {
            if let Ok(conn) = database::init_database() {
                if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                    match event_service::update_event_full(
                        &conn,
                        &e_uuid,
                        event_type_val,
                        parsed_date,
                        notes_val,
                    )
                    .await
                    {
                        Ok(_) => {
                            success.set(true);
                            saving_signal.set(false);
                            on_navigate.call(Screen::ProfileDetail(quail_id_clone.clone()));
                        }
                        Err(e) => {
                            error.set(t!("error-save", error: e.to_string()));
                            saving_signal.set(false);
                        }
                    }
                }
            } else {
                error.set(t!("error-db-unavailable"));
                saving_signal.set(false);
            }
        });
    };

    // Delete handler
    let event_id_for_delete = event_id.clone();
    let quail_id_for_delete = quail_id.clone();
    let handle_delete = move || {
        let event_id_clone = event_id_for_delete.clone();
        let quail_id_clone = quail_id_for_delete.clone();
        spawn(async move {
            if let Ok(conn) = database::init_database() {
                if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                    match event_service::delete_event(&conn, &e_uuid).await {
                        Ok(_) => on_navigate.call(Screen::ProfileDetail(quail_id_clone.clone())),
                        Err(e) => error.set(t!("error-delete", error: e.to_string())),
                    }
                }
            }
        });
    };

    rsx! {
        div { style: "padding:16px; max-width:600px; margin:0 auto;",
            // Header
            div { style: "display:flex; align-items:center; gap:12px; margin-bottom:20px;",
                button {
                    style: "padding:8px 12px; background:#e0e0e0; border-radius:8px;",
                    onclick: move |_| on_navigate.call(Screen::ProfileDetail(quail_id.clone())),
                    "←"
                }
                h1 { style: "margin:0; font-size:22px; color:#0066cc;", {t!("event-edit-title")} }
            }
            if !error().is_empty() {
                div { style: "background:#ffe6e6; padding:12px; border-radius:8px; color:#c00; margin-bottom:16px;",
                    "⚠️ "
                    {error()}
                }
            }
            if success() {
                div { style: "background:#e6ffe6; padding:12px; border-radius:8px; color:#060; margin-bottom:16px;",
                    "✓ "
                    {t!("updated")}
                }
            }
            if let Some(_) = event() {
                // Event type
                div { style: "margin-bottom:16px;",
                    label { style: "display:block; font-weight:600; margin-bottom:6px;",
                        {t!("field-type")}
                    }
                    select {
                        value: event_type().as_str(),
                        onchange: move |ev| {
                            let v = ev.value();
                            event_type.set(EventType::from_str(v.as_str()));
                        },
                        style: "width:100%; padding:10px; border:1px solid #ccc; border-radius:8px;",
                        option { value: "born", {t!("event-type-born")} }
                        option { value: "alive", {t!("event-type-alive")} }
                        option { value: "sick", {t!("event-type-sick")} }
                        option { value: "healthy", {t!("event-type-healthy")} }
                        option { value: "marked_for_slaughter", {t!("event-type-marked")} }
                        option { value: "slaughtered", {t!("event-type-slaughtered")} }
                        option { value: "died", {t!("event-type-died")} }
                    }
                }
                // Date
                div { style: "margin-bottom:16px;",
                    label { style: "display:block; font-weight:600; margin-bottom:6px;",
                        {t!("field-date")}
                    }
                    input {
                        r#type: "date",
                        value: "{event_date_str}",
                        oninput: move |ev| event_date_str.set(ev.value()),
                        style: "width:100%; padding:10px; border:1px solid #ccc; border-radius:8px;",
                    }
                }
                // Notes
                div { style: "margin-bottom:16px;",
                    label { style: "display:block; font-weight:600; margin-bottom:6px;",
                        {t!("field-notes")}
                    }
                    textarea {
                        value: "{notes}",
                        oninput: move |ev| notes.set(ev.value()),
                        style: "width:100%; padding:10px; border:1px solid #ccc; border-radius:8px; min-height:120px;",
                    }
                }
                // Photos grid
                div { style: "margin-bottom:20px;",
                    label { style: "display:block; font-weight:600; margin-bottom:6px;",
                        {t!("photos-count", count : photos().len())}
                    }
                    EventPhotoGallery { event_id: event_id.clone(), photos: photos.clone() }
                    // Add buttons (always visible)
                    div { style: "display:flex; gap:12px;",
                        button {
                            disabled: uploading(),
                            style: "flex:1; padding:10px; background:rgba(0,0,0,0.6); color:white; border-radius:8px;",
                            onclick: {
                                move |_| {
                                    uploading.set(true);
                                    error.set(String::new());
                                    #[cfg(target_os = "android")]
                                    let event_id_clone = event_id_for_gallery.clone();
                                    spawn(async move {
                                        #[cfg(target_os = "android")]
                                        {
                                            match crate::camera::pick_images() {
                                                Ok(paths) => {
                                                    if let Ok(conn) = database::init_database() {
                                                        if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                                                            match crate::services::photo_service::get_or_create_event_collection(
                                                                &conn,
                                                                &e_uuid,
                                                            ) {
                                                                Ok(collection_id) => {
                                                                    for p in paths {
                                                                        let ps = p.to_string_lossy().to_string();
                                                                        let _ = crate::services::photo_service::add_photo_to_collection(
                                                                                &conn,
                                                                                &collection_id,
                                                                                ps,
                                                                            )
                                                                            .await;
                                                                    }
                                                                    if let Ok(list) = crate::services::photo_service::list_collection_photos(
                                                                        &conn,
                                                                        &collection_id,
                                                                    ) {
                                                                        photos.set(list);
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    log::error!("Failed to create event collection: {}", e);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    error.set(t!("error-pick-images", error : e.to_string()))
                                                }
                                            }
                                        }
                                        #[cfg(not(target_os = "android"))]
                                        {
                                            error.set(t!("error-android-only-gallery"));
                                        }
                                        uploading.set(false);
                                    });
                                }
                            },
                            if uploading() {
                                "⏳"
                            } else {
                                "🖼️ "
                                {t!("action-gallery")}
                            }
                        }
                        button {
                            disabled: uploading(),
                            style: "flex:1; padding:10px; background:rgba(0,0,0,0.6); color:white; border-radius:8px;",
                            onclick: {
                                move |_| {
                                    uploading.set(true);
                                    error.set(String::new());
                                    #[cfg(target_os = "android")]
                                    let event_id_clone = event_id_for_camera.clone();
                                    spawn(async move {
                                        #[cfg(target_os = "android")]
                                        {
                                            match crate::camera::capture_photo() {
                                                Ok(p) => {
                                                    if let Ok(conn) = database::init_database() {
                                                        if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                                                            let ps = p.to_string_lossy().to_string();
                                                            match crate::services::photo_service::get_or_create_event_collection(
                                                                &conn,
                                                                &e_uuid,
                                                            ) {
                                                                Ok(collection_id) => {
                                                                    let _ = crate::services::photo_service::add_photo_to_collection(
                                                                            &conn,
                                                                            &collection_id,
                                                                            ps,
                                                                        )
                                                                        .await;
                                                                    if let Ok(list) = crate::services::photo_service::list_collection_photos(
                                                                        &conn,
                                                                        &collection_id,
                                                                    ) {
                                                                        photos.set(list);
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    log::error!("Failed to create event collection: {}", e);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    error.set(t!("error-capture-photo", error : e.to_string()))
                                                }
                                            }
                                        }
                                        #[cfg(not(target_os = "android"))]
                                        {
                                            error.set(t!("error-android-only-camera"));
                                        }
                                        uploading.set(false);
                                    });
                                }
                            },
                            if uploading() {
                                "⏳"
                            } else {
                                "📷 "
                                {t!("action-photo")}
                            }
                        }
                    }
                }
                // Action buttons
                div { style: "display:flex; gap:12px;",
                    button {
                        disabled: saving(),
                        style: "flex:1; padding:14px; background:#0066cc; color:white; border-radius:8px; font-weight:600;",
                        onclick: move |_| handle_save(),
                        if saving() {
                            "⏳ "
                            {t!("action-saving")}
                        } else {
                            "✓ "
                            {t!("action-save")}
                        }
                    }
                    button {
                        disabled: saving(),
                        style: "flex:1; padding:14px; background:#e0e0e0; color:#333; border-radius:8px; font-weight:600;",
                        onclick: {
                            let quail_id_for_cancel = quail_id.clone();
                            move |_| on_navigate.call(Screen::ProfileDetail(quail_id_for_cancel.clone()))
                        },
                        {t!("action-cancel")}
                    }
                    button {
                        disabled: saving(),
                        style: "flex:1; padding:14px; background:#ffdddd; color:#cc0000; border-radius:8px; font-weight:600;",
                        onclick: move |_| handle_delete(),
                        "🗑️ "
                        {t!("action-delete")}
                    }
                }
            } else {
                div { style: "padding:40px; text-align:center; color:#666;", {t!("loading-event")} }
            }
        }
    }
}
