use crate::services::export_service;
use crate::services::export_service::ExportProgress;
use dioxus::prelude::*;
use dioxus_i18n::tid;

// ─── Export card ──────────────────────────────────────────────────────────────

/// Card component that manages data export to ZIP files.
#[component]
pub fn ExportCard() -> Element {
    let mut export_progress = use_signal_sync(|| None::<ExportProgress>);
    let mut export_status = use_signal_sync(|| String::new());
    let mut is_exporting = use_signal_sync(|| false);

    let handle_export = move |_| {
        if is_exporting() {
            return; //Prevent multiple simultaneous exports
        }

        is_exporting.set(true);
        export_status.set(tid!("export-in-progress"));
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
                        *s = format!("✅ {}\n📁 {}", tid!("export-success"), path.display())
                    });
                    progress_sig.with_mut(|s| *s = Some(ExportProgress::Complete));
                }
                Err(e) => {
                    status_sig.with_mut(|s| *s = format!("❌ {}: {}", tid!("export-failed"), e));
                    progress_sig.with_mut(|s| *s = None);
                }
            }
            exporting_sig.set(false);
        });
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "💾 " {tid!("export-title")} }

            p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;",
                {tid!("export-description")}
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
pub fn ImportCard() -> Element {
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
                    import_status.set(format!("❌ {}: {}", tid!("import-failed"), e));
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

        // Fallback for non-Android
        #[cfg(not(target_os = "android"))]
        {
            import_status.set("⚠️ File picker only available on Android".to_string());
        }
    };

    rsx! {
        div { class: "card", style: "margin-bottom: 16px;",
            h2 { style: "margin: 0 0 12px 0; font-size: 18px; color: #0066cc;", "📂 " {tid!("import-title")} }

            p { style: "margin: 0 0 12px 0; font-size: 13px; color: #666;",
                {tid!("import-description")}
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
