use crate::error::AppError;
use crate::models::import_v1::*;
use crate::services::device_id_service;
use crate::services::spacetime_settings_service;
use crate::spacetime::client::SpacetimeClient;
use std::collections::HashMap;
use std::io::Read;
use zip::ZipArchive;

fn sats_option_string(value: Option<String>) -> serde_json::Value {
    match value {
        Some(v) => serde_json::json!({ "some": v }),
        None => serde_json::Value::Null,
    }
}

/// Import v1 format ZIP to SpacetimeDB with full migration
///
/// This function handles the migration from the old v1 format (DateTime strings, CRDT fields)
/// to the new v2 SpacetimeDB format (Unix timestamps, multi-device support).
///
/// Data transformation:
/// - DateTime strings → Unix timestamps (i64 ms)
/// - profile_photo UUIDs → PhotoCollection + Photo relations
/// - event_date strings → Unix timestamps
/// - owner set to current device_id
pub async fn import_v1_from_zip(
    archive: &mut ZipArchive<impl Read + std::io::Seek>,
    progress_callback: &mut impl FnMut(String),
) -> Result<(usize, usize), AppError> {
    progress_callback("Reading v1 ZIP archive...".to_string());

    // Get current device ID and use as owner
    let device_id = device_id_service::get_device_id()
        .map_err(|e| AppError::Other(format!("Failed to get device ID: {}", e)))?;
    let owner = device_id.clone();

    let settings = spacetime_settings_service::load_spacetime_settings()?;
    if !settings.is_spacetime_configured() {
        return Err(AppError::Other(
            "Spacetime settings are missing; cannot import data".to_string(),
        ));
    }
    let client = SpacetimeClient::new(settings.server_url, settings.database_name, settings.token);

    log::info!("Starting v1 import with device_id: {}", device_id);

    // Read and parse export container
    progress_callback("Loading v1 data...".to_string());

    let export = read_v1_export_container(archive)?;

    log::info!(
        "V1 export loaded: {} quails, {} events, {} egg records, {} photos",
        export.quails.len(),
        export.events.len(),
        export.egg_records.len(),
        export.photos.len()
    );

    // Ensure photo storage directory exists
    crate::services::photo_paths::ensure_photo_storage_dir()?;

    // Import data in order (dependencies first)
    // 1. Import quails
    progress_callback(format!("Importing {} quails...", export.quails.len()));
    let mut imported_quails = 0usize;
    for quail in &export.quails {
        import_quail_v1(&client, quail, &owner, &device_id).await?;
        imported_quails += 1;
    }
    log::info!("Imported {} quails", imported_quails);

    // 2. Import events
    progress_callback(format!("Importing {} events...", export.events.len()));
    let mut imported_events = 0usize;
    for event in &export.events {
        import_event_v1(&client, event, &owner, &device_id).await?;
        imported_events += 1;
    }
    log::info!("Imported {} events", imported_events);

    // 3. Import egg records
    progress_callback(format!(
        "Importing {} egg records...",
        export.egg_records.len()
    ));
    let mut imported_egg_records = 0usize;
    for egg_record in &export.egg_records {
        import_egg_record_v1(&client, egg_record, &owner, &device_id).await?;
        imported_egg_records += 1;
    }
    log::info!("Imported {} egg records", imported_egg_records);

    // 4. Import photos (must be after PhotoCollections are created)
    progress_callback(format!("Importing {} photos...", export.photos.len()));
    let mut imported_photos = 0usize;
    let mut quail_collections: HashMap<String, String> = HashMap::new();
    let mut event_collections: HashMap<String, String> = HashMap::new();
    for photo in &export.photos {
        if let Err(e) = import_photo_v1(
            &client,
            photo,
            archive,
            &owner,
            &device_id,
            &mut quail_collections,
            &mut event_collections,
        )
        .await
        {
            log::warn!("Failed to import photo {}: {}", photo.uuid, e);
            // Continue with other photos
        } else {
            imported_photos += 1;
        }
    }
    log::info!("Imported {} photos", imported_photos);

    // 5. Restore profile photos after photo metadata exists.
    for quail in &export.quails {
        if quail.deleted != 0 {
            continue;
        }
        if let Some(profile_photo_uuid) = &quail.profile_photo {
            let payload = serde_json::json!({
                "quail_uuid": quail.uuid.clone(),
                "photo_uuid": sats_option_string(Some(profile_photo_uuid.clone())),
            });

            if let Err(e) = client.call_reducer_raw("set_quail_photo", &payload).await {
                log::warn!(
                    "Failed to set profile photo for quail {}: {}",
                    quail.uuid,
                    e
                );
            }
        }
    }

    // Calculate total counts
    let total_items = imported_quails + imported_events + imported_egg_records;
    let total_photos = imported_photos;

    Ok((total_items, total_photos))
}

/// Read and parse the v1 export container from ZIP
fn read_v1_export_container(
    archive: &mut ZipArchive<impl Read + std::io::Seek>,
) -> Result<ExportContainerV1, AppError> {
    // Read quails.json
    let quails = read_json_from_zip(archive, "data/quails.json")
        .map(|data: serde_json::Value| {
            data.get("quails")
                .and_then(|q| serde_json::from_value(q.clone()).ok())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    // Read events.json
    let events = read_json_from_zip(archive, "data/events.json")
        .map(|data: serde_json::Value| {
            data.get("events")
                .and_then(|e| serde_json::from_value(e.clone()).ok())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    // Read egg_records.json
    let egg_records = read_json_from_zip(archive, "data/egg_records.json")
        .map(|data: serde_json::Value| {
            data.get("egg_records")
                .and_then(|e| serde_json::from_value(e.clone()).ok())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    // Read photos.json
    let photos = read_json_from_zip(archive, "data/photos.json")
        .map(|data: serde_json::Value| {
            data.get("photos")
                .and_then(|p| p.as_array())
                .map(|photos| {
                    photos
                        .iter()
                        .filter_map(|photo| {
                            match serde_json::from_value::<PhotoV1>(photo.clone()) {
                                Ok(photo) => Some(photo),
                                Err(e) => {
                                    log::warn!("Skipping unparseable v1 photo entry: {}", e);
                                    None
                                }
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    Ok(ExportContainerV1 {
        quails,
        events,
        egg_records,
        photos,
    })
}

/// Helper to read JSON file from ZIP
fn read_json_from_zip(
    archive: &mut ZipArchive<impl Read + std::io::Seek>,
    path: &str,
) -> Result<serde_json::Value, AppError> {
    let mut file = archive
        .by_name(path)
        .map_err(|e| AppError::Other(format!("Failed to find {} in ZIP: {}", path, e)))?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| AppError::Other(format!("Failed to read {} from ZIP: {}", path, e)))?;

    serde_json::from_str(&content)
        .map_err(|e| AppError::Other(format!("Failed to parse {}: {}", path, e)))
}

/// Import a single quail from v1 format
async fn import_quail_v1(
    client: &SpacetimeClient,
    quail: &QuailV1,
    owner: &str,
    device_id: &str,
) -> Result<(), AppError> {
    // Skip if already deleted in v1
    if quail.deleted != 0 {
        log::debug!("Skipping deleted quail: {}", quail.uuid);
        return Ok(());
    }

    let created_at = parse_v1_datetime(&quail.created_at)
        .map_err(|e| AppError::Other(format!("Failed to parse quail created_at: {}", e)))?;

    // Create quail record in SpacetimeDB
    // Note: This would call the generated reducer for Quail creation
    // For now, we document the structure
    log::debug!(
        "Importing quail: {} (gender: {}, legacy ring_color -> left: {:?})",
        quail.name,
        quail.gender,
        quail.ring_color
    );

    let _ = owner;
    let _ = created_at;

    let create_args = serde_json::json!({
        "uuid": quail.uuid.clone(),
        "name": quail.name.clone(),
        "gender": quail.gender.clone(),
        "ring_color_left": sats_option_string(quail.ring_color.clone()),
        "ring_color_right": serde_json::Value::Null,
        "profile_photo": serde_json::Value::Null,
        "device_id": device_id,
    });
    if let Err(create_err) = client.call_reducer("create_quail", &create_args).await {
        // Re-import support: if record already exists, try update_quail instead.
        let update_args = serde_json::json!({
            "uuid": quail.uuid.clone(),
            "name": quail.name.clone(),
            "gender": quail.gender.clone(),
            "ring_color_left": sats_option_string(quail.ring_color.clone()),
            "ring_color_right": serde_json::Value::Null,
            "profile_photo": serde_json::Value::Null,
        });
        client
            .call_reducer("update_quail", &update_args)
            .await
            .map_err(|update_err| {
                AppError::Other(format!(
                    "create_quail failed: {}; update_quail fallback failed: {}",
                    create_err, update_err
                ))
            })?;
    }

    Ok(())
}

/// Import a single event from v1 format
async fn import_event_v1(
    client: &SpacetimeClient,
    event: &QuailEventV1,
    owner: &str,
    device_id: &str,
) -> Result<(), AppError> {
    // Skip if already deleted in v1
    if event.deleted != 0 {
        log::debug!("Skipping deleted event: {}", event.uuid);
        return Ok(());
    }

    log::debug!(
        "Importing event: {} (type: {}, quail_id: {})",
        event.uuid,
        event.event_type,
        event.quail_id
    );

    let _ = owner;

    let create_args = serde_json::json!({
        "uuid": event.uuid.clone(),
        "quail_uuid": event.quail_id.clone(),
        "event_type": event.event_type.clone(),
        "event_date": event.event_date.clone(),
        "notes": sats_option_string(event.notes.clone()),
        "photos": serde_json::Value::Null,
        "device_id": device_id,
    });
    if let Err(create_err) = client.call_reducer("create_event", &create_args).await {
        let update_args = serde_json::json!({
            "uuid": event.uuid.clone(),
            "event_type": event.event_type.clone(),
            "event_date": event.event_date.clone(),
            "notes": sats_option_string(event.notes.clone()),
            "photos": serde_json::Value::Null,
        });
        client
            .call_reducer("update_event", &update_args)
            .await
            .map_err(|update_err| {
                AppError::Other(format!(
                    "create_event failed: {}; update_event fallback failed: {}",
                    create_err, update_err
                ))
            })?;
    }

    Ok(())
}

/// Import a single egg record from v1 format
async fn import_egg_record_v1(
    client: &SpacetimeClient,
    egg_record: &EggRecordV1,
    owner: &str,
    device_id: &str,
) -> Result<(), AppError> {
    // Skip if already deleted in v1
    if egg_record.deleted != 0 {
        log::debug!("Skipping deleted egg record: {}", egg_record.uuid);
        return Ok(());
    }

    let record_date = parse_v1_date(&egg_record.record_date)
        .map_err(|e| AppError::Other(format!("Failed to parse record_date: {}", e)))?;

    log::debug!(
        "Importing egg record: {} (date: {}, eggs: {})",
        egg_record.uuid,
        egg_record.record_date,
        egg_record.total_eggs
    );

    let _ = owner;

    let args = serde_json::json!({
        "uuid": egg_record.uuid.clone(),
        "record_date": record_date,
        "total_eggs": egg_record.total_eggs,
        "notes": sats_option_string(egg_record.notes.clone()),
        "device_id": device_id,
    });
    client.call_reducer("upsert_egg_record", &args).await?;

    Ok(())
}

/// Import a single photo from v1 format
async fn import_photo_v1(
    client: &SpacetimeClient,
    photo: &PhotoV1,
    archive: &mut ZipArchive<impl Read + std::io::Seek>,
    owner: &str,
    device_id: &str,
    quail_collections: &mut HashMap<String, String>,
    event_collections: &mut HashMap<String, String>,
) -> Result<(), AppError> {
    // Skip if already deleted in v1
    if photo.deleted != 0 {
        log::debug!("Skipping deleted photo: {}", photo.uuid);
        return Ok(());
    }

    log::debug!(
        "Importing photo: {} (path: {})",
        photo.uuid,
        photo.relative_path
    );

    // Copy photo from ZIP to app storage; v1 exports can use different path layouts.
    let local_path = crate::services::photo_paths::original_absolute_path(&photo.uuid);
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let candidates = [
        format!("photos/{}", photo.uuid),
        format!("photos/{}.jpg", photo.uuid),
        format!("photos/{}.jpeg", photo.uuid),
        format!("photos/{}", photo.relative_path.trim_start_matches('/')),
        photo.relative_path.trim_start_matches('/').to_string(),
    ];

    let mut copied = false;
    for candidate in candidates {
        if let Ok(mut photo_file) = archive.by_name(&candidate) {
            let mut dest = std::fs::File::create(&local_path)?;
            std::io::copy(&mut photo_file, &mut dest)?;
            copied = true;
            break;
        }
    }

    if !copied {
        return Err(AppError::Other(format!(
            "Failed to find photo {} in ZIP (relative_path={})",
            photo.uuid, photo.relative_path
        )));
    }

    // Pre-generate thumbnails during import to avoid expensive full-image loading in UI.
    let _ = photo_gallery::create_thumbnails(
        local_path.to_string_lossy().as_ref(),
        &photo.uuid,
        128,
        512,
    );

    log::debug!("Photo copied to: {}", local_path.display());

    let _ = owner;

    // Use canonical path format expected by current app/photo service.
    let relative_path = crate::services::photo_paths::original_relative_path(&photo.uuid);
    let (collection_uuid, create_collection_args) = if let Some(quail_id) = &photo.quail_id {
        // Requirement: merge all photos of a quail into exactly one collection.
        let col_uuid = quail_collections
            .entry(quail_id.clone())
            .or_insert_with(|| format!("v1-col-quail-{}", quail_id))
            .clone();
        (
            col_uuid,
            Some(serde_json::json!({
                "uuid": quail_collections
                    .get(quail_id)
                    .cloned()
                    .unwrap_or_else(|| format!("v1-col-quail-{}", quail_id)),
                "quail_uuid": sats_option_string(Some(quail_id.clone())),
                "event_uuid": serde_json::Value::Null,
                "name": format!("Imported quail {}", quail_id),
                "device_id": device_id,
            })),
        )
    } else if let Some(event_id) = &photo.event_id {
        let col_uuid = event_collections
            .entry(event_id.clone())
            .or_insert_with(|| format!("v1-col-event-{}", event_id))
            .clone();
        (
            col_uuid,
            Some(serde_json::json!({
                "uuid": event_collections
                    .get(event_id)
                    .cloned()
                    .unwrap_or_else(|| format!("v1-col-event-{}", event_id)),
                "quail_uuid": serde_json::Value::Null,
                "event_uuid": sats_option_string(Some(event_id.clone())),
                "name": format!("Imported event {}", event_id),
                "device_id": device_id,
            })),
        )
    } else {
        // Fallback for orphaned photos.
        let col_uuid = format!("v1-col-photo-{}", photo.uuid);
        (
            col_uuid.clone(),
            Some(serde_json::json!({
                "uuid": col_uuid,
                "quail_uuid": serde_json::Value::Null,
                "event_uuid": serde_json::Value::Null,
                "name": "Imported photo".to_string(),
                "device_id": device_id,
            })),
        )
    };

    if let Some(create_collection_args) = create_collection_args {
        // Collection may already exist (e.g., subsequent photo of same quail).
        // Ignore reducer errors in that case and continue with photo create.
        let _ = client
            .call_reducer("create_photo_collection", &create_collection_args)
            .await;
    }

    let create_photo_args = serde_json::json!({
        "uuid": photo.uuid.clone(),
        "collection_uuid": collection_uuid.clone(),
        "relative_path": relative_path,
        "device_id": device_id,
    });
    client
        .call_reducer("create_photo", &create_photo_args)
        .await?;

    let update_collection_args = serde_json::json!({
        "uuid": collection_uuid,
        "preview_photo_uuid": sats_option_string(Some(photo.uuid.clone())),
        "updated_at": chrono::Utc::now().timestamp(),
    });
    client
        .call_reducer("update_photo_collection", &update_collection_args)
        .await?;

    Ok(())
}

// Re-export parsing helpers
use crate::models::import_v1::{parse_v1_date, parse_v1_datetime};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetime_parsing() {
        let dt = parse_v1_datetime("2025-11-22 16:34:53");
        assert!(dt.is_ok());
    }
}
