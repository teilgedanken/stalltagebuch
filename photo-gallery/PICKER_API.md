# Photo Gallery Picker API

The picker module provides platform-specific image picking functionality for photo galleries.

## Features

- **Camera capture**: Take photos using the device camera
- **Gallery picking**: Select single or multiple images from gallery
- **Permission checking**: Query camera permissions
- **Platform separation**: Android JNI implementation with stub for other platforms
- **Configurable**: MainActivity class name can be customized for different apps

## Platform Support

### Android

Full support via JNI integration with MainActivity:
- Camera capture
- Single image picking from gallery  
- Multiple image picking from gallery
- Permission checking

Requires an Activity with the following methods:
- `launchCamera()` - Start camera intent
- `launchImagePicker()` - Start single image picker intent
- `launchImagePickerMulti()` - Start multi image picker intent
- `hasCameraPermission()` - Check camera permission
- Static methods: `getInstance()`, `getLastPhotoPath()`, `getLastPhotoPaths()`, `getLastError()`, `clearLastError()`

### Other Platforms

Stub implementations that return platform-not-supported errors.

## API Reference

### Functions

#### `pick_image() -> Result<PathBuf, PickerError>`

Pick a single image from the gallery.

**Returns:** Absolute path to the selected image

**Example:**
```rust
use photo_gallery::picker::pick_image;

match pick_image() {
    Ok(path) => println!("Selected: {:?}", path),
    Err(e) => eprintln!("Error: {}", e),
}
```

#### `pick_images() -> Result<Vec<PathBuf>, PickerError>`

Pick multiple images from the gallery.

**Returns:** Vector of absolute paths to the selected images

**Example:**
```rust
use photo_gallery::picker::pick_images;

match pick_images() {
    Ok(paths) => {
        println!("Selected {} images", paths.len());
        for path in paths {
            println!("  - {:?}", path);
        }
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

#### `capture_photo() -> Result<PathBuf, PickerError>`

Capture a photo using the camera.

**Returns:** Absolute path to the captured image

**Example:**
```rust
use photo_gallery::picker::capture_photo;

match capture_photo() {
    Ok(path) => println!("Photo saved: {:?}", path),
    Err(e) => eprintln!("Error: {}", e),
}
```

#### `has_camera_permission() -> Result<bool, PickerError>`

Check if camera permission is granted.

**Returns:** `true` if permission is granted, `false` otherwise

**Example:**
```rust
use photo_gallery::picker::has_camera_permission;

if has_camera_permission()? {
    println!("Camera permission granted");
} else {
    println!("Camera permission not granted");
}
```

### Configuration

#### `AndroidPickerConfig`

Configuration for Android picker behavior.

**Fields:**
- `main_activity_class: String` - Fully qualified class name in slash format (e.g., "dev/dioxus/main/MainActivity")

**Example:**
```rust
use photo_gallery::picker::{AndroidPickerConfig, pick_image_with_config};

let config = AndroidPickerConfig {
    main_activity_class: "com/myapp/main/MainActivity".to_string(),
};

match pick_image_with_config(&config) {
    Ok(path) => println!("Selected: {:?}", path),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Advanced API

For apps that use a different MainActivity class name:

- `pick_image_with_config(&AndroidPickerConfig) -> Result<PathBuf, PickerError>`
- `pick_images_with_config(&AndroidPickerConfig) -> Result<Vec<PathBuf>, PickerError>`
- `capture_photo_with_config(&AndroidPickerConfig) -> Result<PathBuf, PickerError>`
- `has_camera_permission_with_config(&AndroidPickerConfig) -> Result<bool, PickerError>`

### Error Handling

#### `PickerError`

Error types returned by picker functions:

- `PermissionDenied(String)` - User denied permission or permission not available
- `Timeout(String)` - User did not make selection within timeout (60 seconds)
- `Cancelled(String)` - User cancelled the picker
- `PlatformNotSupported(String)` - Platform does not support this operation
- `Other(String)` - Other errors

**Example:**
```rust
use photo_gallery::picker::{pick_image, PickerError};

match pick_image() {
    Ok(path) => {
        println!("Selected: {:?}", path);
    }
    Err(PickerError::PermissionDenied(msg)) => {
        eprintln!("Permission denied: {}", msg);
    }
    Err(PickerError::Timeout(msg)) => {
        eprintln!("Timeout: {}", msg);
    }
    Err(PickerError::Cancelled(msg)) => {
        eprintln!("Cancelled: {}", msg);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Integration with Main App

The main application crate can provide a thin wrapper to convert `PickerError` to its own error type:

```rust
use photo_gallery::picker;
use crate::error::AppError;

pub fn pick_image() -> Result<PathBuf, AppError> {
    picker::pick_image().map_err(|e| match e {
        picker::PickerError::PermissionDenied(msg) => AppError::PermissionDenied(msg),
        picker::PickerError::Timeout(msg) => AppError::PermissionDenied(msg),
        _ => AppError::PermissionDenied(e.to_string()),
    })
}
```

## Android MainActivity Requirements

The Android MainActivity must implement:

```kotlin
class MainActivity : WryActivity() {
    companion object {
        @JvmField
        @Volatile
        var instance: MainActivity? = null
        
        @JvmStatic
        fun getInstance(): MainActivity? = instance
        
        @JvmStatic
        fun getLastPhotoPath(): String? = currentPhotoPath
        
        @JvmStatic
        fun getLastPhotoPaths(): String? = currentPhotoPaths
        
        @JvmStatic
        fun getLastError(): String? = lastError
        
        @JvmStatic
        fun clearLastError() {
            lastError = null
        }
        
        @Volatile
        private var currentPhotoPath: String? = null
        @Volatile
        private var currentPhotoPaths: String? = null
        @Volatile
        private var lastError: String? = null
    }
    
    fun launchCamera() { /* Launch camera intent */ }
    fun launchImagePicker() { /* Launch single image picker */ }
    fun launchImagePickerMulti() { /* Launch multi image picker */ }
    fun hasCameraPermission(): Boolean { /* Check permission */ }
}
```

See the stalltagebuch project's MainActivity.kt for a complete reference implementation.

## Timeouts

All picker operations have a 60-second timeout. If the user does not make a selection within this time, a `Timeout` error is returned.

## Thread Safety

The picker uses polling to communicate with the MainActivity. All operations are thread-safe due to the use of `@Volatile` fields in the MainActivity companion object.

## Dependencies

### Android
- `jni = "0.21"`
- `ndk-context = "0.1"`

These dependencies are automatically included when targeting Android.

## Migration from camera.rs

If your app previously used a `camera.rs` module, you can create a compatibility wrapper:

```rust
// src/camera.rs
use photo_gallery::picker;
use crate::error::AppError;
use std::path::PathBuf;

pub use picker::AndroidPickerConfig;

fn picker_error_to_app_error(e: picker::PickerError) -> AppError {
    match e {
        picker::PickerError::PermissionDenied(msg) => AppError::PermissionDenied(msg),
        _ => AppError::PermissionDenied(e.to_string()),
    }
}

pub fn pick_image() -> Result<PathBuf, AppError> {
    picker::pick_image().map_err(picker_error_to_app_error)
}

pub fn pick_images() -> Result<Vec<PathBuf>, AppError> {
    picker::pick_images().map_err(picker_error_to_app_error)
}

pub fn capture_photo() -> Result<PathBuf, AppError> {
    picker::capture_photo().map_err(picker_error_to_app_error)
}

pub fn has_camera_permission() -> Result<bool, AppError> {
    picker::has_camera_permission().map_err(picker_error_to_app_error)
}
```

This maintains backward compatibility with existing code while using the photo-gallery picker implementation.
