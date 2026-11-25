# dioxus-gallery

A reusable photo gallery component library for Dioxus 0.8.0-alpha.3 applications.

## Features

- **Pure UI Component**: No database or file system dependencies
- **Flexible Configuration**: Support for deletion, selection, and fullscreen viewing
- **Callback-Based**: Parent app maintains control of data operations
- **Responsive Grid Layout**: Automatically adapts to container width

## Components

### Gallery

Main component for displaying a grid of images with optional interactions.

```rust
use dioxus::prelude::*;
use dioxus_gallery_components::{Gallery, GalleryConfig, GalleryItem};

#[component]
fn MyGallery() -> Element {
    let items = vec![
        GalleryItem {
            id: "1".to_string(),
            data_url: "data:image/jpeg;base64,...".to_string(),
            caption: Some("Photo 1".to_string()),
        }
    ];

    let config = GalleryConfig {
        allow_delete: true,
        allow_select: true,
        selected_id: Some("1".to_string()),
    };

    rsx! {
        Gallery {
            items: items,
            config: config,
            on_delete: move |id| {
                // Handle deletion in your app
            },
            on_select: move |id| {
                // Handle selection
            },
            on_view_fullscreen: move |id| {
                // Handle fullscreen view
            },
        }
    }
}
```

### FullscreenViewer

Component for viewing photos in fullscreen mode with navigation.

```rust
use dioxus_gallery_components::{FullscreenViewer, GalleryItem};

#[component]
fn MyViewer(current_item: GalleryItem, all_items: Vec<GalleryItem>) -> Element {
    rsx! {
        FullscreenViewer {
            current_item: current_item,
            all_items: all_items,
            allow_delete: true,
            on_close: move |_| {
                // Handle close
            },
            on_delete: move |id| {
                // Handle deletion
            },
            on_navigate_prev: move |_| {
                // Handle previous navigation
            },
            on_navigate_next: move |_| {
                // Handle next navigation
            },
        }
    }
}
```

## API Reference

### GalleryItem

```rust
pub struct GalleryItem {
    pub id: String,           // Unique identifier
    pub data_url: String,     // Image data (base64 or URL)
    pub caption: Option<String>, // Optional caption
}
```

### GalleryConfig

```rust
pub struct GalleryConfig {
    pub allow_delete: bool,         // Show delete buttons
    pub allow_select: bool,         // Enable selection mode
    pub selected_id: Option<String>, // Currently selected item
}
```

### Event Handlers

**Gallery Component:**
- **on_delete**: `EventHandler<String>` - Called when user requests deletion
- **on_select**: `EventHandler<String>` - Called when user selects an item
- **on_view_fullscreen**: `EventHandler<String>` - Called when user wants fullscreen view

**FullscreenViewer Component:**
- **on_close**: `EventHandler<()>` - Called when user closes the viewer
- **on_delete**: `EventHandler<String>` - Called when user deletes current item
- **on_navigate_prev**: `EventHandler<()>` - Called when user navigates to previous item
- **on_navigate_next**: `EventHandler<()>` - Called when user navigates to next item

## Design Principles

1. **Separation of Concerns**: UI rendering is separate from data management
2. **Parent Control**: All data operations are handled by the parent via callbacks
3. **Zero Dependencies**: Only depends on Dioxus core
4. **Reusability**: Can be used in any Dioxus 0.8.0-alpha.3 application

## License

MIT OR Apache-2.0
