use crate::error::AppError;
use crate::services::nextcloud_webdav::NextcloudWebDav;
use crate::services::photo_paths;
use crate::services::photo_sync_metadata::{SpacetimePhotoMetadataClient, load_runtime};

const MAX_UPLOAD_RETRIES: i32 = 5;

pub async fn upload_photos_batch_with_progress<F>(mut on_progress: F) -> Result<usize, AppError>
where
    F: FnMut(usize, usize),
{
    let runtime = load_runtime()?;
    let metadata_client = SpacetimePhotoMetadataClient::new(&runtime).await?;
    let webdav = NextcloudWebDav::new(&runtime).await?;

    let photos = metadata_client
        .query_pending_photos(MAX_UPLOAD_RETRIES)
        .await?;
    let total = photos.len();
    let mut uploaded = 0usize;
    on_progress(0, total);

    for (index, photo) in photos.iter().enumerate() {
        let current = index + 1;

        let absolute_path = photo_paths::original_absolute_path(&photo.uuid);
        let local_exists = absolute_path.exists();
        if !local_exists {
            let error = format!("local original not found: {}", absolute_path.display());
            metadata_client
                .update_photo_sync_status(
                    &photo.uuid,
                    "error",
                    Some(error),
                    photo.retry_count.saturating_add(1),
                )
                .await?;
            on_progress(current, total);
            continue;
        }

        metadata_client
            .update_photo_sync_status(&photo.uuid, "uploading", None, photo.retry_count)
            .await?;

        let upload_result = webdav
            .upload_original(&absolute_path, &photo.relative_path)
            .await;

        match upload_result {
            Ok(()) => {
                metadata_client
                    .update_photo_sync_status(&photo.uuid, "synced", None, 0)
                    .await?;
                uploaded += 1;
            }
            Err(err) => {
                metadata_client
                    .update_photo_sync_status(
                        &photo.uuid,
                        "error",
                        Some(err.to_string()),
                        photo.retry_count.saturating_add(1),
                    )
                    .await?;
            }
        }

        on_progress(current, total);
    }

    Ok(uploaded)
}

/// Uploads photos that are missing on the remote
pub async fn upload_photos_batch() -> Result<usize, AppError> {
    upload_photos_batch_with_progress(|_, _| {}).await
}
