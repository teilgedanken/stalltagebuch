use crate::error::AppError;
use crate::services::{nextcloud_webdav::NextcloudWebDav, photo_sync_metadata};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct RemotePhotoCleanupResult {
    pub deleted_paths: Vec<String>,
    pub failed_paths: Vec<(String, String)>,
}

pub async fn find_orphaned_remote_photos(
    known_relative_paths: &[String],
) -> Result<Vec<String>, AppError> {
    let runtime = photo_sync_metadata::load_runtime()?;
    let webdav = NextcloudWebDav::new(&runtime).await?;
    let known_paths: HashSet<String> = known_relative_paths
        .iter()
        .map(|path| normalize_relative_path(path))
        .filter(|path| !path.is_empty())
        .collect();

    let mut orphaned_paths: Vec<String> = webdav
        .list_remote_photos()
        .await?
        .into_iter()
        .map(|path| normalize_relative_path(&path))
        .filter(|path| !path.is_empty() && !known_paths.contains(path))
        .collect();

    orphaned_paths.sort_unstable();
    orphaned_paths.dedup();
    Ok(orphaned_paths)
}

pub async fn delete_remote_photos(
    relative_paths: &[String],
) -> Result<RemotePhotoCleanupResult, AppError> {
    let runtime = photo_sync_metadata::load_runtime()?;
    let webdav = NextcloudWebDav::new(&runtime).await?;
    let mut result = RemotePhotoCleanupResult::default();
    let mut unique_paths = relative_paths
        .iter()
        .map(|path| normalize_relative_path(path))
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();

    unique_paths.sort_unstable();
    unique_paths.dedup();

    for relative_path in unique_paths {
        match webdav.delete_remote_photo(&relative_path).await {
            Ok(()) => result.deleted_paths.push(relative_path),
            Err(error) => result.failed_paths.push((relative_path, error.to_string())),
        }
    }

    Ok(result)
}

fn normalize_relative_path(path: &str) -> String {
    path.trim().trim_start_matches('/').replace('\\', "/")
}
