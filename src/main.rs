use dioxus::prelude::*;
use dioxus_i18n::prelude::*;
use photo_gallery::PhotoGalleryContext;

mod camera;
mod components;
mod database;
mod error;
mod i18n;
mod image_processing;
mod models;
mod services;

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
        log::error!("Encountered panic: {:?}", info);
        log::error!("Backtrace:\n{:?}", std::backtrace::Backtrace::capture());
    }));
    log::info!("App start: Stalltagebuch wird gestartet");
    dioxus::launch(App);
}

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
fn App() -> Element {
    let mut current_screen = use_signal(|| Screen::Home);
    use_init_i18n(i18n::init_i18n);

    // Provide PhotoGalleryContext to photo-gallery components (storage path from service)
    use_context_provider(|| PhotoGalleryContext::new(services::photo_service::get_storage_path()));

    // Auto-start background sync if configured
    use_effect(move || match database::init_database() {
        Ok(conn) => match services::sync_service::load_sync_settings(&conn) {
            Ok(Some(settings)) if settings.enabled => {
                log::info!("Auto-starting background sync");
                services::background_sync::start_background_sync();
            }
            _ => {}
        },
        Err(e) => {
            log::error!("Failed to check sync settings: {}", e);
        }
    });

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        div { style: "display: flex; flex-direction: column; height: 100vh; font-family: sans-serif;",

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
