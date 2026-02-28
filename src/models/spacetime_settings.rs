use serde::{Deserialize, Serialize};

/// Connection settings for SpacetimeDB.
///
/// Stored persistently via the app's config file so that the user only needs
/// to enter them once.  The `token` field is the bearer token obtained either
/// through the Nextcloud Login Flow or via `spacetime login`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SpacetimeSettings {
    /// Base URL of the SpacetimeDB instance, e.g.
    /// `https://testnet.spacetimedb.com` or a self-hosted instance.
    pub server_url: String,
    /// Name of the published database (as given to `spacetime publish`).
    pub database_name: String,
    /// Bearer token for authentication.
    pub token: String,
    /// Nextcloud base URL used for quail photo storage.
    pub nextcloud_url: String,
    /// Nextcloud username.
    pub nextcloud_username: String,
    /// Nextcloud app password for WebDAV photo uploads.
    pub nextcloud_app_password: String,
    /// Remote path on the Nextcloud server where photos are stored.
    pub nextcloud_remote_path: String,
}

impl SpacetimeSettings {
    pub fn is_spacetime_configured(&self) -> bool {
        !self.server_url.is_empty() && !self.database_name.is_empty() && !self.token.is_empty()
    }

    pub fn is_nextcloud_configured(&self) -> bool {
        !self.nextcloud_url.is_empty()
            && !self.nextcloud_username.is_empty()
            && !self.nextcloud_app_password.is_empty()
    }
}
