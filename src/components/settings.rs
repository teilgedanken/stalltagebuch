use crate::Screen;
use crate::models::{SpacetimeSettings, SyncSettings};
use crate::services::export_service::ExportProgress;
use crate::services::{export_service, spacetime_settings_service};
use crate::spacetime::{ConnectionState, use_spacetimedb_context};
use chrono::{Local, TimeZone};
use dioxus::prelude::*;
use dioxus_i18n::t;
use serde::{Deserialize, Serialize};

fn format_hms(ts_ms: i64) -> String {
    match Local.timestamp_millis_opt(ts_ms).single() {
        Some(dt) => dt.format(&t!("log-time-format")).to_string(), // the format string for the chrono format time.
        None => String::from("--:--:--"),
    }
}

// ─── SpacetimeDB settings card ────────────────────────────────────────────────

/// Card component that displays connection status and allows configuring
/// SpacetimeDB connection settings.
/// Note: Full integration with the generated SDK hooks is in progress.
#[component]
fn SpacetimeDbCard() -> Element {
    let ctx = use_spacetimedb_context();

    // Load persisted settings on mount.
    let saved = spacetime_settings_service::load_spacetime_settings().unwrap_or_default();
    let mut server_url = use_signal(|| saved.server_url.clone());
    let mut database_name = use_signal(|| saved.database_name.clone());
    let mut token = use_signal(|| saved.token.clone());
    let mut status_msg = use_signal(|| String::new());

    let conn_state = ctx.state;

    let save_and_connect = move |_| {
        let url = server_url();
        let db = database_name();
        let tok = token();

        if url.is_empty() || db.is_empty() {
            status_msg.set("⚠️ Please fill in server URL, database name".to_string());
            return;
        }

        let settings = SpacetimeSettings {
            server_url: url.clone(),
            database_name: db.clone(),
            token: tok.clone(),
            ..spacetime_settings_service::load_spacetime_settings().unwrap_or_default()
        };
        match spacetime_settings_service::save_spacetime_settings(&settings) {
            Ok(()) => {
                status_msg.set("✅ Settings saved.".to_string());
                // TODO: Trigger reconnection with new settings via generated SDK hooks
            }
            Err(e) => {
                status_msg.set(format!("❌ Save failed: {e}"));
                return;
            }
        }
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "🗄️ SpacetimeDB" }

            // Connection state indicator
            {
                match conn_state() {
                    ConnectionState::Disconnected => rsx! {
                        p { style: "margin: 0 0 10px 0; color: #888;", "Not connected" }
                    },
                    ConnectionState::Connecting => rsx! {
                        p { style: "margin: 0 0 10px 0; color: #f57c00;", "⏳ Connecting…" }
                    },
                    ConnectionState::Connected(_, _) => rsx! {
                        p { style: "margin: 0 0 10px 0; color: #2e7d32;", "✅ Connected" }
                    },
                    ConnectionState::Error => rsx! {
                        p { style: "margin: 0 0 10px 0; color: #c62828;", "❌ Error connecting" }
                    },
                }
            }

            // Server URL
            label { style: "display: block; font-size: 13px; color: #555; margin-bottom: 4px;",
                "Server URL (e.g. https://testnet.spacetimedb.com)"
            }
            input {
                r#type: "text",
                value: "{server_url}",
                style: "width: 100%; box-sizing: border-box; padding: 8px; border: 1px solid #ddd; border-radius: 6px; font-size: 14px; margin-bottom: 10px;",
                oninput: move |e| server_url.set(e.value()),
            }

            // Database name
            label { style: "display: block; font-size: 13px; color: #555; margin-bottom: 4px;",
                "Database name (e.g. stalltagebuch)"
            }
            input {
                r#type: "text",
                value: "{database_name}",
                style: "width: 100%; box-sizing: border-box; padding: 8px; border: 1px solid #ddd; border-radius: 6px; font-size: 14px; margin-bottom: 10px;",
                oninput: move |e| database_name.set(e.value()),
            }

            // Auth token
            label { style: "display: block; font-size: 13px; color: #555; margin-bottom: 4px;",
                "Auth token (from spacetime login)"
            }
            input {
                r#type: "text",
                value: "{token}",
                style: "width: 100%; box-sizing: border-box; padding: 8px; border: 1px solid #ddd; border-radius: 6px; font-size: 14px; margin-bottom: 12px;",
                oninput: move |e| token.set(e.value()),
            }

            div { style: "display: flex; gap: 8px;",
                button {
                    class: "btn-primary",
                    style: "flex: 1;",
                    onclick: save_and_connect,
                    "💾 Save Settings"
                }
            }

            if !status_msg().is_empty() {
                p { style: "margin: 8px 0 0 0; font-size: 13px; color: #555;", "{status_msg}" }
            }

            details { style: "margin-top: 12px;",
                summary { style: "cursor: pointer; font-size: 13px; color: #0066cc;",
                    "ℹ️ How to deploy the server module"
                }
                div { style: "margin-top: 8px; font-size: 13px; color: #555; line-height: 1.6;",
                    p {
                        "1. Install the SpacetimeDB CLI from "
                        a {
                            href: "https://spacetimedb.com/install",
                            target: "_blank",
                            "spacetimedb.com/install"
                        }
                        " (verify the script before running)."
                    }
                    p {
                        "2. From the project root: "
                        code {
                            "spacetime publish stalltagebuch-server --project-path stalltagebuch-server"
                        }
                    }
                    p {
                        "3. Get your token: "
                        code { "spacetime login" }
                        " (or use an identity token from the web console)."
                    }
                    p {
                        "Dioxus bindings are auto-generated with: "
                        code {
                            "spacetime generate --lang dioxus --out-dir src/spacetime_module_bindings --module-path stalltagebuch-server"
                        }
                    }
                }
            }
        }
    }
}

// ─── Devices card ─────────────────────────────────────────────────────────────

/// Card component that displays all registered devices and highlights the current one.
#[component]
fn DevicesCard() -> Element {
    // Subscribe to the devices table
    crate::spacetime::use_subscription(&["SELECT * FROM devices"]);

    let devices = crate::spacetime_module_bindings::dioxus::use_table_devices();
    let mut current_device_id = use_signal(|| String::new());

    // Get current device ID on mount
    use_effect(
        move || match crate::services::device_id_service::get_device_id() {
            Ok(id) => {
                current_device_id.set(id);
            }
            Err(e) => {
                log::error!("Failed to get current device ID: {}", e);
            }
        },
    );

    let devices_vec = devices().clone();
    let current_id = current_device_id();

    // Format timestamp from i64 (seconds) to readable string
    let format_timestamp = |seconds: i64| -> String {
        if let Some(dt) = Local.timestamp_opt(seconds, 0).single() {
            dt.format("%d.%m.%Y %H:%M:%S").to_string()
        } else {
            "Unknown".to_string()
        }
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "📱 Geräte" }

            if devices_vec.is_empty() {
                p { style: "margin: 0; color: #888; font-size: 14px;",
                    "Keine Geräte registriert"
                }
            } else {
                div { style: "display: flex; flex-direction: column; gap: 8px;",
                    {
                        devices_vec.iter().map(|device| {
                            let is_current = device.device_id == current_id;
                            let bg_color = if is_current { "#e3f2fd" } else { "#f5f5f5" };
                            let border_color = if is_current { "4px solid #0066cc" } else { "1px solid #ddd" };

                            rsx! {
                                div { style: "padding: 12px; border-radius: 6px; border-left: {border_color}; background: {bg_color}; display: flex; align-items: center; gap: 12px;",
                                    div { style: "flex: 1;",
                                        div { style: "display: flex; align-items: center; gap: 8px; margin-bottom: 6px;",
                                            span { style: "font-weight: 600; font-size: 14px;",
                                                "{device.name.as_ref().unwrap_or(&device.device_id)}"
                                            }
                                            if is_current {
                                                span { style: "background: #0066cc; color: white; padding: 2px 8px; border-radius: 12px; font-size: 11px; font-weight: 600;",
                                                    "Dieses Gerät"
                                                }
                                            }
                                        }
                                        p { style: "margin: 0; font-size: 12px; color: #666;",
                                            "Zuletzt gesehen: {format_timestamp(device.last_seen)}"
                                        }
                                        if device.first_seen != device.last_seen {
                                            p { style: "margin: 4px 0 0 0; font-size: 12px; color: #999;",
                                                "Registriert: {format_timestamp(device.first_seen)}"
                                            }
                                        }
                                    }
                                }
                            }
                        })
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum NetworkStatus {
    Checking,
    Online,
    Offline(String),
}

#[component]
fn NetworkCheckCard() -> Element {
    let mut network_status = use_signal(|| NetworkStatus::Checking);

    // Check network connectivity on mount
    use_effect(move || {
        spawn(async move {
            // Try to connect to a reliable service
            match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
            {
                Ok(client) => {
                    match client
                        .get("https://www.google.com/generate_204")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.status().is_success() || response.status().as_u16() == 204 {
                                network_status.set(NetworkStatus::Online);
                            } else {
                                network_status.set(NetworkStatus::Offline(format!(
                                    "HTTP Status: {}",
                                    response.status()
                                )));
                            }
                        }
                        Err(e) => {
                            network_status.set(NetworkStatus::Offline(format!(
                                "{}: {}",
                                t!("error-network"),
                                e
                            )));
                        }
                    }
                }
                Err(e) => {
                    network_status.set(NetworkStatus::Offline(format!(
                        "{}: {}",
                        t!("error-client"),
                        e
                    )));
                }
            }
        });
    });

    let recheck = move |_| {
        network_status.set(NetworkStatus::Checking);
        spawn(async move {
            match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
            {
                Ok(client) => {
                    match client
                        .get("https://www.google.com/generate_204")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.status().is_success() || response.status().as_u16() == 204 {
                                network_status.set(NetworkStatus::Online);
                            } else {
                                network_status.set(NetworkStatus::Offline(format!(
                                    "HTTP Status: {}",
                                    response.status()
                                )));
                            }
                        }
                        Err(e) => {
                            network_status.set(NetworkStatus::Offline(format!(
                                "{}: {}",
                                t!("error-network"),
                                e
                            )));
                        }
                    }
                }
                Err(e) => {
                    network_status.set(NetworkStatus::Offline(format!(
                        "{}: {}",
                        t!("error-client"),
                        e
                    )));
                }
            }
        });
    };

    rsx! {
        match network_status() {
            NetworkStatus::Checking => rsx! {
                div { class: "card", style: "margin-bottom: 16px;",
                    div { style: "display: flex; align-items: center; gap: 12px;",
                        div { style: "font-size: 24px;", "🔄" }
                        div {
                            p { style: "margin: 0; font-weight: 600; font-size: 14px;", {t!("network-checking")} } // Network connectivity check in progress
                        }
                    }
                }
            },
            NetworkStatus::Online => rsx! {},
            NetworkStatus::Offline(error) => rsx! {
                div { class: "card", style: "margin-bottom: 16px;",
                    div {
                        div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 12px;",
                            div { style: "font-size: 24px;", "❌" }
                            div {
                                p {
                                    style: "margin: 0; font-weight: 600; font-size: 14px; color: #c62828;",
                                    {t!("network-offline")} // No internet connection message
                                }
                                p { style: "margin: 0; font-size: 12px; color: #666;", "{error}" }
                            }
                        }
                        button { class: "btn-primary", style: "width: 100%;", onclick: recheck,
                            "🔄 "
                            {t!("action-retry")}
                        }
                    }
                }
            },
        }
    }
}

// ─── Export card ──────────────────────────────────────────────────────────────

/// Card component that manages data export to ZIP files.
#[component]
fn ExportCard() -> Element {
    let mut export_progress = use_signal(|| None::<ExportProgress>);
    let mut export_status = use_signal(|| String::new());
    let mut is_exporting = use_signal(|| false);

    let handle_export = move |_| {
        if is_exporting() {
            return; //Prevent multiple simultaneous exports
        }

        is_exporting.set(true);
        export_status.set(t!("export-in-progress"));
        export_progress.set(Some(ExportProgress::Starting));

        spawn(async move {
            let mut progress_sig = export_progress;
            let mut status_sig = export_status;
            let mut exporting_sig = is_exporting;

            match export_service::export_to_zip(move |p| {
                progress_sig.with_mut(|s| *s = Some(p));
            })
            .await
            {
                Ok(path) => {
                    status_sig.with_mut(|s| {
                        *s = format!("✅ {}\n📁 {}", t!("export-success"), path.display())
                    });
                    progress_sig.with_mut(|s| *s = Some(ExportProgress::Complete));
                }
                Err(e) => {
                    status_sig.with_mut(|s| *s = format!("❌ {}: {}", t!("export-failed"), e));
                    progress_sig.with_mut(|s| *s = None);
                }
            }
            exporting_sig.set(false);
        });
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "💾 " {t!("export-title")} }

            p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;",
                {t!("export-description")}
            }

            if let Some(progress) = export_progress() {
                div { style: "padding: 8px; background: #f0f0f0; border-radius: 6px; margin-bottom: 12px; font-size: 12px;",
                    match progress {
                        ExportProgress::Starting => rsx! { "🔄 Initializing…" },
                        ExportProgress::ReadingQuails => rsx! { "📚 Reading quails…" },
                        ExportProgress::ReadingEvents => rsx! { "📅 Reading events…" },
                        ExportProgress::ReadingEggRecords => rsx! { "🥚 Reading egg records…" },
                        ExportProgress::ReadingPhotos => rsx! { "📷 Reading photos…" },
                        ExportProgress::PackingZip => rsx! { "📦 Creating ZIP…" },
                        ExportProgress::Complete => rsx! { "✅ Complete!" },
                    }
                }
            }

            button {
                class: "btn-primary",
                style: "width: 100%;",
                disabled: is_exporting(),
                onclick: handle_export,
                if is_exporting() { "⏳ Exporting…" } else { "📤 Export Now" }
            }

            if !export_status().is_empty() {
                p { style: "margin: 8px 0 0 0; font-size: 12px; color: #555; white-space: pre-wrap;",
                    "{export_status}"
                }
            }
        }
    }
}

// ─── Import card ──────────────────────────────────────────────────────────────

/// Card component that manages data import from ZIP files.
#[component]
fn ImportCard() -> Element {
    let import_progress = use_signal(|| None::<String>);
    let mut import_status = use_signal(|| String::new());
    let mut is_importing = use_signal(|| false);

    let handle_import = move |_| {
        if is_importing() {
            return; // Prevent multiple simultaneous imports
        }

        // Trigger Android file picker
        #[cfg(target_os = "android")]
        {
            spawn(async move {
                if let Err(e) = crate::camera::launch_document_picker() {
                    import_status.set(format!("❌ {}: {}", t!("import-failed"), e));
                    return;
                }

                // Wait for the user to pick a document and for MainActivity to copy it.
                let mut selected_path = None;
                for _ in 0..120 {
                    if let Some(path) = crate::camera::get_last_document_path() {
                        selected_path = Some(path);
                        break;
                    }

                    if let Some(err) = crate::camera::get_last_error() {
                        import_status.set(format!("❌ {}: {}", t!("import-failed"), err));
                        return;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }

                if let Some(path) = selected_path {
                    is_importing.set(true);
                    import_status.set(t!("import-in-progress"));

                    let mut progress_sig = import_progress;
                    let mut status_sig = import_status;
                    let mut importing_sig = is_importing;

                    match crate::services::import_service::import_from_zip(&path, move |msg| {
                        progress_sig.with_mut(|s| *s = Some(msg));
                    })
                    .await
                    {
                        Ok((count, photo_count)) => {
                            status_sig.with_mut(|s| {
                                *s = format!(
                                    "✅ {} ({} items, {} photos)",
                                    t!("import-success"),
                                    count,
                                    photo_count
                                )
                            });
                            progress_sig.with_mut(|s| *s = None);
                        }
                        Err(e) => {
                            status_sig
                                .with_mut(|s| *s = format!("❌ {}: {}", t!("import-failed"), e));
                            progress_sig.with_mut(|s| *s = None);
                        }
                    }
                    importing_sig.set(false);
                } else {
                    import_status.set(format!("❌ {}: no file selected", t!("import-failed")));
                }
            });
        }

        // Fallback for non-Android
        #[cfg(not(target_os = "android"))]
        {
            import_status.set("⚠️ File picker only available on Android".to_string());
        }
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "📂 " {t!("import-title")} }

            p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;",
                {t!("import-description")}
            }

            if let Some(progress) = import_progress() {
                div { style: "padding: 8px; background: #f0f0f0; border-radius: 6px; margin-bottom: 12px; font-size: 12px;",
                    "{progress}"
                }
            }

            button {
                class: "btn-primary",
                style: "width: 100%;",
                disabled: is_importing(),
                onclick: handle_import,
                if is_importing() { "⏳ Importing…" } else { "📥 Select ZIP File" }
            }

            if !import_status().is_empty() {
                p { style: "margin: 8px 0 0 0; font-size: 12px; color: #555; white-space: pre-wrap;",
                    "{import_status}"
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoginFlowInit {
    poll: PollInfo,
    login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PollInfo {
    token: String,
    endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoginFlowResult {
    server: String,
    #[serde(rename = "loginName")]
    login_name: String,
    #[serde(rename = "appPassword")]
    app_password: String,
}

#[derive(Clone, PartialEq)]
enum LoginState {
    NotStarted,
    InitiatingFlow,
    WaitingForUser {
        poll_url: String,
        token: String,
        login_url: String,
    },
    Success,
    Error(String),
}

#[derive(Clone, PartialEq)]
enum ConnectionStatus {
    Checking,
    Connected,
    Failed(String),
}

fn to_sync_settings(saved: &SpacetimeSettings) -> Option<SyncSettings> {
    if !saved.is_nextcloud_configured() || saved.nextcloud_remote_path.trim().is_empty() {
        return None;
    }

    Some(SyncSettings::new(
        saved.nextcloud_url.clone(),
        saved.nextcloud_username.clone(),
        saved.nextcloud_app_password.clone(),
        saved.nextcloud_remote_path.clone(),
    ))
}

#[component]
pub fn SettingsScreen(on_navigate: EventHandler<Screen>) -> Element {
    let mut server_url = use_signal(|| String::from("https://"));
    let mut remote_path = use_signal(|| String::from("/Stalltagebuch"));
    let mut login_state = use_signal(|| LoginState::NotStarted);
    let mut current_settings = use_signal(|| None::<SyncSettings>);
    let mut status_message = use_signal(|| String::new());
    // Separater bool für laufende Synchronisierung, damit Anzeige sicher zurückgesetzt wird
    let mut is_syncing = use_signal(|| false);
    let connection_status = use_signal(|| None::<ConnectionStatus>);
    let mut background_sync_running =
        use_signal(|| crate::services::background_sync::is_background_sync_running());
    // Live Countdown & Log
    let mut sync_eta = use_signal(|| crate::services::background_sync::next_sync_eta_seconds());
    let mut sync_log = use_signal(|| crate::services::background_sync::get_sync_log());

    // Ticker Effekt (1s Interval) aktualisiert ETA und Log ohne User-Interaktion
    use_effect(move || {
        // Spawn ticker loop (kein Cleanup nötig für einfache 1s Timer)
        spawn(async move {
            loop {
                sync_eta.set(crate::services::background_sync::next_sync_eta_seconds());
                sync_log.set(crate::services::background_sync::get_sync_log());
                background_sync_running
                    .set(crate::services::background_sync::is_background_sync_running());
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    });

    // Load existing settings on mount
    use_effect(move || {
        if let Ok(saved) = spacetime_settings_service::load_spacetime_settings() {
            if let Some(sync_cfg) = to_sync_settings(&saved) {
                server_url.set(sync_cfg.server_url.clone());
                remote_path.set(sync_cfg.remote_path.clone());
                current_settings.set(Some(sync_cfg));
                login_state.set(LoginState::Success);
            }
        }
    });

    // Start Nextcloud Login Flow v2
    let start_login = move |_| {
        let server = server_url();
        let remote_path_value = remote_path();
        login_state.set(LoginState::InitiatingFlow);

        spawn(async move {
            let url = format!("{}/index.php/login/v2", server.trim_end_matches('/'));

            // Create a properly configured HTTP client
            let client = match reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .connect_timeout(std::time::Duration::from_secs(10))
                .tcp_keepalive(std::time::Duration::from_secs(30))
                .user_agent("Stalltagebuch/0.1.0")
                .build()
            {
                Ok(client) => client,
                Err(e) => {
                    login_state.set(LoginState::Error(format!(
                        "{}: {:?}",
                        t!("error-client"),
                        e
                    )));
                    return;
                }
            };

            match client.post(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<LoginFlowInit>().await {
                            Ok(flow) => {
                                let poll_url = flow.poll.endpoint.clone();
                                let token = flow.poll.token.clone();
                                let login_url = flow.login.clone();

                                // Set state to show login URL
                                login_state.set(LoginState::WaitingForUser {
                                    poll_url: poll_url.clone(),
                                    token: token.clone(),
                                    login_url: login_url.clone(),
                                });

                                // Start polling immediately in background
                                spawn(async move {
                                    // Create a properly configured HTTP client for polling
                                    let poll_client = match reqwest::Client::builder()
                                        .timeout(std::time::Duration::from_secs(30))
                                        .connect_timeout(std::time::Duration::from_secs(10))
                                        .tcp_keepalive(std::time::Duration::from_secs(30))
                                        .user_agent("Stalltagebuch/0.1.0")
                                        .pool_idle_timeout(std::time::Duration::from_secs(90))
                                        .pool_max_idle_per_host(4)
                                        .build()
                                    {
                                        Ok(client) => client,
                                        Err(e) => {
                                            log::error!(
                                                "LoginFlow: HTTP-Client für Polling konnte nicht erstellt werden: {:?}",
                                                e
                                            );
                                            login_state.set(LoginState::Error(format!(
                                                "{}: {:?}",
                                                t!("error-client"),
                                                e
                                            )));
                                            return;
                                        }
                                    };

                                    // Small delay to ensure network is ready and user can open browser
                                    #[cfg(not(target_arch = "wasm32"))]
                                    {
                                        log::debug!(
                                            "LoginFlow: kurze Wartezeit vor Start des Pollings"
                                        );
                                        tokio::time::sleep(std::time::Duration::from_millis(500))
                                            .await;
                                    }
                                    #[cfg(target_arch = "wasm32")]
                                    gloo_timers::future::sleep(std::time::Duration::from_millis(
                                        500,
                                    ))
                                    .await;

                                    // Poll bis zu 60 Versuche (~5 Minuten bei 404, bei Netzfehlern mit Backoff)
                                    let mut consecutive_errors: u32 = 0;
                                    for attempt in 0..60 {
                                        log::debug!("LoginFlow: Polling Versuch {}", attempt + 1);

                                        // Standard-Wartezeit (wird in den Branches gesetzt)
                                        #[allow(unused_assignments)]
                                        let mut wait_after_secs: u64 = 5; // Startwert, wird in Branches überschrieben

                                        match poll_client
                                            .post(&poll_url)
                                            .form(&[("token", &token)])
                                            .header("User-Agent", "Stalltagebuch/0.1.0")
                                            .header("Accept", "application/json")
                                            .send()
                                            .await
                                        {
                                            Ok(response) => {
                                                if response.status().as_u16() == 200 {
                                                    log::info!(
                                                        "LoginFlow: Polling erfolgreich (200). Verarbeite Zugangsdaten…"
                                                    );
                                                    match response.json::<LoginFlowResult>().await {
                                                        Ok(result) => {
                                                            // Create WebDAV client and folder
                                                            let webdav_url = format!(
                                                                "{}/remote.php/dav/files/{}",
                                                                result.server.trim_end_matches('/'),
                                                                result.login_name
                                                            );

                                                            match reqwest_dav::ClientBuilder::new()
                                                                .set_host(webdav_url)
                                                                .set_auth(reqwest_dav::Auth::Basic(
                                                                    result.login_name.clone(),
                                                                    result.app_password.clone(),
                                                                ))
                                                                .build()
                                                            {
                                                                Ok(client) => {
                                                                    // Try to create the folder
                                                                    match client
                                                                        .mkcol(&remote_path_value)
                                                                        .await
                                                                    {
                                                                        Ok(_) => {
                                                                            log::info!(
                                                                                "LoginFlow: Remote-Ordner erstellt: {}",
                                                                                remote_path_value
                                                                            );
                                                                        }
                                                                        Err(e) => {
                                                                            // Folder might already exist (405)
                                                                            log::debug!(
                                                                                "LoginFlow: Ordner-Erstellung Hinweis (evtl. bereits vorhanden): {}",
                                                                                e
                                                                            );
                                                                        }
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    log::error!(
                                                                        "LoginFlow: WebDAV-Client Fehler: {:?}",
                                                                        e
                                                                    );
                                                                    login_state.set(LoginState::Error(
                                                                        format!("{}: {:?}", t!("error-webdav-client"), e),
                                                                    ));
                                                                    return;
                                                                }
                                                            }

                                                            // Save credentials
                                                            let settings = SyncSettings::new(
                                                                result.server,
                                                                result.login_name,
                                                                result.app_password,
                                                                remote_path_value.clone(),
                                                            );

                                                            if let Ok(mut st_settings) = spacetime_settings_service::load_spacetime_settings() {
                                                                st_settings.nextcloud_url = settings.server_url.clone();
                                                                st_settings.nextcloud_username = settings.username.clone();
                                                                st_settings.nextcloud_app_password = settings.app_password.clone();
                                                                st_settings.nextcloud_remote_path = settings.remote_path.clone();

                                                                if let Err(e) = spacetime_settings_service::save_spacetime_settings(&st_settings) {
                                                                    log::error!("Failed to persist Nextcloud settings: {}", e);
                                                                }
                                                            }

                                                            current_settings.set(Some(settings));
                                                            login_state.set(LoginState::Success);
                                                            status_message.set(format!(
                                                                "\u{2705} {}",
                                                                t!("sync-login-success-folder")
                                                            ));
                                                            log::info!(
                                                                "LoginFlow: Zugangsdaten gespeichert und Login abgeschlossen."
                                                            );
                                                            return;
                                                        }
                                                        Err(e) => {
                                                            log::error!(
                                                                "LoginFlow: JSON-Parse der Poll-Antwort fehlgeschlagen: {}",
                                                                e
                                                            );
                                                            login_state.set(LoginState::Error(
                                                                format!(
                                                                    "{}: {}",
                                                                    t!("error-json"),
                                                                    e
                                                                ),
                                                            ));
                                                            return;
                                                        }
                                                    }
                                                } else if response.status().as_u16() != 404 {
                                                    log::warn!(
                                                        "LoginFlow: Unerwarteter HTTP-Status beim Polling: {}",
                                                        response.status()
                                                    );
                                                    login_state.set(LoginState::Error(format!(
                                                        "{}: {}",
                                                        t!("error-unexpected-status"),
                                                        response.status()
                                                    )));
                                                    return;
                                                }
                                                // 404 means waiting, continue polling
                                                log::debug!(
                                                    "LoginFlow: Polling noch nicht bestätigt (404). Weiter warten…"
                                                );
                                                consecutive_errors = 0; // reset on valid response
                                                wait_after_secs = 5;
                                            }
                                            Err(e) => {
                                                // Netzfehler: mit Exponential-Backoff weiterprobieren statt früh abzubrechen
                                                consecutive_errors =
                                                    consecutive_errors.saturating_add(1);

                                                let kind = if e.is_timeout() {
                                                    "timeout"
                                                } else if e.is_connect() {
                                                    "connect"
                                                } else if e.is_request() {
                                                    "request"
                                                } else {
                                                    "other"
                                                };

                                                // Backoff: 5s, 10s, 20s, dann Deckel 30s
                                                let backoff = 5u64.saturating_mul(
                                                    1u64 << (consecutive_errors
                                                        .saturating_sub(1)
                                                        .min(2))
                                                        as u32,
                                                );
                                                wait_after_secs = backoff.min(30);

                                                log::warn!(
                                                    "LoginFlow: Netzfehler beim Polling ({} in Folge, Typ: {}): {} – Backoff {}s",
                                                    consecutive_errors,
                                                    kind,
                                                    e,
                                                    wait_after_secs
                                                );
                                            }
                                        }

                                        // Warten vor nächstem Poll (404: 5s, Netzfehler: Backoff)
                                        #[cfg(not(target_arch = "wasm32"))]
                                        {
                                            tokio::time::sleep(std::time::Duration::from_secs(
                                                wait_after_secs,
                                            ))
                                            .await;
                                        }
                                        #[cfg(target_arch = "wasm32")]
                                        gloo_timers::future::sleep(std::time::Duration::from_secs(
                                            wait_after_secs,
                                        ))
                                        .await;
                                    }

                                    log::error!("LoginFlow: Polling-Timeout nach 5 Minuten.");
                                    login_state.set(LoginState::Error(
                                        t!("error-login-timeout").to_string(),
                                    ));
                                });
                            }
                            Err(e) => {
                                log::error!(
                                    "LoginFlow: JSON-Parse der Flow-Initialisierung fehlgeschlagen: {}",
                                    e
                                );
                                login_state.set(LoginState::Error(format!(
                                    "{}: {}",
                                    t!("error-json"),
                                    e
                                )));
                            }
                        }
                    } else {
                        log::warn!(
                            "LoginFlow: Server antwortete mit Status {} bei Flow-Start",
                            response.status()
                        );
                        login_state.set(LoginState::Error(format!(
                            "{}: {}",
                            t!("error-server"),
                            response.status()
                        )));
                    }
                }
                Err(e) => {
                    log::error!("LoginFlow: Verbindungsfehler beim Flow-Start: {}", e);
                    login_state.set(LoginState::Error(format!(
                        "{}: {}",
                        t!("error-connection"),
                        e
                    )));
                }
            }
        });
    };

    let delete_settings = move |_| {
        if let Ok(mut st_settings) = spacetime_settings_service::load_spacetime_settings() {
            st_settings.nextcloud_url.clear();
            st_settings.nextcloud_username.clear();
            st_settings.nextcloud_app_password.clear();
            st_settings.nextcloud_remote_path.clear();
            if let Err(e) = spacetime_settings_service::save_spacetime_settings(&st_settings) {
                log::error!("Failed to remove Nextcloud settings: {}", e);
            }
        }
        current_settings.set(None);
        login_state.set(LoginState::NotStarted);
        status_message.set(format!("\u{2705} {}", t!("sync-settings-deleted")));
    };

    rsx! {
        div { style: "padding: 16px; max-width: 600px; margin: 0 auto;",
            // Header
            div { style: "display: flex; align-items: center; margin-bottom: 24px;",
                button {
                    class: "btn-back",
                    onclick: move |_| on_navigate.call(Screen::Home),
                    "← "
                    {t!("action-back")}
                }
                h1 { style: "flex: 1; text-align: center; margin: 0; font-size: 24px; color: #0066cc;",
                    "⚙️ "
                    {t!("settings-title")}
                }
                div { style: "width: 80px;" }
            }

            // Statusanzeige
            if is_syncing() {
                div { style: "padding: 12px; margin-bottom: 16px; background: #fff3cd; border-radius: 8px; border-left: 4px solid #ffb300; display: flex; align-items: center; gap: 8px;",
                    span { style: "font-size: 18px;", "🔄" }
                    span { "{status_message}" }
                }
            } else if !status_message().is_empty() {
                div { style: "padding: 12px; margin-bottom: 16px; background: #f0f0f0; border-radius: 8px; border-left: 4px solid #0066cc;",
                    "{status_message}"
                }
            }

            // Network connectivity check
            NetworkCheckCard {}

            // ── SpacetimeDB connection ─────────────────────────────────────
            SpacetimeDbCard {}

            // ── Registered devices ─────────────────────────────────────────
            DevicesCard {}

            // ── Export/Import ──────────────────────────────────────────────
            ExportCard {}
            ImportCard {}

            // Current settings display (Nextcloud photo storage)
            if let Some(settings) = current_settings() {
                div {
                    class: "card",
                    style: "margin-bottom: 16px; background: #e8f5e9;",
                    h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #2e7d32;",
                        "\u{2705} " // Sync configured successfully heading
                        {t!("sync-configured")}
                    }
                    p { style: "margin: 4px 0; font-size: 14px;",
                        strong {
                            {t!("sync-server")}
                            ": "
                        } // Server URL label
                        "{settings.server_url}"
                    }
                    p { style: "margin: 4px 0; font-size: 14px;",
                        strong {
                            {t!("sync-username")}
                            ": "
                        } // Username label
                        "{settings.username}"
                    }
                    p { style: "margin: 4px 0; font-size: 14px;",
                        strong {
                            {t!("sync-path")}
                            ": "
                        } // Remote path label
                        "{settings.remote_path}"
                        " "
                        match connection_status() {
                            Some(ConnectionStatus::Checking) => rsx! {
                                span { class: "spinner", style: "font-size: 12px;", "⏳" }
                            },
                            Some(ConnectionStatus::Connected) => rsx! {
                                span { style: "color: green; font-weight: bold;", "✓" }
                            },
                            Some(ConnectionStatus::Failed(ref err)) => rsx! {
                                span { style: "color: red; font-weight: bold;", title: "{err}", "⚠️" }
                            },
                            None => rsx! {
                                span {}
                            },
                        }
                    }
                    if let Some(last_sync) = settings.last_sync {
                        p { style: "margin: 4px 0; font-size: 14px;",
                            strong {
                                {t!("sync-last-sync")}
                                ": "
                            } // Last sync timestamp label
                            "{last_sync}"
                        }
                    }

                    div { style: "display: flex; gap: 12px; margin-top: 12px;",
                        button {
                            class: "btn-primary",
                            style: "flex: 1;",
                            onclick: move |_| {
                                spawn(async move {
                                    is_syncing.set(true);
                                    status_message.set("🔄 Vollständige Synchronisation...".to_string());
                                    match crate::services::background_sync::sync_now().await {
                                        Ok(stats) => {
                                            status_message
                                                .set(
                                                    format!(
                                                        "✅ Sync erfolgreich: {} Ops heruntergeladen, {} Fotos hochgeladen",
                                                        stats.operations_downloaded,
                                                        stats.photos_uploaded,
                                                    ),
                                                );
                                            // Settings will be reloaded via SpacetimeDB subscriptions
                                        }
                                        Err(e) => {
                                            status_message.set(format!("❌ {}: {}", t!("sync-failed"), e));
                                        }
                                    }
                                    is_syncing.set(false);
                                });
                            },
                            {format!("🔄 {}", t!("sync-now"))}
                        }
                        button {
                            class: "btn-danger",
                            style: "flex: 1;",
                            onclick: delete_settings,
                            "🗑️ "
                            {t!("sync-delete-config")}
                        }
                    }

                    // Background sync toggle
                    div { style: "margin-top: 16px; padding: 12px; background: #f0f7ff; border-radius: 8px; border-left: 4px solid #0066cc;",
                        div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px;",
                            div {
                                p { style: "margin: 0; font-weight: 600; font-size: 14px;",
                                    "🔄 Automatische Synchronisation"
                                }
                                p { style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                                    {
                                        let interval = crate::services::background_sync::sync_interval_seconds();
                                        format!("Synchronisiert alle {} Sekunden im Hintergrund", interval)
                                    }
                                }
                            }
                            button {
                                class: if background_sync_running() { "btn-danger" } else { "btn-primary" },
                                onclick: move |_| {
                                    if background_sync_running() {
                                        crate::services::background_sync::stop_background_sync();
                                        background_sync_running.set(false);
                                        status_message
                                            .set("⏸️ Automatische Synchronisation gestoppt".to_string());
                                    } else {
                                        crate::services::background_sync::start_background_sync();
                                        background_sync_running.set(true);
                                        status_message
                                            .set("▶️ Automatische Synchronisation gestartet".to_string());
                                    }
                                },
                                if background_sync_running() {
                                    "⏸️ Stoppen"
                                } else {
                                    "▶️ Starten"
                                }
                            }
                        }
                        if background_sync_running() {
                            p { style: "margin: 8px 0 0 0; font-size: 12px; color: #2e7d32; font-weight: 600;",
                                "✓ Läuft im Hintergrund – nächster Sync in: "
                                span { style: "font-weight: 700;", "{sync_eta().unwrap_or(0)}s" }
                            }
                        }
                    }

                    // Photo Upload Progress
                    {
                        let mut upload_progress = use_signal(|| (0usize, 0usize));
                        use_coroutine(move |_: UnboundedReceiver<()>| async move {
                            let mut rx = crate::services::background_sync::subscribe_upload_progress();
                            loop {
                                match rx.changed().await {
                                    Ok(_) => {
                                        let progress = *rx.borrow_and_update();
                                        upload_progress.set(progress);
                                    }
                                    Err(_) => break,
                                }
                            }
                        });
                        let (current, total) = upload_progress();
                        if total > 0 {
                            let percent = if total > 0 {
                                (current as f64 / total as f64 * 100.0) as usize
                            } else {
                                0
                            };
                            rsx! {
                                div { style: "margin-top: 16px; padding: 12px; background: #fff3cd; border-radius: 8px; border-left: 4px solid #ffb300;",
                                    div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 8px;",
                                        span { style: "font-size: 24px;", "📤" }
                                        div { style: "flex: 1;",
                                            p { style: "margin: 0; font-weight: 600; font-size: 14px;", "Fotos werden hochgeladen..." }
                                            p { style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                                                "{current} von {total} Fotos hochgeladen ({percent}%)"
                                            }
                                        }
                                    }
                                    div { style: "width: 100%; background: #e0e0e0; border-radius: 4px; height: 8px; overflow: hidden;",
                                        div { style: "height: 100%; background: linear-gradient(90deg, #0066cc, #0088ff); transition: width 0.3s ease; width: {percent}%;" }
                                    }
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }

                    // Session Sync Log Anzeige
                    div { style: "margin-top: 16px; padding: 12px; background: #fff; border-radius: 8px; border: 1px solid #ddd;",
                        h3 { style: "margin: 0 0 8px 0; font-size: 16px;",
                            "📝 Sync Sitzung (flüchtig)"
                        }
                        {
                            let log_entries = sync_log();
                            if log_entries.is_empty() {
                                rsx! {
                                    p { style: "margin: 0; font-size: 12px; color: #666;", "Noch keine Einträge" }
                                }
                            } else {
                                rsx! {
                                    div { style: "display: flex; flex-direction: column; gap: 4px; max-height: 180px; overflow-y: auto;",
                                        for entry in log_entries {
                                            div { style: "font-size: 12px; padding: 4px 6px; background: #f8f9fa; border-radius: 4px; border-left: 3px solid #0066cc;",
                                                span { style: "color: #333;",
                                                    "{format_hms(entry.ts_ms)}: Ops {entry.operations_downloaded} · Fotos {entry.photos_uploaded}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Cleanup orphaned photos
                    div { style: "margin-top: 16px; padding: 12px; background: #fff; border-radius: 8px; border: 1px solid #ddd;",
                        h3 { style: "margin: 0 0 8px 0; font-size: 16px;",
                            {t!("backup-cleanup-title")}
                        }
                        p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;",
                            {t!("backup-cleanup-description")}
                        }
                        button {
                            class: "btn-danger",
                            style: "width: 100%;",
                            onclick: {
                                let mut status_message = status_message.clone();
                                move |_| {
                                    let confirmed = if cfg!(target_os = "android") { true } else { true };
                                    if confirmed {
                                        status_message.set(t!("backup-db-error", error : "Feature not yet available for SpacetimeDB".to_string()));
                                    }
                                }
                            },
                            {t!("backup-cleanup-button")}
                        }
                    }

                    // Daten-Export / -Import
                    div { style: "margin-top: 16px; padding: 12px; background: #fff; border-radius: 8px; border: 1px solid #ddd;",
                        h3 { style: "margin: 0 0 8px 0; font-size: 16px;",
                            {t!("backup-export-title")}
                        }
                        p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;",
                            {t!("backup-export-description")}
                        }
                        div { style: "display: flex; flex-direction: column; gap: 8px;",
                            button {
                                class: "btn-primary",
                                style: "width: 100%;",
                                onclick: {
                                    let mut status_message = status_message.clone();
                                    move |_| {
                                        status_message.set(t!("backup-db-error", error : "Feature not yet available for SpacetimeDB".to_string()));
                                    }
                                },
                                {t!("backup-export-button")}
                            }
                            button {
                                class: "btn-danger",
                                style: "width: 100%;",
                                onclick: {
                                    let mut status_message = status_message.clone();
                                    move |_| {
                                        spawn(async move {
                                            let base_dir = if cfg!(target_os = "android") {
                                                std::path::PathBuf::from(
                                                    "/storage/emulated/0/Android/data/de.teilgedanken.stalltagebuch/files/exports",
                                                )
                                            } else {
                                                std::path::PathBuf::from("./exports")
                                            };
                                            let import_path = base_dir.join("import.zip");
                                            if !import_path.exists() {
                                                status_message
                                                    .set(
                                                        t!(
                                                            "backup-import-missing", path : import_path.display()
                                                            .to_string()
                                                        ),
                                                    );
                                                return;
                                            }
                                            status_message.set(t!("backup-db-error", error : "Feature not yet available for SpacetimeDB".to_string()));
                                        });
                                    }
                                },
                                {t!("backup-import-button")}
                            }
                        }
                    }
                }
            } else {
                // Setup form
                div { class: "card",
                    h2 {
                        style: "margin: 0 0 16px 0; font-size: 18px; color: #333;",
                        {t!("sync-setup-title")} // Setup sync heading
                    }

                    // Server URL
                    div { style: "margin-bottom: 16px;",
                        label {
                            style: "display: block; margin-bottom: 4px; font-weight: 600; font-size: 14px;",
                            {t!("sync-server-url")} // Server URL input label
                        }
                        input {
                            r#type: "url",
                            value: "{server_url}",
                            oninput: move |e| server_url.set(e.value()),
                            placeholder: "https://cloud.example.com",
                            style: "width: 100%; padding: 10px; font-size: 16px; border: 1px solid #ccc; border-radius: 4px;",
                        }
                        p {
                            style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                            {t!("sync-server-hint")} // Server URL hint text
                        }
                    }

                    // Remote Path
                    div { style: "margin-bottom: 16px;",
                        label {
                            style: "display: block; margin-bottom: 4px; font-weight: 600; font-size: 14px;",
                            {t!("sync-path-label")} // Remote path input label
                        }
                        input {
                            r#type: "text",
                            value: "{remote_path}",
                            oninput: move |e| remote_path.set(e.value()),
                            placeholder: "/Stalltagebuch",
                            style: "width: 100%; padding: 10px; font-size: 16px; border: 1px solid #ccc; border-radius: 4px;",
                        }
                        p {
                            style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                            {t!("sync-path-hint")} // Remote path hint text
                        }
                    }

                    // Login button and status
                    match login_state() {
                        LoginState::NotStarted => rsx! {
                            button {
                                class: "btn-primary",
                                onclick: start_login,
                                disabled: server_url().trim().is_empty() || !server_url().starts_with("http"),
                                "🔐 "
                                {t!("sync-login")}
                            }
                        },
                        LoginState::InitiatingFlow => rsx! {
                            div { style: "padding: 12px; background: #fff3cd; border-radius: 4px; text-align: center;",
                                "🔄 "
                                {t!("sync-connecting")}
                            }
                        },
                        LoginState::WaitingForUser { login_url, poll_url: _, token: _ } => {
                            rsx! {
                                div { style: "padding: 12px; background: #d1ecf1; border-radius: 4px;",
                                    div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 12px;",
                                        div { style: "font-size: 32px; animation: spin 2s linear infinite;", "💠" }
                                        div {
                                            p { style: "margin: 0; font-weight: 600; font-size: 16px;", {t!("sync-waiting")} } // Waiting for login message
                                            p { style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                                                {t!("sync-polling-background")} // Polling in background message
                                            }
                                        }
                                    }
                                    p { style: "margin: 0 0 12px 0; font-size: 14px;", {t!("sync-login-instructions")} } // Login instructions
                                    a {
                                        href: "{login_url}",
                                        target: "_blank",
                                        style: "display: block; padding: 12px; background: #0066cc; color: white; text-decoration: none; border-radius: 4px; text-align: center; font-weight: 600;",
                                        "🌐 "
                                        {t!("sync-login-browser")}
                                    }
                                }
                            }
                        }
                        LoginState::Success => rsx! {
                            div { style: "padding: 12px; background: #d4edda; border-radius: 4px; text-align: center; color: #155724;",
                                "\u{2705} " // Login success message
                                {t!("sync-login-success")}
                            }
                        },
                        LoginState::Error(error) => rsx! {
                            div { style: "padding: 12px; background: #f8d7da; border-radius: 4px; color: #721c24;",
                                p { style: "margin: 0 0 12px 0; font-weight: 600;",
                                    "\u{274c} "
                                    {t!("sync-error")}
                                } // Login error heading // Login error heading
                                p { style: "margin: 0; font-size: 14px;", "{error}" }
                                button {
                                    class: "btn-primary",
                                    style: "margin-top: 12px;",
                                    onclick: move |_| login_state.set(LoginState::NotStarted),
                                    "🔄 Erneut versuchen"
                                }
                            }
                        },
                    }

                    // Info box
                    div { style: "margin-top: 16px; padding: 12px; background: #f8f9fa; border-radius: 4px; border-left: 4px solid #0066cc;",
                        p { style: "margin: 0 0 8px 0; font-size: 14px; font-weight: 600;",
                            "\u{2139}\u{fe0f} " // How login works heading
                            {t!("sync-login-info-title")}
                        }
                        ul {
                            style: "margin: 0; padding-left: 20px; font-size: 13px; color: #555;",
                            li { {t!("sync-login-step1")} } // Step 1: Click login button
                            li { {t!("sync-login-step2")} } // Step 2: Open browser link
                            li { {t!("sync-login-step3")} } // Step 3: Login to Nextcloud
                            li { {t!("sync-login-step4")} } // Step 4: Confirm access
                            li { {t!("sync-login-step5")} } // Step 5: Return to app
                        }
                    }
                }
            }
        }
    }
}
