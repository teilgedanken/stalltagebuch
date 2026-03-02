use crate::spacetime;
use dioxus::prelude::*;
use dioxus_i18n::t;
use photo_gallery::CollectionFullscreen;

#[component]
pub fn ProfilePhotoCard(quail_id: String, profile_photo: Option<String>) -> Element {
    let create_photo_collection = spacetime::use_reducer_create_photo_collection();
    let create_photo = spacetime::use_reducer_create_photo();
    let set_quail_photo = spacetime::use_reducer_set_quail_photo();
    let photos_table = spacetime::use_table_photos();

    // relative_paths for fullscreen viewer (filled when opening fullscreen)
    let fullscreen_paths = use_signal(|| Vec::<String>::new());
    let current_photo_index = use_signal(|| 0usize);
    let mut show_fullscreen = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(|| String::new());
    let has_profile_photo = profile_photo.is_some();
    let quail_id_open = quail_id.clone();
    let photos = photos_table.read().clone();

    // Note: Photo download retry is now handled via SpacetimeDB subscriptions
    use_effect(move || {
        log::debug!("ProfilePhotoCard mounted - photos will sync via SpacetimeDB subscriptions");
    });

    rsx! {
        div { style: "width: 100%; aspect-ratio: 1/1; background: #f0f0f0; border-radius: 12px; overflow: hidden; display: flex; align-items: center; justify-content: center; position: relative;",
            // Hauptbild (klickbar fuer Galerie)
            div {
                style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; cursor: pointer;",
                onclick: move |_| {
                    let mut fullscreen_paths_sig = fullscreen_paths.clone();
                    let mut current_idx_sig = current_photo_index.clone();
                    let mut show_fullscreen_sig = show_fullscreen.clone();
                    let quail_id_open = quail_id_open.clone();
                    let photos_clone = photos.clone();
                    spawn(async move {
                        // Filter photos for this quail's collection
                        let paths: Vec<String> = photos_clone
                            .iter()
                            .filter(|p| p.collection_uuid == quail_id_open)
                            .map(|p| p.relative_path.clone())
                            .collect();
                        if !paths.is_empty() {
                            log::debug!(
                                "ProfilePhotoCard: opening fullscreen with {} photos",
                                paths.len()
                            );
                            fullscreen_paths_sig.set(paths.clone());
                            current_idx_sig.set(0);
                            show_fullscreen_sig.set(true);
                        }
                    });
                },
                // Display photo from SpacetimeDB directly
                if let Some(photo_uuid_str) = profile_photo.as_ref() {
                    // Find the photo in the table to get its relative_path
                    if let Some(photo) = photos.iter().find(|p| p.uuid == *photo_uuid_str) {
                        photo_gallery::PreviewImage {
                            relative_path: photo.relative_path.clone(),
                            alt: "Profile Photo".to_string(),
                        }
                    } else {
                        div { style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 64px; color: #ccc;",
                            "🐦"
                        }
                    }
                } else {
                    div { style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 64px; color: #ccc;",
                        "🐦"
                    }
                }
            }

            // Galerie (Mehrfachauswahl)
            button {
                style: "position:absolute; bottom:12px; left:12px; padding:10px 14px; background:rgba(0,0,0,0.45); color:white; backdrop-filter:blur(4px); border-radius:8px; font-size:14px; display:flex; align-items:center; gap:6px; cursor:pointer; z-index:11;",
                disabled: uploading(),
                onclick: {
                    let quail_id_for_gallery = quail_id.clone();
                    let create_photo_collection_gallery = create_photo_collection.clone();
                    let create_photo_gallery = create_photo.clone();
                    let set_quail_photo_gallery = set_quail_photo.clone();
                    let needs_profile_photo = !has_profile_photo;
                    move |e| {
                        e.stop_propagation();
                        uploading.set(true);
                        upload_error.set(String::new());

                        let quail_id_clone = quail_id_for_gallery.clone();
                        let create_photo_collection_gallery_fn = create_photo_collection_gallery
                            .clone();
                        let create_photo_gallery_fn = create_photo_gallery.clone();
                        let set_quail_photo_gallery_fn = set_quail_photo_gallery.clone();
                        spawn(async move {
                            #[cfg(target_os = "android")]
                            {
                                match crate::camera::pick_images() {
                                    Ok(paths) => {
                                        if let Ok(quail_uuid) = uuid::Uuid::parse_str(&quail_id_clone) {
                                            let collection_uuid = quail_uuid;
                                            create_photo_collection_gallery_fn(
                                                spacetime::CreatePhotoCollectionArgs {
                                                    uuid: collection_uuid.to_string(),
                                                    quail_uuid: Some(quail_uuid.to_string()),
                                                    event_uuid: None,
                                                    name: format!(
                                                        "Quail-{}",
                                                        quail_uuid
                                                            .to_string()
                                                            .chars()
                                                            .take(8)
                                                            .collect::<String>(),
                                                    ),
                                                },
                                            );
                                            let mut profile_photo_set = false;
                                            for pth in paths {
                                                let path_str = pth.to_string_lossy().to_string();
                                                let photo_uuid = uuid::Uuid::new_v4().to_string();
                                                create_photo_gallery_fn(spacetime::CreatePhotoArgs {
                                                    uuid: photo_uuid.to_string(),
                                                    collection_uuid: collection_uuid.to_string(),
                                                    relative_path: path_str,
                                                });
                                                if needs_profile_photo && !profile_photo_set {
                                                    set_quail_photo_gallery_fn(
                                                        quail_uuid.to_string(),
                                                        Some(photo_uuid.to_string()),
                                                    );
                                                    profile_photo_set = true;
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        upload_error.set(format!(
                                            "{}: {}",
                                            t!("error-selection-failed"),
                                            e
                                        ));
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
                    let quail_id_for_camera = quail_id.clone();
                    let create_photo_collection_camera = create_photo_collection.clone();
                    let create_photo_camera = create_photo.clone();
                    let set_quail_photo_camera = set_quail_photo.clone();
                    let needs_profile_photo = !has_profile_photo;
                    move |e| {
                        e.stop_propagation();
                        uploading.set(true);
                        upload_error.set(String::new());

                        let quail_id_clone = quail_id_for_camera.clone();
                        let create_photo_collection_camera_fn = create_photo_collection_camera
                            .clone();
                        let create_photo_camera_fn = create_photo_camera.clone();
                        let set_quail_photo_camera_fn = set_quail_photo_camera.clone();
                        spawn(async move {
                            #[cfg(target_os = "android")]
                            {
                                match crate::camera::capture_photo() {
                                    Ok(path) => {
                                        let path_str = path.to_string_lossy().to_string();
                                        if let Ok(quail_uuid) = uuid::Uuid::parse_str(&quail_id_clone) {
                                            let collection_uuid = quail_uuid;
                                            create_photo_collection_camera_fn(
                                                spacetime::CreatePhotoCollectionArgs {
                                                    uuid: collection_uuid.to_string(),
                                                    quail_uuid: Some(quail_uuid.to_string()),
                                                    event_uuid: None,
                                                    name: format!(
                                                        "Quail-{}",
                                                        quail_uuid
                                                            .to_string()
                                                            .chars()
                                                            .take(8)
                                                            .collect::<String>(),
                                                    ),
                                                },
                                            );
                                            let photo_uuid = uuid::Uuid::new_v4().to_string();
                                            create_photo_camera_fn(spacetime::CreatePhotoArgs {
                                                uuid: photo_uuid.to_string(),
                                                collection_uuid: collection_uuid.to_string(),
                                                relative_path: path_str,
                                            });
                                            if needs_profile_photo {
                                                set_quail_photo_camera_fn(
                                                    quail_uuid.to_string(),
                                                    Some(photo_uuid.to_string()),
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        upload_error.set(format!(
                                            "{}: {}",
                                            t!("error-capture-failed"),
                                            e
                                        ));
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

        if !upload_error().is_empty() {
            div { style: "padding: 12px; background: #ffe6e6; border-radius: 8px; color: #cc0000; font-size: 14px; margin-top: 12px;",
                "⚠️ "
                {upload_error}
            }
        }

        if show_fullscreen() && !fullscreen_paths().is_empty() {
            CollectionFullscreen {
                photo_paths: fullscreen_paths(),
                initial_index: current_photo_index(),
                on_close: move |_| show_fullscreen.set(false),
            }
        }
    }
}
