//! Dioxus UI components for photo gallery
//!
//! This module provides reusable photo display components.
//! Components accept pre-loaded data URLs or relative paths to load their own data
//! from the file system. Database interaction is handled by the parent app.

#[cfg(feature = "components")]
use dioxus::prelude::*;

#[cfg(feature = "components")]
use std::path::PathBuf;
#[cfg(feature = "components")]
use uuid::Uuid;

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
        use base64::{Engine as _, engine::general_purpose};

        let abs_path = if relative_path.starts_with('/') {
            PathBuf::from(relative_path)
        } else {
            PathBuf::from(&self.storage_path).join(relative_path)
        };
        let original_path = abs_path.clone();

        let file_path = match size {
            PhotoSize::Original => abs_path,
            PhotoSize::Small => {
                let mut thumb_path = abs_path.clone();
                if let Some(name) = abs_path.file_stem() {
                    thumb_path.set_file_name(format!("{}_128.webp", name.to_string_lossy()));
                }
                thumb_path
            }
            PhotoSize::Medium => {
                let mut thumb_path = abs_path.clone();
                if let Some(name) = abs_path.file_stem() {
                    thumb_path.set_file_name(format!("{}_512.webp", name.to_string_lossy()));
                }
                thumb_path
            }
        };

        self.ensure_thumbnail_exists(&original_path, size);

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

    fn ensure_thumbnail_exists(&self, original_path: &std::path::Path, size: PhotoSize) {
        if !matches!(size, PhotoSize::Small | PhotoSize::Medium) {
            return;
        }

        let Some(stem) = original_path.file_stem().and_then(|s| s.to_str()) else {
            return;
        };
        if Uuid::parse_str(stem).is_err() {
            return;
        }

        let Some(parent) = original_path.parent() else {
            return;
        };
        let target_name = match size {
            PhotoSize::Small => format!("{}_128.webp", stem),
            PhotoSize::Medium => format!("{}_512.webp", stem),
            PhotoSize::Original => return,
        };
        let target_path = parent.join(target_name);
        if target_path.exists() || !original_path.exists() {
            return;
        }

        let _ = crate::thumbnail::create_thumbnails(
            original_path.to_string_lossy().as_ref(),
            stem,
            400,
            512,
        );
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
// Internal helper: try to load a photo data-url for relative_path and size.
fn load_photo(ctx: PhotoGalleryContext, relative_path: String, size: PhotoSize) -> Option<String> {
    ctx.load_photo_data(&relative_path, size)
}

#[cfg(feature = "components")]
/// Thumbnail image component - displays a small photo
///
/// Accepts a relative path to the photo and loads from storage
#[component]
pub fn ThumbnailImage(
    relative_path: String,
    #[props(default = "Photo".to_string())] alt: String,
    #[props(default = 0u32)] refresh_token: u32,
    #[props(default = false)] fill: bool,
) -> Element {
    let context = use_context::<PhotoGalleryContext>();

    // Track relative_path changes with memo for reactive dependency tracking
    let path_for_key = relative_path.clone();
    let path_key = use_memo(move || (path_for_key.clone(), refresh_token));

    // Use resource for async photo loading instead of blocking in effect
    let photo_resource = use_resource(move || {
        let ctx = context.clone();
        let (path, _refresh_token) = path_key();
        async move {
            // Simulate async by spawning off-thread would be ideal, but for now
            // we'll do the synchronous load in the resource which is better than
            // doing it in an effect since it can show loading state properly
            load_photo(ctx, path, PhotoSize::Small)
        }
    });

    let container_style = if fill {
        "position: absolute; top: 0; left: 0; width: 100%; height: 100%; overflow: hidden; background: #f0f0f0;"
    } else {
        "width: 128px; height: 128px; border-radius: 8px; overflow: hidden; background: #f0f0f0;"
    };
    let img_style = if fill {
        "width: 100%; height: 100%; object-fit: cover;"
    } else {
        "width: 100%; height: 100%; object-fit: cover;"
    };

    rsx! {
        div { style: "{container_style}",
            match photo_resource() {
                None => rsx! {
                    div { style: "display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; color: #999;",
                        "⏳"
                    }
                },
                Some(Some(url)) => rsx! {
                    img {
                        src: "{url}",
                        alt: "{alt}",
                        style: "{img_style}",
                    }
                },
                Some(None) => rsx! {
                    div { style: "display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; color: #999;",
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
/// Accepts a relative path to the photo and loads from storage
#[component]
pub fn PreviewImage(
    relative_path: String,
    #[props(default = "Photo".to_string())] alt: String,
    #[props(default = 0u32)] refresh_token: u32,
) -> Element {
    let context = use_context::<PhotoGalleryContext>();

    // Track relative_path changes with memo for reactive dependency tracking
    let path_for_key = relative_path.clone();
    let path_key = use_memo(move || (path_for_key.clone(), refresh_token));

    // Use resource for async photo loading instead of blocking in effect
    let photo_resource = use_resource(move || {
        let ctx = context.clone();
        let (path, _refresh_token) = path_key();
        async move { load_photo(ctx, path, PhotoSize::Medium) }
    });

    rsx! {
        div { style: "max-width: 512px; max-height: 512px; border-radius: 8px; overflow: hidden; background: #f0f0f0;",
            match photo_resource() {
                None => rsx! {
                    div { style: "width: 100%; height: 400px; display: flex; align-items: center; justify-content: center; color: #999;",
                        "⏳"
                    }
                },
                Some(Some(url)) => rsx! {
                    img {
                        src: "{url}",
                        alt: "{alt}",
                        style: "max-width: 100%; max-height: 100%; object-fit: contain;",
                    }
                },
                Some(None) => rsx! {
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
/// Accepts a relative path to the photo and loads from storage
#[component]
pub fn FullscreenImage(relative_path: String, on_close: EventHandler<()>) -> Element {
    let context = use_context::<PhotoGalleryContext>();

    // Track relative_path changes with memo for reactive dependency tracking
    let path_for_key = relative_path.clone();
    let path_key = use_memo(move || path_for_key.clone());

    // Use resource for async photo loading instead of blocking in effect
    let photo_resource = use_resource(move || {
        let ctx = context.clone();
        let path = path_key();
        async move { load_photo(ctx, path, PhotoSize::Original) }
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
                match photo_resource() {
                    None => rsx! {
                        div { style: "color: white; font-size: 48px;", "⏳" }
                    },
                    Some(Some(url)) => rsx! {
                        img {
                            src: "{url}",
                            style: "max-width: 100%; max-height: 100%; object-fit: contain;",
                        }
                    },
                    Some(None) => rsx! {
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
/// Accepts a relative path to the preview photo
#[component]
pub fn ThumbnailCollection(
    #[props(default = None)] preview_path: Option<String>,
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
            if let Some(path) = preview_path {
                ThumbnailImage {
                    relative_path: path,
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
/// Accepts a relative path to the preview photo
#[component]
pub fn PreviewCollection(
    #[props(default = None)] preview_path: Option<String>,
    #[props(default = None)] on_click: Option<EventHandler<()>>,
) -> Element {
    rsx! {
        div {
            style: "max-width: 512px; cursor: pointer;",
            onclick: move |_| {
                if let Some(handler) = &on_click {
                    handler.call(());
                }
            },
            if let Some(path) = preview_path {
                PreviewImage {
                    relative_path: path,
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
/// Fullscreen collection viewer for displaying multiple photos
///
/// Accepts a list of relative paths to photos and displays them in fullscreen
#[component]
pub fn CollectionFullscreen(
    #[props(default = vec![])] photo_paths: Vec<String>,
    #[props(default = 0)] initial_index: usize,
    on_close: EventHandler<()>,
) -> Element {
    let mut current_index = use_signal(|| initial_index);
    let context = use_context::<PhotoGalleryContext>();

    let photo_count = photo_paths.len();
    let has_prev = current_index() > 0;
    let has_next = current_index() < photo_count.saturating_sub(1);

    let current_photo_data = {
        if current_index() < photo_paths.len() {
            load_photo(
                context.clone(),
                photo_paths[current_index()].clone(),
                PhotoSize::Original,
            )
        } else {
            None
        }
    };

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
                    if let Some(data) = current_photo_data {
                        img {
                            src: "{data}",
                            style: "max-width: 100%; max-height: 100%; object-fit: contain;",
                        }
                    } else {
                        div { style: "color: white; font-size: 24px;", "Loading..." }
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
