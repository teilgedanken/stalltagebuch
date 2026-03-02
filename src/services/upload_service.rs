use crate::error::AppError;

/// Uploads photos that are missing on the remote
pub async fn upload_photos_batch() -> Result<usize, AppError> {
    // TODO: Implement photo upload logic  
    log::debug!("Photo upload stub - SpacetimeDB sync not yet fully implemented");
    Ok(0)
}
