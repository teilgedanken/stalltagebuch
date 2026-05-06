use dioxus::prelude::*;
use dioxus_i18n::prelude::*;
use image_processing::CropRect;
use models::SpacetimeSettings;
use photo_gallery::PhotoGalleryContext;

mod camera;
mod components;
mod dioxus_spacetime_module_bindings;
mod error;
mod i18n;
mod image_processing;
mod models;
mod services;
mod spacetime;

use components::{
    AddProfileScreen, CropEditor, EggTrackingScreen, EventAdd, EventEditScreen, HomeScreen,
    NavigationBar, ProfileDetailScreen, ProfileEditScreen, ProfileListScreen, SettingsScreen,
    StatisticsScreen,
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const BULMA_CSS: Asset = asset!("/assets/bulma.css");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    init_logger();
    // Panic hook: capture backtraces and log them so Android logcat contains useful info
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(message) = info.payload().downcast_ref::<&str>() {
            (*message).to_string()
        } else if let Some(message) = info.payload().downcast_ref::<String>() {
            message.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        if let Some(location) = info.location() {
            log::error!(
                "Encountered panic at {}:{}:{}: {}",
                location.file(),
                location.line(),
                location.column(),
                payload
            );
        } else {
            log::error!("Encountered panic: {}", payload);
        }
        log::error!("Backtrace:\n{:?}", std::backtrace::Backtrace::capture());
    }));
    init_android_tls_verifier();
    log::info!("App start: Stalltagebuch wird gestartet");
    dioxus::launch(App);
}

#[cfg(target_os = "android")]
fn init_android_tls_verifier() {
    use dioxus::prelude::jni::objects::JObject;
    use ndk_context::android_context;

    let vm_ptr = android_context().vm() as *mut dioxus::prelude::jni::sys::JavaVM;
    let context_ptr = android_context().context() as dioxus::prelude::jni::sys::jobject;

    if vm_ptr.is_null() || context_ptr.is_null() {
        log::error!("Android TLS verifier init failed: missing VM or context pointer");
        return;
    }

    let vm = match unsafe { dioxus::prelude::jni::JavaVM::from_raw(vm_ptr) } {
        Ok(vm) => vm,
        Err(error) => {
            log::error!(
                "Android TLS verifier init failed: JavaVM from_raw error: {}",
                error
            );
            return;
        }
    };

    let mut env = match vm.attach_current_thread() {
        Ok(guard) => guard,
        Err(error) => {
            log::error!(
                "Android TLS verifier init failed: attach thread error: {}",
                error
            );
            return;
        }
    };

    let context_global = unsafe { JObject::from_raw(context_ptr) };
    let context_local = match env.new_local_ref(&context_global) {
        Ok(local) => local,
        Err(error) => {
            log::error!(
                "Android TLS verifier init failed: new_local_ref error: {}",
                error
            );
            return;
        }
    };
    std::mem::forget(context_global);

    match rustls_platform_verifier::android::init_with_env(&mut env, context_local) {
        Ok(()) => {
            log::info!("Android TLS verifier initialized");
        }
        Err(error) => {
            log::error!("Android TLS verifier init failed: {}", error);
        }
    }
}

#[cfg(not(target_os = "android"))]
fn init_android_tls_verifier() {}

#[inline]
fn init_logger() {
    #[cfg(target_os = "android")]
    {
        use android_logger::Config;
        use log::LevelFilter;
        // Single-init logger to Android logcat with app tag
        android_logger::init_once(
            Config::default()
                .with_max_level(LevelFilter::Debug)
                .with_tag("stalltagebuch"),
        );
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .format_timestamp_millis()
            .try_init();
    }
}

/// Screen navigation for the app
#[derive(Clone, PartialEq, Debug)]
pub enum Screen {
    Home,
    ProfileList,
    ProfileDetail(String),
    ProfileEdit(String),
    AddProfile,
    EventAdd {
        quail_id: String,
        quail_name: String,
    },
    EventEdit {
        event_id: String,
        quail_id: String,
    },
    EggTracking(Option<String>), // Date in YYYY-MM-DD format
    Statistics,
    Settings,
    Crop {
        photo_path: String,
        on_complete: Box<Screen>, // Screen to return to after crop
    },
}

#[derive(Clone, PartialEq, Debug)]
struct SpacetimeRuntimeConfig {
    uri: String,
    module: String,
    token: Option<String>,
}

impl SpacetimeRuntimeConfig {
    fn from_settings(settings: &SpacetimeSettings) -> Self {
        let uri = if !settings.server_url.is_empty() {
            settings.server_url.clone()
        } else {
            "http://127.0.0.1:3000".to_string()
        };

        let module = if !settings.database_name.is_empty() {
            settings.database_name.clone()
        } else {
            "stalltagebuch-server".to_string()
        };

        let token = if settings.token.trim().is_empty() {
            None
        } else {
            Some(settings.token.clone())
        };

        Self { uri, module, token }
    }
}

#[component]
fn PhotoSyncManager() -> Element {
    spacetime::use_subscription(&["SELECT * FROM photos"]);

    let ctx = spacetime::use_spacetimedb_context();
    let connection_state = ctx.state;
    let photos = spacetime::use_table_photos();
    let mut sync_in_flight = use_signal_sync(|| false);
    let mut last_triggered_pending = use_signal_sync(|| 0usize);

    use_effect(move || {
        let is_connected = matches!(
            connection_state(),
            spacetime::ConnectionState::Connected(_, _)
        );
        let pending_count = photos()
            .iter()
            .filter(|photo| {
                matches!(
                    photo.sync_status.as_str(),
                    "local_only" | "pending" | "error"
                )
            })
            .count();

        if !is_connected {
            last_triggered_pending.set(0);
            return;
        }

        if pending_count == 0 {
            last_triggered_pending.set(0);
            return;
        }

        if sync_in_flight() || pending_count == last_triggered_pending() {
            return;
        }

        sync_in_flight.set(true);
        last_triggered_pending.set(pending_count);

        // dioxus::spawn runs on the Dioxus runtime thread, so signal updates are safe.
        // sync_now() is pure async I/O — no UI blocking.
        spawn(async move {
            if let Err(error) = services::background_sync::sync_now().await {
                log::warn!("Automatic photo sync skipped/failed: {}", error);
            }
            sync_in_flight.set(false);
        });
    });

    rsx! {}
}

#[component]
fn SpacetimeSession(
    mut current_screen: Signal<Screen>,
    spacetime_uri: String,
    spacetime_module: String,
    saved_token: Option<String>,
    on_spacetime_settings_saved: EventHandler<SpacetimeSettings>,
) -> Element {
    // Provide the generated SpacetimeDB context tree-wide.
    // Pass the saved token so we reconnect as the same client identity.
    let _spacetime_ctx =
        spacetime::use_spacetimedb_context_provider(&spacetime_uri, &spacetime_module, saved_token);

    // Watch for successful connections and persist the authentication token
    let _token_persist = spacetime::use_persist_spacetime_token();

    // Automatically register this device when connected to SpacetimeDB
    let _device_registration = spacetime::use_register_device();

    rsx! {
        PhotoSyncManager {}

        // Main Content
        div { style: "flex: 1; overflow-y: auto;",
            match current_screen() {
                Screen::Home => rsx! {
                    HomeScreen { on_navigate: move |s| current_screen.set(s) }
                },
                Screen::ProfileList => rsx! {
                    ProfileListScreen { on_navigate: move |s| current_screen.set(s) }
                },
                Screen::ProfileDetail(id) => rsx! {
                    ProfileDetailScreen { quail_id: id, on_navigate: move |s| current_screen.set(s) }
                },
                Screen::ProfileEdit(id) => rsx! {
                    ProfileEditScreen { quail_id: id, on_navigate: move |s| current_screen.set(s) }
                },
                Screen::AddProfile => rsx! {
                    AddProfileScreen { on_navigate: move |s| current_screen.set(s) }
                },
                Screen::EventAdd { quail_id, quail_name } => {
                    rsx! {
                        EventAdd {
                            quail_id,
                            quail_name,
                            on_navigate: move |s| current_screen.set(s),
                        }
                    }
                }
                Screen::EventEdit { event_id, quail_id } => rsx! {
                    EventEditScreen { event_id, quail_id, on_navigate: move |s| current_screen.set(s) }
                },
                Screen::EggTracking(date_opt) => rsx! {
                    EggTrackingScreen { date: date_opt, on_navigate: move |s| current_screen.set(s) }
                },
                Screen::Statistics => rsx! {
                    StatisticsScreen { on_navigate: move |s| current_screen.set(s) }
                },
                Screen::Settings => rsx! {
                    SettingsScreen {
                        on_navigate: move |s| current_screen.set(s),
                        on_spacetime_settings_saved,
                    }
                },
                Screen::Crop { photo_path, on_complete } => {
                    let on_complete_cancel = on_complete.clone();

                    rsx! {
                        CropEditor {
                            image_path: photo_path.clone(),
                            on_crop: move |crop_rect| {
                                let photo_path_inner = photo_path.clone();
                                let on_complete_after = on_complete.clone();

                                // dioxus::spawn stays on the Dioxus thread so signal updates
                                // are safe. crop_and_process_photo internally uses
                                // tokio::task::spawn_blocking for CPU-heavy work.
                                spawn(async move {
                                    let photo_uuid =
                                        std::path::PathBuf::from(&photo_path_inner)
                                            .file_stem()
                                            .and_then(|s| s.to_str())
                                            .map(|s| {
                                                s.split('-')
                                                    .next()
                                                    .unwrap_or(s)
                                                    .to_string()
                                            })
                                            .unwrap_or_else(|| "unknown".to_string());

                                    match crate::services::photo_service::crop_and_process_photo(
                                        photo_path_inner,
                                        crop_rect,
                                        photo_uuid.clone(),
                                        0,
                                    )
                                    .await
                                    {
                                        Ok((_, _, _, new_version)) => {
                                            log::info!(
                                                "Photo cropped successfully: version {}",
                                                new_version
                                            );
                                        }
                                        Err(e) => {
                                            log::error!("Failed to crop photo: {}", e);
                                        }
                                    }
                                    current_screen.set(*on_complete_after);
                                });
                            },
                            on_cancel: move |_| {
                                current_screen.set(*on_complete_cancel.clone());
                            },
                        }
                    }
                }
            }
        }

        // Bottom Navigation Bar
        NavigationBar {
            current_screen: current_screen(),
            on_navigate: move |screen| current_screen.set(screen),
        }
    }
}

#[component]
fn App() -> Element {
    use_init_i18n(i18n::init_i18n);

    // Provide PhotoGalleryContext to photo-gallery components (storage path from service)
    use_context_provider(|| PhotoGalleryContext::new(services::photo_service::get_storage_path()));

    let saved_settings =
        services::spacetime_settings_service::load_spacetime_settings().unwrap_or_default();
    let is_configured = saved_settings.is_spacetime_configured();

    // Prefer the persisted token helper so we keep identity semantics unchanged.
    let mut settings_for_runtime = saved_settings.clone();
    if let Some(saved_token) = spacetime::load_saved_token() {
        settings_for_runtime.token = saved_token;
    }

    let mut runtime_config =
        use_signal(move || SpacetimeRuntimeConfig::from_settings(&settings_for_runtime));
    let mut session_generation = use_signal_sync(|| 0_u64);

    // Navigate directly to Settings on first launch (no SpacetimeDB config yet)
    let current_screen = use_signal(move || {
        if is_configured {
            Screen::Home
        } else {
            Screen::Settings
        }
    });

    let on_spacetime_settings_saved = move |settings: SpacetimeSettings| {
        runtime_config.set(SpacetimeRuntimeConfig::from_settings(&settings));
        session_generation.with_mut(|generation| {
            *generation = generation.saturating_add(1);
        });
        log::info!(
            "SpacetimeDB settings updated, restarting connection session (generation={})",
            session_generation()
        );
    };

    let runtime = runtime_config();

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: BULMA_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        div { style: "display: flex; flex-direction: column; min-height: 100vh;",
            // We intentionally swap wrapper element types to force a hard subtree remount.
            // This reliably restarts the Spacetime provider effect after credentials are saved.
            if session_generation() % 2 == 0 {
                div {
                    SpacetimeSession {
                        current_screen,
                        spacetime_uri: runtime.uri,
                        spacetime_module: runtime.module,
                        saved_token: runtime.token,
                        on_spacetime_settings_saved,
                    }
                }
            } else {
                section {
                    SpacetimeSession {
                        current_screen,
                        spacetime_uri: runtime.uri,
                        spacetime_module: runtime.module,
                        saved_token: runtime.token,
                        on_spacetime_settings_saved,
                    }
                }
            }
        }
    }
}
