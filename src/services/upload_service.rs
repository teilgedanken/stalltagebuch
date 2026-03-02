use crate::error::AppError;

/// Get device ID for sync operations
pub fn get_device_id() -> Result<String, AppError> {
    use uuid::Uuid;
    // TODO: Persist device_id to SpacetimeDB settings table
    Ok(Uuid::new_v4().to_string())
}

/// Uploads a batch of operations to sync/ops/<device>/<YYYYMM>/<ULID>.ndjson
///
/// NOTE: This is a stub function. Full sync implementation will be handled by SpacetimeDB
/// in a future phase. For now, returns Ok() to allow the app to compile and run.
pub async fn upload_ops_batch(
) -> Result<(), AppError> {
    
    // TODO: Implement real sync upload logic with SpacetimeDB
    log::debug!("Upload sync stub operations - SpacetimeDB sync not yet fully implemented");
    Ok(())
}

/// Uploads photos that are missing on the remote
pub async fn upload_photos_batch() -> Result<usize, AppError> {
    // TODO: Implement photo upload logic  
    log::debug!("Photo upload stub - SpacetimeDB sync not yet fully implemented");
    Ok(0)
}
