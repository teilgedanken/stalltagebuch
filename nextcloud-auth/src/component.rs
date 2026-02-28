use crate::models::{LoginState, NextcloudCredentials};
use crate::service::NextcloudAuthService;
use dioxus::prelude::*;

/// Props for the NextcloudAuthComponent
#[derive(Props, Clone, PartialEq)]
pub struct NextcloudAuthProps {
    /// Server URL to authenticate against
    pub server_url: String,
    /// Callback when authentication succeeds
    pub on_success: EventHandler<NextcloudCredentials>,
    /// Callback when authentication fails
    #[props(default)]
    pub on_error: Option<EventHandler<String>>,
    /// Custom labels for UI elements (optional)
    #[props(default)]
    pub labels: Option<AuthLabels>,
}

/// Custom labels for the authentication UI
#[derive(Clone, PartialEq, Default)]
pub struct AuthLabels {
    pub login_button: String,
    pub connecting: String,
    pub waiting: String,
    pub polling_background: String,
    pub instructions: String,
    pub open_browser: String,
    pub login_success: String,
    pub error_title: String,
    pub retry_button: String,
    pub info_title: String,
    pub step1: String,
    pub step2: String,
    pub step3: String,
    pub step4: String,
    pub step5: String,
}

/// Nextcloud authentication component
///
/// This component provides a UI for authenticating with Nextcloud using Login Flow v2.
/// It handles the entire flow including initiating the login, showing the login URL,
/// and polling for completion.
///
/// # Example
/// ```rust,ignore
/// NextcloudAuthComponent {
///     server_url: "https://cloud.example.com".to_string(),
///     on_success: move |creds| {
///         // Handle successful authentication
///     },
///     on_error: move |error| {
///         // Handle authentication error
///     },
///     labels: Some(default_labels()),
/// }
/// ```
#[component]
pub fn NextcloudAuthComponent(props: NextcloudAuthProps) -> Element {
    let mut login_state = use_signal(|| LoginState::NotStarted);

    let labels = props.labels.clone().unwrap_or_else(|| AuthLabels {
        login_button: "🔐 Login to Nextcloud".to_string(),
        connecting: "🔄 Connecting...".to_string(),
        waiting: "Waiting for login...".to_string(),
        polling_background: "Polling in background...".to_string(),
        instructions: "Please click the button below to open your browser and login.".to_string(),
        open_browser: "🌐 Open Browser to Login".to_string(),
        login_success: "✅ Login successful!".to_string(),
        error_title: "❌ Login Error".to_string(),
        retry_button: "🔄 Retry".to_string(),
        info_title: "ℹ️ How Login Works".to_string(),
        step1: "Click the login button".to_string(),
        step2: "Open the browser link".to_string(),
        step3: "Login to your Nextcloud instance".to_string(),
        step4: "Authorize access".to_string(),
        step5: "Return to the app".to_string(),
    });

    let start_login = {
        let server_url = props.server_url.clone();
        let on_success = props.on_success.clone();
        let on_error = props.on_error.clone();

        move |_| {
            login_state.set(LoginState::InitiatingFlow);
            let server_url = server_url.clone();
            let on_success = on_success.clone();
            let on_error = on_error.clone();

            spawn(async move {
                let auth_service = NextcloudAuthService::new(server_url);

                match auth_service.initiate_login().await {
                    Ok(flow) => {
                        let poll_url = flow.poll.endpoint.clone();
                        let token = flow.poll.token.clone();
                        let login_url = flow.login.clone();

                        // Set state to show login URL
                        login_state.set(LoginState::WaitingForUser {
                            login_url: login_url.clone(),
                            poll_url: poll_url.clone(),
                            token: token.clone(),
                        });

                        // Start polling in background
                        spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                            let mut consecutive_errors = 0;
                            for attempt in 0..60 {
                                log::debug!("Login polling attempt {}", attempt + 1);

                                let wait_secs = match auth_service
                                    .poll_login(&poll_url, &token)
                                    .await
                                {
                                    Ok(Some(credentials)) => {
                                        log::info!("Login successful!");
                                        login_state.set(LoginState::Success(credentials.clone()));
                                        on_success.call(credentials);
                                        return;
                                    }
                                    Ok(None) => {
                                        // Still waiting
                                        consecutive_errors = 0;
                                        5
                                    }
                                    Err(e) => {
                                        consecutive_errors += 1;
                                        log::warn!(
                                            "Poll error (attempt {}): {}",
                                            consecutive_errors,
                                            e
                                        );

                                        // Exponential backoff
                                        5u64.saturating_mul(1 << consecutive_errors.min(2)).min(30)
                                    }
                                };

                                tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                            }

                            let error_msg =
                                "Login timeout - no response after 5 minutes".to_string();
                            log::error!("Login timeout");
                            login_state.set(LoginState::Error(error_msg.clone()));
                            if let Some(handler) = on_error {
                                handler.call(error_msg);
                            }
                        });
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to initiate login: {}", e);
                        log::error!("{}", error_msg);
                        login_state.set(LoginState::Error(error_msg.clone()));
                        if let Some(handler) = on_error {
                            handler.call(error_msg);
                        }
                    }
                }
            });
        }
    };

    rsx! {
        div { class: "nextcloud-auth",
            match login_state() {
                LoginState::NotStarted => rsx! {
                    button {
                        class: "btn-primary",
                        onclick: start_login,
                        "{labels.login_button}"
                    }
                },
                LoginState::InitiatingFlow => rsx! {
                    div {
                        style: "padding: 12px; background: #fff3cd; border-radius: 4px; text-align: center;",
                        "{labels.connecting}"
                    }
                },
                LoginState::WaitingForUser { login_url, poll_url: _, token: _ } => rsx! {
                    div {
                        style: "padding: 12px; background: #d1ecf1; border-radius: 4px;",
                        div {
                            style: "display: flex; align-items: center; gap: 12px; margin-bottom: 12px;",
                            div {
                                style: "font-size: 32px; animation: spin 2s linear infinite;",
                                "💠"
                            }
                            div {
                                p {
                                    style: "margin: 0; font-weight: 600; font-size: 16px;",
                                    "{labels.waiting}"
                                }
                                p {
                                    style: "margin: 4px 0 0 0; font-size: 12px; color: #666;",
                                    "{labels.polling_background}"
                                }
                            }
                        }
                        p {
                            style: "margin: 0 0 12px 0; font-size: 14px;",
                            "{labels.instructions}"
                        }
                        a {
                            href: "{login_url}",
                            target: "_blank",
                            style: "display: block; padding: 12px; background: #0066cc; color: white; text-decoration: none; border-radius: 4px; text-align: center; font-weight: 600;",
                            "{labels.open_browser}"
                        }
                    }
                },
                LoginState::Success(_) => rsx! {
                    div {
                        style: "padding: 12px; background: #d4edda; border-radius: 4px; text-align: center; color: #155724;",
                        "{labels.login_success}"
                    }
                },
                LoginState::Error(error) => rsx! {
                    div {
                        style: "padding: 12px; background: #f8d7da; border-radius: 4px; color: #721c24;",
                        p {
                            style: "margin: 0 0 12px 0; font-weight: 600;",
                            "{labels.error_title}"
                        }
                        p { style: "margin: 0; font-size: 14px;", "{error}" }
                        button {
                            class: "btn-primary",
                            style: "margin-top: 12px;",
                            onclick: move |_| login_state.set(LoginState::NotStarted),
                            "{labels.retry_button}"
                        }
                    }
                },
            }

            // Info box
            div {
                style: "margin-top: 16px; padding: 12px; background: #f8f9fa; border-radius: 4px; border-left: 4px solid #0066cc;",
                p {
                    style: "margin: 0 0 8px 0; font-size: 14px; font-weight: 600;",
                    "{labels.info_title}"
                }
                ul {
                    style: "margin: 0; padding-left: 20px; font-size: 13px; color: #555;",
                    li { "{labels.step1}" }
                    li { "{labels.step2}" }
                    li { "{labels.step3}" }
                    li { "{labels.step4}" }
                    li { "{labels.step5}" }
                }
            }
        }
    }
}
