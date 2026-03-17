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
    Success,
    Error(String),
}

#[component]
pub fn NextcloudLoginCard(
    on_configured: EventHandler<SpacetimeSettings>,
    on_status_message: EventHandler<String>,
) -> Element {
    let mut server_url = use_signal_sync(|| String::from("https://"));
    let mut remote_path = use_signal_sync(|| String::from("/Stalltagebuch"));
    let mut login_state = use_signal_sync(|| LoginState::NotStarted);

    use_effect(move || {
        if let Ok(saved) = spacetime_settings_service::load_spacetime_settings() {
            if saved.is_nextcloud_configured() && !saved.nextcloud_remote_path.trim().is_empty() {
                server_url.set(saved.nextcloud_url.clone());
                remote_path.set(saved.nextcloud_remote_path.clone());
                login_state.set(LoginState::Success);
                on_configured.call(saved);
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
                                            log::error!(
                                                "LoginFlow: HTTP-Client fuer Polling konnte nicht erstellt werden: {:?}",
                                                e
                                            );
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
                                                                    match client
                                                                        .mkcol(&remote_path_value)
                                                                        .await
                                                                    {
                                                                        Ok(_) => {}
                                                                        Err(e) => {
                                                                            log::debug!(
                                                                                "LoginFlow: Ordner-Erstellung Hinweis (evtl. bereits vorhanden): {}",
                                                                                e
                                                                            );
                                                                        }
                                                                    }
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

                                                            on_configured.call(st_settings);
                                                            login_state.set(LoginState::Success);
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
                                            Err(e) => {
                                                consecutive_errors =
                                                    consecutive_errors.saturating_add(1);
                                                let backoff = 5u64.saturating_mul(
                                                    1u64 << (consecutive_errors
                                                        .saturating_sub(1)
                                                        .min(2))
                                                        as u32,
                                                );
                                                wait_after_secs = backoff.min(30);
                                                log::warn!(
                                                    "LoginFlow: Netzfehler beim Polling: {} (Backoff {}s)",
                                                    e,
                                                    wait_after_secs
                                                );
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

    rsx! {
        div { class: "card",
            h2 {
                style: "margin: 0 0 16px 0; font-size: 18px; color: #333;",
                {tid!("sync-setup-title")}
            }

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
                p {
                    style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                    {tid!("sync-server-hint")}
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
                p {
                    style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                    {tid!("sync-path-hint")}
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
                            div { style: "display: flex; align-items: center; gap: 12px; margin-bottom: 12px;",
                                div { style: "font-size: 32px; animation: spin 2s linear infinite;", "💠" }
                                div {
                                    p { style: "margin: 0; font-weight: 600; font-size: 16px;", {tid!("sync-waiting")} }
                                    p { style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                                        {tid!("sync-polling-background")}
                                    }
                                }
                            }
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
                LoginState::Success => rsx! {
                    div { style: "padding: 12px; background: #d4edda; border-radius: 4px; text-align: center; color: #155724;",
                        "✅ "
                        {tid!("sync-login-success")}
                    }
                },
                LoginState::Error(error) => rsx! {
                    div { style: "padding: 12px; background: #f8d7da; border-radius: 4px; color: #721c24;",
                        p { style: "margin: 0 0 12px 0; font-weight: 600;",
                            "❌ "
                            {tid!("sync-error")}
                        }
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

            div { style: "margin-top: 16px; padding: 12px; background: #f8f9fa; border-radius: 4px; border-left: 4px solid #0066cc;",
                p { style: "margin: 0 0 8px 0; font-size: 14px; font-weight: 600;",
                    "ℹ️ "
                    {tid!("sync-login-info-title")}
                }
                ul {
                    style: "margin: 0; padding-left: 20px; font-size: 13px; color: #555;",
                    li { {tid!("sync-login-step1")} }
                    li { {tid!("sync-login-step2")} }
                    li { {tid!("sync-login-step3")} }
                    li { {tid!("sync-login-step4")} }
                    li { {tid!("sync-login-step5")} }
                }
            }
        }
    }
}
