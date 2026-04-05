use crate::{
    Screen,
    models::{EventType, QuailEvent},
    spacetime,
};
use chrono::NaiveDate;
use dioxus::prelude::*;
use dioxus_gallery_components::{Gallery, GalleryConfig, GalleryItem};
use dioxus_i18n::tid;
use photo_gallery::{Photo, PhotoGalleryConfig, PhotoGalleryService, PhotoSize};

/// Helper component to load and display event photos using Gallery
#[component]
fn EventPhotoGallery(
    event_id: String,
    photos: Signal<Vec<Photo>>,
    delete_photo_fn: EventHandler<String>,
) -> Element {
    let photo_list = photos();

    let photo_config = PhotoGalleryConfig {
        storage_path: crate::services::photo_service::get_storage_path(),
        enable_thumbnails: true,
        thumbnail_small_size: 256,
        thumbnail_medium_size: 512,
    };
    let photo_service = PhotoGalleryService::new(photo_config);

    let gallery_items: Vec<GalleryItem> = photo_list
        .iter()
        .filter_map(|photo| {
            let thumb_or_original = photo_service
                .get_photo_file_path(&photo.relative_path, PhotoSize::Small)
                .or_else(|| {
                    photo_service.get_photo_file_path(&photo.relative_path, PhotoSize::Medium)
                })
                .or_else(|| {
                    photo_service.get_photo_file_path(&photo.relative_path, PhotoSize::Original)
                });

            let abs_path = thumb_or_original?;
            let data_url = crate::image_processing::image_path_to_data_url(&abs_path).ok()?;
            Some(GalleryItem {
                id: photo.uuid.clone(),
                data_url,
                caption: None,
            })
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
            div { class: "notification is-light has-text-centered", "⏳ Loading photos..." }
        } else {
            Gallery {
                key: "{gallery_key}",
                items: gallery_items,
                config: gallery_config,
                on_delete: move |photo_id: String| {
                    delete_photo_fn.call(photo_id.clone());
                    // Note: Photo deletion now handled via SpacetimeDB subscriptions
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
    let delete_photo_gallery = delete_photo_reducer.clone();

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
    // Retry failed downloads beim Mount
    use_effect(move || {
        // Note: photo retry logic now managed via SpacetimeDB subscriptions
        log::debug!("Photo retry stub - SpacetimeDB sync not yet fully implemented");
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
            let photo_list: Vec<Photo> = photos_table()
                .iter()
                .filter(|p| p.collection_uuid == event_collection_uuid)
                .map(|p| Photo {
                    uuid: p.uuid.clone(),
                    id: 0,
                    collection_uuid: p.collection_uuid.clone(),
                    relative_path: p.relative_path.clone(),
                    sync_status: p.sync_status.clone(),
                    sync_error: p.sync_error.clone(),
                    last_sync_attempt: p.last_sync_attempt,
                    retry_count: p.retry_count,
                    owner: p.owner.clone(),
                })
                .collect();

            if photos() != photo_list {
                photos.set(photo_list);
            }
        }
    });

    // Save handler
    let event_id_for_save = event_id.clone();
    let quail_id_for_save = quail_id.clone();
    let mut handle_save = move || {
        // Check if connected to Spacetime
        if connection().is_none() {
            error.set(tid!("error-not-connected"));
            return;
        }

        saving.set(true);
        error.set(String::new());

        if event_date_str().is_empty() {
            error.set(tid!("error-empty-date"));
            saving.set(false);
            return;
        }
        let parsed_date = match NaiveDate::parse_from_str(&event_date_str(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => {
                error.set(tid!("error-invalid-date"));
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
                if let Err(err) = update_reducer(spacetime::UpdateEventArgs {
                    uuid: e_uuid.to_string(),
                    event_type: event_type_val.as_str().to_string(),
                    event_date: parsed_date.format("%Y-%m-%d").to_string(),
                    notes: notes_val,
                    photos: None,
                }) {
                    error.set(err.to_string());
                    saving.set(false);
                    return;
                }

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
        if connection().is_none() {
            error.set(tid!("error-not-connected"));
            return;
        }

        let event_id_clone = event_id_for_delete.clone();
        let quail_id_clone = quail_id_for_delete.clone();
        let delete_reducer_call = delete_event_reducer.clone();
        let on_navigate_delete = on_navigate.clone();

        spawn(async move {
            if let Ok(e_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                if let Err(err) = delete_reducer_call(e_uuid.to_string()) {
                    error.set(err.to_string());
                    return;
                }

                on_navigate_delete.call(Screen::ProfileDetail(quail_id_clone.clone()));
            }
        });
    };

    rsx! {
        section { class: "section pt-4 pb-3",
            div { class: "container is-max-tablet",
                div { class: "level mb-4",
                    div { class: "level-left",
                        button {
                            class: "button is-light",
                            onclick: move |_| on_navigate.call(Screen::ProfileDetail(quail_id.clone())),
                            "← "
                            {tid!("action-back")}
                        }
                    }
                    div { class: "level-item",
                        h1 { class: "title is-4 mb-0", {tid!("event-edit-title")} }
                    }
                    div { class: "level-right" }
                }

                if !error().is_empty() {
                    div { class: "notification is-danger is-light",
                        "⚠️ "
                        {error()}
                    }
                }

                if success() {
                    div { class: "notification is-success is-light",
                        "✓ "
                        {tid!("updated")}
                    }
                }

                if let Some(_) = event() {
                    div { class: "box",
                        div { class: "field",
                            label { class: "label", {tid!("field-type")} }
                            div { class: "control",
                                div { class: "select is-fullwidth",
                                    select {
                                        value: event_type().as_str(),
                                        onchange: move |ev| {
                                            let v = ev.value();
                                            event_type.set(EventType::from_str(v.as_str()));
                                        },
                                        option { value: "born", {tid!("event-type-born")} }
                                        option { value: "alive", {tid!("event-type-alive")} }
                                        option { value: "sick", {tid!("event-type-sick")} }
                                        option { value: "healthy", {tid!("event-type-healthy")} }
                                        option { value: "marked_for_slaughter", {tid!("event-type-marked")} }
                                        option { value: "slaughtered", {tid!("event-type-slaughtered")} }
                                        option { value: "died", {tid!("event-type-died")} }
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
                                    value: "{event_date_str}",
                                    oninput: move |ev| event_date_str.set(ev.value()),
                                }
                            }
                        }

                        div { class: "field",
                            label { class: "label", {tid!("field-notes")} }
                            div { class: "control",
                                textarea {
                                    class: "textarea",
                                    style: "min-height: 120px;",
                                    value: "{notes}",
                                    oninput: move |ev| notes.set(ev.value()),
                                }
                            }
                        }

                        div { class: "field",
                            label { class: "label", {tid!("photos-count", count : photos().len())} }
                            EventPhotoGallery {
                                event_id: event_id.clone(),
                                photos: photos.clone(),
                                delete_photo_fn: move |photo_id: String| {
                                    if let Err(err) = delete_photo_gallery(photo_id) {
                                        log::error!("Failed to delete photo via SpacetimeDB reducer: {err}");
                                    }
                                },
                            }

                            div { class: "field is-grouped mt-3",
                                p { class: "control is-expanded",
                                    button {
                                        class: "button is-dark is-fullwidth",
                                        disabled: uploading(),
                                        onclick: {
                                            #[cfg(target_os = "android")]
                                            let create_photo_collection_gallery = create_photo_collection.clone();
                                            #[cfg(target_os = "android")]
                                            let create_photo_gallery = create_photo.clone();
                                            move |_| {
                                                uploading.set(true);
                                                error.set(String::new());
                                                #[cfg(target_os = "android")]
                                                let event_id_clone = event_id_for_gallery.clone();
                                                #[cfg(target_os = "android")]
                                                let create_photo_collection_gallery_call =
                                                    create_photo_collection_gallery.clone();
                                                #[cfg(target_os = "android")]
                                                let create_photo_gallery_call = create_photo_gallery.clone();
                                                spawn(async move {
                                                    #[cfg(target_os = "android")]
                                                    {
                                                        let device_id = crate::services::device_id_service::get_device_id()
                                                            .unwrap_or_else(|_| "unknown-device".to_string());
                                                        match crate::camera::pick_images() {
                                                            Ok(paths) => {
                                                                if let Ok(event_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                                                                    let collection_uuid = event_uuid.to_string();

                                                                    if let Err(err) = create_photo_collection_gallery_call(spacetime::CreatePhotoCollectionArgs {
                                                                        uuid: collection_uuid.clone(),
                                                                        quail_uuid: None,
                                                                        event_uuid: Some(event_uuid.to_string()),
                                                                        name: format!(
                                                                            "Event-{}",
                                                                            event_uuid.to_string().chars().take(8).collect::<String>()
                                                                        ),
                                                                        device_id: device_id.clone(),
                                                                    }) {
                                                                        error.set(format!(
                                                                            "{}: {}",
                                                                            tid!("error-pick-images", error: ""),
                                                                            err
                                                                        ));
                                                                        uploading.set(false);
                                                                        return;
                                                                    }

                                                                    for picked_path in paths {
                                                                        let source = picked_path.to_string_lossy().to_string();
                                                                        match crate::services::photo_service::process_photo(source).await {
                                                                            Ok((relative_original, _, _)) => {
                                                                                if let Some(photo_uuid) = std::path::Path::new(&relative_original)
                                                                                    .file_stem()
                                                                                    .and_then(|s| s.to_str())
                                                                                {
                                                                                    if let Err(err) = create_photo_gallery_call(spacetime::CreatePhotoArgs {
                                                                                        uuid: photo_uuid.to_string(),
                                                                                        collection_uuid: collection_uuid.clone(),
                                                                                        relative_path: relative_original,
                                                                                        device_id: device_id.clone(),
                                                                                    }) {
                                                                                        error.set(format!(
                                                                                            "{}: {}",
                                                                                            tid!("error-pick-images", error: ""),
                                                                                            err
                                                                                        ));
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(err) => {
                                                                                error.set(format!(
                                                                                    "{}: {}",
                                                                                    tid!("error-pick-images", error: ""),
                                                                                    err
                                                                                ));
                                                                            }
                                                                        }
                                                                    }
                                                                } else {
                                                                    error.set(tid!("error-invalid-event-id"));
                                                                }
                                                            }
                                                            Err(e) => {
                                                                error.set(tid!("error-pick-images", error : e.to_string()));
                                                            }
                                                        }
                                                    }
                                                    #[cfg(not(target_os = "android"))]
                                                    {
                                                        error.set(tid!("error-android-only-gallery"));
                                                    }
                                                    uploading.set(false);
                                                });
                                            }
                                        },
                                        if uploading() {
                                            "⏳"
                                        } else {
                                            "🖼️ "
                                            {tid!("action-gallery")}
                                        }
                                    }
                                }

                                p { class: "control is-expanded",
                                    button {
                                        class: "button is-dark is-fullwidth",
                                        disabled: uploading(),
                                        onclick: {
                                            #[cfg(target_os = "android")]
                                            let create_photo_collection_camera = create_photo_collection.clone();
                                            #[cfg(target_os = "android")]
                                            let create_photo_camera = create_photo.clone();
                                            move |_| {
                                                uploading.set(true);
                                                error.set(String::new());
                                                #[cfg(target_os = "android")]
                                                let event_id_clone = event_id_for_camera.clone();
                                                #[cfg(target_os = "android")]
                                                let create_photo_collection_camera_call =
                                                    create_photo_collection_camera.clone();
                                                #[cfg(target_os = "android")]
                                                let create_photo_camera_call = create_photo_camera.clone();
                                                spawn(async move {
                                                    #[cfg(target_os = "android")]
                                                    {
                                                        let device_id = crate::services::device_id_service::get_device_id()
                                                            .unwrap_or_else(|_| "unknown-device".to_string());
                                                        match crate::camera::capture_photo() {
                                                            Ok(path) => {
                                                                if let Ok(event_uuid) = uuid::Uuid::parse_str(&event_id_clone) {
                                                                    let collection_uuid = event_uuid.to_string();

                                                                    if let Err(err) = create_photo_collection_camera_call(spacetime::CreatePhotoCollectionArgs {
                                                                        uuid: collection_uuid.clone(),
                                                                        quail_uuid: None,
                                                                        event_uuid: Some(event_uuid.to_string()),
                                                                        name: format!(
                                                                            "Event-{}",
                                                                            event_uuid.to_string().chars().take(8).collect::<String>()
                                                                        ),
                                                                        device_id: device_id.clone(),
                                                                    }) {
                                                                        error.set(format!(
                                                                            "{}: {}",
                                                                            tid!("error-capture-photo", error: ""),
                                                                            err
                                                                        ));
                                                                        uploading.set(false);
                                                                        return;
                                                                    }

                                                                    let source = path.to_string_lossy().to_string();
                                                                    match crate::services::photo_service::process_photo(source).await {
                                                                        Ok((relative_original, _, _)) => {
                                                                            if let Some(photo_uuid) = std::path::Path::new(&relative_original)
                                                                                .file_stem()
                                                                                .and_then(|s| s.to_str())
                                                                            {
                                                                                if let Err(err) = create_photo_camera_call(spacetime::CreatePhotoArgs {
                                                                                    uuid: photo_uuid.to_string(),
                                                                                    collection_uuid,
                                                                                    relative_path: relative_original,
                                                                                    device_id,
                                                                                }) {
                                                                                    error.set(format!(
                                                                                        "{}: {}",
                                                                                        tid!("error-capture-photo", error: ""),
                                                                                        err
                                                                                    ));
                                                                                }
                                                                            }
                                                                        }
                                                                        Err(err) => {
                                                                            error.set(format!(
                                                                                "{}: {}",
                                                                                tid!("error-capture-photo", error: ""),
                                                                                err
                                                                            ));
                                                                        }
                                                                    }
                                                                } else {
                                                                    error.set(tid!("error-invalid-event-id"));
                                                                }
                                                            }
                                                            Err(e) => {
                                                                error.set(tid!("error-capture-photo", error : e.to_string()));
                                                            }
                                                        }
                                                    }
                                                    #[cfg(not(target_os = "android"))]
                                                    {
                                                        error.set(tid!("error-android-only-camera"));
                                                    }
                                                    uploading.set(false);
                                                });
                                            }
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
                        }

                        div { class: "field has-addons mt-5",
                            p { class: "control is-expanded",
                                button {
                                    class: "button is-primary is-fullwidth",
                                    disabled: saving(),
                                    onclick: move |_| handle_save(),
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
                                    onclick: {
                                        let quail_id_for_cancel = quail_id.clone();
                                        move |_| on_navigate.call(Screen::ProfileDetail(quail_id_for_cancel.clone()))
                                    },
                                    {tid!("action-cancel")}
                                }
                            }

                            p { class: "control is-expanded",
                                button {
                                    class: "button is-danger is-light is-fullwidth",
                                    disabled: saving(),
                                    onclick: move |_| handle_delete(),
                                    "🗑️ "
                                    {tid!("action-delete")}
                                }
                            }
                        }
                    }
                } else {
                    div { class: "notification is-light has-text-centered", {tid!("loading-event")} }
                }
            }
        }
    }
}
