use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use dioxus::prelude::*;
use dioxus_i18n::tid;
use serde::{Deserialize, Serialize};

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
    Error(String),
}

#[component]
pub fn NextcloudCard(on_status_message: EventHandler<String>) -> Element {
    let mut server_url = use_signal_sync(|| String::from("https://"));
    let mut remote_path = use_signal_sync(|| String::from("/Stalltagebuch"));
    let mut login_state = use_signal_sync(|| LoginState::NotStarted);
    let mut current_settings = use_signal_sync(|| None::<SpacetimeSettings>);
    let mut details_expanded = use_signal_sync(|| false);
    let mut is_syncing = use_signal_sync(|| false);
    let mut upload_progress = use_signal_sync(|| (0usize, 0usize));

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

    crate::spacetime::use_subscription(&["SELECT * FROM photos"]);
    let photos = crate::spacetime::use_table_photos();
    let pending_count = use_memo(move || {
        photos()
            .iter()
            .filter(|photo| {
                matches!(
                    photo.sync_status.as_str(),
                    "local_only" | "pending" | "error"
                )
            })
            .count()
    });
    let uploading_count = use_memo(move || {
        photos()
            .iter()
            .filter(|photo| matches!(photo.sync_status.as_str(), "uploading" | "downloading"))
            .count()
    });
    let synced_count = use_memo(move || {
        photos()
            .iter()
            .filter(|photo| photo.sync_status == "synced")
            .count()
    });
    let error_count = use_memo(move || {
        photos()
            .iter()
            .filter(|photo| matches!(photo.sync_status.as_str(), "error" | "download_failed"))
            .count()
    });

    use_effect(move || {
        if let Ok(saved) = spacetime_settings_service::load_spacetime_settings() {
            if saved.is_nextcloud_configured() && !saved.nextcloud_remote_path.trim().is_empty() {
                server_url.set(saved.nextcloud_url.clone());
                remote_path.set(saved.nextcloud_remote_path.clone());
                current_settings.set(Some(saved));
            }
        }
    });

    let start_login = move |_| {
        let server = server_url();
        let remote_path_value = remote_path();
        login_state.set(LoginState::InitiatingFlow);

        spawn(async move {
            let url = format!("{}/index.php/login/v2", server.trim_end_matches('/'));

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
                        tid!("error-client"),
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

                                login_state.set(LoginState::WaitingForUser {
                                    poll_url: poll_url.clone(),
                                    token: token.clone(),
                                    login_url,
                                });

                                spawn(async move {
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
                                            login_state.set(LoginState::Error(format!(
                                                "{}: {:?}",
                                                tid!("error-client"),
                                                e
                                            )));
                                            return;
                                        }
                                    };

                                    #[cfg(not(target_arch = "wasm32"))]
                                    {
                                        tokio::time::sleep(std::time::Duration::from_millis(500))
                                            .await;
                                    }
                                    #[cfg(target_arch = "wasm32")]
                                    gloo_timers::future::sleep(std::time::Duration::from_millis(
                                        500,
                                    ))
                                    .await;

                                    let mut consecutive_errors: u32 = 0;
                                    for _attempt in 0..60 {
                                        #[allow(unused_assignments)]
                                        let mut wait_after_secs: u64 = 5;

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
                                                    match response.json::<LoginFlowResult>().await {
                                                        Ok(result) => {
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
                                                                    let _ = client
                                                                        .mkcol(&remote_path_value)
                                                                        .await;
                                                                }
                                                                Err(e) => {
                                                                    login_state.set(LoginState::Error(format!(
                                                                        "{}: {:?}",
                                                                        tid!("error-webdav-client"),
                                                                        e
                                                                    )));
                                                                    return;
                                                                }
                                                            }

                                                            let mut st_settings = spacetime_settings_service::load_spacetime_settings()
                                                                .unwrap_or_default();
                                                            st_settings.nextcloud_url =
                                                                result.server;
                                                            st_settings.nextcloud_username =
                                                                result.login_name;
                                                            st_settings.nextcloud_app_password =
                                                                result.app_password;
                                                            st_settings.nextcloud_remote_path =
                                                                remote_path_value.clone();

                                                            if let Err(e) = spacetime_settings_service::save_spacetime_settings(&st_settings) {
                                                                log::error!("Failed to persist Nextcloud settings: {}", e);
                                                            }

                                                            current_settings.set(Some(st_settings));
                                                            details_expanded.set(false);
                                                            login_state.set(LoginState::NotStarted);
                                                            on_status_message.call(format!(
                                                                "✅ {}",
                                                                tid!("sync-login-success-folder")
                                                            ));
                                                            return;
                                                        }
                                                        Err(e) => {
                                                            login_state.set(LoginState::Error(
                                                                format!(
                                                                    "{}: {}",
                                                                    tid!("error-json"),
                                                                    e
                                                                ),
                                                            ));
                                                            return;
                                                        }
                                                    }
                                                } else if response.status().as_u16() != 404 {
                                                    login_state.set(LoginState::Error(format!(
                                                        "{}: {}",
                                                        tid!("error-unexpected-status"),
                                                        response.status()
                                                    )));
                                                    return;
                                                }

                                                consecutive_errors = 0;
                                                wait_after_secs = 5;
                                            }
                                            Err(_) => {
                                                consecutive_errors =
                                                    consecutive_errors.saturating_add(1);
                                                let backoff = 5u64.saturating_mul(
                                                    1u64 << (consecutive_errors
                                                        .saturating_sub(1)
                                                        .min(2))
                                                        as u32,
                                                );
                                                wait_after_secs = backoff.min(30);
                                            }
                                        }

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

                                    login_state.set(LoginState::Error(
                                        tid!("error-login-timeout").to_string(),
                                    ));
                                });
                            }
                            Err(e) => {
                                login_state.set(LoginState::Error(format!(
                                    "{}: {}",
                                    tid!("error-json"),
                                    e
                                )));
                            }
                        }
                    } else {
                        login_state.set(LoginState::Error(format!(
                            "{}: {}",
                            tid!("error-server"),
                            response.status()
                        )));
                    }
                }
                Err(e) => {
                    login_state.set(LoginState::Error(format!(
                        "{}: {}",
                        tid!("error-connection"),
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
        details_expanded.set(false);
        login_state.set(LoginState::NotStarted);
        on_status_message.call(format!("✅ {}", tid!("sync-settings-deleted")));
    };

    let run_sync = move |_| {
        spawn(async move {
            is_syncing.set(true);
            on_status_message.call(tid!("sync-status-running-full").to_string());
            match crate::services::background_sync::sync_now().await {
                Ok(stats) => {
                    on_status_message.call(
                        tid!("sync-status-success-photos", count : stats.photos_uploaded)
                            .to_string(),
                    );
                }
                Err(e) => {
                    on_status_message.call(format!("❌ {}: {}", tid!("sync-failed"), e));
                }
            }
            is_syncing.set(false);
        });
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "☁️ Nextcloud" }

            if let Some(settings) = current_settings() {
                button {
                    class: "btn-primary",
                    style: "width: 100%; display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; background: #2e7d32;",
                    onclick: move |_| details_expanded.set(!details_expanded()),
                    span { "✅ Verbunden" }
                    span { if details_expanded() { "▾" } else { "▸" } }
                }

                if details_expanded() {
                    div { style: "padding: 12px; background: #f8fff8; border: 1px solid #d7ebd8; border-radius: 8px;",
                        p { style: "margin: 4px 0; font-size: 14px;",
                            strong { {tid!("sync-server")} ": " }
                            "{settings.nextcloud_url}"
                        }
                        p { style: "margin: 4px 0; font-size: 14px;",
                            strong { {tid!("sync-username")} ": " }
                            "{settings.nextcloud_username}"
                        }
                        p { style: "margin: 4px 0; font-size: 14px;",
                            strong { {tid!("sync-path")} ": " }
                            "{settings.nextcloud_remote_path}"
                        }

                        div { style: "display: flex; gap: 12px; margin-top: 12px;",
                            button {
                                class: "btn-primary",
                                style: "flex: 1;",
                                disabled: is_syncing(),
                                onclick: run_sync,
                                if is_syncing() { "⏳" } else { "🔄" }
                                " "
                                {tid!("sync-now")}
                            }
                            button {
                                class: "btn-danger",
                                style: "flex: 1;",
                                onclick: delete_settings,
                                "🗑️ "
                                {tid!("sync-delete-config")}
                            }
                        }

                        div { style: "margin-top: 16px; padding: 12px; background: #f0f7ff; border-radius: 8px; border-left: 4px solid #0066cc;",
                            p { style: "margin: 0 0 10px 0; font-weight: 600; font-size: 14px;", {tid!("sync-photo-status-title")} }
                            p { style: "margin: 4px 0; font-size: 12px; color: #333;", {tid!("sync-photo-status-pending", count : pending_count())} }
                            p { style: "margin: 4px 0; font-size: 12px; color: #333;", {tid!("sync-photo-status-active", count : uploading_count())} }
                            p { style: "margin: 4px 0; font-size: 12px; color: #2e7d32;", {tid!("sync-photo-status-synced", count : synced_count())} }
                            if error_count() > 0 {
                                p { style: "margin: 4px 0; font-size: 12px; color: #c62828;", {tid!("sync-photo-status-error", count : error_count())} }
                            }
                        }

                        {
                            let (current, total) = upload_progress();
                            if total > 0 {
                                let percent = if total > 0 {
                                    (current as f64 / total as f64 * 100.0) as usize
                                } else {
                                    0
                                };
                                rsx! {
                                    div { style: "margin-top: 16px; padding: 12px; background: #fff3cd; border-radius: 8px; border-left: 4px solid #ffb300;",
                                        p { style: "margin: 0 0 8px 0; font-weight: 600; font-size: 14px;", {tid!("sync-upload-progress-title")} }
                                        p { style: "margin: 0 0 8px 0; font-size: 12px; color: #666;", {tid!("sync-upload-progress-detail", current : current, total : total, percent : percent)} }
                                        div { style: "width: 100%; background: #e0e0e0; border-radius: 4px; height: 8px; overflow: hidden;",
                                            div { style: "height: 100%; background: linear-gradient(90deg, #0066cc, #0088ff); transition: width 0.3s ease; width: {percent}%;" }
                                        }
                                    }
                                }
                            } else {
                                rsx! {}
                            }
                        }

                        div { style: "margin-top: 16px;",
                            button {
                                class: "btn-danger",
                                style: "width: 100%;",
                                onclick: move |_| {
                                    on_status_message.call(tid!("backup-db-error", error : "Feature not yet available for SpacetimeDB".to_string()));
                                },
                                {tid!("backup-cleanup-button")}
                            }
                        }
                    }
                }
            } else {
                div { style: "margin-bottom: 16px;",
                    label {
                        style: "display: block; margin-bottom: 4px; font-weight: 600; font-size: 14px;",
                        {tid!("sync-server-url")}
                    }
                    input {
                        r#type: "url",
                        value: "{server_url}",
                        oninput: move |e| server_url.set(e.value()),
                        placeholder: "https://cloud.example.com",
                        style: "width: 100%; padding: 10px; font-size: 16px; border: 1px solid #ccc; border-radius: 4px;",
                    }
                }

                div { style: "margin-bottom: 16px;",
                    label {
                        style: "display: block; margin-bottom: 4px; font-weight: 600; font-size: 14px;",
                        {tid!("sync-path-label")}
                    }
                    input {
                        r#type: "text",
                        value: "{remote_path}",
                        oninput: move |e| remote_path.set(e.value()),
                        placeholder: "/Stalltagebuch",
                        style: "width: 100%; padding: 10px; font-size: 16px; border: 1px solid #ccc; border-radius: 4px;",
                    }
                }

                match login_state() {
                    LoginState::NotStarted => rsx! {
                        button {
                            class: "btn-primary",
                            onclick: start_login,
                            disabled: server_url().trim().is_empty() || !server_url().starts_with("http"),
                            "🔐 "
                            {tid!("sync-login")}
                        }
                    },
                    LoginState::InitiatingFlow => rsx! {
                        div { style: "padding: 12px; background: #fff3cd; border-radius: 4px; text-align: center;",
                            "🔄 "
                            {tid!("sync-connecting")}
                        }
                    },
                    LoginState::WaitingForUser { login_url, poll_url: _, token: _ } => {
                        rsx! {
                            div { style: "padding: 12px; background: #d1ecf1; border-radius: 4px;",
                                p { style: "margin: 0 0 12px 0; font-size: 14px;", {tid!("sync-login-instructions")} }
                                a {
                                    href: "{login_url}",
                                    target: "_blank",
                                    style: "display: block; padding: 12px; background: #0066cc; color: white; text-decoration: none; border-radius: 4px; text-align: center; font-weight: 600;",
                                    "🌐 "
                                    {tid!("sync-login-browser")}
                                }
                            }
                        }
                    }
                    LoginState::Error(error) => rsx! {
                        div { style: "padding: 12px; background: #f8d7da; border-radius: 4px; color: #721c24;",
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
            }
        }
    }
}
