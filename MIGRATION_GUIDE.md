# Photo Management Refactoring Migration Guide

This guide explains how to migrate from the old photo management code to the new crate-based architecture.

## Overview

The photo management functionality has been refactored into two separate crates:

1. **`photo-gallery`**: Core photo management, thumbnail generation, and WebDAV sync
2. **`nextcloud-auth`**: Nextcloud Login Flow v2 authentication

## Benefits

- **Better modularity**: Each crate has a focused responsibility
- **Reusability**: Both crates can be used in other projects
- **Testability**: Smaller, focused units are easier to test
- **Clear boundaries**: Well-defined APIs between components
- **Platform separation**: Platform-specific code (Android JNI) stays in the main crate

## Migration Steps

### 1. Update Dependencies

In your `Cargo.toml`:

```toml
[dependencies]
photo-gallery = { path = "photo-gallery", features = ["sync"] }
nextcloud-auth = { path = "nextcloud-auth" }
```

### 2. Update Photo Model Imports

**Before:**
```rust
use crate::models::photo::{Photo, PhotoSize, PhotoResult};
```

**After:**
```rust
use photo_gallery::{Photo, PhotoSize, PhotoResult};
// Or through re-export in models/photo.rs
use crate::models::photo::{Photo, PhotoSize, PhotoResult};
```

### 3. Initialize Photo Gallery Service

**Before:**
```rust
// Direct function calls in photo_service.rs
add_quail_photo(&conn, quail_id, path, None).await?;
```

**After:**
```rust
use photo_gallery::{PhotoGalleryService, PhotoGalleryConfig};

// Initialize once (e.g., in main or using OnceLock)
let config = PhotoGalleryConfig {
    storage_path: get_storage_path(),
    enable_thumbnails: true,
    thumbnail_small_size: 128,
    thumbnail_medium_size: 512,
};
let service = PhotoGalleryService::new(config);

// Use the service
service.add_quail_photo(&conn, quail_id, path, None).await?;
```

### 4. Update Photo Service Functions

The photo service wrapper in `src/services/photo_service.rs` maintains the same API, but now delegates to the `photo-gallery` crate internally. No changes needed for existing callers.

### 5. Migrate Nextcloud Authentication

#### Settings Screen Component

**Before:**
```rust
// In settings.rs - inline login flow implementation
let start_login = move |_| {
    login_state.set(LoginState::InitiatingFlow);
    spawn(async move {
        let url = format!("{}/index.php/login/v2", server.trim_end_matches('/'));
        // ... lots of manual polling logic
    });
};
```

**After:**
```rust
use nextcloud_auth::{NextcloudAuthComponent, AuthLabels};

// Use the component
NextcloudAuthComponent {
    server_url: server_url(),
    on_success: move |creds| {
        // Save credentials
        let settings = SyncSettings::new(
            creds.server_url,
            creds.username,
            creds.app_password,
            remote_path(),
        );
        save_sync_settings(&conn, &settings).unwrap();
        current_settings.set(Some(settings));
        login_state.set(LoginState::Success);
    },
    on_error: move |error| {
        status_message.set(format!("❌ {}", error));
    },
    labels: Some(create_i18n_labels()),
}
```

#### Programmatic Authentication

**Before:**
```rust
// Manual implementation of login flow in sync service
```

**After:**
```rust
use nextcloud_auth::NextcloudAuthService;

let auth_service = NextcloudAuthService::new(server_url);
let credentials = auth_service.authenticate_with_polling(60, 5).await?;
```

### 6. Update Photo Sync Operations

**Before:**
```rust
// In upload_service.rs or download_service.rs
let webdav_url = format!("{}/remote.php/dav/files/{}", ...);
let client = reqwest_dav::ClientBuilder::new()
    .set_host(webdav_url)
    .set_auth(...)
    .build()?;
client.put(&remote_path, file_data).await?;
```

**After:**
```rust
use photo_gallery::{PhotoSyncService, PhotoSyncConfig};

let sync_config = PhotoSyncConfig {
    server_url: settings.server_url.clone(),
    username: settings.username.clone(),
    app_password: settings.app_password.clone(),
    remote_path: settings.remote_path.clone(),
};

let sync_service = PhotoSyncService::new(sync_config);

// Upload
sync_service.upload_photo_with_retry(local_path, relative_path, 3).await?;

// Download
sync_service.download_photo_with_retry(relative_path, local_path, 3).await?;
```

## Code Organization

### Before

```
src/
├── models/
│   └── photo.rs (Photo, PhotoSize, PhotoResult)
├── services/
│   ├── photo_service.rs (all photo logic)
│   ├── upload_service.rs (photo upload)
│   └── download_service.rs (photo download)
└── components/
    └── settings.rs (inline auth logic)
```

### After

```
photo-gallery/
├── src/
│   ├── models.rs (Photo, PhotoSize, PhotoResult, PhotoGalleryConfig)
│   ├── service.rs (PhotoGalleryService)
│   ├── thumbnail.rs (thumbnail generation)
│   └── sync.rs (PhotoSyncService)

nextcloud-auth/
├── src/
│   ├── models.rs (LoginState, NextcloudCredentials)
│   ├── service.rs (NextcloudAuthService)
│   └── component.rs (NextcloudAuthComponent)

src/
├── models/
│   └── photo.rs (re-exports from photo-gallery)
├── services/
│   └── photo_service.rs (thin wrapper over photo-gallery)
├── components/
│   └── settings.rs (uses NextcloudAuthComponent)
└── camera.rs (Android-specific, stays here)
```

## Platform-Specific Code

### Android Camera/Gallery Integration

**Stays in main crate**: `src/camera.rs`

The camera and gallery picker functionality requires Android JNI and should remain in the main application crate. The photo-gallery crate only handles what happens after a photo is selected.

```rust
// In main crate
use crate::camera;

// Pick/capture photo (platform-specific)
let path = camera::pick_image()?;

// Process photo (uses photo-gallery crate)
let photo_uuid = photo_service::add_quail_photo(&conn, quail_id, path, None).await?;
```

## Error Handling

### Converting Errors

The photo-gallery crate has its own error type. The main crate's `photo_service.rs` provides conversion:

```rust
fn convert_error(e: photo_gallery::PhotoGalleryError) -> AppError {
    match e {
        photo_gallery::PhotoGalleryError::DatabaseError(e) => AppError::Database(e),
        photo_gallery::PhotoGalleryError::NotFound(msg) => AppError::NotFound(msg),
        photo_gallery::PhotoGalleryError::IoError(e) => AppError::IoError(e),
        photo_gallery::PhotoGalleryError::ThumbnailError(e) => {
            AppError::Other(format!("Thumbnail error: {}", e))
        }
        photo_gallery::PhotoGalleryError::Other(msg) => AppError::Other(msg),
    }
}
```

## Internationalization (i18n)

The `nextcloud-auth` component supports custom labels for internationalization:

```rust
use nextcloud_auth::AuthLabels;
use dioxus_i18n::t;

fn create_i18n_labels() -> AuthLabels {
    AuthLabels {
        login_button: t!("sync-login").to_string(),
        connecting: t!("sync-connecting").to_string(),
        waiting: t!("sync-waiting").to_string(),
        // ... etc
    }
}

NextcloudAuthComponent {
    server_url: server_url(),
    on_success: on_success_handler,
    labels: Some(create_i18n_labels()),
}
```

## Testing

### Unit Tests

Each crate can be tested independently:

```bash
# Test photo-gallery
cd photo-gallery
cargo test

# Test nextcloud-auth
cd nextcloud-auth
cargo test
```

### Integration Tests

Test the integration in the main crate:

```bash
# In main crate
cargo test
```

## Build Configuration

### Android Build

The Android build script (`build_android.sh`) doesn't need changes. The workspace configuration automatically includes the new crates.

### Features

The `photo-gallery` crate has an optional `sync` feature:

```toml
[dependencies]
photo-gallery = { path = "photo-gallery" } # Without sync
photo-gallery = { path = "photo-gallery", features = ["sync"] } # With sync
```

## Breaking Changes

### API Changes

1. **Photo models** are now in `photo_gallery` crate instead of `crate::models::photo`
2. **Thumbnail creation** is internal to photo-gallery crate
3. **WebDAV operations** are encapsulated in `PhotoSyncService`
4. **Auth flow** is handled by `NextcloudAuthService` or `NextcloudAuthComponent`

### Non-Breaking Changes

The main crate's `photo_service.rs` maintains backward compatibility by providing the same function signatures.

## Rollback Plan

If issues arise, you can temporarily roll back:

1. Restore `src/models/photo.rs` to define models locally
2. Restore `src/services/photo_service_old.rs` as `photo_service.rs`
3. Remove photo-gallery and nextcloud-auth from dependencies
4. Commit and push

## Next Steps

1. ✅ Create and test photo-gallery crate
2. ✅ Create and test nextcloud-auth crate
3. ✅ Update main crate dependencies
4. ✅ Migrate photo service
5. ⏳ Migrate settings screen to use NextcloudAuthComponent
6. ⏳ Test end-to-end workflow
7. ⏳ Update documentation
8. ⏳ Code review and security scan

## Questions?

For questions or issues with the migration, please refer to:

- [photo-gallery README](photo-gallery/README.md)
- [nextcloud-auth README](nextcloud-auth/README.md)
- GitHub Issues

## License

The new crates maintain the same license as the main project: MIT OR Apache-2.0
