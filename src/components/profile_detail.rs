use crate::database;
// image loading is handled by photo_gallery components (PreviewCollection / FullscreenCollection)
use crate::models::{EventType, Gender};
use crate::spacetime;
use crate::Screen;
use dioxus::prelude::*;
use dioxus_i18n::t;
// Photo type is not needed in this file - preview/fullscreen components handle loading
use photo_gallery::CollectionFullscreen;
use spacetimedb_sdk::DbContext;

/// Helper function to resolve photo path to absolute path
/// Handles both full paths (starting with /) and relative filenames
// resolve_photo_path and image_to_data_url logic is no longer needed here

#[component]
pub fn ProfileDetailScreen(quail_id: String, on_navigate: EventHandler<Screen>) -> Element {
    let quails = spacetime::use_table_quails();
    let all_events = spacetime::use_table_quail_events();
    #[cfg(target_os = "android")]
    let photo_collections = spacetime::use_table_photo_collections();
    #[cfg(target_os = "android")]
    let _photos = spacetime::use_table_photos();
    let connection = spacetime::use_connection();
    #[cfg(target_os = "android")]
    let create_photo_collection = spacetime::use_reducer_create_photo_collection();
    #[cfg(target_os = "android")]
    let create_photo = spacetime::use_reducer_create_photo();
    #[cfg(target_os = "android")]
    let set_quail_photo = spacetime::use_reducer_set_quail_photo();
    spacetime::use_subscription(&[
        "SELECT * FROM quails",
        "SELECT * FROM quail_events",
        "SELECT * FROM photo_collections",
        "SELECT * FROM photos",
    ]);
    
    // preview UUID shown in the main profile area (PreviewCollection)
    let mut quail_photo_collection_uuid = use_signal(|| None::<uuid::Uuid>);
    // uuids for fullscreen viewer (filled when opening fullscreen)
    let fullscreen_uuids = use_signal(|| Vec::<uuid::Uuid>::new());
    let current_photo_index = use_signal(|| 0usize);
    let mut show_fullscreen = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(|| String::new());

    #[cfg(target_os = "android")]
    let quail_id_for_gallery = quail_id.clone();
    #[cfg(target_os = "android")]
    let quail_id_for_camera = quail_id.clone();
    // clone used for opening fullscreen without moving the original id
    let quail_id_for_fullscreen = quail_id.clone();

    // Clone reducers for use in button handlers
    #[cfg(target_os = "android")]
    let create_photo_collection_gallery = create_photo_collection.clone();
    #[cfg(target_os = "android")]
    let create_photo_gallery = create_photo.clone();
    #[cfg(target_os = "android")]
    let create_photo_collection_camera = create_photo_collection.clone();
    #[cfg(target_os = "android")]
    let create_photo_camera = create_photo.clone();
    #[cfg(target_os = "android")]
    let set_quail_photo_gallery = set_quail_photo.clone();
    #[cfg(target_os = "android")]
    let set_quail_photo_camera = set_quail_photo.clone();

    // Set default preview collection UUID from profile UUID.
    let quail_id_for_preview = quail_id.clone();
    use_effect(move || {
        if let Ok(uuid) = uuid::Uuid::parse_str(&quail_id_for_preview) {
            quail_photo_collection_uuid.set(Some(uuid));
        }
    });

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

    let quail_id_for_profile_memo = quail_id.clone();
    let profile = use_memo(move || {
        let owner = connection
            .read()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        quails
            .read()
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
        let owner = connection
            .read()
            .as_ref()
            .and_then(|conn| conn.try_identity())
            .map(|id| id.to_string());

        let mut rows: Vec<spacetime::QuailEvent> = all_events
            .read()
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
                .then_with(|| b.id.cmp(&a.id))
        });
        rows
    });

    rsx! {
        div { style: "padding: 16px; max-width: 800px; margin: 0 auto;",
            // Header
            div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 24px;",
                button {
                    style: "padding: 8px 16px; background: #e0e0e0; color: #333; border-radius: 8px; font-size: 16px;",
                    onclick: move |_| on_navigate.call(Screen::ProfileList),
                    "← "
                    {t!("action-back")}
                }
                h1 {
                    style: "margin: 0; font-size: 26px; color: #0066cc; font-weight: 700;",
                    {t!("profile-detail-title")} // Profile
                }
            }

            if let Some(p) = profile() {
                div { style: "display: flex; flex-direction: column; gap: 24px;",
                    // Bild mit Plus-Button - zeigt Profilfoto, klickbar für Vollbild-Galerie
                    div { style: "width: 100%; aspect-ratio: 1/1; background: #f0f0f0; border-radius: 12px; overflow: hidden; display: flex; align-items: center; justify-content: center; position: relative;",
                        // Hauptbild (klickbar für Galerie)
                        div {
                            style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; cursor: pointer;",
                            onclick: move |_| {
                                let quail_id_open = quail_id_for_fullscreen.clone();
                                let mut fullscreen_uuids_sig = fullscreen_uuids.clone();
                                let mut current_idx_sig = current_photo_index.clone();
                                let mut show_fullscreen_sig = show_fullscreen.clone();
                                spawn(async move {
                                    if let Ok(conn) = database::init_database() {
                                        if let Ok(uuid) = uuid::Uuid::parse_str(&quail_id_open) {
                                            if let Ok(Some(collection_id)) = crate::services::photo_service::get_quail_collection(
                                                &conn,
                                                &uuid,
                                            ) {
                                                if let Ok(list) = crate::services::photo_service::list_collection_photos(
                                                    &conn,
                                                    &collection_id,
                                                ) {
                                                    let uuids = list
                                                        .into_iter()
                                                        .map(|p| p.uuid)
                                                        .collect::<Vec<uuid::Uuid>>();
                                                    if !uuids.is_empty() {
                                                        log::debug!(
                                                            "ProfileDetail: opening fullscreen with {} photos", uuids
                                                            .len()
                                                        );
                                                        fullscreen_uuids_sig.set(uuids);
                                                        current_idx_sig.set(0);
                                                        show_fullscreen_sig.set(true);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            },
                            // Display photo from SpacetimeDB directly
                            if let Some(photo_uuid_str) = p.profile_photo.as_ref() {
                                if let Ok(photo_uuid) = uuid::Uuid::parse_str(photo_uuid_str) {
                                    {rsx! {
                                        photo_gallery::PreviewImage {
                                            key: "{photo_uuid_str}",
                                            photo_uuid: Some(photo_uuid),
                                            alt: "Profile Photo".to_string(),
                                        }
                                    }}
                                } else {
                                    {rsx! {
                                        div {
                                            style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 64px; color: #ccc;",
                                            "🐦"
                                        }
                                    }}
                                }
                            } else {
                                {rsx! {
                                    div {
                                        style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 64px; color: #ccc;",
                                        "🐦"
                                    }
                                }}
                            }
                        }
                        // Zwei halbtransparente Overlay-Buttons (Galerie Mehrfach / Kamera Einzel)
                        // Galerie (Mehrfachauswahl)
                        button {
                            style: "position:absolute; bottom:12px; left:12px; padding:10px 14px; background:rgba(0,0,0,0.45); color:white; backdrop-filter:blur(4px); border-radius:8px; font-size:14px; display:flex; align-items:center; gap:6px; cursor:pointer; z-index:11;",
                            disabled: uploading(),
                            onclick: {
                                move |e| {
                                    e.stop_propagation();
                                    uploading.set(true);
                                    upload_error.set(String::new());
                                    #[cfg(target_os = "android")]
                                    let quail_id_clone = quail_id_for_gallery.clone();
                                    #[cfg(target_os = "android")]
                                    let needs_profile_photo = profile()
                                        .as_ref()
                                        .and_then(|p| p.profile_photo.as_ref())
                                        .is_none();
                                    #[cfg(target_os = "android")]
                                    let create_photo_collection_gallery_fn =
                                        create_photo_collection_gallery.clone();
                                    #[cfg(target_os = "android")]
                                    let create_photo_gallery_fn = create_photo_gallery.clone();
                                    #[cfg(target_os = "android")]
                                    let set_quail_photo_gallery_fn = set_quail_photo_gallery.clone();
                                    spawn(async move {
                                        #[cfg(target_os = "android")]
                                        {
                                            match crate::camera::pick_images() {
                                                Ok(paths) => {
                                                    if let Ok(conn) = database::init_database() {
                                                        if let Ok(quail_uuid) = uuid::Uuid::parse_str(&quail_id_clone) {
                                                            // Create or get PhotoCollection for this quail
                                                            let collection_uuid = quail_uuid;
                                                            match crate::services::photo_service::get_or_create_quail_collection(
                                                                &conn,
                                                                &quail_uuid,
                                                            ) {
                                                                Ok(collection_id) => {
                                                                    // Ensure collection exists in Spacetime
                                                                    create_photo_collection_gallery_fn(spacetime::CreatePhotoCollectionArgs {
                                                                        uuid: collection_uuid.to_string(),
                                                                        quail_uuid: Some(quail_uuid.to_string()),
                                                                        event_uuid: None,
                                                                        name: format!("Quail-{}", quail_uuid.to_string().chars().take(8).collect::<String>()),
                                                                    });

                                                                    let mut profile_photo_set = false;
                                                                    for pth in paths {
                                                                        let path_str = pth.to_string_lossy().to_string();
                                                                        match crate::services::photo_service::add_photo_to_collection(
                                                                                &conn,
                                                                                &collection_id,
                                                                                path_str.clone(),
                                                                            )
                                                                            .await
                                                                        {
                                                                            Ok(photo_uuid) => {
                                                                                // Register photo in Spacetime
                                                                                create_photo_gallery_fn(spacetime::CreatePhotoArgs {
                                                                                    uuid: photo_uuid.to_string(),
                                                                                    collection_uuid: collection_uuid.to_string(),
                                                                                    relative_path: path_str,
                                                                                });

                                                                                if needs_profile_photo && !profile_photo_set {
                                                                                    // Update SpacetimeDB
                                                                                    set_quail_photo_gallery_fn(
                                                                                        quail_uuid.to_string(),
                                                                                        Some(photo_uuid.to_string()),
                                                                                    );
                                                                                    // Update local DB for UI compatibility
                                                                                    if let Err(e) = crate::services::photo_service::set_profile_photo(&conn, &quail_uuid, &photo_uuid).await {
                                                                                        log::warn!("Failed to update local profile photo: {}", e);
                                                                                    }
                                                                                    profile_photo_set = true;
                                                                                }
                                                                            }
                                                                            Err(e) => {
                                                                                upload_error.set(format!("Fehler beim Speichern: {}", e));
                                                                                break;
                                                                            }
                                                                        }
                                                                    }
                                                                    // Update preview collection UUID from Spacetime table
                                                                    if let Some(preview_uuid) = photo_collections.read().iter().find(|c| c.uuid == collection_uuid.to_string()).and_then(|c| c.preview_photo_uuid.clone()) {
                                                                        if let Ok(uuid) = uuid::Uuid::parse_str(&preview_uuid) {
                                                                            quail_photo_collection_uuid.set(Some(uuid));
                                                                        }
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    upload_error
                                                                        .set(format!("Fehler beim Erstellen der Sammlung: {}", e));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    upload_error
                                                        .set(format!("{}: {}", t!("error-selection-failed"), e))
                                                }
                                            }
                                        }
                                        #[cfg(not(target_os = "android"))]
                                        {
                                            upload_error.set(t!("error-multiselect-android-only"));
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
                        // Kamera (Einzelfoto)
                        button {
                            style: "position:absolute; bottom:12px; right:12px; padding:10px 14px; background:rgba(0,0,0,0.45); color:white; backdrop-filter:blur(4px); border-radius:8px; font-size:14px; display:flex; align-items:center; gap:6px; cursor:pointer; z-index:11;",
                            disabled: uploading(),
                            onclick: {
                                move |e| {
                                    e.stop_propagation();
                                    uploading.set(true);
                                    upload_error.set(String::new());
                                    #[cfg(target_os = "android")]
                                    let quail_id_clone = quail_id_for_camera.clone();
                                    #[cfg(target_os = "android")]
                                    let needs_profile_photo = profile()
                                        .as_ref()
                                        .and_then(|p| p.profile_photo.as_ref())
                                        .is_none();
                                    #[cfg(target_os = "android")]
                                    let create_photo_collection_camera_fn =
                                        create_photo_collection_camera.clone();
                                    #[cfg(target_os = "android")]
                                    let create_photo_camera_fn = create_photo_camera.clone();
                                    #[cfg(target_os = "android")]
                                    let set_quail_photo_camera_fn = set_quail_photo_camera.clone();
                                    spawn(async move {
                                        #[cfg(target_os = "android")]
                                        {
                                            match crate::camera::capture_photo() {
                                                Ok(path) => {
                                                    if let Ok(conn) = database::init_database() {
                                                        let path_str = path.to_string_lossy().to_string();
                                                        if let Ok(quail_uuid) = uuid::Uuid::parse_str(&quail_id_clone) {
                                                            // Create or get PhotoCollection for this quail
                                                            let collection_uuid = quail_uuid;
                                                            match crate::services::photo_service::get_or_create_quail_collection(
                                                                &conn,
                                                                &quail_uuid,
                                                            ) {
                                                                Ok(collection_id) => {
                                                                    // Ensure collection exists in Spacetime
                                                                    create_photo_collection_camera_fn(spacetime::CreatePhotoCollectionArgs {
                                                                        uuid: collection_uuid.to_string(),
                                                                        quail_uuid: Some(quail_uuid.to_string()),
                                                                        event_uuid: None,
                                                                        name: format!("Quail-{}", quail_uuid.to_string().chars().take(8).collect::<String>()),
                                                                    });

                                                                    match crate::services::photo_service::add_photo_to_collection(
                                                                            &conn,
                                                                            &collection_id,
                                                                            path_str.clone(),
                                                                        )
                                                                        .await
                                                                    {
                                                                        Ok(photo_uuid) => {
                                                                            // Register photo in Spacetime
                                                                            create_photo_camera_fn(spacetime::CreatePhotoArgs {
                                                                                uuid: photo_uuid.to_string(),
                                                                                collection_uuid: collection_uuid.to_string(),
                                                                                relative_path: path_str,
                                                                            });

                                                                            if needs_profile_photo {
                                                                                // Update SpacetimeDB
                                                                                set_quail_photo_camera_fn(
                                                                                    quail_uuid.to_string(),
                                                                                    Some(photo_uuid.to_string()),
                                                                                );
                                                                                // Update local DB for UI compatibility
                                                                                if let Err(e) = crate::services::photo_service::set_profile_photo(&conn, &quail_uuid, &photo_uuid).await {
                                                                                    log::warn!("Failed to update local profile photo: {}", e);
                                                                                }
                                                                            }

                                                                            // Update preview collection UUID from Spacetime table
                                                                            if let Some(preview_uuid) = photo_collections.read().iter().find(|c| c.uuid == collection_uuid.to_string()).and_then(|c| c.preview_photo_uuid.clone()) {
                                                                                if let Ok(uuid) = uuid::Uuid::parse_str(&preview_uuid) {
                                                                                    quail_photo_collection_uuid.set(Some(uuid));
                                                                                }
                                                                            }
                                                                        }
                                                                        Err(e) => {
                                                                            upload_error
                                                                                .set(format!("{}: {}", t!("error-save-failed"), e))
                                                                        }
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    upload_error
                                                                        .set(format!("{}: {}", t!("error-collection-failed"), e))
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    upload_error
                                                        .set(format!("{}: {}", t!("error-capture-failed"), e))
                                                }
                                            }
                                        }
                                        #[cfg(not(target_os = "android"))]
                                        {
                                            upload_error.set(t!("error-camera-android-only"));
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

                    // Upload Error anzeigen falls vorhanden
                    if !upload_error().is_empty() {
                        div { style: "padding: 12px; background: #ffe6e6; border-radius: 8px; color: #cc0000; font-size: 14px; margin-top: 12px;",
                            "⚠️ "
                            {upload_error}
                        }
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
                                            {t!("status-born")}
                                        }
                                    },
                                    EventType::Alive => rsx! {
                                        span { style: "padding:6px 14px; background:#e0ffe6; border-radius:16px; font-size:13px; color:#228833;",
                                            "✅ "
                                            {t!("status-alive")}
                                        }
                                    },
                                    EventType::Sick => rsx! {
                                        span { style: "padding:6px 14px; background:#ffe0e0; border-radius:16px; font-size:13px; color:#cc3333;",
                                            "🤒 "
                                            {t!("status-sick")}
                                        }
                                    },
                                    EventType::Healthy => rsx! {
                                        span { style: "padding:6px 14px; background:#e0ffe6; border-radius:16px; font-size:13px; color:#228833;",
                                            "💪 "
                                            {t!("status-healthy")}
                                        }
                                    },
                                    EventType::MarkedForSlaughter => {
                                        rsx! {
                                            span { style: "padding:6px 14px; background:#fff3e0; border-radius:16px; font-size:13px; color:#ff8800;",
                                                "🥩 "
                                                {t!("status-marked")}
                                            }
                                        }
                                    }
                                    EventType::Slaughtered => rsx! {
                                        span { style: "padding:6px 14px; background:#f0f0f0; border-radius:16px; font-size:13px; color:#666;",
                                            "🥩 "
                                            {t!("status-slaughtered")}
                                        }
                                    },
                                    EventType::Died => rsx! {
                                        span { style: "padding:6px 14px; background:#f0f0f0; border-radius:16px; font-size:13px; color:#666;",
                                            "🪦 "
                                            {t!("status-died")}
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

                    // Events Timeline
                    div { style: "margin-top:24px;",
                        div { style: "display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;",
                            h3 { style: "margin:0; font-size:18px; color:#333; font-weight:600;",
                                "📅 "
                                {t!("events-timeline-title")}
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
                                {t!("action-add-event")} // Add event
                            }
                        }

                        if events().is_empty() {
                            div {
                                style: "padding:24px; text-align:center; background:#f5f5f5; border-radius:8px; color:#999;",
                                {t!("events-empty")} // No events available
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
                        {t!("action-edit")}
                    }
                }
            } else {
                div { style: "padding:48px; text-align:center;",
                    div { style: "font-size:48px; margin-bottom:16px;", "⏳" }
                    div { style: "color:#666;", {t!("loading-profile")} } // Loading profile...
                }
            }

            // Vollbild-Galerie Overlay
            if show_fullscreen() && !fullscreen_uuids().is_empty() {
                CollectionFullscreen {
                    photo_uuids: fullscreen_uuids(),
                    initial_index: current_photo_index(),
                    on_close: move |_| show_fullscreen.set(false),
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
