use crate::models::SpacetimeSettings;
use crate::services::spacetime_settings_service;
use dioxus::prelude::*;
use dioxus_i18n::tid;
use nextcloud_auth::{AuthLabels, NextcloudAuthComponent, NextcloudCredentials};

#[component]
pub fn NextcloudCard(
    on_status_message: EventHandler<String>,
    on_nextcloud_config_changed: EventHandler<bool>,
) -> Element {
    let mut server_url = use_signal_sync(|| String::from("https://"));
    let mut remote_path = use_signal_sync(|| String::from("/Stalltagebuch"));
    let mut current_settings = use_signal_sync(|| None::<SpacetimeSettings>);
    let mut details_expanded = use_signal_sync(|| false);
    let mut is_syncing = use_signal_sync(|| false);
    let mut upload_progress = use_signal_sync(|| (0usize, 0usize));
    let mut cleanup_candidates = use_signal_sync(|| Vec::<String>::new());
    let mut is_cleanup_scanning = use_signal_sync(|| false);
    let mut is_cleanup_deleting = use_signal_sync(|| false);

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
                on_nextcloud_config_changed.call(true);
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
            on_nextcloud_config_changed.call(true);
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
        cleanup_candidates.set(Vec::new());
        details_expanded.set(false);
        on_nextcloud_config_changed.call(false);
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

    let scan_orphaned_photos = move |_| {
        if is_cleanup_scanning() || is_cleanup_deleting() {
            return;
        }

        let known_relative_paths: Vec<String> = photos()
            .iter()
            .map(|photo| photo.relative_path.clone())
            .collect();

        spawn(async move {
            cleanup_candidates.set(Vec::new());
            is_cleanup_scanning.set(true);
            on_status_message.call(tid!("backup-cleanup-status-scanning").to_string());

            match crate::services::nextcloud_cleanup_service::find_orphaned_remote_photos(
                &known_relative_paths,
            )
            .await
            {
                Ok(orphaned_paths) => {
                    if orphaned_paths.is_empty() {
                        on_status_message.call(tid!("backup-cleanup-status-none").to_string());
                    } else {
                        on_status_message.call(
                            tid!("backup-cleanup-status-review", count : orphaned_paths.len())
                                .to_string(),
                        );
                        cleanup_candidates.set(orphaned_paths);
                    }
                }
                Err(error) => {
                    on_status_message.call(
                        tid!("backup-cleanup-status-failed", error : error.to_string()).to_string(),
                    );
                }
            }

            is_cleanup_scanning.set(false);
        });
    };

    let cancel_cleanup = move |_| {
        cleanup_candidates.set(Vec::new());
    };

    let confirm_cleanup = move |_| {
        if is_cleanup_scanning() || is_cleanup_deleting() {
            return;
        }

        let orphaned_paths = cleanup_candidates();
        if orphaned_paths.is_empty() {
            return;
        }

        spawn(async move {
            is_cleanup_deleting.set(true);
            on_status_message.call(
                tid!("backup-cleanup-status-deleting", count : orphaned_paths.len()).to_string(),
            );

            match crate::services::nextcloud_cleanup_service::delete_remote_photos(&orphaned_paths)
                .await
            {
                Ok(result) => {
                    if result.failed_paths.is_empty() {
                        cleanup_candidates.set(Vec::new());
                        on_status_message.call(
                            tid!("backup-cleanup-status-success", count : result.deleted_paths.len())
                                .to_string(),
                        );
                    } else {
                        let first_error = result
                            .failed_paths
                            .first()
                            .map(|(_, error)| error.clone())
                            .unwrap_or_default();
                        cleanup_candidates.set(
                            result
                                .failed_paths
                                .iter()
                                .map(|(path, _)| path.clone())
                                .collect(),
                        );
                        on_status_message.call(
                            tid!(
                                "backup-cleanup-status-partial",
                                deleted : result.deleted_paths.len(),
                                failed : result.failed_paths.len(),
                                error : first_error
                            )
                            .to_string(),
                        );
                    }
                }
                Err(error) => {
                    on_status_message.call(
                        tid!("backup-cleanup-status-failed", error : error.to_string()).to_string(),
                    );
                }
            }

            is_cleanup_deleting.set(false);
        });
    };

    rsx! {
        div { class: "box mb-4",
            h2 { class: "title is-5 mb-3", "☁️ Nextcloud" }

            if let Some(settings) = current_settings() {
                div { class: "field has-addons mt-4 is-fullwidth",
                    p { class: "control is-expanded",
                        onclick: move |_| details_expanded.set(!details_expanded()),
                        button { class: "button is-success is-light is-small is-fullwidth",
                            span { "✅ Verbunden" }
                            span { if details_expanded() { "▾" } else { "▸" } }
                        }
                    }
                    if details_expanded() {
                        p { class: "control is-expanded",
                            button {
                                class: "button is-danger is-light is-small is-fullwidth",
                                onclick: delete_settings,
                                span { class: "icon is-small", "🗑️" }
                                span { {tid!("sync-delete-config")} }
                            }
                        }
                    }

            }

                if details_expanded() {
                    div { class: "notification is-success is-light mt-3",
                        p {
                            strong { {tid!("sync-server")} ": " }
                            "{settings.nextcloud_url}"
                        }
                        p {
                            strong { {tid!("sync-username")} ": " }
                            "{settings.nextcloud_username}"
                        }
                        p {
                            strong { {tid!("sync-path")} ": " }
                            "{settings.nextcloud_remote_path}"
                        }

                        div { class: "field has-addons mt-4",
                            p { class: "control",
                                button {
                                    class: "button is-link is-small",
                                    disabled: is_syncing(),
                                    onclick: run_sync,
                                    span { class: "icon is-small", if is_syncing() { "⏳" } else { "🔄" } }
                                    span { {tid!("sync-now")} }
                                }
                            }

                        }

                        div { class: "message is-info mt-4",
                            div { class: "message-header", {tid!("sync-photo-status-title")} }
                            div { class: "message-body",
                                p { class: "mb-1", {tid!("sync-photo-status-pending", count : pending_count())} }
                                p { class: "mb-1", {tid!("sync-photo-status-active", count : uploading_count())} }
                                p { class: "mb-1", {tid!("sync-photo-status-synced", count : synced_count())} }
                                if error_count() > 0 {
                                    p { class: "has-text-danger", {tid!("sync-photo-status-error", count : error_count())} }
                                }
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
                                    div { class: "message is-warning mt-4",
                                        div { class: "message-header", {tid!("sync-upload-progress-title")} }
                                        div { class: "message-body",
                                            p { class: "mb-2", {tid!("sync-upload-progress-detail", current : current, total : total, percent : percent)} }
                                            progress {
                                                class: "progress is-link",
                                                value: "{current}",
                                                max: "{total}",
                                                "{percent}%"
                                            }
                                        }
                                    }
                                }
                            } else {
                                rsx! {}
                            }
                        }

                        div { class: "field mt-4",
                            div { class: "control",
                                button {
                                    class: "button is-warning is-fullwidth",
                                    disabled: is_syncing() || is_cleanup_scanning() || is_cleanup_deleting(),
                                    onclick: scan_orphaned_photos,
                                    if is_cleanup_scanning() || is_cleanup_deleting() {
                                        {tid!("action-loading")}
                                    } else {
                                        {tid!("backup-cleanup-button")}
                                    }
                                }
                            }
                        }

                        {
                            let cleanup_candidates = cleanup_candidates();
                            if cleanup_candidates.is_empty() {
                                rsx! {}
                            } else {
                                rsx! {
                                    div { class: "message is-warning mt-4",
                                        div { class: "message-header",
                                            {tid!("backup-cleanup-review-title", count : cleanup_candidates.len())}
                                        }
                                        div { class: "message-body",
                                            p { class: "mb-3", {tid!("backup-cleanup-review-description")} }
                                            div { class: "content", style: "max-height: 180px; overflow-y: auto;",
                                                ul {
                                                    for relative_path in cleanup_candidates {
                                                        li { "{relative_path}" }
                                                    }
                                                }
                                            }
                                            div { class: "buttons mt-3",
                                                button {
                                                    class: "button is-light",
                                                    disabled: is_cleanup_deleting(),
                                                    onclick: cancel_cleanup,
                                                    {tid!("action-cancel")}
                                                }
                                                button {
                                                    class: "button is-danger",
                                                    disabled: is_cleanup_deleting(),
                                                    onclick: confirm_cleanup,
                                                    if is_cleanup_deleting() {
                                                        {tid!("action-loading")}
                                                    } else {
                                                        {tid!("action-delete-permanently")}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { class: "field",
                    label { class: "label", {tid!("sync-server-url")} }
                    div { class: "control",
                        input {
                            class: "input",
                            r#type: "url",
                            value: "{server_url}",
                            oninput: move |e| server_url.set(e.value()),
                            placeholder: "https://cloud.example.com",
                        }
                    }
                }

                div { class: "field",
                    label { class: "label", {tid!("sync-path-label")} }
                    div { class: "control",
                        input {
                            class: "input",
                            r#type: "text",
                            value: "{remote_path}",
                            oninput: move |e| remote_path.set(e.value()),
                            placeholder: "/Stalltagebuch",
                        }
                    }
                }

                if server_url().trim().is_empty() || !server_url().starts_with("http") {
                    button {
                        class: "button is-link is-fullwidth",
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
