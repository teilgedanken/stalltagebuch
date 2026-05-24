use dioxus::prelude::*;

/// Represents a single item in the gallery
#[derive(Clone, PartialEq, Debug)]
pub struct GalleryItem {
    /// Unique identifier for the item
    pub id: String,
    /// Data URL or path to the image (can be base64-encoded)
    pub data_url: String,
    /// Optional caption or description
    pub caption: Option<String>,
}

/// Configuration for the Gallery component
#[derive(Clone, PartialEq, Default)]
pub struct GalleryConfig {
    /// Whether to show delete buttons
    pub allow_delete: bool,
    /// Whether items are selectable (e.g., for profile photo selection)
    pub allow_select: bool,
    /// ID of the currently selected item (for selection mode)
    pub selected_id: Option<String>,
    /// Whether to show edit/crop buttons
    pub allow_edit: bool,
}

/// A reusable photo gallery component for Dioxus
///
/// This component displays a grid of images with optional delete and select functionality.
/// It handles the UI rendering while delegating data operations to the parent via callbacks.
///
/// # Example
/// ```rust,ignore
/// Gallery {
///     items: vec![
///         GalleryItem {
///             id: "1".to_string(),
///             data_url: "data:image/jpeg;base64,...".to_string(),
///             caption: None,
///         }
///     ],
///     config: GalleryConfig {
///         allow_delete: true,
///         allow_select: false,
///         selected_id: None,
///     },
///     on_delete: move |id| {
///         // Handle deletion in parent
///     },
///     on_select: move |id| {
///         // Handle selection in parent
///     },
///     on_view_fullscreen: move |id| {
///         // Handle fullscreen view
///     },
/// }
/// ```
#[component]
pub fn Gallery(
    /// List of gallery items to display
    items: Vec<GalleryItem>,
    /// Gallery configuration
    #[props(default)]
    config: GalleryConfig,
    /// Callback when user requests to delete an item
    #[props(default)]
    on_delete: Option<EventHandler<String>>,
    /// Callback when user selects an item (for profile photo selection, etc.)
    #[props(default)]
    on_select: Option<EventHandler<String>>,
    /// Callback when user wants to view an item in fullscreen
    #[props(default)]
    on_view_fullscreen: Option<EventHandler<String>>,
    /// Callback when user wants to edit/crop an item
    #[props(default)]
    on_edit: Option<EventHandler<String>>,
) -> Element {
    if items.is_empty() {
        return rsx! {
            div {
                style: "padding: 24px; text-align: center; background: #f5f5f5; border-radius: 8px; color: #999;",
                "No photos available"
            }
        };
    }

    rsx! {
        div {
            style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: 12px;",
            for item in items {
                GalleryItemView {
                    item: item.clone(),
                    is_selected: config.selected_id.as_ref().map(|s| s.as_str()) == Some(&item.id),
                    allow_delete: config.allow_delete,
                    allow_select: config.allow_select,
                    allow_edit: config.allow_edit,
                    on_delete: on_delete.clone(),
                    on_select: on_select.clone(),
                    on_view_fullscreen: on_view_fullscreen.clone(),
                    on_edit: on_edit.clone(),
                }
            }
        }
    }
}

/// Internal component for rendering a single gallery item
#[component]
fn GalleryItemView(
    item: GalleryItem,
    is_selected: bool,
    allow_delete: bool,
    allow_select: bool,
    allow_edit: bool,
    on_delete: Option<EventHandler<String>>,
    on_select: Option<EventHandler<String>>,
    on_view_fullscreen: Option<EventHandler<String>>,
    on_edit: Option<EventHandler<String>>,
) -> Element {
    let border_color = if is_selected { "#0066cc" } else { "#e0e0e0" };
    let photo_style = format!(
        "position: relative; aspect-ratio: 1/1; border-radius: 8px; overflow: hidden; border: 2px solid {};",
        border_color
    );

    rsx! {
        div {
            key: "{item.id}",
            style: "{photo_style}",
            // Image
            img {
                src: "{item.data_url}",
                style: "width: 100%; height: 100%; object-fit: cover; cursor: pointer;",
                onclick: {
                    let item_id = item.id.clone();
                    move |_| {
                        if allow_select {
                            if let Some(handler) = &on_select {
                                handler.call(item_id.clone());
                            }
                        } else if let Some(handler) = &on_view_fullscreen {
                            handler.call(item_id.clone());
                        }
                    }
                },
            }
            // Delete button
            if allow_delete {
                button {
                    style: "position: absolute; top: 4px; right: 4px; width: 28px; height: 28px; background: rgba(204, 0, 0, 0.9); color: white; border-radius: 50%; font-size: 14px; display: flex; align-items: center; justify-content: center; cursor: pointer; border: none;",
                    onclick: {
                        let item_id = item.id.clone();
                        move |_| {
                            if let Some(handler) = &on_delete {
                                handler.call(item_id.clone());
                            }
                        }
                    },
                    "×"
                }
            }
            // Edit/Crop button
            if allow_edit {
                button {
                    style: "position: absolute; top: 4px; right: 36px; width: 28px; height: 28px; background: rgba(66, 165, 245, 0.9); color: white; border-radius: 50%; font-size: 14px; display: flex; align-items: center; justify-content: center; cursor: pointer; border: none;",
                    onclick: {
                        let item_id = item.id.clone();
                        move |_| {
                            if let Some(handler) = &on_edit {
                                handler.call(item_id.clone());
                            }
                        }
                    },
                    "✂️"
                }
            }
            // Selection indicator
            if is_selected && allow_select {
                div {
                    style: "position: absolute; bottom: 4px; right: 4px; width: 24px; height: 24px; background: #0066cc; border-radius: 50%; display: flex; align-items: center; justify-content: center; color: white; font-size: 16px;",
                    "✓"
                }
            }
        }
    }
}

/// A fullscreen photo viewer component
///
/// Displays a single photo in fullscreen with navigation and action buttons
#[component]
pub fn FullscreenViewer(
    /// Current item being viewed
    current_item: GalleryItem,
    /// All items in the gallery for navigation
    all_items: Vec<GalleryItem>,
    /// Whether to show delete button
    #[props(default = true)]
    allow_delete: bool,
    /// Callback when user closes the viewer
    on_close: EventHandler<()>,
    /// Callback when user deletes the current item
    #[props(default)]
    on_delete: Option<EventHandler<String>>,
    /// Callback when user navigates to previous item
    #[props(default)]
    on_navigate_prev: Option<EventHandler<()>>,
    /// Callback when user navigates to next item
    #[props(default)]
    on_navigate_next: Option<EventHandler<()>>,
) -> Element {
    let current_index = all_items
        .iter()
        .position(|item| item.id == current_item.id)
        .unwrap_or(0);

    let has_prev = current_index > 0;
    let has_next = current_index < all_items.len().saturating_sub(1);

    rsx! {
        div {
            style: "position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0, 0, 0, 0.95); z-index: 1000; display: flex; flex-direction: column;",
            // Header with close button
            div {
                style: "display: flex; justify-content: space-between; align-items: center; padding: 16px; background: rgba(0, 0, 0, 0.7);",
                div {
                    style: "color: white; font-size: 16px;",
                    if let Some(caption) = &current_item.caption {
                        "{caption}"
                    } else {
                        ""
                    }
                }
                button {
                    style: "width: 40px; height: 40px; background: rgba(255, 255, 255, 0.2); color: white; border-radius: 50%; font-size: 24px; cursor: pointer; border: none;",
                    onclick: move |_| on_close.call(()),
                    "×"
                }
            }
            // Main image area
            div {
                style: "flex: 1; display: flex; align-items: center; justify-content: center; padding: 20px; position: relative;",
                // Previous button
                if has_prev {
                    button {
                        style: "position: absolute; left: 20px; width: 50px; height: 50px; background: rgba(255, 255, 255, 0.3); color: white; border-radius: 50%; font-size: 24px; cursor: pointer; border: none;",
                        onclick: move |_| {
                            if let Some(handler) = &on_navigate_prev {
                                handler.call(());
                            }
                        },
                        "‹"
                    }
                }
                // Image
                img {
                    src: "{current_item.data_url}",
                    style: "max-width: 100%; max-height: 100%; object-fit: contain;",
                }
                // Next button
                if has_next {
                    button {
                        style: "position: absolute; right: 20px; width: 50px; height: 50px; background: rgba(255, 255, 255, 0.3); color: white; border-radius: 50%; font-size: 24px; cursor: pointer; border: none;",
                        onclick: move |_| {
                            if let Some(handler) = &on_navigate_next {
                                handler.call(());
                            }
                        },
                        "›"
                    }
                }
            }
            // Footer with actions
            div {
                style: "display: flex; justify-content: center; gap: 16px; padding: 16px; background: rgba(0, 0, 0, 0.7);",
                if allow_delete {
                    button {
                        style: "padding: 12px 24px; background: #cc0000; color: white; border-radius: 8px; font-size: 16px; cursor: pointer; border: none;",
                        onclick: {
                            let item_id = current_item.id.clone();
                            move |_| {
                                if let Some(handler) = &on_delete {
                                    handler.call(item_id.clone());
                                }
                            }
                        },
                        "🗑️ Delete"
                    }
                }
            }
        }
    }
}
