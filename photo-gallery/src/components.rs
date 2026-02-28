//! Dioxus UI components for photo gallery
//!
//! This module provides reusable photo display components.
//! Components can either accept pre-loaded data URLs or load their own data
//! from the database using photo UUIDs.

#[cfg(feature = "components")]
use dioxus::prelude::*;

#[cfg(feature = "components")]
use rusqlite::Connection;
#[cfg(feature = "components")]
use uuid::Uuid;

#[cfg(feature = "components")]
use std::path::PathBuf;

#[cfg(feature = "components")]
/// Configuration for photo gallery components
#[derive(Clone)]
pub struct PhotoGalleryContext {
    pub storage_path: String,
}

#[cfg(feature = "components")]
impl PhotoGalleryContext {
    pub fn new(storage_path: String) -> Self {
        Self { storage_path }
    }

    /// Load photo data from storage and convert to data URL
    fn load_photo_data(&self, relative_path: &str, size: PhotoSize) -> Option<String> {
        use base64::{engine::general_purpose, Engine as _};

        let abs_path = if relative_path.starts_with('/') {
            PathBuf::from(relative_path)
        } else {
            PathBuf::from(&self.storage_path).join(relative_path)
        };

        let file_path = match size {
            PhotoSize::Original => abs_path,
            PhotoSize::Small => {
                let mut thumb_path = abs_path.clone();
                if let Some(name) = abs_path.file_stem() {
                    thumb_path.set_file_name(format!("{}_small.webp", name.to_string_lossy()));
                }
                thumb_path
            }
            PhotoSize::Medium => {
                let mut thumb_path = abs_path.clone();
                if let Some(name) = abs_path.file_stem() {
                    thumb_path.set_file_name(format!("{}_medium.webp", name.to_string_lossy()));
                }
                thumb_path
            }
        };

        log::debug!(
            "load_photo_data: file_path={:?} exists={}",
            &file_path,
            file_path.exists()
        );

        if file_path.exists() {
            match std::fs::read(&file_path) {
                Ok(bytes) => {
                    let mime_type =
                        if file_path.extension().and_then(|s| s.to_str()) == Some("webp") {
                            "image/webp"
                        } else {
                            "image/jpeg"
                        };
                    let encoded = general_purpose::STANDARD.encode(&bytes);
                    return Some(format!("data:{};base64,{}", mime_type, encoded));
                }
                Err(e) => {
                    log::warn!(
                        "load_photo_data: failed to read {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }
        }
        None
    }
}

#[cfg(feature = "components")]
#[derive(Debug, Clone, Copy)]
enum PhotoSize {
    Original,
    Small,
    Medium,
}

#[cfg(feature = "components")]
#[derive(Debug, Clone)]
enum ImageLoadState {
    Loading,
    Loaded(String),
    Failed,
}

#[cfg(feature = "components")]
/// Helper function to query photo path from database
/// This is meant to be called by the parent component before rendering
pub fn get_photo_path(conn: &Connection, photo_uuid: &Uuid) -> Result<String, String> {
    conn.query_row(
        "SELECT COALESCE(relative_path, path) FROM photos WHERE uuid = ?1 AND deleted = 0",
        [photo_uuid.to_string()],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to load photo {}: {}", photo_uuid, e))
}

#[cfg(feature = "components")]
// Internal helper: try to load a photo data-url for `photo_uuid` and `size`.
// If the thumbnail (or requested size) isn't available locally and sync is enabled
// this will spawn a background download attempt and — after a successful download
// — set the provided `state` to Loaded so the component updates.
fn load_or_download_photo(
    ctx: PhotoGalleryContext,
    photo_uuid: uuid::Uuid,
    size: PhotoSize,
    state: Option<Signal<ImageLoadState>>,
) -> Option<String> {
    use stalltagebuch_database::connection;

    if let Ok(conn) = connection::init_database() {
        if let Ok(path) = get_photo_path(&conn, &photo_uuid) {
            // path is the DB value (usually a relative filename). Also compute the
            // absolute path we will try to read so we can debug missing-file issues.
            let computed_abs = if path.starts_with('/') {
                std::path::PathBuf::from(&path)
            } else {
                std::path::PathBuf::from(ctx.storage_path.clone()).join(&path)
            };

            log::debug!(
                "load_or_download_photo: db_path={} exists_rel? {} abs_path={} exists_abs? {}",
                path,
                std::path::Path::new(&path).exists(),
                computed_abs.display(),
                computed_abs.exists(),
            );
            if let Some(data) = ctx.load_photo_data(&path, size) {
                return Some(data);
            }

            // Not found locally — schedule a per-photo download (feature gated)
            #[cfg(feature = "sync")]
            {
                use crate::{PhotoSyncConfig, PhotoSyncService};

                let rel = path.clone();
                let storage = ctx.storage_path.clone();

                if let Some(mut state_sig) = state.clone() {
                    // Spawn async background download and try to set state when done.
                    // Wrap the async future in a panic-catcher so panics are logged instead
                    // of bringing down the UI thread.
                    let fut = async move {
                        if let Ok(conn2) = connection::init_database() {
                            // Read sync_settings row (simple 1-row table)
                            let settings_row: Result<(String, String, String, String, i64), _> = conn2.query_row(
                                "SELECT server_url, username, app_password, remote_path, enabled FROM sync_settings LIMIT 1",
                                [],
                                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                            );

                            if let Ok((server_url, username, app_password, remote_path, enabled)) =
                                settings_row
                            {
                                if enabled == 1 {
                                    let cfg = PhotoSyncConfig {
                                        server_url,
                                        username,
                                        app_password,
                                        remote_path,
                                    };
                                    let sync_svc = PhotoSyncService::new(cfg);

                                    let local_path = if rel.starts_with('/') {
                                        rel.clone()
                                    } else {
                                        format!("{}/{}", storage.trim_end_matches('/'), rel)
                                    };

                                    // Download (with retries) and then try to load the requested size
                                    let _ = sync_svc
                                        .download_photo_with_retry(&rel, &local_path, 3)
                                        .await;

                                    if let Some(data) = ctx.load_photo_data(&rel, size) {
                                        state_sig.set(ImageLoadState::Loaded(data));
                                    }
                                }
                            }
                        }
                    }; // end fut
                       // spawn download task (errors are logged by the inner code)
                    spawn(fut);
                }
            }
        }
    }

    None
}

#[cfg(feature = "components")]
/// Helper function to get preview photo path for a collection
pub fn get_collection_preview_path(
    conn: &Connection,
    collection_id: &Uuid,
) -> Result<Option<String>, String> {
    // Get preview photo UUID from collection

    use std::str::FromStr as _;

    let preview_uuid: Option<String> = conn
        .query_row(
            "SELECT preview_photo_uuid FROM photo_collections WHERE uuid = ?1",
            [collection_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to load collection {}: {}", collection_id, e))?;
    let preview_uuid = preview_uuid
        .as_deref()
        .map(uuid::Uuid::from_str)
        .expect("Parsing UUID failed");

    if let Some(uuid) = preview_uuid.ok() {
        get_photo_path(conn, &uuid).map(Some)
    } else {
        // No preview set, try to get first photo in collection
        let first_photo: Option<String> = conn
            .query_row(
                "SELECT COALESCE(relative_path, path) FROM photos 
                 WHERE collection_id = ?1 AND deleted = 0 
                 ORDER BY created_at ASC LIMIT 1",
                [collection_id.to_string()],
                |row| row.get(0),
            )
            .ok();
        Ok(first_photo)
    }
}

#[cfg(feature = "components")]
/// Helper function to get preview photo UUID for a collection.
/// Returns the `preview_photo_uuid` if set, otherwise the first photo UUID in the collection.
pub fn get_collection_preview_uuid(
    conn: &Connection,
    collection_id: &uuid::Uuid,
) -> Result<uuid::Uuid, String> {
    // Try explicit preview_uuid

    use std::str::FromStr;
    let preview_uuid: Option<String> = conn
        .query_row(
            "SELECT preview_photo_uuid FROM photo_collections WHERE uuid = ?1",
            [collection_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to load collection {}: {}", collection_id, e))?;
    let preview_uuid = preview_uuid
        .as_deref()
        .map(uuid::Uuid::from_str)
        .expect("Parsing UUID failed");

    if let Ok(uuid) = preview_uuid {
        Ok(uuid)
    } else {
        // No preview set, try to get first photo UUID in collection
        let first_uuid: Option<String> = conn
            .query_row(
                "SELECT uuid FROM photos WHERE collection_id = ?1 AND deleted = 0 ORDER BY created_at ASC LIMIT 1",
                [collection_id.to_string()],
                |row| row.get(0),
            )
            .ok();
        let first_uuid = first_uuid
            .as_deref()
            .map(uuid::Uuid::from_str)
            .expect("Parsing UUID failed");
        first_uuid.map_err(|e| e.to_string())
    }
}

#[cfg(feature = "components")]
/// Helper function to get all photo paths in a collection
pub fn get_collection_photos(
    conn: &Connection,
    collection_id: &str,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(relative_path, path) FROM photos 
             WHERE collection_id = ?1 AND deleted = 0 
             ORDER BY created_at ASC",
        )
        .map_err(|e| format!("Failed to prepare statement: {}", e))?;

    let paths = stmt
        .query_map([collection_id], |row| row.get(0))
        .map_err(|e| format!("Failed to query photos: {}", e))?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| format!("Failed to collect photos: {}", e))?;

    Ok(paths)
}

#[cfg(feature = "components")]
/// Thumbnail image component - displays a small photo
///
/// Can be used in two ways:
/// 1. With pre-loaded data_url (backward compatible)
/// 2. With relative_path (loads from storage)
#[component]
pub fn ThumbnailImage(
    #[props(default = None)] photo_uuid: Option<Uuid>,
    #[props(default = "Photo".to_string())] alt: String,
) -> Element {
    let mut image_state = use_signal(|| ImageLoadState::Loading);
    let context = use_context::<PhotoGalleryContext>();

    // Load photo data: only supported input is photo_uuid now
    use_effect(move || {
        if let Some(uuid) = photo_uuid.clone() {
            // If photo_uuid is provided, attempt to load (or schedule download)
            if let Some(data) = load_or_download_photo(
                context.clone(),
                uuid.clone(),
                PhotoSize::Small,
                Some(image_state.clone()),
            ) {
                image_state.set(ImageLoadState::Loaded(data));
                return;
            }
            image_state.set(ImageLoadState::Failed);
            return;
        } else {
            // no uuid provided -> fail (no longer supports direct paths/data urls)
            image_state.set(ImageLoadState::Failed);
        }
    });

    rsx! {
        div { style: "width: 128px; height: 128px; border-radius: 8px; overflow: hidden; background: #f0f0f0;",
            match image_state() {
                ImageLoadState::Loading => rsx! {
                    div { style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; color: #999;",
                        "⏳"
                    }
                },
                ImageLoadState::Loaded(url) => rsx! {
                    img {
                        src: "{url}",
                        alt: "{alt}",
                        style: "width: 100%; height: 100%; object-fit: cover;",
                    }
                },
                ImageLoadState::Failed => rsx! {
                    div { style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; color: #999;",
                        "📷"
                    }
                },
            }
        }
    }
}

#[cfg(feature = "components")]
/// Preview image component - displays a medium-sized photo
///
/// Can be used with data_url or relative_path.
/// Note: If you have a photo_uuid, resolve it to a relative_path before passing to this component.
#[component]
pub fn PreviewImage(
    #[props(default = None)] photo_uuid: Option<uuid::Uuid>,
    #[props(default = "Photo".to_string())] alt: String,
) -> Element {
    let mut image_state = use_signal(|| ImageLoadState::Loading);
    let context = use_context::<PhotoGalleryContext>();

    use_effect(move || {
        if let Some(uuid) = photo_uuid.clone() {
            if let Some(data) = load_or_download_photo(
                context.clone(),
                uuid.clone(),
                PhotoSize::Medium,
                Some(image_state.clone()),
            ) {
                image_state.set(ImageLoadState::Loaded(data));
                return;
            }
        }
        // No uuid or failed to load -> mark failed
        image_state.set(ImageLoadState::Failed);
    });

    rsx! {
        div { style: "max-width: 512px; max-height: 512px; border-radius: 8px; overflow: hidden; background: #f0f0f0;",
            match image_state() {
                ImageLoadState::Loading => rsx! {
                    div { style: "width: 100%; height: 400px; display: flex; align-items: center; justify-content: center; color: #999;",
                        "⏳"
                    }
                },
                ImageLoadState::Loaded(url) => rsx! {
                    img {
                        src: "{url}",
                        alt: "{alt}",
                        style: "max-width: 100%; max-height: 100%; object-fit: contain;",
                    }
                },
                ImageLoadState::Failed => rsx! {
                    div { style: "width: 100%; height: 400px; display: flex; align-items: center; justify-content: center; color: #999;",
                        "📷"
                    }
                },
            }
        }
    }
}

#[cfg(feature = "components")]
/// Fullscreen image component - displays a single photo in fullscreen with close button
///
/// Can be used with data_url or relative_path
#[component]
pub fn FullscreenImage(
    #[props(default = None)] photo_uuid: Option<Uuid>,
    on_close: EventHandler<()>,
) -> Element {
    let mut image_state = use_signal(|| ImageLoadState::Loading);
    let context = use_context::<PhotoGalleryContext>();

    use_effect(move || {
        if let Some(uuid) = photo_uuid.clone() {
            if let Some(data) = load_or_download_photo(
                context.clone(),
                uuid.clone(),
                PhotoSize::Original,
                Some(image_state.clone()),
            ) {
                image_state.set(ImageLoadState::Loaded(data));
                return;
            }
        }
        image_state.set(ImageLoadState::Failed);
    });

    rsx! {
        div { style: "position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0, 0, 0, 0.95); z-index: 1000; display: flex; flex-direction: column;",
            div { style: "display: flex; justify-content: flex-end; padding: 16px; background: rgba(0, 0, 0, 0.7);",
                button {
                    style: "width: 40px; height: 40px; background: rgba(255, 255, 255, 0.2); color: white; border-radius: 50%; font-size: 24px; cursor: pointer; border: none;",
                    onclick: move |_| on_close.call(()),
                    "×"
                }
            }
            div { style: "flex: 1; display: flex; align-items: center; justify-content: center; padding: 20px;",
                match image_state() {
                    ImageLoadState::Loading => rsx! {
                        div { style: "color: white; font-size: 48px;", "⏳" }
                    },
                    ImageLoadState::Loaded(url) => rsx! {
                        img {
                            src: "{url}",
                            style: "max-width: 100%; max-height: 100%; object-fit: contain;",
                        }
                    },
                    ImageLoadState::Failed => rsx! {
                        div { style: "color: white; font-size: 48px;", "📷" }
                    },
                }
            }
        }
    }
}

#[cfg(feature = "components")]
/// Thumbnail collection component
///
/// Can be used with preview_data_url, preview_relative_path, or neither (shows placeholder)
#[component]
pub fn ThumbnailCollection(
    #[props(default = None)] preview_photo_uuid: Option<Uuid>,
    #[props(default = None)] on_click: Option<EventHandler<()>>,
) -> Element {
    rsx! {
        div {
            style: "width: 128px; height: 128px; cursor: pointer;",
            onclick: move |_| {
                if let Some(handler) = &on_click {
                    handler.call(());
                }
            },
            if preview_photo_uuid.is_some() {
                ThumbnailImage {
                    photo_uuid: preview_photo_uuid.clone(),
                    alt: "Collection preview".to_string(),
                }
            } else {
                div { style: "width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; background: #f0f0f0; border-radius: 8px; color: #999;",
                    "📷"
                }
            }
        }
    }
}

#[cfg(feature = "components")]
/// Preview collection component
///
/// Can be used with preview_data_url, preview_relative_path, or neither (shows placeholder)
#[component]
pub fn PreviewCollection(
    #[props(default = None)] preview_collection_uuid: Option<uuid::Uuid>,
    #[props(default = None)] on_click: Option<EventHandler<()>>,
) -> Element {
    let preview_photo_uuid = if let Some(collection_uuid) = preview_collection_uuid {
        // Try to get preview photo UUID from collection
        if let Ok(conn) = stalltagebuch_database::connection::init_database() {
            get_collection_preview_uuid(&conn, &collection_uuid).ok()
        } else {
            None
        }
    } else {
        None
    };
    rsx! {
        div {
            style: "max-width: 512px; cursor: pointer;",
            onclick: move |_| {
                if let Some(handler) = &on_click {
                    handler.call(());
                }
            },
            if preview_photo_uuid.is_some() {
                PreviewImage {
                    photo_uuid: preview_photo_uuid.clone(),
                    alt: "Collection preview".to_string(),
                }
            } else {
                div { style: "width: 100%; height: 400px; display: flex; align-items: center; justify-content: center; background: #f0f0f0; border-radius: 8px; color: #999;",
                    "📷"
                }
            }
        }
    }
}

#[cfg(feature = "components")]
/// Fullscreen collection viewer
///
/// Can be used with photo_data_urls or photo_relative_paths
#[component]
pub fn CollectionFullscreen(
    #[props(default = vec![])] photo_uuids: Vec<Uuid>,
    #[props(default = 0)] initial_index: usize,
    on_close: EventHandler<()>,
) -> Element {
    let mut current_index = use_signal(|| initial_index);
    let mut loaded_urls = use_signal(|| vec![]);
    let context = use_context::<PhotoGalleryContext>();

    // Load photos from UUIDs by resolving them through the DB and reading files
    use_effect(move || {
        if !photo_uuids.is_empty() {
            let mut urls = Vec::new();
            for uuid in photo_uuids.iter() {
                if let Some(data) =
                    load_or_download_photo(context.clone(), uuid.clone(), PhotoSize::Original, None)
                {
                    urls.push(data);
                }
            }
            loaded_urls.set(urls);
        }
    });

    let photo_count = loaded_urls.read().len();
    let has_prev = current_index() > 0;
    let has_next = current_index() < photo_count.saturating_sub(1);

    rsx! {
        div { style: "position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0, 0, 0, 0.95); z-index: 1000; display: flex; flex-direction: column;",
            div { style: "display: flex; justify-content: space-between; align-items: center; padding: 16px; background: rgba(0, 0, 0, 0.7);",
                div { style: "color: white; font-size: 16px;", "{current_index() + 1} / {photo_count}" }
                button {
                    style: "width: 40px; height: 40px; background: rgba(255, 255, 255, 0.2); color: white; border-radius: 50%; font-size: 24px; cursor: pointer; border: none;",
                    onclick: move |_| on_close.call(()),
                    "×"
                }
            }
            div { style: "flex: 1; display: flex; align-items: center; justify-content: center; padding: 20px; position: relative;",
                if has_prev {
                    button {
                        style: "position: absolute; left: 20px; width: 50px; height: 50px; background: rgba(255, 255, 255, 0.3); color: white; border-radius: 50%; font-size: 24px; cursor: pointer; border: none;",
                        onclick: move |_| {
                            let idx = current_index();
                            if idx > 0 {
                                current_index.set(idx - 1);
                            }
                        },
                        "‹"
                    }
                }
                if photo_count > 0 {
                    img {
                        src: "{loaded_urls.read()[current_index()]}",
                        style: "max-width: 100%; max-height: 100%; object-fit: contain;",
                    }
                } else {
                    div { style: "color: white; font-size: 24px;", "No photos in collection" }
                }
                if has_next {
                    button {
                        style: "position: absolute; right: 20px; width: 50px; height: 50px; background: rgba(255, 255, 255, 0.3); color: white; border-radius: 50%; font-size: 24px; cursor: pointer; border: none;",
                        onclick: move |_| {
                            let idx = current_index();
                            if idx < photo_count - 1 {
                                current_index.set(idx + 1);
                            }
                        },
                        "›"
                    }
                }
            }
        }
    }
}
