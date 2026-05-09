use crate::error::AppError;
use crate::services::nextcloud_webdav::NextcloudWebDav;
use crate::services::photo_paths;
use crate::services::photo_sync_metadata::{
    PhotoSyncRow, SpacetimePhotoMetadataClient, load_runtime,
};
use std::collections::HashSet;
use std::path::Path;

const MAX_UPLOAD_RETRIES: i32 = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct PhotoUploadFailure {
    pub uuid: String,
    pub relative_path: String,
    pub error: String,
    pub retry_count: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UploadBatchStats {
    pub photos_uploaded: usize,
    pub failed_photos: Vec<PhotoUploadFailure>,
    pub local_original_files: usize,
    pub remote_original_files: usize,
    pub all_updated: bool,
}

pub async fn upload_photos_batch_with_progress<F>(
    mut on_progress: F,
) -> Result<UploadBatchStats, AppError>
where
    F: FnMut(usize, usize),
{
    let runtime = load_runtime()?;
    let metadata_client = SpacetimePhotoMetadataClient::new(&runtime).await?;
    let webdav = NextcloudWebDav::new(&runtime).await?;

    let pending_photos = metadata_client
        .query_pending_photos(MAX_UPLOAD_RETRIES)
        .await?;
    let total = pending_photos.len();
    let mut uploaded = 0usize;
    let mut failed_photos = Vec::new();
    log::info!(
        "Photo sync upload batch started: {} pending photo(s)",
        total
    );
    on_progress(0, total);

    for (index, photo) in pending_photos.iter().enumerate() {
        let current = index + 1;
        log::debug!(
            "Uploading photo {}/{} uuid={} relative_path={}",
            current,
            total,
            photo.uuid,
            photo.relative_path
        );

        let absolute_path = photo_paths::relative_to_absolute(&photo.relative_path);
        let local_exists = absolute_path.exists();
        if !local_exists {
            let error = format!(
                "local sync source not found for {}: {}",
                photo.relative_path,
                absolute_path.display()
            );
            log::warn!("{error}");
            metadata_client
                .update_photo_sync_status(
                    &photo.uuid,
                    "error",
                    Some(error),
                    photo.retry_count.saturating_add(1),
                )
                .await?;
            failed_photos.push(PhotoUploadFailure {
                uuid: photo.uuid.clone(),
                relative_path: photo.relative_path.clone(),
                error: format!(
                    "local sync source not found for {}: {}",
                    photo.relative_path,
                    absolute_path.display()
                ),
                retry_count: photo.retry_count.saturating_add(1),
            });
            on_progress(current, total);
            continue;
        }

        metadata_client
            .update_photo_sync_status(&photo.uuid, "uploading", None, photo.retry_count)
            .await?;

        // Upload currently active photo path from metadata.
        let mut photo_error = webdav
            .upload_original(&absolute_path, &photo.relative_path)
            .await
            .err()
            .map(|err| err.to_string());

        // Upload all versioned originals that are present locally.
        if photo_error.is_none() {
            let versioned_paths = collect_versioned_original_paths(photo)?;
            for versioned_path in versioned_paths {
                let versioned_local = photo_paths::relative_to_absolute(&versioned_path);
                match webdav
                    .upload_original(&versioned_local, &versioned_path)
                    .await
                {
                    Ok(()) => {
                        log::info!("Uploaded versioned original: {}", versioned_path);
                    }
                    Err(err) => {
                        let message = format!(
                            "failed to upload versioned original {}: {}",
                            versioned_path, err
                        );
                        log::warn!("{message}");
                        photo_error = Some(message);
                        break;
                    }
                }
            }
        }

        match photo_error {
            None => {
                metadata_client
                    .update_photo_sync_status(&photo.uuid, "synced", None, 0)
                    .await?;
                log::info!(
                    "Uploaded photo uuid={} path={}",
                    photo.uuid,
                    photo.relative_path
                );
                uploaded += 1;
            }
            Some(error) => {
                log::warn!(
                    "Photo upload failed uuid={} path={}: {}",
                    photo.uuid,
                    photo.relative_path,
                    error
                );
                metadata_client
                    .update_photo_sync_status(
                        &photo.uuid,
                        "error",
                        Some(error.clone()),
                        photo.retry_count.saturating_add(1),
                    )
                    .await?;
                failed_photos.push(PhotoUploadFailure {
                    uuid: photo.uuid.clone(),
                    relative_path: photo.relative_path.clone(),
                    error,
                    retry_count: photo.retry_count.saturating_add(1),
                });
            }
        }

        on_progress(current, total);
    }

    // Reconcile missing remote files:
    // for all DB photo entries, upload local originals/versioned originals that are absent on Nextcloud.
    let all_photos = metadata_client.query_all_photos().await?;
    let mut local_original_paths: HashSet<String> = HashSet::new();
    for photo in &all_photos {
        for candidate in build_original_candidates(photo)? {
            let local_path = photo_paths::relative_to_absolute(&candidate);
            if local_path.exists() {
                local_original_paths.insert(candidate);
            }
        }
    }

    let mut remote_paths: HashSet<String> = webdav
        .list_remote_photos()
        .await?
        .into_iter()
        .map(|p| normalize_relative_path(&p))
        .filter(|p| is_original_file_path(p))
        .collect();

    for photo in &all_photos {
        let candidates = build_original_candidates(photo)?;
        let mut first_error: Option<String> = None;
        let mut uploaded_any_for_photo = false;

        for candidate in candidates {
            if remote_paths.contains(&candidate) {
                continue;
            }

            let local_path = photo_paths::relative_to_absolute(&candidate);
            if !local_path.exists() {
                continue;
            }

            log::info!(
                "Uploading missing remote original for photo uuid={} path={}",
                photo.uuid,
                candidate
            );

            match webdav.upload_original(&local_path, &candidate).await {
                Ok(()) => {
                    remote_paths.insert(candidate.clone());
                    uploaded += 1;
                    uploaded_any_for_photo = true;
                }
                Err(err) => {
                    let error = format!("missing-remote upload failed for {}: {}", candidate, err);
                    if first_error.is_none() {
                        first_error = Some(error.clone());
                    }
                    failed_photos.push(PhotoUploadFailure {
                        uuid: photo.uuid.clone(),
                        relative_path: candidate,
                        error,
                        retry_count: photo.retry_count.saturating_add(1),
                    });
                }
            }
        }

        if let Some(error) = first_error {
            metadata_client
                .update_photo_sync_status(
                    &photo.uuid,
                    "error",
                    Some(error),
                    photo.retry_count.saturating_add(1),
                )
                .await?;
        } else if uploaded_any_for_photo {
            metadata_client
                .update_photo_sync_status(&photo.uuid, "synced", None, 0)
                .await?;
        }
    }

    log::info!(
        "Photo sync upload batch finished: {} uploaded, {} total",
        uploaded,
        total
    );

    let remote_original_files = local_original_paths
        .iter()
        .filter(|path| remote_paths.contains(*path))
        .count();
    let local_original_files = local_original_paths.len();
    let all_updated = remote_original_files == local_original_files;

    Ok(UploadBatchStats {
        photos_uploaded: uploaded,
        failed_photos,
        local_original_files,
        remote_original_files,
        all_updated,
    })
}

fn build_original_candidates(photo: &PhotoSyncRow) -> Result<Vec<String>, AppError> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    let primary = normalize_relative_path(&photo.relative_path);
    if is_original_file_path(&primary) && seen.insert(primary.clone()) {
        candidates.push(primary);
    }

    for versioned in collect_versioned_original_paths(photo)? {
        if seen.insert(versioned.clone()) {
            candidates.push(versioned);
        }
    }

    Ok(candidates)
}

fn normalize_relative_path(path: &str) -> String {
    path.trim_start_matches('/').replace('\\', "/")
}

fn is_original_file_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    !(lower.ends_with("_128.webp") || lower.ends_with("_512.webp") || lower.ends_with(".webp"))
}

fn collect_versioned_original_paths(photo: &PhotoSyncRow) -> Result<Vec<String>, AppError> {
    let normalized_relative = normalize_relative_path(&photo.relative_path);
    let relative_path = Path::new(&normalized_relative);
    let relative_dir = relative_path.parent().unwrap_or_else(|| Path::new(""));
    let absolute_dir = photo_paths::photo_storage_root().join(relative_dir);

    if !absolute_dir.exists() {
        return Ok(Vec::new());
    }

    let prefix = format!("{}-v", photo.uuid);
    let mut versioned_paths = Vec::new();

    for entry in std::fs::read_dir(&absolute_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.starts_with(&prefix) {
            continue;
        }
        if !is_original_file_path(&file_name) {
            continue;
        }

        let rel = if relative_dir.as_os_str().is_empty() {
            file_name
        } else {
            relative_dir
                .join(file_name)
                .to_string_lossy()
                .replace('\\', "/")
        };
        versioned_paths.push(rel);
    }

    versioned_paths.sort_unstable();
    versioned_paths.dedup();
    Ok(versioned_paths)
}
