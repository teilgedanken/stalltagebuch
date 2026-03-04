use crate::error::AppError;
use crate::services::photo_sync_metadata::PhotoSyncRuntime;

pub struct NextcloudWebDav {
    client: reqwest_dav::Client,
    remote_root: String,
}

impl NextcloudWebDav {
    pub async fn new(runtime: &PhotoSyncRuntime) -> Result<Self, AppError> {
        let webdav_url = format!(
            "{}/remote.php/dav/files/{}",
            runtime.nextcloud_server_url.trim_end_matches('/'),
            runtime.nextcloud_username
        );

        let client = reqwest_dav::ClientBuilder::new()
            .set_host(webdav_url)
            .set_auth(reqwest_dav::Auth::Basic(
                runtime.nextcloud_username.clone(),
                runtime.nextcloud_app_password.clone(),
            ))
            .build()
            .map_err(|e| AppError::Other(format!("failed to build WebDAV client: {e:?}")))?;

        let remote_root = runtime
            .nextcloud_remote_path
            .trim_end_matches('/')
            .to_string();
        let this = Self {
            client,
            remote_root,
        };
        this.ensure_sync_photos_dirs().await?;
        Ok(this)
    }

    pub async fn upload_original(
        &self,
        local_path: &std::path::Path,
        relative_path: &str,
    ) -> Result<(), AppError> {
        let bytes = std::fs::read(local_path)?;
        let remote_path = format!("{}/sync/photos/{}", self.remote_root, relative_path);
        self.client
            .put(&remote_path, bytes)
            .await
            .map_err(|e| AppError::Other(format!("WebDAV upload failed: {e:?}")))?;
        Ok(())
    }

    pub async fn download_original(
        &self,
        relative_path: &str,
        local_path: &std::path::Path,
    ) -> Result<(), AppError> {
        let remote_path = format!("{}/sync/photos/{}", self.remote_root, relative_path);
        let response = self
            .client
            .get(&remote_path)
            .await
            .map_err(|e| AppError::Other(format!("WebDAV download failed: {e:?}")))?;

        let bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::Other(format!("failed to read WebDAV response body: {e}")))?;

        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(local_path, &bytes)?;
        Ok(())
    }

    async fn ensure_sync_photos_dirs(&self) -> Result<(), AppError> {
        // Ignore MKCOL errors to support already-existing directories.
        let sync_dir = format!("{}/sync", self.remote_root);
        let photos_dir = format!("{}/photos", sync_dir);
        let _ = self.client.mkcol(&sync_dir).await;
        let _ = self.client.mkcol(&photos_dir).await;
        Ok(())
    }
}
