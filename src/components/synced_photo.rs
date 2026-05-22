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

    // Bridge the `relative_path` prop into a Signal so that `use_effect` can subscribe
    // to it reactively.  A plain `use_memo(move || path_for_effect.clone())` where
    // `path_for_effect` is a captured String captures the value at mount time and never
    // re-evaluates, so it misses later prop updates (e.g. after a remote crop sync).
    let mut path_signal = use_signal(|| relative_path.clone());
    if path_signal.peek().as_str() != relative_path.as_str() {
        path_signal.set(relative_path.clone());
    }

    // UUID doesn't change for an existing photo slot (the parent keyed div ensures the
    // component is remounted when photo_uuid changes), so a plain clone is fine here.
    let uuid_for_effect = photo_uuid.clone();

    use_effect(move || {
        // Subscribing to path_signal makes this effect re-run whenever relative_path
        // changes (e.g. after a crop on another device is synced via SpacetimeDB).
        let _ = path_signal();

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
                div { style: "position:absolute; right:6px; bottom:6px; width:22px; height:22px; border-radius:50%; background:rgba(0, 0, 0, 0.55); color:#fff; display:flex; align-items:center; justify-content:center; font-size:11px;",
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

    // Same signal-bridging fix as SyncedThumbnailImage – see comment there.
    let mut path_signal = use_signal(|| relative_path.clone());
    if path_signal.peek().as_str() != relative_path.as_str() {
        path_signal.set(relative_path.clone());
    }

    let uuid_for_effect = photo_uuid.clone();

    use_effect(move || {
        let _ = path_signal();

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
            PreviewImage { relative_path, alt, refresh_token: rerender_tick() }

            if download_error().is_some() {
                div {
                    style: "position:absolute; right:10px; bottom:10px; width:26px; height:26px; border-radius:50%; background:rgba(198, 40, 40, 0.92); color:#fff; display:flex; align-items:center; justify-content:center; font-size:14px; font-weight:700;",
                    title: "sync-error",
                    "!"
                }
            } else if is_downloading() {
                div { style: "position:absolute; right:10px; bottom:10px; width:26px; height:26px; border-radius:50%; background:rgba(0, 0, 0, 0.6); color:#fff; display:flex; align-items:center; justify-content:center; font-size:12px;",
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

    // Bridge photo_items into a Signal for the same reason as the path_signal above:
    // a use_memo over a captured Vec sees only the initial value and misses later updates.
    let mut items_signal = use_signal(|| photo_items.clone());
    if *items_signal.peek() != photo_items {
        items_signal.set(photo_items.clone());
    }

    use_effect(move || {
        let current_items = items_signal();

        if current_items.is_empty() {
            is_downloading.set(false);
            download_error.set(None);
            return;
        }

        let photo_uuids: Vec<String> = current_items.iter().map(|(uuid, _)| uuid.clone()).collect();

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
                div { style: "position: fixed; top: 72px; left: 50%; transform: translateX(-50%); z-index: 1100; background: rgba(198, 40, 40, 0.95); color: #fff; padding: 8px 12px; border-radius: 8px; font-size: 13px; font-weight: 600;",
                    "⚠ Sync-Fehler"
                }
            } else if is_downloading() {
                div { style: "position: fixed; top: 72px; left: 50%; transform: translateX(-50%); z-index: 1100; background: rgba(0, 0, 0, 0.65); color: #fff; padding: 8px 12px; border-radius: 8px; font-size: 13px;",
                    "⏳ Lädt aus Nextcloud..."
                }
            }
        }
    }
}
