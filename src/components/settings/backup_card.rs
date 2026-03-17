use crate::models::export::{
    DeviceExport, EggRecordExport, ExportData, PhotoCollectionExport, PhotoExport,
    QuailEventExport, QuailExport,
};
use crate::services::export_service;
use crate::services::export_service::{ExportProgress, ExportStats};
use crate::services::spacetime_settings_service;
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
    ]);

    let devices = crate::spacetime::use_table_devices();
    let quails = crate::spacetime::use_table_quails();
    let quail_events = crate::spacetime::use_table_quail_events();
    let egg_records = crate::spacetime::use_table_egg_records();
    let photos = crate::spacetime::use_table_photos();
    let photo_collections = crate::spacetime::use_table_photo_collections();

    let mut export_progress = use_signal_sync(|| None::<ExportProgress>);
    let mut export_status = use_signal_sync(|| String::new());
    let mut export_stats = use_signal_sync(|| None::<ExportStats>);
    let mut is_exporting = use_signal_sync(|| false);

    let import_progress = use_signal(|| None::<String>);
    let mut import_status = use_signal(|| String::new());
    #[allow(unused_mut)]
    let mut is_importing = use_signal(|| false);

    let mut is_backup_uploading = use_signal_sync(|| false);
    let mut is_nextcloud_configured = use_signal_sync(|| false);
    let mut include_photo_files = use_signal_sync(|| true);

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
        export_stats.set(None);

        let include_images = include_photo_files();

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
                    status_sig.with_mut(|s| {
                        *s = format!("✅ {}\n📁 {}", tid!("export-success"), path_str)
                    });
                    progress_sig.with_mut(|s| *s = Some(ExportProgress::Complete));
                    export_stats.with_mut(|s| *s = Some(stats));
                }
                Err(e) => {
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
                                *s = format!(
                                    "✅ {} ({} items, {} photos)",
                                    tid!("import-success"),
                                    count,
                                    photo_count
                                )
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
                    import_status.set(format!("❌ {}: no file selected", tid!("import-failed")));
                }
            });
        }

        #[cfg(not(target_os = "android"))]
        {
            import_status.set("⚠️ File picker only available on Android".to_string());
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

        let include_images = include_photo_files();

        spawn(async move {
            match create_export_archive(include_images, Box::new(|_| {})).await {
                Ok(stats) => {
                    match crate::services::backup_service::upload_backup_to_nextcloud(stats.path)
                        .await
                    {
                        Ok(filename) => {
                            on_status_message.call(
                                tid!("backup-upload-success", filename : filename).to_string(),
                            );
                        }
                        Err(error) => {
                            on_status_message.call(
                                tid!("backup-upload-failed", error : error.to_string()).to_string(),
                            );
                        }
                    }
                }
                Err(error) => {
                    on_status_message
                        .call(tid!("backup-upload-failed", error : error.to_string()).to_string());
                }
            }
            is_backup_uploading.set(false);
        });
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "💾 Backup" }

            div { style: "padding-bottom: 12px; border-bottom: 1px solid #e5e5e5; margin-bottom: 12px;",
                p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;", {tid!("export-description")} }

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
                    if is_exporting() { "⏳ Exporting…" } else { "📤 Export" }
                }

                if !export_status().is_empty() {
                    p { style: "margin: 8px 0 0 0; font-size: 12px; color: #555; white-space: pre-wrap;", "{export_status}" }
                }

                if let Some(stats) = export_stats() {
                    div { style: "padding: 10px 12px; background: #eef4ff; border: 1px solid #cce0ff; border-radius: 6px; margin-top: 8px; font-size: 12px; color: #333;",
                        p { style: "margin: 0 0 6px 0; font-weight: bold; font-size: 13px;", "📊 Zusammenfassung" }
                        p { style: "margin: 0 0 3px 0;",
                            "🐦 {stats.quails} Wachteln  ·  📅 {stats.events} Ereignisse  ·  🥚 {stats.egg_records} Eiereinträge"
                        }
                        p { style: "margin: 0 0 3px 0;", "📷 {stats.photos_meta} Foto-Einträge" }
                        if stats.photos_files_included > 0 || stats.photos_files_missing > 0 {
                            p { style: "margin: 0 0 3px 0;",
                                "🖼️ {stats.photos_files_included} Bilddateien enthalten"
                                if stats.photos_files_missing > 0 {
                                    span { style: "color: #aa6600;",
                                        " · ⚠️ {stats.photos_files_missing} fehlend"
                                    }
                                }
                            }
                        }
                        p { style: "margin: 0;",
                            {
                                let bytes = stats.zip_size_bytes;
                                if bytes < 1024 {
                                    format!("📦 {bytes} B")
                                } else if bytes < 1_048_576 {
                                    format!("📦 {:.1} KB", bytes as f64 / 1024.0)
                                } else {
                                    format!("📦 {:.2} MB", bytes as f64 / 1_048_576.0)
                                }
                            }
                        }
                    }
                }
            }

            div { style: "padding-bottom: 12px; border-bottom: 1px solid #e5e5e5; margin-bottom: 12px;",
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
                    if is_importing() { "⏳ Importing…" } else { "📥 Import" }
                }

                if !import_status().is_empty() {
                    p { style: "margin: 8px 0 0 0; font-size: 12px; color: #555; white-space: pre-wrap;", "{import_status}" }
                }
            }

            div {
                p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;", {tid!("backup-export-description")} }

                label { style: "display: flex; align-items: center; gap: 8px; margin: 0 0 12px 0; font-size: 13px; color: #333;",
                    input {
                        r#type: "checkbox",
                        checked: include_photo_files(),
                        onchange: move |e| include_photo_files.set(e.checked()),
                    }
                    "Bilddateien im Backup enthalten"
                }

                button {
                    class: "btn-primary",
                    style: "width: 100%;",
                    disabled: is_backup_uploading() || !is_nextcloud_configured(),
                    onclick: upload_backup,
                    if is_backup_uploading() {
                        {tid!("backup-upload-button-running")}
                    } else {
                        {tid!("backup-upload-button")}
                    }
                }

                if !is_nextcloud_configured() {
                    p { style: "margin: 8px 0 0 0; font-size: 12px; color: #a66;",
                        "Nextcloud nicht verbunden"
                    }
                }
            }
        }
    }
}
