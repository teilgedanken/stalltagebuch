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
) -> Element {
    let mut image_state = use_signal(|| ImageLoadState::Loading);
    let context = use_context::<PhotoGalleryContext>();

    // Load photo data from relative path
    use_effect(move || {
        if let Some(data) = load_photo(context.clone(), relative_path.clone(), PhotoSize::Small) {
            image_state.set(ImageLoadState::Loaded(data));
        } else {
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
/// Accepts a relative path to the photo and loads from storage
#[component]
pub fn PreviewImage(
    relative_path: String,
    #[props(default = "Photo".to_string())] alt: String,
) -> Element {
    let mut image_state = use_signal(|| ImageLoadState::Loading);
    let context = use_context::<PhotoGalleryContext>();

    use_effect(move || {
        if let Some(data) = load_photo(context.clone(), relative_path.clone(), PhotoSize::Medium) {
            image_state.set(ImageLoadState::Loaded(data));
        } else {
            image_state.set(ImageLoadState::Failed);
        }
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
/// Accepts a relative path to the photo and loads from storage
#[component]
pub fn FullscreenImage(relative_path: String, on_close: EventHandler<()>) -> Element {
    let mut image_state = use_signal(|| ImageLoadState::Loading);
    let context = use_context::<PhotoGalleryContext>();

    use_effect(move || {
        if let Some(data) = load_photo(context.clone(), relative_path.clone(), PhotoSize::Original)
        {
            image_state.set(ImageLoadState::Loaded(data));
        } else {
            image_state.set(ImageLoadState::Failed);
        }
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
