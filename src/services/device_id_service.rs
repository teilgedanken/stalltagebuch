//! Device ID service for Android.
//!
//! Retrieves a unique device identifier (ANDROID_ID) that persists across app restarts
//! but may change on factory reset or app reinstallation.

use crate::error::StalltagebuchError;

#[cfg(target_os = "android")]
use jni::{
    JNIEnv,
    objects::{JObject, JString, JValue},
};
#[cfg(target_os = "android")]
use ndk_context::android_context;

/// Get the Android device ID (ANDROID_ID).
///
/// This ID is unique per device per app and persists across app restarts.
/// It changes only on factory reset or app reinstallation.
///
/// On non-Android platforms, returns a default value for testing.
#[cfg(target_os = "android")]
pub fn get_device_id() -> Result<String, StalltagebuchError> {
    use jni::JavaVM;
    use std::sync::OnceLock;

    static DEVICE_ID: OnceLock<String> = OnceLock::new();

    // Return cached device ID if available
    if let Some(id) = DEVICE_ID.get() {
        return Ok(id.clone());
    }

    // Get the device ID via JNI
    let vm_ptr = android_context().vm() as *mut *const jni::sys::JNIInvokeInterface_;
    let vm = unsafe { JavaVM::from_raw(vm_ptr) }
        .map_err(|e| StalltagebuchError::JniError(format!("Failed to get JavaVM: {}", e)))?;

    let mut env = vm
        .attach_current_thread()
        .map_err(|e| StalltagebuchError::JniError(format!("Failed to attach thread: {}", e)))?;

    let context_ptr = android_context().context() as jni::sys::jobject;
    if context_ptr.is_null() {
        return Err(StalltagebuchError::JniError(
            "Android context is null".to_string(),
        ));
    }

    // Create a local ref from the global context ref; don't drop the global handle.
    let context_global = unsafe { JObject::from_raw(context_ptr) };
    let context_local = env
        .new_local_ref(&context_global)
        .map_err(|e| StalltagebuchError::JniError(format!("Failed to get local context ref: {}", e)))?;
    std::mem::forget(context_global);

    let device_id = get_android_id(&mut env, &context_local)?;

    // Cache the device ID
    let _ = DEVICE_ID.set(device_id.clone());

    Ok(device_id)
}

#[cfg(target_os = "android")]
fn get_android_id(env: &mut JNIEnv, context_obj: &JObject) -> Result<String, StalltagebuchError> {
    // Get ContentResolver from context
    let content_resolver = env
        .call_method(
            context_obj,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )
        .map_err(|e| StalltagebuchError::JniError(format!("Failed to get ContentResolver: {}", e)))?
        .l()
        .map_err(|e| {
            StalltagebuchError::JniError(format!("Failed to cast ContentResolver: {}", e))
        })?;

    // Get android_id string constant
    let secure_class = env
        .find_class("android/provider/Settings$Secure")
        .map_err(|e| {
            StalltagebuchError::JniError(format!("Failed to find Settings.Secure class: {}", e))
        })?;

    let android_id_field = env
        .get_static_field(&secure_class, "ANDROID_ID", "Ljava/lang/String;")
        .map_err(|e| {
            StalltagebuchError::JniError(format!("Failed to get ANDROID_ID field: {}", e))
        })?
        .l()
        .map_err(|e| StalltagebuchError::JniError(format!("Failed to cast ANDROID_ID: {}", e)))?;

    // Call Settings.Secure.getString(contentResolver, ANDROID_ID)
    let device_id_obj = env
        .call_static_method(
            secure_class,
            "getString",
            "(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;",
            &[
                JValue::Object(&content_resolver),
                JValue::Object(&android_id_field),
            ],
        )
        .map_err(|e| StalltagebuchError::JniError(format!("Failed to call getString: {}", e)))?
        .l()
        .map_err(|e| StalltagebuchError::JniError(format!("Failed to cast result: {}", e)))?;

    // Convert to Rust string
    let device_id_jstring = JString::from(device_id_obj);
    let device_id: String = env
        .get_string(&device_id_jstring)
        .map_err(|e| StalltagebuchError::JniError(format!("Failed to get string: {}", e)))?
        .into();

    Ok(device_id)
}

/// Non-Android implementation for testing/development
#[cfg(not(target_os = "android"))]
pub fn get_device_id() -> Result<String, StalltagebuchError> {
    // Use hostname as a stable identifier on desktop
    Ok(format!(
        "desktop-{}",
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_non_android() {
        #[cfg(not(target_os = "android"))]
        {
            let device_id = get_device_id().unwrap();
            assert!(device_id.starts_with("desktop-"));
        }
    }
}
