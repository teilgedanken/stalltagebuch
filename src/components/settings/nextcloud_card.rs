use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use dioxus::prelude::*;
use dioxus_i18n::tid;
use nextcloud_auth::{AuthLabels, NextcloudAuthComponent, NextcloudCredentials};

#[component]
pub fn NextcloudCard(on_status_message: EventHandler<String>) -> Element {
    let mut server_url = use_signal_sync(|| String::from("https://"));
    let mut remote_path = use_signal_sync(|| String::from("/Stalltagebuch"));
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

    let on_auth_success = move |credentials: NextcloudCredentials| {
        let remote_path_value = remote_path();
        let server_url_value = credentials.server_url;
        let username_value = credentials.username;
        let app_password_value = credentials.app_password;

        spawn(async move {
            let webdav_url = format!(
                "{}/remote.php/dav/files/{}",
                server_url_value.trim_end_matches('/'),
                username_value
            );

            match reqwest_dav::ClientBuilder::new()
                .set_host(webdav_url)
                .set_auth(reqwest_dav::Auth::Basic(
                    username_value.clone(),
                    app_password_value.clone(),
                ))
                .build()
            {
                Ok(client) => {
                    if let Err(e) = client.mkcol(&remote_path_value).await {
                        log::warn!(
                            "Failed to ensure Nextcloud folder '{}' exists: {}",
                            remote_path_value,
                            e
                        );
                    }
                }
                Err(e) => {
                    log::warn!("Failed to build WebDAV client after successful auth: {}", e);
                }
            }

            let mut st_settings =
                spacetime_settings_service::load_spacetime_settings().unwrap_or_default();
            st_settings.nextcloud_url = server_url_value.clone();
            st_settings.nextcloud_username = username_value;
            st_settings.nextcloud_app_password = app_password_value;
            st_settings.nextcloud_remote_path = remote_path_value;

            if let Err(e) = spacetime_settings_service::save_spacetime_settings(&st_settings) {
                log::error!("Failed to persist Nextcloud settings: {}", e);
            }

            server_url.set(server_url_value);
            current_settings.set(Some(st_settings));
            details_expanded.set(false);
            on_status_message.call(format!("✅ {}", tid!("sync-login-success-folder")));
        });
    };

    let on_auth_error = move |error_message: String| {
        on_status_message.call(format!("❌ {}: {}", tid!("sync-error"), error_message));
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

                if server_url().trim().is_empty() || !server_url().starts_with("http") {
                    button {
                        class: "btn-primary",
                        disabled: true,
                        "🔐 "
                        {tid!("sync-login")}
                    }
                } else {
                    NextcloudAuthComponent {
                        server_url: server_url(),
                        on_success: on_auth_success,
                        on_error: on_auth_error,
                        show_info_box: false,
                        show_success_state: false,
                        labels: Some(AuthLabels {
                            login_button: format!("🔐 {}", tid!("sync-login")),
                            connecting: format!("🔄 {}", tid!("sync-connecting")),
                            waiting: tid!("sync-waiting").to_string(),
                            polling_background: tid!("sync-polling-background").to_string(),
                            instructions: tid!("sync-login-instructions").to_string(),
                            open_browser: format!("🌐 {}", tid!("sync-login-browser")),
                            login_success: format!("✅ {}", tid!("sync-login-success")),
                            error_title: format!("❌ {}", tid!("sync-error")),
                            retry_button: format!("🔄 {}", tid!("action-retry")),
                            info_title: tid!("sync-login-info-title").to_string(),
                            step1: tid!("sync-login-step1").to_string(),
                            step2: tid!("sync-login-step2").to_string(),
                            step3: tid!("sync-login-step3").to_string(),
                            step4: tid!("sync-login-step4").to_string(),
                            step5: tid!("sync-login-step5").to_string(),
                            next_check_in: tid!("sync-next-check-in").to_string(),
                            waiting_icon: "🔄".to_string(),
                        }),
                    }
                }
            }
        }
    }
}
