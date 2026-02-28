# Photo Gallery

A reusable photo gallery management library with thumbnail generation and storage.

## Features

- **Cross-platform photo management**: Works on any platform with Rust support
- **Automatic thumbnail generation**: Creates WebP thumbnails in multiple sizes (small: 128px, medium: 512px)
- **Database integration**: SQLite-based photo metadata storage
- **WebDAV sync** (optional): Upload and download photos to/from Nextcloud or other WebDAV servers
- **Flexible configuration**: Customizable storage paths and thumbnail sizes

## Platform Separation

This crate focuses on cross-platform photo logic. Platform-specific code (e.g., Android JNI camera integration) should remain in the application crate.

## Usage

### Basic Setup

```rust
use photo_gallery::{PhotoGalleryService, PhotoGalleryConfig};

let config = PhotoGalleryConfig {
    storage_path: "/path/to/photos".to_string(),
    enable_thumbnails: true,
    thumbnail_small_size: 128,
    thumbnail_medium_size: 512,
};

let service = PhotoGalleryService::new(config);
```

### Adding Photos

```rust
// Add a photo for a quail
let photo_uuid = service.add_quail_photo(
    &conn,
    quail_id,
    "/path/to/photo.jpg".to_string(),
    None, // Optional operation capture callback
).await?;

// Add a photo for an event
let photo_uuid = service.add_event_photo(
    &conn,
    event_id,
    "/path/to/photo.jpg".to_string(),
    None,
).await?;
```

### Listing Photos

```rust
// List photos for a quail
let photos = service.list_quail_photos(&conn, &quail_uuid)?;

// List photos for an event
let photos = service.list_event_photos(&conn, &event_uuid)?;
```

### Sync Feature (Optional)

Enable WebDAV sync by adding the `sync` feature:

```toml
[dependencies]
photo-gallery = { path = "../photo-gallery", features = ["sync"] }
```

```rust
use photo_gallery::{PhotoSyncService, PhotoSyncConfig};

let sync_config = PhotoSyncConfig {
    server_url: "https://cloud.example.com".to_string(),
    username: "user".to_string(),
    app_password: "app-password".to_string(),
    remote_path: "/Photos".to_string(),
};

let sync_service = PhotoSyncService::new(sync_config);

// Upload a photo
sync_service.upload_photo("/local/path.jpg", "remote/path.jpg").await?;

// Download a photo
sync_service.download_photo("remote/path.jpg", "/local/path.jpg").await?;
```

## Architecture

- `models.rs`: Core data structures (Photo, PhotoSize, PhotoResult, PhotoGalleryConfig)
- `service.rs`: Main photo management service (CRUD operations)
- `thumbnail.rs`: Thumbnail generation and image processing
- `sync.rs`: WebDAV sync functionality (optional, requires `sync` feature)

## Database Schema

The crate expects a `photos` table with the following columns:

- `uuid`: TEXT PRIMARY KEY
- `quail_id`: TEXT (optional)
- `event_id`: TEXT (optional)
- `path`: TEXT
- `relative_path`: TEXT
- `thumbnail_path`: TEXT (optional)
- `thumbnail_small_path`: TEXT (optional)
- `thumbnail_medium_path`: TEXT (optional)
- `sync_status`: TEXT (optional)
- `sync_error`: TEXT (optional)
- `retry_count`: INTEGER (optional)
- `deleted`: INTEGER (0 or 1)

## License

MIT OR Apache-2.0
