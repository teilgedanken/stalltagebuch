// This module is now a thin wrapper around photo-gallery's picker functionality
// to maintain backward compatibility with existing code.

use crate::error::AppError;
use std::path::PathBuf;

#[cfg(target_os = "android")]
use ::jni::{jni_sig, jni_str};

// Helper function to convert PickerError to AppError
fn picker_error_to_app_error(e: photo_gallery::picker::PickerError) -> AppError {
    match e {
        photo_gallery::picker::PickerError::PermissionDenied(msg) => {
            AppError::PermissionDenied(msg)
        }
        photo_gallery::picker::PickerError::Timeout(msg) => AppError::PermissionDenied(msg),
        photo_gallery::picker::PickerError::Cancelled(msg) => AppError::PermissionDenied(msg),
        photo_gallery::picker::PickerError::PlatformNotSupported(msg) => {
            AppError::PermissionDenied(msg)
        }
        photo_gallery::picker::PickerError::Other(msg) => AppError::PermissionDenied(msg),
    }
}

#[allow(dead_code)]
pub fn pick_image() -> Result<PathBuf, AppError> {
    photo_gallery::picker::pick_image().map_err(picker_error_to_app_error)
}

#[allow(dead_code)]
pub fn pick_images() -> Result<Vec<PathBuf>, AppError> {
    photo_gallery::picker::pick_images().map_err(picker_error_to_app_error)
}

#[allow(dead_code)]
pub fn capture_photo() -> Result<PathBuf, AppError> {
    photo_gallery::picker::capture_photo().map_err(picker_error_to_app_error)
}

#[allow(dead_code)]
pub fn has_camera_permission() -> Result<bool, AppError> {
    photo_gallery::picker::has_camera_permission().map_err(picker_error_to_app_error)
}

/// Gets the path of the last selected document (e.g., ZIP file).
/// Returns `None` if no selection has been made or the operation was cancelled.
/// On non-Android platforms, always returns `None`.
#[cfg(target_os = "android")]
pub fn get_last_document_path() -> Option<PathBuf> {
    get_main_activity_static_string("getLastDocumentPath", "()Ljava/lang/String;")
        .map(PathBuf::from)
}

/// Launches the Android document picker implemented in MainActivity.
#[cfg(target_os = "android")]
pub fn launch_document_picker() -> Result<(), AppError> {
    with_android_env(|env| {
        let activity_class = load_main_activity_class(env)?;

        // Call MainActivity.getInstance() to invoke instance method launchDocumentPicker().
        let instance = env
            .call_static_method(
                &activity_class,
                jni_str!("getInstance"),
                jni_sig!("()Ldev/dioxus/main/MainActivity;"),
                &[],
            )
            .map_err(|e| {
                clear_pending_exception(env);
                AppError::PermissionDenied(format!("getInstance failed: {}", e))
            })?
            .l()
            .map_err(|e| {
                clear_pending_exception(env);
                AppError::PermissionDenied(format!("getInstance invalid: {}", e))
            })?;

        if instance.is_null() {
            return Err(AppError::PermissionDenied(
                "MainActivity instance is null".to_string(),
            ));
        }

        env.call_method(
            &instance,
            jni_str!("launchDocumentPicker"),
            jni_sig!("()V"),
            &[],
        )
        .map_err(|e| {
            clear_pending_exception(env);
            AppError::PermissionDenied(format!("launchDocumentPicker failed: {}", e))
        })?;

        clear_pending_exception(env);
        Ok(())
    })
}

#[cfg(target_os = "android")]
pub fn get_last_error() -> Option<String> {
    get_main_activity_static_string("getLastError", "()Ljava/lang/String;")
}

#[cfg(target_os = "android")]
fn with_android_env<T>(
    f: impl FnOnce(&mut ::jni::Env<'_>) -> Result<T, AppError>,
) -> Result<T, AppError> {
    use ndk_context::android_context;

    let ctx = android_context();
    let vm_ptr = ctx.vm() as *mut ::jni::sys::JavaVM;
    if vm_ptr.is_null() {
        return Err(AppError::PermissionDenied("JavaVM unavailable".to_string()));
    }

    let vm = unsafe { ::jni::JavaVM::from_raw(vm_ptr) };
    vm.attach_current_thread(f)
}

#[cfg(target_os = "android")]
fn clear_pending_exception(env: &mut ::jni::Env<'_>) {
    if env.exception_check() {
        let _ = env.exception_clear();
    }
}

#[cfg(target_os = "android")]
fn load_main_activity_class<'a>(
    env: &mut ::jni::Env<'a>,
) -> Result<jni::objects::JClass<'a>, AppError> {
    use jni::objects::{JClass, JObject, JString, JValue};

    let at_cls = env
        .find_class(::jni::strings::JNIString::new("android/app/ActivityThread"))
        .map_err(|e| AppError::PermissionDenied(format!("ActivityThread missing: {}", e)))?;

    let at = env
        .call_static_method(
            &at_cls,
            ::jni::strings::JNIString::new("currentActivityThread"),
            ::jni::signature::RuntimeMethodSignature::from_str("()Landroid/app/ActivityThread;")
                .map_err(|e| AppError::PermissionDenied(format!("Invalid method signature: {}", e)))?
                .method_signature(),
            &[],
        )
        .map_err(|e| {
            clear_pending_exception(env);
            AppError::PermissionDenied(format!("currentActivityThread failed: {}", e))
        })?
        .l()
        .map_err(|e| AppError::PermissionDenied(format!("ActivityThread invalid: {}", e)))?;

    let app = env
        .call_method(
            &at,
            ::jni::strings::JNIString::new("getApplication"),
            ::jni::signature::RuntimeMethodSignature::from_str("()Landroid/app/Application;")
                .map_err(|e| AppError::PermissionDenied(format!("Invalid method signature: {}", e)))?
                .method_signature(),
            &[],
        )
        .map_err(|e| {
            clear_pending_exception(env);
            AppError::PermissionDenied(format!("getApplication failed: {}", e))
        })?
        .l()
        .map_err(|e| AppError::PermissionDenied(format!("Application invalid: {}", e)))?;

    let loader = env
        .call_method(
            &app,
            ::jni::strings::JNIString::new("getClassLoader"),
            ::jni::signature::RuntimeMethodSignature::from_str("()Ljava/lang/ClassLoader;")
                .map_err(|e| AppError::PermissionDenied(format!("Invalid method signature: {}", e)))?
                .method_signature(),
            &[],
        )
        .map_err(|e| {
            clear_pending_exception(env);
            AppError::PermissionDenied(format!("getClassLoader failed: {}", e))
        })?
        .l()
        .map_err(|e| AppError::PermissionDenied(format!("ClassLoader invalid: {}", e)))?;

    let class_name: JString = env
        .new_string("dev.dioxus.main.MainActivity")
        .map_err(|e| AppError::PermissionDenied(format!("new_string failed: {}", e)))?;

    let cls_obj = env
        .call_method(
            &loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&JObject::from(class_name))],
        )
        .map_err(|e| {
            clear_pending_exception(env);
            AppError::PermissionDenied(format!("ClassLoader.loadClass failed: {}", e))
        })?
        .l()
        .map_err(|e| AppError::PermissionDenied(format!("Loaded class invalid: {}", e)))?;

    env.cast_local::<JClass<'a>>(cls_obj)
        .map_err(|e| AppError::PermissionDenied(format!("Loaded class cast failed: {}", e)))
}

#[cfg(target_os = "android")]
fn get_main_activity_static_string(method: &str, sig: &str) -> Option<String> {
    with_android_env(|env| {
        let method_name = jni::strings::JNIString::new(method);
        let method_sig = jni::signature::RuntimeMethodSignature::from_str(sig)
            .map_err(|e| AppError::PermissionDenied(format!("Invalid method signature: {}", e)))?;
        let activity_class = load_main_activity_class(env)?;
        let result = env
            .call_static_method(
                &activity_class,
                &method_name,
                method_sig.method_signature(),
                &[],
            )
            .map_err(|e| {
                clear_pending_exception(env);
                AppError::PermissionDenied(format!("{} failed: {}", method, e))
            })?;

        let obj = result.l().map_err(|e| {
            clear_pending_exception(env);
            AppError::PermissionDenied(format!("{} returned invalid object: {}", method, e))
        })?;

        if obj.is_null() {
            return Ok(None);
        }

        let obj_str = env
            .cast_local::<jni::objects::JString<'_>>(obj)
            .map_err(|e| {
                clear_pending_exception(env);
                AppError::PermissionDenied(format!("{} string cast failed: {}", method, e))
            })?;

        let s = obj_str.try_to_string(env).map_err(|e| {
            clear_pending_exception(env);
            AppError::PermissionDenied(format!("{} string conversion failed: {}", method, e))
        })?;
        Ok(Some::<String>(s.into()))
    })
    .ok()
    .flatten()
}

/// Gets the path of the last selected document (e.g., ZIP file).
/// On non-Android platforms, always returns `None`.
#[cfg(not(target_os = "android"))]
pub fn get_last_document_path() -> Option<PathBuf> {
    None
}

#[cfg(not(target_os = "android"))]
pub fn launch_document_picker() -> Result<(), AppError> {
    Err(AppError::PermissionDenied(
        "Document picker is only available on Android".to_string(),
    ))
}

#[cfg(not(target_os = "android"))]
pub fn get_last_error() -> Option<String> {
    None
}
