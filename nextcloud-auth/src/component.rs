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
    /// Show the informational step list below the auth panel
    #[props(default = true)]
    pub show_info_box: bool,
    /// Show the transient success box before the host switches UI
    #[props(default = true)]
    pub show_success_state: bool,
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
    pub next_check_in: String,
    pub waiting_icon: String,
}

async fn sleep_for(duration: std::time::Duration) {
    tokio::time::sleep(duration).await;
}

fn format_next_check(template: &str, seconds: u64) -> String {
    let seconds_str = seconds.to_string();
    template
        .replace("{ $seconds }", &seconds_str)
        .replace("{$seconds}", &seconds_str)
        .replace("seconds", &seconds_str)
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
    let mut next_poll_in_seconds = use_signal(|| None::<u64>);

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
        next_check_in: "Next check in {seconds} s".to_string(),
        waiting_icon: "🔄".to_string(),
    });

    let start_login = {
        let server_url = props.server_url.clone();
        let on_success = props.on_success.clone();
        let on_error = props.on_error.clone();

        move |_| {
            next_poll_in_seconds.set(None);
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
                        next_poll_in_seconds.set(Some(1));

                        // Start polling in background
                        spawn(async move {
                            sleep_for(std::time::Duration::from_millis(500)).await;

                            let mut consecutive_errors = 0;
                            for attempt in 0..60 {
                                log::debug!("Login polling attempt {}", attempt + 1);
                                next_poll_in_seconds.set(None);

                                let wait_secs = match auth_service
                                    .poll_login(&poll_url, &token)
                                    .await
                                {
                                    Ok(Some(credentials)) => {
                                        log::info!("Login successful!");
                                        next_poll_in_seconds.set(None);
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

                                next_poll_in_seconds.set(Some(wait_secs));
                                for remaining in (1..=wait_secs).rev() {
                                    sleep_for(std::time::Duration::from_secs(1)).await;
                                    next_poll_in_seconds.set(Some(remaining.saturating_sub(1)));
                                }
                            }

                            let error_msg =
                                "Login timeout - no response after 5 minutes".to_string();
                            log::error!("Login timeout");
                            next_poll_in_seconds.set(None);
                            login_state.set(LoginState::Error(error_msg.clone()));
                            if let Some(handler) = on_error {
                                handler.call(error_msg);
                            }
                        });
                    }
                    Err(e) => {
                        next_poll_in_seconds.set(None);
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
                        class: "button is-link is-fullwidth",
                        onclick: start_login,
                        "{labels.login_button}"
                    }
                },
                LoginState::InitiatingFlow => rsx! {
                    div {
                        class: "notification is-warning is-light has-text-centered",
                        "{labels.connecting}"
                    }
                },
                LoginState::WaitingForUser { login_url, poll_url: _, token: _ } => rsx! {
                    div {
                        class: "notification is-info is-light",
                        div {
                            class: "is-flex is-align-items-center mb-3",
                            style: "gap: 12px;",
                            div {
                                style: "font-size: 28px; animation: spin 1.4s linear infinite;",
                                "{labels.waiting_icon}"
                            }
                            div {
                                p {
                                    class: "mb-1 has-text-weight-semibold",
                                    "{labels.waiting}"
                                }
                                p {
                                    class: "is-size-7 has-text-grey mb-1",
                                    "{labels.polling_background}"
                                }
                                if let Some(next_check_in) = next_poll_in_seconds() {
                                    if next_check_in > 0 {
                                        p {
                                            class: "is-size-7",
                                            "{format_next_check(&labels.next_check_in, next_check_in)}"
                                        }
                                    }
                                }
                            }
                        }
                        p {
                            class: "mb-3",
                            "{labels.instructions}"
                        }
                        a {
                            class: "button is-link is-fullwidth",
                            href: "{login_url}",
                            target: "_blank",
                            "{labels.open_browser}"
                        }
                    }
                },
                LoginState::Success(_) => {
                    if props.show_success_state {
                        rsx! {
                            div {
                                class: "notification is-success is-light has-text-centered",
                                "{labels.login_success}"
                            }
                        }
                    } else {
                        rsx! {}
                    }
                },
                LoginState::Error(error) => rsx! {
                    div {
                        class: "notification is-danger is-light",
                        p {
                            class: "mb-3 has-text-weight-semibold",
                            "{labels.error_title}"
                        }
                        p { class: "mb-3", "{error}" }
                        button {
                            class: "button is-link is-light",
                            onclick: move |_| login_state.set(LoginState::NotStarted),
                            "{labels.retry_button}"
                        }
                    }
                },
            }

            if props.show_info_box {
                // Info box
                div {
                    class: "message is-link is-light mt-4",
                    div { class: "message-header", "{labels.info_title}" }
                    div { class: "message-body",
                    p {
                        class: "mb-2 has-text-weight-semibold",
                        "{labels.info_title}"
                    }
                        ul {
                            class: "is-size-7",
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
    }
}
