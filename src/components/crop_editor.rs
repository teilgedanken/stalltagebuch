use crate::image_processing::CropRect;
use dioxus::prelude::*;
use dioxus_i18n::tid;
use std::path::PathBuf;

#[component]
pub fn CropEditor(
    image_path: String,
    on_crop: EventHandler<CropRect>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut image_data = use_signal(String::new);
    let mut crop_x = use_signal(|| 0.1);
    let mut crop_y = use_signal(|| 0.1);
    let mut crop_width = use_signal(|| 0.8);
    let mut crop_height = use_signal(|| 0.8);
    let mut is_dragging = use_signal(|| false);
    let mut drag_handle = use_signal(String::new);
    let mut start_x = use_signal(|| 0.0);
    let mut start_y = use_signal(|| 0.0);

    let image_path_for_effect = image_path.clone();

    // Load image data URL on mount — offload file I/O + base64 to blocking thread pool
    // so large photos don't stall the UI.
    use_effect(move || {
        let path = image_path_for_effect.clone();
        spawn(async move {
            let absolute_path = if path.starts_with('/') {
                path.clone()
            } else {
                let storage_path = crate::services::photo_service::get_storage_path();
                PathBuf::from(storage_path)
                    .join(&path)
                    .to_string_lossy()
                    .to_string()
            };

            let path_for_blocking = absolute_path.clone();
            let result = tokio::task::spawn_blocking(move || {
                crate::image_processing::image_path_to_data_url(&path_for_blocking)
            })
            .await;

            match result {
                Ok(Ok(data_url)) => {
                    image_data.set(data_url);
                    log::debug!("Loaded crop editor image from: {}", absolute_path);
                }
                Ok(Err(e)) => {
                    log::error!(
                        "Failed to load image for crop editor from {}: {}",
                        absolute_path,
                        e
                    );
                }
                Err(e) => {
                    log::error!("Blocking task panicked loading crop image: {}", e);
                }
            }
        });
    });

    let handle_crop_apply = move |_| {
        let crop_rect = CropRect {
            x: crop_x(),
            y: crop_y(),
            width: crop_width(),
            height: crop_height(),
        };
        on_crop.call(crop_rect);
    };

    let handle_crop_cancel = move |_| {
        on_cancel.call(());
    };

    let preview_style = {
        let x = crop_x() * 100.0;
        let y = crop_y() * 100.0;
        let w = crop_width() * 100.0;
        let h = crop_height() * 100.0;
        format!(
            "position: absolute; \
             left: {}%; top: {}%; width: {}%; height: {}%; \
             border: 3px solid rgba(66, 165, 245, 0.8);",
            x, y, w, h
        )
    };

    rsx! {
        div {
            style: "width: 100%; height: 100%; display: flex; flex-direction: column; background: #1a1a1a; color: white; touch-action: none; min-height: 100vh;",
            ontouchstart: move |evt| {
                if let Some(touch) = evt.touches().get(0) {
                    start_x.set(touch.client_coordinates().x as f32);
                    start_y.set(touch.client_coordinates().y as f32);
                    is_dragging.set(true);
                }
            },
            ontouchend: move |_| {
                is_dragging.set(false);
            },
            ontouchmove: move |evt| {
                if !is_dragging() {
                    return;
                }

                if let Some(touch) = evt.touches().get(0) {
                    let current_x = touch.client_coordinates().x as f32;
                    let current_y = touch.client_coordinates().y as f32;
                    let dx = (current_x - start_x()) / 500.0;
                    let dy = (current_y - start_y()) / 500.0;

                    let handle = drag_handle();
                    match handle.as_str() {
                        "nw" => {
                            crop_x
                                .set(
                                    (crop_x() + dx).max(0.0).min(crop_x() + crop_width() - 0.1),
                                );
                            crop_y
                                .set(
                                    (crop_y() + dy).max(0.0).min(crop_y() + crop_height() - 0.1),
                                );
                            crop_width.set((crop_width() - dx).max(0.1).min(1.0 - crop_x()));
                            crop_height.set((crop_height() - dy).max(0.1).min(1.0 - crop_y()));
                        }
                        "ne" => {
                            crop_y
                                .set(
                                    (crop_y() + dy).max(0.0).min(crop_y() + crop_height() - 0.1),
                                );
                            crop_width.set((crop_width() + dx).max(0.1).min(1.0 - crop_x()));
                            crop_height.set((crop_height() - dy).max(0.1).min(1.0 - crop_y()));
                        }
                        "sw" => {
                            crop_x
                                .set(
                                    (crop_x() + dx).max(0.0).min(crop_x() + crop_width() - 0.1),
                                );
                            crop_width.set((crop_width() - dx).max(0.1).min(1.0 - crop_x()));
                            crop_height.set((crop_height() + dy).max(0.1).min(1.0 - crop_y()));
                        }
                        "se" => {
                            crop_width.set((crop_width() + dx).max(0.1).min(1.0 - crop_x()));
                            crop_height.set((crop_height() + dy).max(0.1).min(1.0 - crop_y()));
                        }
                        "n" => {
                            crop_y
                                .set(
                                    (crop_y() + dy).max(0.0).min(crop_y() + crop_height() - 0.1),
                                );
                            crop_height.set((crop_height() - dy).max(0.1).min(1.0 - crop_y()));
                        }
                        "s" => {
                            crop_height.set((crop_height() + dy).max(0.1).min(1.0 - crop_y()));
                        }
                        "w" => {
                            crop_x
                                .set(
                                    (crop_x() + dx).max(0.0).min(crop_x() + crop_width() - 0.1),
                                );
                            crop_width.set((crop_width() - dx).max(0.1).min(1.0 - crop_x()));
                        }
                        "e" => {
                            crop_width.set((crop_width() + dx).max(0.1).min(1.0 - crop_x()));
                        }
                        _ => {}
                    }
                    start_x.set(current_x);
                    start_y.set(current_y);
                }
            },
            onmousemove: move |evt| {
                if !is_dragging() {
                    return;
                }

                let handle = drag_handle();
                let current_x = evt.client_coordinates().x as f32;
                let current_y = evt.client_coordinates().y as f32;
                let dx = (current_x - start_x()) / 500.0; // Normalize to viewport
                let dy = (current_y - start_y()) / 500.0;

                match handle.as_str() {
                    "nw" => {
                        crop_x.set((crop_x() + dx).max(0.0).min(crop_x() + crop_width() - 0.1));
                        crop_y.set((crop_y() + dy).max(0.0).min(crop_y() + crop_height() - 0.1));
                        crop_width.set((crop_width() - dx).max(0.1).min(1.0 - crop_x()));
                        crop_height.set((crop_height() - dy).max(0.1).min(1.0 - crop_y()));
                    }
                    "ne" => {
                        crop_y.set((crop_y() + dy).max(0.0).min(crop_y() + crop_height() - 0.1));
                        crop_width.set((crop_width() + dx).max(0.1).min(1.0 - crop_x()));
                        crop_height.set((crop_height() - dy).max(0.1).min(1.0 - crop_y()));
                    }
                    "sw" => {
                        crop_x.set((crop_x() + dx).max(0.0).min(crop_x() + crop_width() - 0.1));
                        crop_width.set((crop_width() - dx).max(0.1).min(1.0 - crop_x()));
                        crop_height.set((crop_height() + dy).max(0.1).min(1.0 - crop_y()));
                    }
                    "se" => {
                        crop_width.set((crop_width() + dx).max(0.1).min(1.0 - crop_x()));
                        crop_height.set((crop_height() + dy).max(0.1).min(1.0 - crop_y()));
                    }
                    "n" => {
                        crop_y.set((crop_y() + dy).max(0.0).min(crop_y() + crop_height() - 0.1));
                        crop_height.set((crop_height() - dy).max(0.1).min(1.0 - crop_y()));
                    }
                    "s" => {
                        crop_height.set((crop_height() + dy).max(0.1).min(1.0 - crop_y()));
                    }
                    "w" => {
                        crop_x.set((crop_x() + dx).max(0.0).min(crop_x() + crop_width() - 0.1));
                        crop_width.set((crop_width() - dx).max(0.1).min(1.0 - crop_x()));
                    }
                    "e" => {
                        crop_width.set((crop_width() + dx).max(0.1).min(1.0 - crop_x()));
                    }
                    _ => {}
                }

                start_x.set(current_x);
                start_y.set(current_y);
            },
            onmouseup: move |_| {
                is_dragging.set(false);
            },
            onmouseleave: move |_| {
                is_dragging.set(false);
            },

            // Header using Bulma level
            div {
                class: "level",
                style: "background: #1a1a1a; padding: 16px; margin: 0; border-bottom: 1px solid #333;",
                div { class: "level-left",
                    div { class: "level-item",
                        h2 { class: "title is-4", style: "margin: 0;",
                            "{tid!(\"crop_editor_title\")}"
                        }
                    }
                }
                div { class: "level-right",
                    div { class: "level-item",
                        button {
                            class: "delete is-large",
                            onclick: handle_crop_cancel,
                            aria_label: "close",
                        }
                    }
                }
            }

            // Centered content wrapper (image + buttons)
            div { style: "flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; overflow: auto; padding: 16px;",

                // Image container with crop overlay
                div { style: "position: relative; width: 100%; max-width: 600px; aspect-ratio: auto; display: flex; align-items: center; justify-content: center; margin-bottom: 16px;",

                    img {
                        src: "{image_data}",
                        style: "max-width: 100%; max-height: 100%; object-fit: contain;",
                        alt: "Crop preview",
                    }

                    // Semi-transparent overlay for cropped-out areas
                    div { style: "position: absolute; top: 0; left: 0; right: 0; bottom: 0; pointer-events: none;",

                        // Top overlay
                        div {
                            style: format!(
                                "position: absolute; left: 0; top: 0; right: 0; height: {}%; background: rgba(0, 0, 0, 0.5);",
                                crop_y() * 100.0,
                            ),
                        }

                        // Bottom overlay
                        div {
                            style: format!(
                                "position: absolute; left: 0; bottom: 0; right: 0; height: {}%; background: rgba(0, 0, 0, 0.5);",
                                (1.0 - crop_y() - crop_height()) * 100.0,
                            ),
                        }

                        // Left overlay
                        div {
                            style: format!(
                                "position: absolute; top: {}%; left: 0; bottom: {}%; width: {}%; background: rgba(0, 0, 0, 0.5);",
                                crop_y() * 100.0,
                                (1.0 - crop_y() - crop_height()) * 100.0,
                                crop_x() * 100.0,
                            ),
                        }

                        // Right overlay
                        div {
                            style: format!(
                                "position: absolute; top: {}%; right: 0; bottom: {}%; width: {}%; background: rgba(0, 0, 0, 0.5);",
                                crop_y() * 100.0,
                                (1.0 - crop_y() - crop_height()) * 100.0,
                                (1.0 - crop_x() - crop_width()) * 100.0,
                            ),
                        }
                    }

                    // Crop border with handles
                    div { style: "{preview_style}", pointer_events: "none",

                        // Corner handles
                        div {
                            style: "position: absolute; top: -8px; left: -8px; width: 16px; height: 16px; background: #42a5f5; border-radius: 50%; cursor: nwse-resize; pointer-events: auto;",
                            onmousedown: move |evt| {
                                start_x.set(evt.client_coordinates().x as f32);
                                start_y.set(evt.client_coordinates().y as f32);
                                is_dragging.set(true);
                                drag_handle.set("nw".to_string());
                            },
                            ontouchstart: move |evt| {
                                if let Some(touch) = evt.touches().get(0) {
                                    start_x.set(touch.client_coordinates().x as f32);
                                    start_y.set(touch.client_coordinates().y as f32);
                                    is_dragging.set(true);
                                    drag_handle.set("nw".to_string());
                                }
                            },
                        }
                        div {
                            style: "position: absolute; top: -8px; right: -8px; width: 16px; height: 16px; background: #42a5f5; border-radius: 50%; cursor: nesw-resize; pointer-events: auto;",
                            onmousedown: move |evt| {
                                start_x.set(evt.client_coordinates().x as f32);
                                start_y.set(evt.client_coordinates().y as f32);
                                is_dragging.set(true);
                                drag_handle.set("ne".to_string());
                            },
                            ontouchstart: move |evt| {
                                if let Some(touch) = evt.touches().get(0) {
                                    start_x.set(touch.client_coordinates().x as f32);
                                    start_y.set(touch.client_coordinates().y as f32);
                                    is_dragging.set(true);
                                    drag_handle.set("ne".to_string());
                                }
                            },
                        }
                        div {
                            style: "position: absolute; bottom: -8px; left: -8px; width: 16px; height: 16px; background: #42a5f5; border-radius: 50%; cursor: nesw-resize; pointer-events: auto;",
                            onmousedown: move |evt| {
                                start_x.set(evt.client_coordinates().x as f32);
                                start_y.set(evt.client_coordinates().y as f32);
                                is_dragging.set(true);
                                drag_handle.set("sw".to_string());
                            },
                            ontouchstart: move |evt| {
                                if let Some(touch) = evt.touches().get(0) {
                                    start_x.set(touch.client_coordinates().x as f32);
                                    start_y.set(touch.client_coordinates().y as f32);
                                    is_dragging.set(true);
                                    drag_handle.set("sw".to_string());
                                }
                            },
                        }
                        div {
                            style: "position: absolute; bottom: -8px; right: -8px; width: 16px; height: 16px; background: #42a5f5; border-radius: 50%; cursor: nwse-resize; pointer-events: auto;",
                            onmousedown: move |evt| {
                                start_x.set(evt.client_coordinates().x as f32);
                                start_y.set(evt.client_coordinates().y as f32);
                                is_dragging.set(true);
                                drag_handle.set("se".to_string());
                            },
                            ontouchstart: move |evt| {
                                if let Some(touch) = evt.touches().get(0) {
                                    start_x.set(touch.client_coordinates().x as f32);
                                    start_y.set(touch.client_coordinates().y as f32);
                                    is_dragging.set(true);
                                    drag_handle.set("se".to_string());
                                }
                            },
                        }

                        // Edge handles for moving
                        div {
                            style: "position: absolute; top: -5px; left: 8px; right: 8px; height: 10px; cursor: ns-resize; pointer-events: auto;",
                            onmousedown: move |evt| {
                                start_y.set(evt.client_coordinates().y as f32);
                                is_dragging.set(true);
                                drag_handle.set("n".to_string());
                            },
                            ontouchstart: move |evt| {
                                if let Some(touch) = evt.touches().get(0) {
                                    start_y.set(touch.client_coordinates().y as f32);
                                    is_dragging.set(true);
                                    drag_handle.set("n".to_string());
                                }
                            },
                        }
                        div {
                            style: "position: absolute; bottom: -5px; left: 8px; right: 8px; height: 10px; cursor: ns-resize; pointer-events: auto;",
                            onmousedown: move |evt| {
                                start_y.set(evt.client_coordinates().y as f32);
                                is_dragging.set(true);
                                drag_handle.set("s".to_string());
                            },
                            ontouchstart: move |evt| {
                                if let Some(touch) = evt.touches().get(0) {
                                    start_y.set(touch.client_coordinates().y as f32);
                                    is_dragging.set(true);
                                    drag_handle.set("s".to_string());
                                }
                            },
                        }
                        div {
                            style: "position: absolute; left: -5px; top: 8px; bottom: 8px; width: 10px; cursor: ew-resize; pointer-events: auto;",
                            onmousedown: move |evt| {
                                start_x.set(evt.client_coordinates().x as f32);
                                is_dragging.set(true);
                                drag_handle.set("w".to_string());
                            },
                            ontouchstart: move |evt| {
                                if let Some(touch) = evt.touches().get(0) {
                                    start_x.set(touch.client_coordinates().x as f32);
                                    is_dragging.set(true);
                                    drag_handle.set("w".to_string());
                                }
                            },
                        }
                        div {
                            style: "position: absolute; right: -5px; top: 8px; bottom: 8px; width: 10px; cursor: ew-resize; pointer-events: auto;",
                            onmousedown: move |evt| {
                                start_x.set(evt.client_coordinates().x as f32);
                                is_dragging.set(true);
                                drag_handle.set("e".to_string());
                            },
                            ontouchstart: move |evt| {
                                if let Some(touch) = evt.touches().get(0) {
                                    start_x.set(touch.client_coordinates().x as f32);
                                    is_dragging.set(true);
                                    drag_handle.set("e".to_string());
                                }
                            },
                        }
                    }
                }

                // Buttons directly below image
                div { class: "buttons", style: "margin-top: 12px; gap: 8px;",
                    button { class: "button is-light", onclick: handle_crop_cancel,
                        "{tid!(\"crop_editor_cancel\")}"
                    }
                    button { class: "button is-info", onclick: handle_crop_apply,
                        "{tid!(\"crop_editor_apply\")}"
                    }
                }
            }
        }
    }
}
