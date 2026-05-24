use crate::error::AppError;
use crate::services::nextcloud_webdav::NextcloudWebDav;
use crate::services::photo_paths;
use crate::services::photo_sync_metadata::{SpacetimePhotoMetadataClient, load_runtime};
use std::path::{Path, PathBuf};

const MAX_DOWNLOAD_RETRIES: i32 = 5;

/// Generate thumbnails for `photo_path` if the small thumbnail is missing.
///
/// The thumbnail key is derived from the file stem of `photo_path` so that
/// `{stem}_128.webp` / `{stem}_512.webp` end up in the same directory and the
/// photo-gallery service can find them via `get_photo_file_path`.
/// Runs thumbnail creation in a `spawn_blocking` task so the async runtime is not blocked.
async fn ensure_thumbnails_for_path(photo_path: &Path) {
    let stem = match photo_path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s.to_string(),
        None => {
            log::warn!(
                "ensure_thumbnails_for_path: no file stem for {:?}",
                photo_path
            );
            return;
        }
    };

    // Only regenerate if the small thumbnail is actually missing.
    let parent = match photo_path.parent() {
        Some(p) => p.to_path_buf(),
        None => return,
    };
    let small_thumb = parent.join(format!("{}_128.webp", stem));
    if small_thumb.exists() {
        return;
    }

    let path_str = photo_path.to_string_lossy().to_string();
    log::debug!(
        "Generating missing thumbnails for stem='{}' path='{}'",
        stem,
        path_str
    );
    let result = tokio::task::spawn_blocking(move || {
        photo_gallery::create_thumbnails(&path_str, &stem, 400, 512)
    })
    .await;
    match result {
        Ok(Ok((small, medium))) => {
            log::debug!("Thumbnails created: {} / {}", small, medium);
        }
        Ok(Err(e)) => {
            log::warn!("Thumbnail generation failed: {}", e);
        }
        Err(e) => {
            log::warn!("Thumbnail task panicked: {}", e);
        }
    }
}

pub async fn ensure_photo_downloaded(uuid: &str) -> Result<PathBuf, AppError> {
    photo_paths::ensure_photo_storage_dir()?;

    let runtime = load_runtime()?;
    let metadata_client = SpacetimePhotoMetadataClient::new(&runtime).await?;
    let photo = metadata_client
        .query_photo_by_uuid(uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("photo {uuid}")))?;

    let expected_local = photo_paths::relative_to_absolute(&photo.relative_path);
    if expected_local.exists() {
        // Main file exists – ensure thumbnails are present (they may be missing after
        // a sync download on another device or after a crop-and-resize operation).
        ensure_thumbnails_for_path(&expected_local).await;
        // Download older crop versions in the background so we don't block the UI.
        spawn_crop_version_downloads(uuid.to_string(), runtime);
        return Ok(expected_local);
    }

    // NOTE: The previous "legacy fallback" that copied uuid.jpg → uuid-vN.jpg has been
    // removed.  It was incorrect for cropped photos (it put uncropped content into the
    // versioned file and prevented the real crop from ever being downloaded) and was a
    // no-op for original photos (both paths are identical, so the first exists() check
    // already covers that case).

    metadata_client
        .update_photo_sync_status(uuid, "downloading", None, photo.retry_count)
        .await?;

    let webdav = NextcloudWebDav::new(&runtime).await?;

    let download = webdav
        .download_original(&photo.relative_path, &expected_local)
        .await;

    match download {
        Ok(()) => {
            // Generate thumbnails using the file stem of the downloaded path so that the
            // photo-gallery service resolves them correctly.
            // e.g. relative_path="uuid-v1.jpg" → stem="uuid-v1" → "uuid-v1_128.webp"
            ensure_thumbnails_for_path(&expected_local).await;

            // Download older crop versions in the background – don't block the UI on
            // network I/O that isn't needed for the current photo to be displayed.
            spawn_crop_version_downloads(uuid.to_string(), runtime);

            metadata_client
                .update_photo_sync_status(uuid, "synced", None, 0)
                .await?;
            Ok(expected_local)
        }
        Err(err) => {
            metadata_client
                .update_photo_sync_status(
                    uuid,
                    "error",
                    Some(err.to_string()),
                    photo
                        .retry_count
                        .saturating_add(1)
                        .min(MAX_DOWNLOAD_RETRIES),
                )
                .await?;
            Err(AppError::Other(format!("photo download failed: {err}")))
        }
    }
}

/// Spawn a fire-and-forget background task to download older crop versions.
/// This must NOT be awaited in the UI path – it makes WebDAV network calls
/// (including directory-creation probes) that can block for many seconds on a
/// slow or unreachable Nextcloud server.
fn spawn_crop_version_downloads(
    uuid: String,
    runtime: crate::services::photo_sync_metadata::PhotoSyncRuntime,
) {
    tokio::spawn(async move {
        if let Err(e) = download_crop_versions(&uuid, &runtime).await {
            log::debug!("Background crop-version download for {}: {}", uuid, e);
        }
    });
}

/// Download all cropped versions of a photo (uuid-v1.jpg, uuid-v2.jpg, etc.) and
/// ensure thumbnails exist for each one.  Best-effort: stops at the first version
/// that is not found on Nextcloud.
async fn download_crop_versions(
    uuid: &str,
    runtime: &crate::services::photo_sync_metadata::PhotoSyncRuntime,
) -> Result<(), AppError> {
    let webdav = NextcloudWebDav::new(runtime).await?;
    let storage_root = photo_paths::photo_storage_root();

    let mut version = 1;
    loop {
        let versioned_filename = format!("{}-v{}.jpg", uuid, version);
        let versioned_local = storage_root.join(&versioned_filename);

        if versioned_local.exists() {
            // File already present locally – only ensure thumbnails are up to date.
            ensure_thumbnails_for_path(&versioned_local).await;
            version += 1;
            continue;
        }

        // File not present locally – try to fetch it from Nextcloud.
        match webdav
            .download_original(&versioned_filename, &versioned_local)
            .await
        {
            Ok(()) => {
                log::info!(
                    "Downloaded crop version {}: {}",
                    version,
                    versioned_filename
                );
                ensure_thumbnails_for_path(&versioned_local).await;
                version += 1;
            }
            Err(_) => {
                // Version doesn't exist on Nextcloud – stop here.
                break;
            }
        }
    }

    Ok(())
}
