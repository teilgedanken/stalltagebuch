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
