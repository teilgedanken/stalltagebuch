use dioxus::prelude::*;
use photo_gallery::{CollectionFullscreen, PreviewImage, ThumbnailImage};

#[component]
pub fn SyncedThumbnailImage(
    photo_uuid: Option<String>,
    relative_path: String,
    #[props(default = "Photo".to_string())] alt: String,
    #[props(default = false)] fill: bool,
) -> Element {
    let rerender_tick = use_signal(|| 0u32);
    let mut is_downloading = use_signal(|| false);
    let mut download_error = use_signal(|| None::<String>);

    let uuid_for_effect = photo_uuid.clone();
    let path_for_effect = relative_path.clone();
    let path_key = use_memo(move || path_for_effect.clone());

    use_effect(move || {
        let _ = path_key();

        let Some(uuid) = uuid_for_effect.clone() else {
            is_downloading.set(false);
            download_error.set(None);
            return;
        };

        let mut rerender_tick = rerender_tick.clone();
        let mut is_downloading = is_downloading.clone();
        let mut download_error = download_error.clone();
        is_downloading.set(true);
        download_error.set(None);

        spawn(async move {
            match crate::services::download_service::ensure_photo_downloaded(&uuid).await {
                Ok(_) => {
                    rerender_tick.with_mut(|tick| *tick = tick.wrapping_add(1));
                }
                Err(err) => {
                    download_error.set(Some(err.to_string()));
                }
            }
            is_downloading.set(false);
        });
    });

    let wrapper_style = if fill {
        "position: absolute; top: 0; left: 0; width: 100%; height: 100%;"
    } else {
        "position: relative; display: inline-block;"
    };

    rsx! {
        div { style: "{wrapper_style}",
            ThumbnailImage {
                relative_path,
                alt,
                fill,
                refresh_token: rerender_tick(),
            }

            if download_error().is_some() {
                div {
                    style: "position:absolute; right:6px; bottom:6px; width:22px; height:22px; border-radius:50%; background:rgba(198, 40, 40, 0.9); color:#fff; display:flex; align-items:center; justify-content:center; font-size:12px; font-weight:700;",
                    title: "sync-error",
                    "!"
                }
            } else if is_downloading() {
                div {
                    style: "position:absolute; right:6px; bottom:6px; width:22px; height:22px; border-radius:50%; background:rgba(0, 0, 0, 0.55); color:#fff; display:flex; align-items:center; justify-content:center; font-size:11px;",
                    "⏳"
                }
            }
        }
    }
}

#[component]
pub fn SyncedPreviewImage(
    photo_uuid: Option<String>,
    relative_path: String,
    #[props(default = "Photo".to_string())] alt: String,
) -> Element {
    let rerender_tick = use_signal(|| 0u32);
    let mut is_downloading = use_signal(|| false);
    let mut download_error = use_signal(|| None::<String>);

    let uuid_for_effect = photo_uuid.clone();
    let path_for_effect = relative_path.clone();
    let path_key = use_memo(move || path_for_effect.clone());

    use_effect(move || {
        let _ = path_key();

        let Some(uuid) = uuid_for_effect.clone() else {
            is_downloading.set(false);
            download_error.set(None);
            return;
        };

        let mut rerender_tick = rerender_tick.clone();
        let mut is_downloading = is_downloading.clone();
        let mut download_error = download_error.clone();
        is_downloading.set(true);
        download_error.set(None);

        spawn(async move {
            match crate::services::download_service::ensure_photo_downloaded(&uuid).await {
                Ok(_) => {
                    rerender_tick.with_mut(|tick| *tick = tick.wrapping_add(1));
                }
                Err(err) => {
                    download_error.set(Some(err.to_string()));
                }
            }
            is_downloading.set(false);
        });
    });

    rsx! {
        div { style: "position: relative; display: inline-block;",
            PreviewImage {
                relative_path,
                alt,
                refresh_token: rerender_tick(),
            }

            if download_error().is_some() {
                div {
                    style: "position:absolute; right:10px; bottom:10px; width:26px; height:26px; border-radius:50%; background:rgba(198, 40, 40, 0.92); color:#fff; display:flex; align-items:center; justify-content:center; font-size:14px; font-weight:700;",
                    title: "sync-error",
                    "!"
                }
            } else if is_downloading() {
                div {
                    style: "position:absolute; right:10px; bottom:10px; width:26px; height:26px; border-radius:50%; background:rgba(0, 0, 0, 0.6); color:#fff; display:flex; align-items:center; justify-content:center; font-size:12px;",
                    "⏳"
                }
            }
        }
    }
}

#[component]
pub fn SyncedCollectionFullscreen(
    photo_items: Vec<(String, String)>,
    #[props(default = 0usize)] initial_index: usize,
    on_close: EventHandler<()>,
) -> Element {
    let rerender_tick = use_signal(|| 0u32);
    let mut is_downloading = use_signal(|| false);
    let mut download_error = use_signal(|| None::<String>);
    let photo_items_for_effect = photo_items.clone();

    let items_key = use_memo({
        let photo_items = photo_items_for_effect.clone();
        move || {
            photo_items
                .iter()
                .map(|(uuid, path)| format!("{uuid}:{path}"))
                .collect::<Vec<_>>()
                .join("|")
        }
    });

    use_effect(move || {
        let _ = items_key();
        if photo_items_for_effect.is_empty() {
            is_downloading.set(false);
            download_error.set(None);
            return;
        }

        let photo_uuids: Vec<String> = photo_items_for_effect
            .iter()
            .map(|(uuid, _)| uuid.clone())
            .collect();

        let mut rerender_tick = rerender_tick.clone();
        let mut is_downloading = is_downloading.clone();
        let mut download_error = download_error.clone();
        is_downloading.set(true);
        download_error.set(None);

        spawn(async move {
            let mut changed = false;
            let mut first_error: Option<String> = None;
            for uuid in photo_uuids {
                match crate::services::download_service::ensure_photo_downloaded(&uuid).await {
                    Ok(_) => {
                        changed = true;
                    }
                    Err(err) => {
                        if first_error.is_none() {
                            first_error = Some(err.to_string());
                        }
                    }
                }
            }

            if changed {
                rerender_tick.with_mut(|tick| *tick = tick.wrapping_add(1));
            }
            download_error.set(first_error);
            is_downloading.set(false);
        });
    });

    let photo_paths: Vec<String> = photo_items.iter().map(|(_, path)| path.clone()).collect();

    rsx! {
        div { style: "position: relative;",
            CollectionFullscreen {
                key: "synced-fullscreen-{rerender_tick()}",
                photo_paths,
                initial_index,
                on_close,
            }

            if let Some(_err) = download_error() {
                div {
                    style: "position: fixed; top: 72px; left: 50%; transform: translateX(-50%); z-index: 1100; background: rgba(198, 40, 40, 0.95); color: #fff; padding: 8px 12px; border-radius: 8px; font-size: 13px; font-weight: 600;",
                    "⚠ Sync-Fehler"
                }
            } else if is_downloading() {
                div {
                    style: "position: fixed; top: 72px; left: 50%; transform: translateX(-50%); z-index: 1100; background: rgba(0, 0, 0, 0.65); color: #fff; padding: 8px 12px; border-radius: 8px; font-size: 13px;",
                    "⏳ Lädt aus Nextcloud..."
                }
            }
        }
    }
}
