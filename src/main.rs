use dioxus::prelude::*;
use dioxus_i18n::prelude::*;
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
    AddProfileScreen, EggHistoryScreen, EggTrackingScreen, EventAdd, EventEditScreen, HomeScreen,
    NavigationBar, ProfileDetailScreen, ProfileEditScreen, ProfileListScreen, SettingsScreen,
    StatisticsScreen,
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
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
    use jni::errors::Error as JniError;
    use jni::objects::JObject;
    use ndk_context::android_context;

    let vm_ptr = android_context().vm() as *mut jni::sys::JavaVM;
    let context_ptr = android_context().context() as jni::sys::jobject;

    if vm_ptr.is_null() || context_ptr.is_null() {
        log::error!("Android TLS verifier init failed: missing VM or context pointer");
        return;
    }

    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) };
    match vm.attach_current_thread(|env| -> Result<(), JniError> {
        let context_global = unsafe { JObject::from_raw(env, context_ptr) };
        let context_local = env.new_local_ref(&context_global)?;
        std::mem::forget(context_global);

        rustls_platform_verifier::android::init_with_env(env, context_local)
    }) {
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
    EggHistory,
    Statistics,
    Settings,
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
fn App() -> Element {
    let mut current_screen = use_signal(|| Screen::Home);
    use_init_i18n(i18n::init_i18n);

    // Provide PhotoGalleryContext to photo-gallery components (storage path from service)
    use_context_provider(|| PhotoGalleryContext::new(services::photo_service::get_storage_path()));

    // Load saved SpacetimeDB authentication token from persistent storage
    let saved_settings =
        services::spacetime_settings_service::load_spacetime_settings().unwrap_or_default();
    let spacetime_uri = if !saved_settings.server_url.is_empty() {
        saved_settings.server_url.clone()
    } else {
        "http://127.0.0.1:3000".to_string()
    };
    let spacetime_module = if !saved_settings.database_name.is_empty() {
        saved_settings.database_name.clone()
    } else {
        "stalltagebuch-server".to_string()
    };

    // Load the saved authentication token to maintain SpacetimeDB client identity across restarts
    let saved_token = spacetime::load_saved_token();

    // Provide the generated SpacetimeDB context tree-wide.
    // Pass the saved token so we reconnect as the same client identity.
    let _spacetime_ctx =
        spacetime::use_spacetimedb_context_provider(&spacetime_uri, &spacetime_module, saved_token);

    // Watch for successful connections and persist the authentication token
    let _token_persist = spacetime::use_persist_spacetime_token();

    // Automatically register this device when connected to SpacetimeDB
    let _device_registration = spacetime::use_register_device();

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        div { style: "display: flex; flex-direction: column; height: 100vh; font-family: sans-serif;",
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
                    Screen::EggHistory => rsx! {
                        EggHistoryScreen { on_navigate: move |s| current_screen.set(s) }
                    },
                    Screen::Statistics => rsx! {
                        StatisticsScreen { on_navigate: move |s| current_screen.set(s) }
                    },
                    Screen::Settings => rsx! {
                        SettingsScreen { on_navigate: move |s| current_screen.set(s) }
                    },
                }
            }

            // Bottom Navigation Bar
            NavigationBar {
                current_screen: current_screen(),
                on_navigate: move |screen| current_screen.set(screen),
            }
        }
    }
}
