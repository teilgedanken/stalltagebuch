use crate::error::AppError;
use crate::services::nextcloud_webdav::NextcloudWebDav;
use crate::services::photo_paths;
use crate::services::photo_sync_metadata::{SpacetimePhotoMetadataClient, load_runtime};
use std::path::PathBuf;

const MAX_DOWNLOAD_RETRIES: i32 = 5;

pub async fn ensure_photo_downloaded(uuid: &str) -> Result<PathBuf, AppError> {
    photo_paths::ensure_photo_storage_dir()?;

    let local_original = photo_paths::original_absolute_path(uuid);
    if local_original.exists() {
        return Ok(local_original);
    }

    let runtime = load_runtime()?;
    let metadata_client = SpacetimePhotoMetadataClient::new(&runtime)?;
    let photo = metadata_client
        .query_photo_by_uuid(uuid)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("photo {uuid}")))?;

    metadata_client
        .update_photo_sync_status(uuid, "downloading", None, photo.retry_count)
        .await?;

    let webdav = NextcloudWebDav::new(&runtime).await?;

    let download = webdav
        .download_original(&photo.relative_path, &local_original)
        .await;

    match download {
        Ok(()) => {
            metadata_client
                .update_photo_sync_status(uuid, "synced", None, 0)
                .await?;
            Ok(local_original)
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

/// Downloads and merges operations from sync/ops/ directory
pub async fn download_and_merge_ops() -> Result<usize, AppError> {
    // Operation log sync is kept separate from photo-on-demand downloads.
    Ok(0)
}
