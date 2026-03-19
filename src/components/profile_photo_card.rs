use crate::spacetime;
use dioxus::prelude::*;
use dioxus_i18n::tid;
use photo_gallery::CollectionFullscreen;

#[component]
pub fn ProfilePhotoCard(quail_id: String, profile_photo: Option<String>) -> Element {
    let create_photo_collection = spacetime::use_reducer_create_photo_collection();
    let create_photo = spacetime::use_reducer_create_photo();
    let set_quail_photo = spacetime::use_reducer_set_quail_photo();
    let photos_table = spacetime::use_table_photos();
    let photo_collections_table = spacetime::use_table_photo_collections();

    let fullscreen_paths = use_signal(Vec::<String>::new);
    let current_photo_index = use_signal(|| 0usize);
    let mut show_fullscreen = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut upload_error = use_signal(String::new);

    let quail_id_open = quail_id.clone();

    // Clone profile_photo for use in memo (need separate clone for RSX)
    let profile_photo_for_lookup = profile_photo.clone();

    // Dynamically look up photo path from profile_photo UUID
    // This ensures we get the photo even if the photo path changes
    let effective_photo_path = use_memo(move || {
        if let Some(uuid) = &profile_photo_for_lookup {
            photos_table()
                .iter()
                .find(|p| p.uuid == *uuid)
                .map(|p| p.relative_path.clone())
        } else {
            None
        }
    });

    rsx! {
        div { style: "width: 100%; aspect-ratio: 1/1; background: #f0f0f0; border-radius: 12px; overflow: hidden; display: flex; align-items: center; justify-content: center; position: relative;",
            div {
                style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; cursor: pointer;",
                onclick: move |_| {
                    let mut fullscreen_paths_sig = fullscreen_paths.clone();
                    let mut current_idx_sig = current_photo_index.clone();
                    let mut show_fullscreen_sig = show_fullscreen.clone();
                    let quail_id_open = quail_id_open.clone();

                    spawn(async move {
                        // Get all PhotoCollections for this quail
                        let collection_ids: Vec<String> = photo_collections_table()
                            .iter()
                            .filter(|coll| coll.quail_uuid.as_ref() == Some(&quail_id_open))
                            .map(|coll| coll.uuid.clone())
                            .collect();

                        // Get all photos from those collections
                        let paths: Vec<String> = photos_table()
                            .iter()
                            .filter(|p| collection_ids.contains(&p.collection_uuid))
                            .map(|p| p.relative_path.clone())
                            .collect();

                        if !paths.is_empty() {
                            fullscreen_paths_sig.set(paths);
                            current_idx_sig.set(0);
                            show_fullscreen_sig.set(true);
                        }
                    });
                },
                if let Some(_photo_uuid_str) = profile_photo.as_ref() {
                    if let Some(photo_path) = effective_photo_path() {
                        photo_gallery::PreviewImage {
                            key: "{photo_path}",
                            relative_path: photo_path,
                            alt: "Profile Photo".to_string(),
                        }
                    } else {
                        div { style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 64px; color: #ccc;", "🐦" }
                    }
                } else {
                    div { style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; font-size: 64px; color: #ccc;", "🐦" }
                }
            }

            button {
                style: "position:absolute; bottom:12px; left:12px; padding:10px 14px; background:rgba(0,0,0,0.45); color:white; backdrop-filter:blur(4px); border-radius:8px; font-size:14px; display:flex; align-items:center; gap:6px; cursor:pointer; z-index:11;",
                disabled: uploading(),
                onclick: {
                    let _quail_id_for_gallery = quail_id.clone();
                    let _create_photo_collection_gallery = create_photo_collection.clone();
                    let _create_photo_gallery = create_photo.clone();
                    let _set_quail_photo_gallery = set_quail_photo.clone();
                    let _has_profile_photo = profile_photo.is_some();

                    move |e| {
                        e.stop_propagation();
                        uploading.set(true);
                        upload_error.set(String::new());

                        let quail_id_for_gallery = _quail_id_for_gallery.clone();
                        let create_photo_collection_gallery = _create_photo_collection_gallery.clone();
                        let create_photo_gallery = _create_photo_gallery.clone();
                        let set_quail_photo_gallery = _set_quail_photo_gallery.clone();
                        let has_profile_photo = _has_profile_photo;

                        spawn(async move {
                            #[cfg(not(target_os = "android"))]
                            let _ = (
                                &quail_id_for_gallery,
                                &create_photo_collection_gallery,
                                &create_photo_gallery,
                                &set_quail_photo_gallery,
                                has_profile_photo,
                            );

                            #[cfg(target_os = "android")]
                            {
                                let device_id = crate::services::device_id_service::get_device_id()
                                    .unwrap_or_else(|_| "unknown-device".to_string());
                                let needs_profile_photo = !has_profile_photo;

                                match crate::camera::pick_images() {
                                    Ok(paths) => {
                                        if let Ok(quail_uuid) = uuid::Uuid::parse_str(&quail_id_for_gallery) {
                                            let collection_uuid = quail_uuid;
                                            create_photo_collection_gallery(spacetime::CreatePhotoCollectionArgs {
                                                uuid: collection_uuid.to_string(),
                                                quail_uuid: Some(quail_uuid.to_string()),
                                                event_uuid: None,
                                                name: format!(
                                                    "Quail-{}",
                                                    quail_uuid.to_string().chars().take(8).collect::<String>()
                                                ),
                                                device_id: device_id.clone(),
                                            });

                                            let mut profile_photo_set = false;
                                            for picked_path in paths {
                                                let source = picked_path.to_string_lossy().to_string();
                                                match crate::services::photo_service::process_photo(source).await {
                                                    Ok((relative_original, _, _)) => {
                                                        if let Some(photo_uuid) = std::path::Path::new(&relative_original)
                                                            .file_stem()
                                                            .and_then(|s| s.to_str())
                                                        {
                                                            create_photo_gallery(spacetime::CreatePhotoArgs {
                                                                uuid: photo_uuid.to_string(),
                                                                collection_uuid: collection_uuid.to_string(),
                                                                relative_path: relative_original.clone(),
                                                                device_id: device_id.clone(),
                                                            });

                                                            if needs_profile_photo && !profile_photo_set {
                                                                set_quail_photo_gallery(
                                                                    quail_uuid.to_string(),
                                                                    Some(photo_uuid.to_string()),
                                                                );
                                                                profile_photo_set = true;
                                                            }
                                                        }
                                                    }
                                                    Err(err) => {
                                                        upload_error.set(format!(
                                                            "{}: {}",
                                                            tid!("error-selection-failed"),
                                                            err
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        upload_error.set(format!("{}: {}", tid!("error-selection-failed"), e));
                                    }
                                }
                            }
                            #[cfg(not(target_os = "android"))]
                            {
                                upload_error.set(tid!("error-multiselect-android-only"));
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

            button {
                style: "position:absolute; bottom:12px; right:12px; padding:10px 14px; background:rgba(0,0,0,0.45); color:white; backdrop-filter:blur(4px); border-radius:8px; font-size:14px; display:flex; align-items:center; gap:6px; cursor:pointer; z-index:11;",
                disabled: uploading(),
                onclick: {
                    let _quail_id_for_camera = quail_id.clone();
                    let _create_photo_collection_camera = create_photo_collection.clone();
                    let _create_photo_camera = create_photo.clone();
                    let _set_quail_photo_camera = set_quail_photo.clone();
                    let _has_profile_photo = profile_photo.is_some();

                    move |e| {
                        e.stop_propagation();
                        uploading.set(true);
                        upload_error.set(String::new());

                        let quail_id_for_camera = _quail_id_for_camera.clone();
                        let create_photo_collection_camera = _create_photo_collection_camera.clone();
                        let create_photo_camera = _create_photo_camera.clone();
                        let set_quail_photo_camera = _set_quail_photo_camera.clone();
                        let has_profile_photo = _has_profile_photo;

                        spawn(async move {
                            #[cfg(not(target_os = "android"))]
                            let _ = (
                                &quail_id_for_camera,
                                &create_photo_collection_camera,
                                &create_photo_camera,
                                &set_quail_photo_camera,
                                has_profile_photo,
                            );

                            #[cfg(target_os = "android")]
                            {
                                let device_id = crate::services::device_id_service::get_device_id()
                                    .unwrap_or_else(|_| "unknown-device".to_string());
                                let needs_profile_photo = !has_profile_photo;

                                match crate::camera::capture_photo() {
                                    Ok(path) => {
                                        let source = path.to_string_lossy().to_string();
                                        if let Ok(quail_uuid) = uuid::Uuid::parse_str(&quail_id_for_camera) {
                                            let collection_uuid = quail_uuid;
                                            create_photo_collection_camera(spacetime::CreatePhotoCollectionArgs {
                                                uuid: collection_uuid.to_string(),
                                                quail_uuid: Some(quail_uuid.to_string()),
                                                event_uuid: None,
                                                name: format!(
                                                    "Quail-{}",
                                                    quail_uuid.to_string().chars().take(8).collect::<String>()
                                                ),
                                                device_id: device_id.clone(),
                                            });

                                            match crate::services::photo_service::process_photo(source).await {
                                                Ok((relative_original, _, _)) => {
                                                    if let Some(photo_uuid) = std::path::Path::new(&relative_original)
                                                        .file_stem()
                                                        .and_then(|s| s.to_str())
                                                    {
                                                        create_photo_camera(spacetime::CreatePhotoArgs {
                                                            uuid: photo_uuid.to_string(),
                                                            collection_uuid: collection_uuid.to_string(),
                                                            relative_path: relative_original.clone(),
                                                            device_id,
                                                        });

                                                        if needs_profile_photo {
                                                            set_quail_photo_camera(
                                                                quail_uuid.to_string(),
                                                                Some(photo_uuid.to_string()),
                                                            );
                                                        }
                                                    }
                                                }
                                                Err(err) => {
                                                    upload_error.set(format!(
                                                        "{}: {}",
                                                        tid!("error-capture-failed"),
                                                        err
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        upload_error.set(format!("{}: {}", tid!("error-capture-failed"), e));
                                    }
                                }
                            }
                            #[cfg(not(target_os = "android"))]
                            {
                                upload_error.set(tid!("error-camera-android-only"));
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

        if !upload_error().is_empty() {
            div { style: "padding: 12px; background: #ffe6e6; border-radius: 8px; color: #cc0000; font-size: 14px; margin-top: 12px;",
                "⚠️ "
                {upload_error()}
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
