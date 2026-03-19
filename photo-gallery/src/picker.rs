// Platform-specific image picker implementation
//
// This module handles camera and gallery picking for photos. On Android, it uses
// JNI to call MainActivity methods. On other platforms, it returns platform errors.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum PickerError {
    PermissionDenied(String),
    Timeout(String),
    Cancelled(String),
    PlatformNotSupported(String),
    Other(String),
}

impl std::fmt::Display for PickerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PickerError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            PickerError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            PickerError::Cancelled(msg) => write!(f, "Cancelled: {}", msg),
            PickerError::PlatformNotSupported(msg) => write!(f, "Platform not supported: {}", msg),
            PickerError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for PickerError {}

// Android-specific constants and helper functions
const DEFAULT_MAIN_ACTIVITY_CLASS: &str = "dev/dioxus/main/MainActivity";

#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject, JString, JValue};
#[cfg(target_os = "android")]
use jni::{jni_sig, jni_str};
#[cfg(target_os = "android")]
use ndk_context::android_context;

#[cfg(target_os = "android")]
impl From<jni::errors::Error> for PickerError {
    fn from(e: jni::errors::Error) -> Self {
        PickerError::PermissionDenied(format!("JNI error: {}", e))
    }
}

#[cfg(target_os = "android")]
/// Configuration for the picker on Android
///
/// This allows customization of the MainActivity class name for different apps.
#[derive(Debug, Clone)]
pub struct AndroidPickerConfig {
    /// Fully qualified class name in slash format (e.g., "com/example/myapp/MainActivity")
    pub main_activity_class: String,
}
#[cfg(target_os = "android")]
impl Default for AndroidPickerConfig {
    fn default() -> Self {
        Self {
            main_activity_class: DEFAULT_MAIN_ACTIVITY_CLASS.to_string(),
        }
    }
}

#[cfg(target_os = "android")]
fn get_app_class_loader<'a>(env: &mut jni::Env<'a>) -> Result<JObject<'a>, PickerError> {
    // ActivityThread.currentActivityThread()
    let at_cls = env
        .find_class(jni_str!("android/app/ActivityThread"))
        .map_err(|e| PickerError::PermissionDenied(format!("ActivityThread not found: {}", e)))?;
    let at = env
        .call_static_method(
            &at_cls,
            jni_str!("currentActivityThread"),
            jni_sig!("()Landroid/app/ActivityThread;"),
            &[],
        )
        .map_err(|e| PickerError::PermissionDenied(format!("currentActivityThread failed: {}", e)))?
        .l()
        .map_err(|e| {
            PickerError::PermissionDenied(format!("currentActivityThread invalid: {}", e))
        })?;

    // Prefer application class loader
    let app = env
        .call_method(
            &at,
            jni_str!("getApplication"),
            jni_sig!("()Landroid/app/Application;"),
            &[],
        )
        .map_err(|e| PickerError::PermissionDenied(format!("getApplication failed: {}", e)))?
        .l()
        .map_err(|e| PickerError::PermissionDenied(format!("getApplication invalid: {}", e)))?;

    if app.is_null() {
        // Fallback: system context
        let sys_ctx = env
            .call_method(
                &at,
                jni_str!("getSystemContext"),
                jni_sig!("()Landroid/app/ContextImpl;"),
                &[],
            )
            .map_err(|e| PickerError::PermissionDenied(format!("getSystemContext failed: {}", e)))?
            .l()
            .map_err(|e| {
                PickerError::PermissionDenied(format!("getSystemContext invalid: {}", e))
            })?;
        let loader = env
            .call_method(
                &sys_ctx,
                jni_str!("getClassLoader"),
                jni_sig!("()Ljava/lang/ClassLoader;"),
                &[],
            )
            .map_err(|e| {
                PickerError::PermissionDenied(format!("getClassLoader (sys) failed: {}", e))
            })?
            .l()
            .map_err(|e| {
                PickerError::PermissionDenied(format!("getClassLoader (sys) invalid: {}", e))
            })?;
        return Ok(loader);
    }

    let loader = env
        .call_method(
            &app,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )
        .map_err(|e| PickerError::PermissionDenied(format!("getClassLoader failed: {}", e)))?
        .l()
        .map_err(|e| PickerError::PermissionDenied(format!("getClassLoader invalid: {}", e)))?;
    Ok(loader)
}

#[cfg(target_os = "android")]
fn load_class<'a>(
    env: &mut jni::Env<'a>,
    loader: &JObject<'a>,
    fq_slash: &str,
) -> Result<JClass<'a>, PickerError> {
    // Convert dev/dioxus/main/MainActivity -> dev.dioxus.main.MainActivity for ClassLoader.loadClass
    let fq_dot = fq_slash.replace('/', ".");
    let name: JString = env
        .new_string(fq_dot)
        .map_err(|e| PickerError::PermissionDenied(format!("new_string failed: {}", e)))?;
    let cls_obj = env
        .call_method(
            loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&JObject::from(name))],
        )
        .map_err(|e| PickerError::PermissionDenied(format!("ClassLoader.loadClass failed: {}", e)))?
        .l()
        .map_err(|e| PickerError::PermissionDenied(format!("loadClass invalid: {}", e)))?;
    env.cast_local::<JClass<'a>>(cls_obj)
        .map_err(|e| PickerError::PermissionDenied(format!("loadClass cast failed: {}", e)))
}

#[cfg(target_os = "android")]
fn get_activity_instance<'a>(
    env: &mut jni::Env<'a>,
    config: &AndroidPickerConfig,
) -> Result<(JObject<'a>, JClass<'a>), PickerError> {
    let loader = get_app_class_loader(env)?;
    let cls = load_class(env, &loader, &config.main_activity_class)?;

    let signature = format!("()L{};", config.main_activity_class);
    let method_sig = jni::signature::RuntimeMethodSignature::from_str(&signature)
        .map_err(|e| PickerError::PermissionDenied(format!("Invalid method signature: {}", e)))?;
    let field_sig = jni::signature::RuntimeFieldSignature::from_str(&signature)
        .map_err(|e| PickerError::PermissionDenied(format!("Invalid field signature: {}", e)))?;

    // Primary attempt: call the expected static helper generated by `@JvmStatic`
    let instance = match env.call_static_method(
        &cls,
        jni_str!("getInstance"),
        method_sig.method_signature(),
        &[],
    ) {
        Ok(val) => val.l().map_err(|e| {
            PickerError::PermissionDenied(format!("getInstance() returned invalid object: {}", e))
        })?,
        Err(_err) => {
            // Clear any pending Java exception
            if env.exception_check() {
                let _ = env.exception_clear();
            }

            // Try direct static `instance` field first
            if let Ok(field) =
                env.get_static_field(&cls, jni_str!("instance"), field_sig.field_signature())
            {
                let inst = field.l().map_err(|e| {
                    PickerError::PermissionDenied(format!("instance field invalid: {}", e))
                })?;
                if !inst.is_null() {
                    inst
                } else {
                    // instance is present but null; try Companion
                    let comp_signature = format!("L{}$Companion;", config.main_activity_class);
                    let companion_field_sig = jni::signature::RuntimeFieldSignature::from_str(
                        &comp_signature,
                    )
                    .map_err(|e| {
                        PickerError::PermissionDenied(format!("Invalid Companion signature: {}", e))
                    })?;
                    let comp_field = env
                        .get_static_field(
                            &cls,
                            jni_str!("Companion"),
                            companion_field_sig.field_signature(),
                        )
                        .map_err(|e| {
                            PickerError::PermissionDenied(format!(
                                "Failed to get Companion field: {}",
                                e
                            ))
                        })?;

                    let comp_obj = comp_field.l().map_err(|e| {
                        PickerError::PermissionDenied(format!("Companion field invalid: {}", e))
                    })?;

                    if comp_obj.is_null() {
                        return Err(PickerError::PermissionDenied(
                            "MainActivity.Companion is null — activity not initialized?"
                                .to_string(),
                        ));
                    }

                    env.call_method(
                        &comp_obj,
                        jni_str!("getInstance"),
                        method_sig.method_signature(),
                        &[],
                    )
                    .map_err(|e| {
                        PickerError::PermissionDenied(format!(
                            "Companion.getInstance() failed: {}",
                            e
                        ))
                    })?
                    .l()
                    .map_err(|e| {
                        PickerError::PermissionDenied(format!(
                            "Companion.getInstance() returned invalid object: {}",
                            e
                        ))
                    })?
                }
            } else {
                // No instance field — fall back to Companion object access
                let comp_signature = format!("L{}$Companion;", config.main_activity_class);
                let companion_field_sig = jni::signature::RuntimeFieldSignature::from_str(
                    &comp_signature,
                )
                .map_err(|e| {
                    PickerError::PermissionDenied(format!("Invalid Companion signature: {}", e))
                })?;
                let comp_field = env
                    .get_static_field(
                        &cls,
                        jni_str!("Companion"),
                        companion_field_sig.field_signature(),
                    )
                    .map_err(|e| {
                        PickerError::PermissionDenied(format!(
                            "Failed to get Companion field: {}",
                            e
                        ))
                    })?;

                let comp_obj = comp_field.l().map_err(|e| {
                    PickerError::PermissionDenied(format!("Companion field invalid: {}", e))
                })?;

                if comp_obj.is_null() {
                    return Err(PickerError::PermissionDenied(
                        "MainActivity.Companion is null — activity not initialized?".to_string(),
                    ));
                }

                env.call_method(
                    &comp_obj,
                    jni_str!("getInstance"),
                    method_sig.method_signature(),
                    &[],
                )
                .map_err(|e| {
                    PickerError::PermissionDenied(format!("Companion.getInstance() failed: {}", e))
                })?
                .l()
                .map_err(|e| {
                    PickerError::PermissionDenied(format!(
                        "Companion.getInstance() returned invalid object: {}",
                        e
                    ))
                })?
            }
        }
    };

    if instance.is_null() {
        return Err(PickerError::PermissionDenied(
            "MainActivity instance is null - Activity not initialized?".to_string(),
        ));
    }

    Ok((instance, cls))
}

#[cfg(target_os = "android")]
fn with_android_env<T>(
    f: impl FnOnce(&mut jni::Env<'_>) -> Result<T, PickerError>,
) -> Result<T, PickerError> {
    let vm_ptr = android_context().vm() as *mut jni::sys::JavaVM;
    if vm_ptr.is_null() {
        return Err(PickerError::PermissionDenied(
            "JavaVM pointer is null".to_string(),
        ));
    }

    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) };
    vm.attach_current_thread(f)
}

/// Pick a single image from the gallery
///
/// On Android, this launches the gallery picker and waits for user selection.
/// Timeout is 60 seconds. Returns the absolute path to the selected image.
#[cfg(target_os = "android")]
pub fn pick_image() -> Result<PathBuf, PickerError> {
    pick_image_with_config(&AndroidPickerConfig::default())
}

/// Pick a single image from the gallery with custom Android configuration
#[cfg(target_os = "android")]
pub fn pick_image_with_config(config: &AndroidPickerConfig) -> Result<PathBuf, PickerError> {
    with_android_env(|env| {
        // Get MainActivity instance and class
        let (activity, main_cls) = get_activity_instance(env, config)?;

        // Clear previous error
        env.call_static_method(&main_cls, jni_str!("clearLastError"), jni_sig!("()V"), &[])
            .map_err(|e| PickerError::PermissionDenied(format!("clearLastError failed: {}", e)))?;

        // Call launchImagePicker on the activity instance
        env.call_method(
            &activity,
            jni_str!("launchImagePicker"),
            jni_sig!("()V"),
            &[],
        )
        .map_err(|e| PickerError::PermissionDenied(format!("launchImagePicker failed: {}", e)))?;

        // Poll for result (60 seconds timeout)
        for _ in 0..600 {
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Check for photo path
            if let Ok(result) = env.call_static_method(
                &main_cls,
                jni_str!("getLastPhotoPath"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            ) {
                if let Ok(obj) = result.l() {
                    if !obj.is_null() {
                        let obj_str = env.cast_local::<JString<'_>>(obj).map_err(|e| {
                            PickerError::PermissionDenied(format!("String cast failed: {}", e))
                        })?;
                        let path: String = obj_str
                            .try_to_string(env)
                            .map_err(|e| {
                                PickerError::PermissionDenied(format!(
                                    "String conversion failed: {}",
                                    e
                                ))
                            })?
                            ;
                        return Ok(PathBuf::from(path));
                    }
                }
            }

            // Check for error
            if let Ok(result) = env.call_static_method(
                &main_cls,
                jni_str!("getLastError"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            ) {
                if let Ok(obj) = result.l() {
                    if !obj.is_null() {
                        let obj_str = env.cast_local::<JString<'_>>(obj).map_err(|e| {
                            PickerError::PermissionDenied(format!("String cast failed: {}", e))
                        })?;
                        let err: String = obj_str
                            .try_to_string(env)
                            .map_err(|e| {
                                PickerError::PermissionDenied(format!(
                                    "String conversion failed: {}",
                                    e
                                ))
                            })?
                            ;
                        return Err(PickerError::PermissionDenied(err));
                    }
                }
            }
        }

        Err(PickerError::Timeout(
            "Image picker timeout - no selection made".to_string(),
        ))
    })
}

/// Pick multiple images from the gallery
///
/// On Android, this launches the multi-select gallery picker and waits for user selection.
/// Timeout is 60 seconds. Returns a vector of absolute paths to the selected images.
#[cfg(target_os = "android")]
pub fn pick_images() -> Result<Vec<PathBuf>, PickerError> {
    pick_images_with_config(&AndroidPickerConfig::default())
}

/// Pick multiple images from the gallery with custom Android configuration
#[cfg(target_os = "android")]
pub fn pick_images_with_config(config: &AndroidPickerConfig) -> Result<Vec<PathBuf>, PickerError> {
    with_android_env(|env| {
        let (activity, main_cls) = get_activity_instance(env, config)?;

        env.call_static_method(&main_cls, jni_str!("clearLastError"), jni_sig!("()V"), &[])
            .map_err(|e| PickerError::PermissionDenied(format!("clearLastError failed: {}", e)))?;

        env.call_method(
            &activity,
            jni_str!("launchImagePickerMulti"),
            jni_sig!("()V"),
            &[],
        )
        .map_err(|e| {
            PickerError::PermissionDenied(format!("launchImagePickerMulti failed: {}", e))
        })?;

        for _ in 0..600 {
            std::thread::sleep(std::time::Duration::from_millis(100));

            if let Ok(result) = env.call_static_method(
                &main_cls,
                jni_str!("getLastPhotoPaths"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            ) {
                if let Ok(obj) = result.l() {
                    if !obj.is_null() {
                        let obj_str = env.cast_local::<JString<'_>>(obj).map_err(|e| {
                            PickerError::PermissionDenied(format!("String cast failed: {}", e))
                        })?;
                        let combined: String = obj_str
                            .try_to_string(env)
                            .map_err(|e| {
                                PickerError::PermissionDenied(format!(
                                    "String conversion failed: {}",
                                    e
                                ))
                            })?
                            ;
                        let paths = combined
                            .lines()
                            .filter(|l: &&str| !l.trim().is_empty())
                            .map(PathBuf::from)
                            .collect::<Vec<_>>();
                        if !paths.is_empty() {
                            return Ok(paths);
                        }
                    }
                }
            }

            if let Ok(result) = env.call_static_method(
                &main_cls,
                jni_str!("getLastError"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            ) {
                if let Ok(obj) = result.l() {
                    if !obj.is_null() {
                        let obj_str = env.cast_local::<JString<'_>>(obj).map_err(|e| {
                            PickerError::PermissionDenied(format!("String cast failed: {}", e))
                        })?;
                        let err: String = obj_str
                            .try_to_string(env)
                            .map_err(|e| {
                                PickerError::PermissionDenied(format!(
                                    "String conversion failed: {}",
                                    e
                                ))
                            })?
                            ;
                        return Err(PickerError::PermissionDenied(err));
                    }
                }
            }
        }

        Err(PickerError::Timeout(
            "Image picker timeout (multi) - no selection made".to_string(),
        ))
    })
}

/// Capture a photo using the camera
///
/// On Android, this launches the camera app and waits for the user to take a photo.
/// Timeout is 60 seconds. Returns the absolute path to the captured image.
#[cfg(target_os = "android")]
pub fn capture_photo() -> Result<PathBuf, PickerError> {
    capture_photo_with_config(&AndroidPickerConfig::default())
}

/// Capture a photo using the camera with custom Android configuration
#[cfg(target_os = "android")]
pub fn capture_photo_with_config(config: &AndroidPickerConfig) -> Result<PathBuf, PickerError> {
    with_android_env(|env| {
        // Get MainActivity instance and class
        let (activity, main_cls) = get_activity_instance(env, config)?;

        // Clear previous error
        env.call_static_method(&main_cls, jni_str!("clearLastError"), jni_sig!("()V"), &[])
            .map_err(|e| PickerError::PermissionDenied(format!("clearLastError failed: {}", e)))?;

        // Call launchCamera on the activity instance
        env.call_method(&activity, jni_str!("launchCamera"), jni_sig!("()V"), &[])
            .map_err(|e| PickerError::PermissionDenied(format!("launchCamera failed: {}", e)))?;

        // Poll for result (60 seconds timeout)
        for _ in 0..600 {
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Check for photo path
            if let Ok(result) = env.call_static_method(
                &main_cls,
                jni_str!("getLastPhotoPath"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            ) {
                if let Ok(obj) = result.l() {
                    if !obj.is_null() {
                        let obj_str = env.cast_local::<JString<'_>>(obj).map_err(|e| {
                            PickerError::PermissionDenied(format!("String cast failed: {}", e))
                        })?;
                        let path: String = obj_str
                            .try_to_string(env)
                            .map_err(|e| {
                                PickerError::PermissionDenied(format!(
                                    "String conversion failed: {}",
                                    e
                                ))
                            })?
                            ;
                        return Ok(PathBuf::from(path));
                    }
                }
            }

            // Check for error
            if let Ok(result) = env.call_static_method(
                &main_cls,
                jni_str!("getLastError"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            ) {
                if let Ok(obj) = result.l() {
                    if !obj.is_null() {
                        let obj_str = env.cast_local::<JString<'_>>(obj).map_err(|e| {
                            PickerError::PermissionDenied(format!("String cast failed: {}", e))
                        })?;
                        let err: String = obj_str
                            .try_to_string(env)
                            .map_err(|e| {
                                PickerError::PermissionDenied(format!(
                                    "String conversion failed: {}",
                                    e
                                ))
                            })?
                            ;
                        return Err(PickerError::PermissionDenied(err));
                    }
                }
            }
        }

        Err(PickerError::Timeout(
            "Camera timeout - no photo taken".to_string(),
        ))
    })
}

/// Check if camera permission is granted
#[cfg(target_os = "android")]
pub fn has_camera_permission() -> Result<bool, PickerError> {
    has_camera_permission_with_config(&AndroidPickerConfig::default())
}

/// Check if camera permission is granted with custom Android configuration
#[cfg(target_os = "android")]
pub fn has_camera_permission_with_config(
    config: &AndroidPickerConfig,
) -> Result<bool, PickerError> {
    with_android_env(|env| {
        // Get MainActivity instance
        let (activity, _cls) = get_activity_instance(env, config)?;

        // Call hasCameraPermission
        let result = env
            .call_method(
                &activity,
                jni_str!("hasCameraPermission"),
                jni_sig!("()Z"),
                &[],
            )
            .map_err(|e| {
                PickerError::PermissionDenied(format!("hasCameraPermission failed: {}", e))
            })?;

        result
            .z()
            .map_err(|e| PickerError::PermissionDenied(format!("Boolean conversion failed: {}", e)))
    })
}

// Non-Android implementations (stubs that return platform errors)
#[cfg(not(target_os = "android"))]
pub fn pick_image() -> Result<PathBuf, PickerError> {
    Err(PickerError::PlatformNotSupported(
        "Image picker not available on this platform".to_string(),
    ))
}

#[cfg(not(target_os = "android"))]
pub fn pick_images() -> Result<Vec<PathBuf>, PickerError> {
    Err(PickerError::PlatformNotSupported(
        "Multi image picker not available on this platform".to_string(),
    ))
}

#[cfg(not(target_os = "android"))]
pub fn capture_photo() -> Result<PathBuf, PickerError> {
    Err(PickerError::PlatformNotSupported(
        "Camera not available on this platform".to_string(),
    ))
}

#[cfg(not(target_os = "android"))]
pub fn has_camera_permission() -> Result<bool, PickerError> {
    Ok(false)
}
