use crate::models::export::{
    DeviceExport, EggRecordExport, ExportData, PhotoCollectionExport, PhotoExport,
    QuailEventExport, QuailExport,
};
use crate::services::export_service;
use crate::services::export_service::ExportProgress;
use crate::services::spacetime_settings_service;
use chrono::{Local, TimeZone};
use dioxus::prelude::*;
use dioxus_i18n::tid;

#[component]
pub fn BackupCard(on_status_message: EventHandler<String>) -> Element {
    crate::spacetime::use_subscription(&[
        "SELECT * FROM devices",
        "SELECT * FROM quails",
        "SELECT * FROM quail_events",
        "SELECT * FROM egg_records",
        "SELECT * FROM photos",
        "SELECT * FROM photo_collections",
        "SELECT * FROM backups",
    ]);

    let devices = crate::spacetime::use_table_devices();
    let quails = crate::spacetime::use_table_quails();
    let quail_events = crate::spacetime::use_table_quail_events();
    let egg_records = crate::spacetime::use_table_egg_records();
    let photos = crate::spacetime::use_table_photos();
    let photo_collections = crate::spacetime::use_table_photo_collections();

    let mut export_progress = use_signal_sync(|| None::<ExportProgress>);
    let mut export_status = use_signal_sync(|| String::new());
    let mut is_exporting = use_signal_sync(|| false);

    let import_progress = use_signal(|| None::<String>);
    let mut import_status = use_signal(|| String::new());
    #[allow(unused_mut)]
    let mut is_importing = use_signal(|| false);

    let mut is_backup_uploading = use_signal_sync(|| false);
    let mut is_nextcloud_configured = use_signal_sync(|| false);
    let mut include_photo_files = use_signal_sync(|| true);
    let backup_history = crate::spacetime::use_table_backups();
    let mut expanded_backup_id = use_signal_sync(|| None::<String>);
    let mut show_all_backups = use_signal_sync(|| false);
    let create_backup_started = crate::spacetime::use_reducer_create_backup_started();
    let finish_backup = crate::spacetime::use_reducer_finish_backup();
    let create_backup_started_for_export = create_backup_started.clone();
    let finish_backup_for_export = finish_backup.clone();
    let create_backup_started_for_upload = create_backup_started.clone();
    let finish_backup_for_upload = finish_backup.clone();

    use_effect(move || {
        if let Ok(saved) = spacetime_settings_service::load_spacetime_settings() {
            is_nextcloud_configured.set(saved.is_nextcloud_configured());
        }
    });

    let create_export_archive =
        move |include_images: bool, mut progress_callback: Box<dyn FnMut(ExportProgress)>| {
            let devices_snapshot = devices();
            let quails_snapshot = quails();
            let quail_events_snapshot = quail_events();
            let egg_records_snapshot = egg_records();
            let photos_snapshot = photos();
            let photo_collections_snapshot = photo_collections();

            async move {
                progress_callback(ExportProgress::Starting);
                let metadata = export_service::build_export_metadata()?;

                progress_callback(ExportProgress::ReadingQuails);
                let export_devices = devices_snapshot
                    .into_iter()
                    .map(|device| DeviceExport {
                        device_id: device.device_id,
                        name: device.name,
                        comment: device.comment,
                        first_seen: device.first_seen,
                        last_seen: device.last_seen,
                        owner: device.owner,
                    })
                    .collect();
                let export_quails = quails_snapshot
                    .into_iter()
                    .map(|quail| QuailExport {
                        uuid: quail.uuid,
                        name: quail.name,
                        gender: quail.gender,
                        ring_color: quail.ring_color,
                        profile_photo: quail.profile_photo,
                        birthday: quail.birthday,
                        device_id: quail.device_id,
                        owner: quail.owner,
                        created_at: quail.created_at,
                    })
                    .collect();

                progress_callback(ExportProgress::ReadingEvents);
                let export_events = quail_events_snapshot
                    .into_iter()
                    .map(|event| QuailEventExport {
                        uuid: event.uuid,
                        quail_uuid: event.quail_uuid,
                        event_type: event.event_type,
                        event_date: event.event_date,
                        notes: event.notes,
                        photos: event.photos,
                        device_id: event.device_id,
                        owner: event.owner,
                    })
                    .collect();

                progress_callback(ExportProgress::ReadingEggRecords);
                let export_egg_records = egg_records_snapshot
                    .into_iter()
                    .map(|record| EggRecordExport {
                        uuid: record.uuid,
                        record_date: record.record_date,
                        total_eggs: record.total_eggs,
                        notes: record.notes,
                        device_id: record.device_id,
                        owner: record.owner,
                    })
                    .collect();

                progress_callback(ExportProgress::ReadingPhotos);
                let export_photos = photos_snapshot
                    .into_iter()
                    .map(|photo| PhotoExport {
                        uuid: photo.uuid,
                        collection_uuid: photo.collection_uuid,
                        relative_path: photo.relative_path,
                        sync_status: photo.sync_status,
                        sync_error: photo.sync_error,
                        last_sync_attempt: photo.last_sync_attempt,
                        retry_count: photo.retry_count,
                        device_id: photo.device_id,
                        owner: photo.owner,
                        created_at: photo.created_at,
                        updated_at: photo.updated_at,
                    })
                    .collect();
                let export_photo_collections = photo_collections_snapshot
                    .into_iter()
                    .map(|collection| PhotoCollectionExport {
                        uuid: collection.uuid,
                        quail_uuid: collection.quail_uuid,
                        event_uuid: collection.event_uuid,
                        preview_photo_uuid: collection.preview_photo_uuid,
                        name: collection.name,
                        device_id: collection.device_id,
                        owner: collection.owner,
                        created_at: collection.created_at,
                        updated_at: collection.updated_at,
                    })
                    .collect();

                let export = ExportData {
                    metadata,
                    devices: export_devices,
                    quails: export_quails,
                    quail_events: export_events,
                    egg_records: export_egg_records,
                    photos: export_photos,
                    photo_collections: export_photo_collections,
                };

                export_service::export_to_zip(export, include_images, move |progress| {
                    progress_callback(progress)
                })
                .await
            }
        };

    let handle_export = move |_| {
        if is_exporting() {
            return;
        }

        is_exporting.set(true);
        export_status.set(tid!("export-in-progress"));
        export_progress.set(Some(ExportProgress::Starting));

        let include_images = include_photo_files();
        let tracking_id = ulid::Ulid::new().to_string();
        create_backup_started_for_export(crate::spacetime::CreateBackupStartedArgs {
            backup_id: tracking_id.clone(),
            kind: "file".to_string(),
            include_images,
            local_path: None,
        });
        let finish_backup_reducer = finish_backup_for_export.clone();

        spawn(async move {
            let mut progress_sig = export_progress;
            let mut status_sig = export_status;
            let mut exporting_sig = is_exporting;

            match create_export_archive(
                include_images,
                Box::new(move |p| {
                    progress_sig.with_mut(|s| *s = Some(p));
                }),
            )
            .await
            {
                Ok(stats) => {
                    let path_str = stats.path.display().to_string();
                    finish_backup_reducer(crate::spacetime::FinishBackupArgs {
                        backup_id: tracking_id.clone(),
                        status: "success".to_string(),
                        local_path: Some(path_str.clone()),
                        remote_filename: None,
                        zip_size_bytes: Some(stats.zip_size_bytes as i64),
                        quails: stats.quails as i32,
                        events: stats.events as i32,
                        egg_records: stats.egg_records as i32,
                        photos_meta: stats.photos_meta as i32,
                        photos_files_included: stats.photos_files_included as i32,
                        photos_files_missing: stats.photos_files_missing as i32,
                        error_message: None,
                    });
                    status_sig.with_mut(|s| {
                        *s = format!("✅ {}\n📁 {}", tid!("export-success"), path_str)
                    });
                    progress_sig.with_mut(|s| *s = Some(ExportProgress::Complete));
                }
                Err(e) => {
                    finish_backup_reducer(crate::spacetime::FinishBackupArgs {
                        backup_id: tracking_id.clone(),
                        status: "failed".to_string(),
                        local_path: None,
                        remote_filename: None,
                        zip_size_bytes: None,
                        quails: 0,
                        events: 0,
                        egg_records: 0,
                        photos_meta: 0,
                        photos_files_included: 0,
                        photos_files_missing: 0,
                        error_message: Some(e.to_string()),
                    });
                    status_sig.with_mut(|s| *s = format!("❌ {}: {}", tid!("export-failed"), e));
                    progress_sig.with_mut(|s| *s = None);
                }
            }

            exporting_sig.set(false);
        });
    };

    let handle_import = move |_| {
        if is_importing() {
            return;
        }

        #[cfg(target_os = "android")]
        {
            spawn(async move {
                if let Err(e) = crate::camera::launch_document_picker() {
                    import_status.set(format!("❌ {}: {}", tid!("import-failed"), e));
                    return;
                }

                let mut selected_path = None;
                for _ in 0..120 {
                    if let Some(path) = crate::camera::get_last_document_path() {
                        selected_path = Some(path);
                        break;
                    }

                    if let Some(err) = crate::camera::get_last_error() {
                        import_status.set(format!("❌ {}: {}", tid!("import-failed"), err));
                        return;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }

                if let Some(path) = selected_path {
                    is_importing.set(true);
                    import_status.set(tid!("import-in-progress"));

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
                                *s = tid!(
                                    "backup-import-success-with-counts",
                                    count: count,
                                    photos: photo_count
                                )
                                .to_string()
                            });
                            progress_sig.with_mut(|s| *s = None);
                        }
                        Err(e) => {
                            status_sig
                                .with_mut(|s| *s = format!("❌ {}: {}", tid!("import-failed"), e));
                            progress_sig.with_mut(|s| *s = None);
                        }
                    }
                    importing_sig.set(false);
                } else {
                    import_status.set(tid!("backup-import-no-file-selected").to_string());
                }
            });
        }

        #[cfg(not(target_os = "android"))]
        {
            import_status.set(tid!("backup-import-android-only").to_string());
        }
    };

    let upload_backup = move |_| {
        if is_backup_uploading() {
            return;
        }

        let configured = spacetime_settings_service::load_spacetime_settings()
            .map(|s| s.is_nextcloud_configured())
            .unwrap_or(false);
        is_nextcloud_configured.set(configured);

        if !configured {
            on_status_message.call(tid!("sync-setup-title").to_string());
            return;
        }

        is_backup_uploading.set(true);
        on_status_message.call(tid!("backup-upload-running").to_string());
        export_progress.set(Some(ExportProgress::Starting));

        let include_images = include_photo_files();
        let tracking_id = ulid::Ulid::new().to_string();
        create_backup_started_for_upload(crate::spacetime::CreateBackupStartedArgs {
            backup_id: tracking_id.clone(),
            kind: "nextcloud".to_string(),
            include_images,
            local_path: None,
        });
        let finish_backup_reducer = finish_backup_for_upload.clone();

        spawn(async move {
            let mut progress_sig = export_progress;

            match create_export_archive(
                include_images,
                Box::new(move |p| {
                    progress_sig.with_mut(|s| *s = Some(p));
                }),
            )
            .await
            {
                Ok(stats) => {
                    let local_path = stats.path.display().to_string();
                    let upload_path = stats.path.clone();
                    match crate::services::backup_service::upload_backup_to_nextcloud(upload_path)
                        .await
                    {
                        Ok(filename) => {
                            finish_backup_reducer(crate::spacetime::FinishBackupArgs {
                                backup_id: tracking_id.clone(),
                                status: "success".to_string(),
                                local_path: Some(local_path),
                                remote_filename: Some(filename.clone()),
                                zip_size_bytes: Some(stats.zip_size_bytes as i64),
                                quails: stats.quails as i32,
                                events: stats.events as i32,
                                egg_records: stats.egg_records as i32,
                                photos_meta: stats.photos_meta as i32,
                                photos_files_included: stats.photos_files_included as i32,
                                photos_files_missing: stats.photos_files_missing as i32,
                                error_message: None,
                            });
                            on_status_message.call(
                                tid!("backup-upload-success", filename : filename).to_string(),
                            );
                        }
                        Err(error) => {
                            finish_backup_reducer(crate::spacetime::FinishBackupArgs {
                                backup_id: tracking_id.clone(),
                                status: "failed".to_string(),
                                local_path: None,
                                remote_filename: None,
                                zip_size_bytes: None,
                                quails: 0,
                                events: 0,
                                egg_records: 0,
                                photos_meta: 0,
                                photos_files_included: 0,
                                photos_files_missing: 0,
                                error_message: Some(error.to_string()),
                            });
                            on_status_message.call(
                                tid!("backup-upload-failed", error : error.to_string()).to_string(),
                            );
                        }
                    }
                }
                Err(error) => {
                    finish_backup_reducer(crate::spacetime::FinishBackupArgs {
                        backup_id: tracking_id.clone(),
                        status: "failed".to_string(),
                        local_path: None,
                        remote_filename: None,
                        zip_size_bytes: None,
                        quails: 0,
                        events: 0,
                        egg_records: 0,
                        photos_meta: 0,
                        photos_files_included: 0,
                        photos_files_missing: 0,
                        error_message: Some(error.to_string()),
                    });
                    on_status_message
                        .call(tid!("backup-upload-failed", error : error.to_string()).to_string());
                }
            }

            is_backup_uploading.set(false);
        });
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", {tid!("backup-card-title")} }

            div { style: "padding-bottom: 12px; margin-bottom: 12px;",
                p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;",
                    {tid!("backup-card-actions-description")}
                }

                label { style: "display: grid; grid-template-columns: 24px 1fr; align-items: start; column-gap: 8px; margin: 0 0 12px 0; font-size: 13px; color: #333;",
                    input {
                        r#type: "checkbox",
                        style: "width: 18px; height: 18px; margin: 2px 0 0 0;",
                        checked: include_photo_files(),
                        onchange: move |e| include_photo_files.set(e.checked()),
                    }
                    span { style: "min-width: 0;", {tid!("backup-card-include-photo-files")} }
                }

                div { style: "display: flex; gap: 8px;",
                    button {
                        class: "btn-primary",
                        style: "flex: 1;",
                        disabled: is_exporting(),
                        onclick: handle_export,
                        if is_exporting() { {tid!("backup-card-button-file-running")} } else { {tid!("backup-card-button-file")} }
                    }

                    button {
                        class: "btn-primary",
                        style: "flex: 1;",
                        disabled: is_backup_uploading() || !is_nextcloud_configured(),
                        onclick: upload_backup,
                        if is_backup_uploading() {
                            {tid!("backup-upload-button-running")}
                        } else {
                            {tid!("backup-card-button-nextcloud")}
                        }
                    }
                }

                if !is_nextcloud_configured() {
                    p { style: "margin: 8px 0 0 0; font-size: 12px; color: #a66;",
                        {tid!("backup-card-nextcloud-not-connected")}
                    }
                }

                if let Some(progress) = export_progress() {
                    div { style: "padding: 8px; background: #f0f0f0; border-radius: 6px; margin-top: 12px; font-size: 12px;",
                        match progress {
                            ExportProgress::Starting => rsx! { {tid!("backup-card-progress-starting")} },
                            ExportProgress::ReadingQuails => rsx! { {tid!("backup-card-progress-reading-quails")} },
                            ExportProgress::ReadingEvents => rsx! { {tid!("backup-card-progress-reading-events")} },
                            ExportProgress::ReadingEggRecords => rsx! { {tid!("backup-card-progress-reading-egg-records")} },
                            ExportProgress::ReadingPhotos => rsx! { {tid!("backup-card-progress-reading-photos")} },
                            ExportProgress::PackingZip => rsx! { {tid!("backup-card-progress-packing-zip")} },
                            ExportProgress::Complete => rsx! { {tid!("backup-card-progress-complete")} },
                        }
                    }
                }
            }

            div { style: "padding-bottom: 12px; border-bottom: 1px solid #e5e5e5; margin-bottom: 12px;",
                h3 { style: "margin: 0 0 8px 0; font-size: 16px; color: #333;", {tid!("backup-card-history-title")} }

                if backup_history().is_empty() {
                    p { style: "margin: 0; font-size: 12px; color: #777;", {tid!("backup-card-history-empty")} }
                } else {
                    div { style: "display: flex; flex-direction: column; gap: 8px;",
                        for (idx, item) in {
                            let mut items = backup_history();
                            items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                            items.into_iter().enumerate().collect::<Vec<_>>()
                        } {
                            if show_all_backups() || idx == 0 {
                                button {
                                    style: "text-align: left; width: 100%; border: 1px solid #ddd; border-radius: 6px; background: #fafafa; padding: 8px;",
                                    onclick: {
                                        let id = item.backup_id.clone();
                                        move |_| {
                                            if expanded_backup_id().as_ref() == Some(&id) {
                                                expanded_backup_id.set(None);
                                            } else {
                                                expanded_backup_id.set(Some(id.clone()));
                                            }
                                        }
                                    },
                                    div { style: "display: flex; justify-content: space-between; gap: 8px; align-items: center;",
                                        span {
                                            {
                                                let status_icon = match item.status.as_str() {
                                                    "success" => "✅",
                                                    "failed" => "❌",
                                                    _ => "⏳",
                                                };
                                                let kind_icon = if item.kind == "nextcloud" { "☁️" } else { "💾" };
                                                let backup_date = Local
                                                    .timestamp_opt(item.created_at, 0)
                                                    .single()
                                                    .map(|dt| dt.format("%d.%m.%Y %H:%M").to_string())
                                                    .unwrap_or_else(|| tid!("backup-card-unknown").to_string());
                                                format!("{} {} {}", status_icon, kind_icon, backup_date)
                                            }
                                        }
                                    }

                                    if expanded_backup_id().as_ref() == Some(&item.backup_id) {
                                        div { style: "margin-top: 8px; font-size: 12px; color: #444;",
                                            p { style: "margin: 0 0 4px 0;",
                                                {
                                                    let status_label = match item.status.as_str() {
                                                        "success" => tid!("backup-card-status-success"),
                                                        "failed" => tid!("backup-card-status-failed"),
                                                        _ => tid!("backup-card-status-pending"),
                                                    };
                                                    tid!("backup-card-history-status", status: status_label)
                                                }
                                            }
                                            p { style: "margin: 0 0 4px 0;",
                                                {
                                                    let include_images = if item.include_images {
                                                        tid!("backup-card-yes")
                                                    } else {
                                                        tid!("backup-card-no")
                                                    };
                                                    tid!("backup-card-history-include-images", include_images: include_images)
                                                }
                                            }
                                            if let Some(path) = item.local_path.as_ref() {
                                                p { style: "margin: 0 0 4px 0;", {tid!("backup-card-history-file", path: path.clone())} }
                                            }
                                            if let Some(name) = item.remote_filename.as_ref() {
                                                p { style: "margin: 0 0 4px 0;", {tid!("backup-card-history-nextcloud", name: name.clone())} }
                                            }
                                            if let Some(size) = item.zip_size_bytes {
                                                p { style: "margin: 0 0 4px 0;", {tid!("backup-card-history-size", size: size)} }
                                            }
                                            p { style: "margin: 0 0 4px 0;",
                                                {tid!("backup-card-history-items", quails: item.quails, events: item.events, egg_records: item.egg_records)}
                                            }
                                            p { style: "margin: 0;",
                                                {tid!("backup-card-history-photos", photos_meta: item.photos_meta, photos_files_included: item.photos_files_included, photos_files_missing: item.photos_files_missing)}
                                            }
                                            if let Some(err) = item.error_message.as_ref() {
                                                p { style: "margin: 6px 0 0 0; color: #b33;", {tid!("backup-card-history-error", error: err.clone())} }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if !show_all_backups() && backup_history().len() > 1 {
                            button {
                                class: "btn-primary",
                                style: "width: 100%;",
                                onclick: move |_| show_all_backups.set(true),
                                {tid!("backup-card-history-more", count: backup_history().len() - 1)}
                            }
                        }
                    }
                }
            }

            div { style: "padding-bottom: 12px; margin-bottom: 0;",
                h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", {tid!("backup-card-import-title")} }
                p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;", {tid!("import-description")} }

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
                    if is_importing() { {tid!("backup-card-import-button-running")} } else { {tid!("backup-card-import-button")} }
                }

                if !import_status().is_empty() {
                    p { style: "margin: 8px 0 0 0; font-size: 12px; color: #555; white-space: pre-wrap;", "{import_status}" }
                }
            }
        }
    }
}
