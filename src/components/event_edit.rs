use crate::{
    database,
    models::{EventType, QuailEvent},
    services::photo_service,
    spacetime, Screen,
};
use base64::Engine;
use chrono::NaiveDate;
use dioxus::prelude::*;
use dioxus_gallery_components::{Gallery, GalleryConfig, GalleryItem};
use dioxus_i18n::t;
use photo_gallery::{Photo, PhotoResult, PhotoSize};

/// Helper component to load and display event photos using Gallery
#[component]
fn EventPhotoGallery(
    event_id: String,
    photos: Signal<Vec<Photo>>,
    delete_photo_fn: EventHandler<String>,
) -> Element {
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
    let gallery_key = format!(
        "{}:{}",
        event_id,
        gallery_items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );

    rsx! {
        if gallery_items.is_empty() && !photo_list.is_empty() {
            // Loading state
            div { style: "padding: 24px; text-align: center; color: #666;", "⏳ Loading photos..." }
        } else {
            Gallery {
                key: "{gallery_key}",
                items: gallery_items,
                config: gallery_config,
                on_delete: move |photo_id: String| {
                    delete_photo_fn.call(photo_id.clone());
                    spawn(async move {
                        if let Ok(conn) = database::init_database() {
                            if let Ok(uuid) = uuid::Uuid::parse_str(&photo_id) {
                                let _ = photo_service::delete_photo(&conn, &uuid).await;
                            }
                        }
                        // Photos will auto-update via SpacetimeDB subscription
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
    // Spacetime subscriptions and data
    let quail_events = spacetime::use_table_quail_events();
    #[cfg(target_os = "android")]
    let _photo_collections = spacetime::use_table_photo_collections();
    #[cfg(target_os = "android")]
    let photos_table = spacetime::use_table_photos();
    let connection = spacetime::use_connection();
    let update_event_reducer = spacetime::use_reducer_update_event();
    let delete_event_reducer = spacetime::use_reducer_delete_event();
    #[cfg(target_os = "android")]
    let create_photo_collection = spacetime::use_reducer_create_photo_collection();
    #[cfg(target_os = "android")]
    let create_photo = spacetime::use_reducer_create_photo();
    let delete_photo_reducer = spacetime::use_reducer_delete_photo();

    // Subscribe to quail_events and photos tables
    spacetime::use_subscription(&[
        "SELECT * FROM quail_events",
        "SELECT * FROM photo_collections",
        "SELECT * FROM photos",
    ]);

    // Local state signals
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
    let mut saving = use_signal(|| false);

    #[cfg(target_os = "android")]
    let event_id_for_gallery = event_id.clone();
    #[cfg(target_os = "android")]
    let event_id_for_camera = event_id.clone();

    // Clone reducers for use in button handlers
    #[cfg(target_os = "android")]
    let create_photo_collection_gallery = create_photo_collection.clone();
    #[cfg(target_os = "android")]
    let create_photo_gallery = create_photo.clone();
    #[cfg(target_os = "android")]
    let create_photo_collection_camera = create_photo_collection.clone();
    #[cfg(target_os = "android")]
    let create_photo_camera = create_photo.clone();

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

    // Load event + photos from Spacetime table
    let event_id_for_load = event_id.clone();
    use_effect(move || {
        if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_for_load) {
            // Find event in Spacetime table
            if let Some(e) = quail_events()
                .into_iter()
                .find(|ev| ev.uuid == e_uuid.to_string())
            {
                // Extract data from spacetime event
                let parsed_event_type = EventType::from_str(&e.event_type);
                let parsed_date = chrono::NaiveDate::parse_from_str(&e.event_date, "%Y-%m-%d")
                    .unwrap_or_else(|_| chrono::Local::now().naive_local().date());

                event_type.set(parsed_event_type.clone());
                event_date_str.set(parsed_date.format("%Y-%m-%d").to_string());
                notes.set(e.notes.clone().unwrap_or_default());

                // Create a local event for display
                let local_event = QuailEvent {
                    uuid: e_uuid,
                    quail_id: uuid::Uuid::parse_str(&e.quail_uuid)
                        .unwrap_or_else(|_| uuid::Uuid::nil()),
                    event_type: parsed_event_type,
                    event_date: parsed_date,
                    notes: e.notes.clone(),
                    photos: None,
                };
                event.set(Some(local_event));
            }
        }
    });

    // Auto-update photos when SpacetimeDB table changes
    #[cfg(target_os = "android")]
    let event_id_for_photos = event_id.clone();
    use_effect(move || {
        #[cfg(target_os = "android")]
        if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_for_photos) {
            let event_collection_uuid = e_uuid.to_string();
            let photo_list: Vec<Photo> = photos_table
                .read()
                .iter()
                .filter(|p| p.collection_uuid == event_collection_uuid)
                .filter_map(|p| {
                    let uuid = uuid::Uuid::parse_str(&p.uuid).ok()?;
                    Some(Photo {
                        uuid,
                        quail_id: None,
                        event_id: Some(e_uuid),
                        collection_id: Some(e_uuid),
                        path: p.relative_path.clone(),
                        relative_path: Some(p.relative_path.clone()),
                        thumbnail_path: None,
                        thumbnail_small_path: None,
                        thumbnail_medium_path: None,
                        sync_status: Some(p.sync_status.clone()),
                        sync_error: p.sync_error.clone(),
                        retry_count: Some(p.retry_count),
                        created_at: None,
                    })
                })
                .collect();
            photos.set(photo_list);
        }
    });

    // Save handler
    let event_id_for_save = event_id.clone();
    let quail_id_for_save = quail_id.clone();
    let mut handle_save = move || {
        // Check if connected to Spacetime
        if connection.read().is_none() {
            error.set(t!("error-not-connected"));
            return;
        }

        saving.set(true);
        error.set(String::new());

        if event_date_str().is_empty() {
            error.set(t!("error-empty-date"));
            saving.set(false);
            return;
        }
        let parsed_date = match NaiveDate::parse_from_str(&event_date_str(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                error.set(t!("error-invalid-date"));
                saving.set(false);
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

        let update_reducer = update_event_reducer.clone();
        let on_navigate_save = on_navigate.clone();

        spawn(async move {
            if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                // Call the reducer to update the event
                update_reducer(spacetime::UpdateEventArgs {
                    uuid: e_uuid.to_string(),
                    event_type: event_type_val.as_str().to_string(),
                    event_date: parsed_date.format("%Y-%m-%d").to_string(),
                    notes: notes_val,
                    photos: None,
                });

                success.set(true);
                saving.set(false);
                on_navigate_save.call(Screen::ProfileDetail(quail_id_clone.clone()));
            }
        });
    };

    // Delete handler
    let event_id_for_delete = event_id.clone();
    let quail_id_for_delete = quail_id.clone();
    let mut handle_delete = move || {
        // Check if connected to Spacetime
        if connection.read().is_none() {
            error.set(t!("error-not-connected"));
            return;
        }

        let event_id_clone = event_id_for_delete.clone();
        let quail_id_clone = quail_id_for_delete.clone();
        let delete_reducer_call = delete_event_reducer.clone();
        let on_navigate_delete = on_navigate.clone();

        spawn(async move {
            if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                delete_reducer_call(e_uuid.to_string());
                on_navigate_delete.call(Screen::ProfileDetail(quail_id_clone.clone()));
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
                    EventPhotoGallery {
                        event_id: event_id.clone(),
                        photos: photos.clone(),
                        delete_photo_fn: delete_photo_reducer.clone(),
                    }
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
                                    #[cfg(target_os = "android")]
                                    let create_photo_collection_gallery_fn = create_photo_collection_gallery
                                        .clone();
                                    #[cfg(target_os = "android")]
                                    let create_photo_gallery_fn = create_photo_gallery.clone();
                                    spawn(async move {
                                        #[cfg(target_os = "android")]
                                        {
                                            match crate::camera::pick_images() {
                                                Ok(paths) => {
                                                    if let Ok(conn) = database::init_database() {
                                                        if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                                                            let collection_uuid = e_uuid;
                                                            match crate::services::photo_service::get_or_create_event_collection(
                                                                &conn,
                                                                &e_uuid,
                                                            ) {
                                                                Ok(collection_id) => {
                                                                    create_photo_collection_gallery_fn(spacetime::CreatePhotoCollectionArgs {
                                                                        uuid: collection_uuid.to_string(),
                                                                        quail_uuid: None,
                                                                        event_uuid: Some(collection_uuid.to_string()),
                                                                        name: format!(
                                                                            "Event-{}",
                                                                            collection_uuid
                                                                                .to_string()
                                                                                .chars()
                                                                                .take(8)
                                                                                .collect::<String>(),
                                                                        ),
                                                                    });
                                                                    for p in paths {
                                                                        let ps = p.to_string_lossy().to_string();
                                                                        match crate::services::photo_service::add_photo_to_collection(
                                                                                &conn,
                                                                                &collection_id,
                                                                                ps.clone(),
                                                                            )
                                                                            .await
                                                                        {
                                                                            Ok(photo_uuid) => {
                                                                                create_photo_gallery_fn(spacetime::CreatePhotoArgs {
                                                                                    uuid: photo_uuid.to_string(),
                                                                                    collection_uuid: collection_uuid.to_string(),
                                                                                    relative_path: ps,
                                                                                });
                                                                            }
                                                                            Err(e) => {
                                                                                error.set(format!("Fehler beim Speichern: {}", e));
                                                                                break;
                                                                            }
                                                                        }
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
                                                    error.set(t!("error-pick-images", error : e.to_string()));
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
                                    #[cfg(target_os = "android")]
                                    let create_photo_collection_camera_fn = create_photo_collection_camera
                                        .clone();
                                    #[cfg(target_os = "android")]
                                    let create_photo_camera_fn = create_photo_camera.clone();
                                    spawn(async move {
                                        #[cfg(target_os = "android")]
                                        {
                                            match crate::camera::capture_photo() {
                                                Ok(p) => {
                                                    if let Ok(conn) = database::init_database() {
                                                        if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                                                            let ps = p.to_string_lossy().to_string();
                                                            let collection_uuid = e_uuid;
                                                            match crate::services::photo_service::get_or_create_event_collection(
                                                                &conn,
                                                                &e_uuid,
                                                            ) {
                                                                Ok(collection_id) => {
                                                                    create_photo_collection_camera_fn(spacetime::CreatePhotoCollectionArgs {
                                                                        uuid: collection_uuid.to_string(),
                                                                        quail_uuid: None,
                                                                        event_uuid: Some(collection_uuid.to_string()),
                                                                        name: format!(
                                                                            "Event-{}",
                                                                            collection_uuid
                                                                                .to_string()
                                                                                .chars()
                                                                                .take(8)
                                                                                .collect::<String>(),
                                                                        ),
                                                                    });
                                                                    match crate::services::photo_service::add_photo_to_collection(
                                                                            &conn,
                                                                            &collection_id,
                                                                            ps.clone(),
                                                                        )
                                                                        .await
                                                                    {
                                                                        Ok(photo_uuid) => {
                                                                            create_photo_camera_fn(spacetime::CreatePhotoArgs {
                                                                                uuid: photo_uuid.to_string(),
                                                                                collection_uuid: collection_uuid.to_string(),
                                                                                relative_path: ps,
                                                                            });
                                                                        }
                                                                        Err(e) => {
                                                                            error.set(format!("Fehler beim Speichern: {}", e));
                                                                        }
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
                                                    error.set(t!("error-capture-photo", error : e.to_string()));
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
