use crate::error::AppError;
use crate::models::export::{
    DeviceExport, EggRecordExport, ExportData, ExportMetadata, PhotoCollectionExport, PhotoExport,
    QuailEventExport, QuailExport,
};
use crate::services::spacetime_settings_service;
use crate::spacetime::client::SpacetimeClient;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::Read;
use zip::ZipArchive;

fn sats_option<T: Serialize>(value: Option<T>) -> serde_json::Value {
    match value {
        Some(value) => serde_json::json!({ "some": value }),
        None => serde_json::Value::Null,
    }
}

#[derive(Debug, Deserialize)]
struct ZipDataSection {
    devices: Vec<DeviceExport>,
    quails: Vec<QuailExport>,
    quail_events: Vec<QuailEventExport>,
    egg_records: Vec<EggRecordExport>,
    photos: Vec<PhotoExport>,
    photo_collections: Vec<PhotoCollectionExport>,
}

pub async fn import_v2_from_zip(
    archive: &mut ZipArchive<impl Read + std::io::Seek>,
    progress_callback: &mut impl FnMut(String),
) -> Result<(usize, usize), AppError> {
    progress_callback("Reading v2 ZIP archive...".to_string());

    let settings = spacetime_settings_service::load_spacetime_settings()?;
    if !settings.is_spacetime_configured() {
        return Err(AppError::Other(
            "Spacetime settings are missing; cannot import data".to_string(),
        ));
    }
    let client = SpacetimeClient::new(settings.server_url, settings.database_name, settings.token);

    progress_callback("Loading v2 data...".to_string());
    let export = read_v2_export_data(archive)?;

    log::info!(
        "V2 export loaded: {} devices, {} quails, {} events, {} egg records, {} collections, {} photos",
        export.devices.len(),
        export.quails.len(),
        export.quail_events.len(),
        export.egg_records.len(),
        export.photo_collections.len(),
        export.photos.len()
    );

    crate::services::photo_paths::ensure_photo_storage_dir()?;

    progress_callback(format!("Registering {} devices...", export.devices.len()));
    for device in &export.devices {
        import_device_v2(&client, device).await?;
    }

    progress_callback(format!("Importing {} quails...", export.quails.len()));
    let mut imported_quails = 0usize;
    for quail in &export.quails {
        import_quail_v2(&client, quail).await?;
        imported_quails += 1;
    }

    progress_callback(format!("Importing {} events...", export.quail_events.len()));
    let mut imported_events = 0usize;
    for event in &export.quail_events {
        import_event_v2(&client, event).await?;
        imported_events += 1;
    }

    progress_callback(format!(
        "Importing {} egg records...",
        export.egg_records.len()
    ));
    let mut imported_egg_records = 0usize;
    for egg_record in &export.egg_records {
        import_egg_record_v2(&client, egg_record).await?;
        imported_egg_records += 1;
    }

    progress_callback(format!(
        "Importing {} photo collections...",
        export.photo_collections.len()
    ));
    for collection in &export.photo_collections {
        import_photo_collection_v2(&client, collection).await;
    }

    progress_callback(format!("Importing {} photos...", export.photos.len()));
    let mut imported_photos = 0usize;
    for photo in &export.photos {
        if let Err(err) = import_photo_v2(&client, photo, archive).await {
            log::warn!("Failed to import v2 photo {}: {}", photo.uuid, err);
            continue;
        }
        imported_photos += 1;
    }

    progress_callback("Restoring profile photos...".to_string());
    for quail in &export.quails {
        if let Some(profile_photo_uuid) = &quail.profile_photo {
            let payload = serde_json::json!({
                "quail_uuid": quail.uuid.clone(),
                "photo_uuid": sats_option(Some(profile_photo_uuid.clone())),
            });

            if let Err(err) = client.call_reducer_raw("set_quail_photo", &payload).await {
                log::warn!(
                    "Failed to set profile photo for quail {}: {}",
                    quail.uuid,
                    err
                );
            }
        }
    }

    progress_callback("Restoring collection previews...".to_string());
    for collection in &export.photo_collections {
        if let Some(preview_photo_uuid) = &collection.preview_photo_uuid {
            let update_args = serde_json::json!({
                "uuid": collection.uuid.clone(),
                "preview_photo_uuid": sats_option(Some(preview_photo_uuid.clone())),
                "updated_at": collection.updated_at,
            });

            if let Err(err) = client
                .call_reducer("update_photo_collection", &update_args)
                .await
            {
                log::warn!(
                    "Failed to restore preview photo for collection {}: {}",
                    collection.uuid,
                    err
                );
            }
        }
    }

    Ok((
        imported_quails + imported_events + imported_egg_records,
        imported_photos,
    ))
}

fn read_v2_export_data(
    archive: &mut ZipArchive<impl Read + std::io::Seek>,
) -> Result<ExportData, AppError> {
    let metadata: ExportMetadata = read_json_from_zip(archive, "metadata.json")?;
    if metadata.format_version != 2 {
        return Err(AppError::Other(format!(
            "Expected V2 export, got format_version={} instead",
            metadata.format_version
        )));
    }

    let data: ZipDataSection = read_json_from_zip(archive, "data.json")?;

    Ok(ExportData {
        metadata,
        devices: data.devices,
        quails: data.quails,
        quail_events: data.quail_events,
        egg_records: data.egg_records,
        photos: data.photos,
        photo_collections: data.photo_collections,
    })
}

fn read_json_from_zip<T: DeserializeOwned>(
    archive: &mut ZipArchive<impl Read + std::io::Seek>,
    path: &str,
) -> Result<T, AppError> {
    let mut file = archive
        .by_name(path)
        .map_err(|e| AppError::Other(format!("Failed to find {} in ZIP: {}", path, e)))?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .map_err(|e| AppError::Other(format!("Failed to read {} from ZIP: {}", path, e)))?;

    serde_json::from_str(&content)
        .map_err(|e| AppError::Other(format!("Failed to parse {}: {}", path, e)))
}

async fn import_device_v2(client: &SpacetimeClient, device: &DeviceExport) -> Result<(), AppError> {
    let args = serde_json::json!({
        "device_id": device.device_id.clone(),
        "name": sats_option(device.name.clone()),
        "comment": sats_option(device.comment.clone()),
    });

    client.call_reducer("register_device", &args).await
}

async fn import_quail_v2(client: &SpacetimeClient, quail: &QuailExport) -> Result<(), AppError> {
    let create_args = serde_json::json!({
        "uuid": quail.uuid.clone(),
        "name": quail.name.clone(),
        "gender": quail.gender.clone(),
        "ring_color_left": sats_option(quail.ring_color_left.clone()),
        "ring_color_right": sats_option(quail.ring_color_right.clone()),
        "profile_photo": serde_json::Value::Null,
        "device_id": quail.device_id.clone(),
    });

    if let Err(create_err) = client.call_reducer("create_quail", &create_args).await {
        let update_args = serde_json::json!({
            "uuid": quail.uuid.clone(),
            "name": quail.name.clone(),
            "gender": quail.gender.clone(),
            "ring_color_left": sats_option(quail.ring_color_left.clone()),
            "ring_color_right": sats_option(quail.ring_color_right.clone()),
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

async fn import_event_v2(
    client: &SpacetimeClient,
    event: &QuailEventExport,
) -> Result<(), AppError> {
    let create_args = serde_json::json!({
        "uuid": event.uuid.clone(),
        "quail_uuid": event.quail_uuid.clone(),
        "event_type": event.event_type.clone(),
        "event_date": event.event_date.clone(),
        "notes": sats_option(event.notes.clone()),
        "photos": sats_option(event.photos.clone()),
        "device_id": event.device_id.clone(),
    });

    if let Err(create_err) = client.call_reducer("create_event", &create_args).await {
        let update_args = serde_json::json!({
            "uuid": event.uuid.clone(),
            "event_type": event.event_type.clone(),
            "event_date": event.event_date.clone(),
            "notes": sats_option(event.notes.clone()),
            "photos": sats_option(event.photos.clone()),
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

async fn import_egg_record_v2(
    client: &SpacetimeClient,
    egg_record: &EggRecordExport,
) -> Result<(), AppError> {
    let args = serde_json::json!({
        "uuid": egg_record.uuid.clone(),
        "record_date": egg_record.record_date,
        "total_eggs": egg_record.total_eggs,
        "notes": sats_option(egg_record.notes.clone()),
        "device_id": egg_record.device_id.clone(),
    });

    client.call_reducer("upsert_egg_record", &args).await
}

async fn import_photo_collection_v2(client: &SpacetimeClient, collection: &PhotoCollectionExport) {
    let args = serde_json::json!({
        "uuid": collection.uuid.clone(),
        "quail_uuid": sats_option(collection.quail_uuid.clone()),
        "event_uuid": sats_option(collection.event_uuid.clone()),
        "name": collection.name.clone(),
        "device_id": collection.device_id.clone(),
    });

    if let Err(err) = client.call_reducer("create_photo_collection", &args).await {
        log::warn!(
            "Failed to create photo collection {}: {}",
            collection.uuid,
            err
        );
    }
}

async fn import_photo_v2(
    client: &SpacetimeClient,
    photo: &PhotoExport,
    archive: &mut ZipArchive<impl Read + std::io::Seek>,
) -> Result<(), AppError> {
    let relative_path = photo.relative_path.trim_start_matches('/');
    let target_relative_path = if relative_path.is_empty() {
        crate::services::photo_paths::original_relative_path(&photo.uuid)
    } else {
        relative_path.to_string()
    };
    let local_path = crate::services::photo_paths::relative_to_absolute(&target_relative_path);
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let candidates = [
        format!("photos/{}", target_relative_path),
        target_relative_path.clone(),
        format!("photos/{}.jpg", photo.uuid),
        format!("photos/{}.jpeg", photo.uuid),
        format!("photos/{}", photo.uuid),
    ];

    let mut copied = false;
    for candidate in candidates {
        if let Ok(mut photo_file) = archive.by_name(&candidate) {
            let mut destination = std::fs::File::create(&local_path)?;
            std::io::copy(&mut photo_file, &mut destination)?;
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

    let _ = photo_gallery::create_thumbnails(
        local_path.to_string_lossy().as_ref(),
        &photo.uuid,
        128,
        512,
    );

    let create_photo_args = serde_json::json!({
        "uuid": photo.uuid.clone(),
        "collection_uuid": photo.collection_uuid.clone(),
        "relative_path": target_relative_path,
        "device_id": photo.device_id.clone(),
    });
    client
        .call_reducer("create_photo", &create_photo_args)
        .await?;

    let sync_args = serde_json::json!({
        "uuid": photo.uuid.clone(),
        "sync_status": photo.sync_status.clone(),
        "sync_error": sats_option(photo.sync_error.clone()),
        "last_sync_attempt": sats_option(photo.last_sync_attempt),
        "retry_count": photo.retry_count,
    });
    if let Err(err) = client
        .call_reducer("update_photo_sync_status", &sync_args)
        .await
    {
        log::warn!(
            "Failed to restore sync metadata for photo {}: {}",
            photo.uuid,
            err
        );
    }

    Ok(())
}
