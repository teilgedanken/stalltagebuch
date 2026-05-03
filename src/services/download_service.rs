use crate::error::AppError;
use crate::services::nextcloud_webdav::NextcloudWebDav;
use crate::services::photo_paths;
use crate::services::photo_sync_metadata::{SpacetimePhotoMetadataClient, load_runtime};
use std::path::PathBuf;

const MAX_DOWNLOAD_RETRIES: i32 = 5;

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
        // Main file exists, but check for crop versions (best-effort)
        download_crop_versions(uuid, &runtime).await.ok();
        return Ok(expected_local);
    }

    // Legacy fallback: older code stored downloaded files as <uuid>.jpg.
    // If that file exists, mirror it into the DB-defined relative path location.
    let legacy_local = photo_paths::original_absolute_path(uuid);
    if legacy_local.exists() {
        if let Some(parent) = expected_local.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&legacy_local, &expected_local)?;
        download_crop_versions(uuid, &runtime).await.ok();
        return Ok(expected_local);
    }

    metadata_client
        .update_photo_sync_status(uuid, "downloading", None, photo.retry_count)
        .await?;

    let webdav = NextcloudWebDav::new(&runtime).await?;

    let download = webdav
        .download_original(&photo.relative_path, &expected_local)
        .await;

    match download {
        Ok(()) => {
            // Proactively generate thumbnails so they exist when the UI re-renders.
            // This avoids a synchronous thumbnail-generation block in the render path.
            let thumb_path = expected_local.to_string_lossy().to_string();
            let thumb_uuid = uuid.to_string();
            let _ = tokio::task::spawn_blocking(move || {
                photo_gallery::create_thumbnails(&thumb_path, &thumb_uuid, 400, 512)
            })
            .await;

            // Also download crop versions (best-effort, don't fail if these don't exist)
            download_crop_versions(uuid, &runtime).await.ok();

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

/// Download all cropped versions of a photo (uuid-v1.jpg, uuid-v2.jpg, etc.)
/// This is best-effort: if a version doesn't exist on Nextcloud, we skip it.
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

        // Try to download this version
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
                version += 1;
                // Continue looking for more versions
            }
            Err(_) => {
                // Version doesn't exist on Nextcloud, stop checking
                break;
            }
        }
    }

    Ok(())
}
