use crate::error::AppError;
use crate::services::{nextcloud_webdav::NextcloudWebDav, photo_sync_metadata};
use std::path::PathBuf;

/// Uploads an already-created ZIP export to Nextcloud backup storage.
///
/// Returns the uploaded filename on success.
pub async fn upload_backup_to_nextcloud(zip_path: PathBuf) -> Result<String, AppError> {
    let runtime = photo_sync_metadata::load_runtime()?;
    let webdav = NextcloudWebDav::new(&runtime).await?;

    let filename = zip_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AppError::Other("invalid backup filename".to_string()))?
        .to_string();

    let bytes = std::fs::read(&zip_path)?;
    webdav.upload_backup(&bytes, &filename).await?;

    Ok(filename)
}
