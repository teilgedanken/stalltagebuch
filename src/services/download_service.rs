use crate::error::AppError;
use crate::services::{crdt_service, sync_paths, sync_service};
use rusqlite::Connection;
use std::collections::HashMap;

/// Downloads and merges operations from sync/ops/ directory
///
/// This is a minimal skeleton for the new multi-master sync downloader.
pub async fn download_and_merge_ops(conn: &Connection) -> Result<usize, AppError> {
    let settings = sync_service::load_sync_settings(conn)?
        .ok_or_else(|| AppError::NotFound("Sync settings not configured".to_string()))?;

    if !settings.enabled {
        return Err(AppError::Validation("Sync disabled".to_string()));
    }

    // Create WebDAV client
    let webdav_url = format!(
        "{}/remote.php/dav/files/{}",
        settings.server_url.trim_end_matches('/'),
        settings.username
    );

    let client = reqwest_dav::ClientBuilder::new()
        .set_host(webdav_url)
        .set_auth(reqwest_dav::Auth::Basic(
            settings.username.clone(),
            settings.app_password.clone(),
        ))
        .build()
        .map_err(|e| AppError::Other(format!("WebDAV client error: {:?}", e)))?;

    // Get local manifest from sync_checkpoint
    let mut manifest = load_manifest(conn)?;

    let ops_base_path = format!(
        "{}/{}",
        settings.remote_path.trim_end_matches('/'),
        sync_paths::OPS_DIR
    );

    // List all devices in ops/
    let device_dirs = list_directory(&client, &ops_base_path).await?;

    let mut all_ops = Vec::new();

    for device_dir in device_dirs {
        let device_path = format!("{}/{}", ops_base_path, device_dir);

        // List all year-month directories for this device
        let month_dirs = list_directory(&client, &device_path).await?;

        for month_dir in month_dirs {
            let month_path = format!("{}/{}", device_path, month_dir);

            // List all NDJSON files in this month
            let files = list_files_with_etags(&client, &month_path).await?;

            for (filename, etag) in files {
                let file_path = format!("{}/{}", month_path, filename);

                // Check if we already have this version
                if manifest.get(&file_path) == Some(&etag) {
                    continue; // Already downloaded
                }

                // Download and parse
                let response = client
                    .get(&file_path)
                    .await
                    .map_err(|e| AppError::Other(format!("Download failed: {:?}", e)))?;

                let content_bytes = response
                    .bytes()
                    .await
                    .map_err(|e| AppError::Other(format!("Read response failed: {:?}", e)))?;

                let content_str = String::from_utf8(content_bytes.to_vec())
                    .map_err(|e| AppError::Other(format!("UTF-8 decode failed: {}", e)))?;

                // Parse NDJSON
                for line in content_str.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }

                    let op: crdt_service::Operation = serde_json::from_str(line)
                        .map_err(|e| AppError::Other(format!("JSON parse failed: {}", e)))?;

                    all_ops.push(op);
                }

                // Update manifest
                manifest.insert(file_path.clone(), etag);
            }
        }
    }

    // Sort operations by clock (deterministic total order)
    all_ops.sort_by(|a, b| a.clock.cmp(&b.clock));

    // Apply operations (multi-master CRDT only)
    let ops_applied = apply_operations(conn, &all_ops)?;

    // Best-effort: Lade alle fehlenden Fotodateien (aus relative_path) herunter
    // Use photo-gallery download service
    let download_config = photo_gallery::PhotoDownloadConfig {
        server_url: settings.server_url.clone(),
        username: settings.username.clone(),
        password: settings.app_password.clone(),
        remote_path: settings.remote_path.clone(),
        storage_path: get_storage_path(),
    };
    let download_service = photo_gallery::PhotoDownloadService::new(download_config);
    let downloaded_files = download_service
        .download_missing_photos(conn)
        .await
        .map_err(|e| AppError::Other(format!("Photo download error: {}", e)))?;

    // Debug: Anzahl Events nach Merge
    if let Ok(count_events) = conn.query_row::<i64, _, _>(
        "SELECT COUNT(*) FROM quail_events WHERE deleted = 0",
        [],
        |row| row.get(0),
    ) {
        log::info!("CRDT: Events im lokalen DB nach Merge: {}", count_events);
    }

    // Save updated manifest
    save_manifest(conn, &manifest)?;

    log::info!(
        "Downloaded and merged {} operations from {} files ({} photos downloaded)",
        ops_applied,
        manifest.len(),
        downloaded_files
    );

    Ok(ops_applied)
}

/// Get the storage path based on platform
fn get_storage_path() -> String {
    #[cfg(target_os = "android")]
    {
        "/storage/emulated/0/Android/data/de.teilgedanken.stalltagebuch/files/photos".to_string()
    }

    #[cfg(not(target_os = "android"))]
    {
        "./photos".to_string()
    }
}

/// Lists directory contents (subdirectories or files)
/// Returns empty Vec if directory doesn't exist (404)
async fn list_directory(client: &reqwest_dav::Client, path: &str) -> Result<Vec<String>, AppError> {
    let list_result = match client.list(path, reqwest_dav::Depth::Number(1)).await {
        Ok(result) => result,
        Err(e) => {
            // Directory doesn't exist yet (404) - return empty list
            log::debug!("Directory {} doesn't exist or is empty: {:?}", path, e);
            return Ok(Vec::new());
        }
    };

    let mut names = Vec::new();

    for item in list_result {
        if let reqwest_dav::list_cmd::ListEntity::File(file) = item {
            let name = file
                .href
                .trim_end_matches('/')
                .split('/')
                .last()
                .unwrap_or("")
                .to_string();
            if !name.is_empty() {
                names.push(name);
            }
        } else if let reqwest_dav::list_cmd::ListEntity::Folder(folder) = item {
            let name = folder
                .href
                .trim_end_matches('/')
                .split('/')
                .last()
                .unwrap_or("")
                .to_string();
            if !name.is_empty() && name != path.split('/').last().unwrap_or("") {
                names.push(name);
            }
        }
    }

    Ok(names)
}

/// Lists files with their ETags
/// Returns empty Vec if directory doesn't exist (404)
async fn list_files_with_etags(
    client: &reqwest_dav::Client,
    path: &str,
) -> Result<Vec<(String, String)>, AppError> {
    let list_result = match client.list(path, reqwest_dav::Depth::Number(1)).await {
        Ok(result) => result,
        Err(e) => {
            // Directory doesn't exist yet (404) - return empty list
            log::debug!("Directory {} doesn't exist or is empty: {:?}", path, e);
            return Ok(Vec::new());
        }
    };

    let mut files = Vec::new();

    for item in list_result {
        if let reqwest_dav::list_cmd::ListEntity::File(file) = item {
            let filename = file
                .href
                .trim_end_matches('/')
                .split('/')
                .last()
                .unwrap_or("")
                .to_string();

            if filename.ends_with(".ndjson") {
                let etag = file.tag.unwrap_or_default();
                files.push((filename, etag));
            }
        }
    }

    Ok(files)
}

/// Loads manifest from sync_checkpoint table
fn load_manifest(conn: &Connection) -> Result<HashMap<String, String>, AppError> {
    let mut manifest = HashMap::new();

    let mut stmt = conn
        .prepare("SELECT path, etag FROM sync_manifest")
        .or_else(|_| {
            // Table doesn't exist yet, create it
            conn.execute(
                "CREATE TABLE IF NOT EXISTS sync_manifest (
                    path TEXT PRIMARY KEY,
                    etag TEXT NOT NULL
                )",
                [],
            )?;
            conn.prepare("SELECT path, etag FROM sync_manifest")
        })?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    for row in rows {
        let (path, etag) = row?;
        manifest.insert(path, etag);
    }

    Ok(manifest)
}

/// Saves manifest to sync_manifest table
fn save_manifest(conn: &Connection, manifest: &HashMap<String, String>) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;

    for (path, etag) in manifest {
        tx.execute(
            "INSERT OR REPLACE INTO sync_manifest (path, etag) VALUES (?1, ?2)",
            rusqlite::params![path, etag],
        )?;
    }

    tx.commit()?;

    Ok(())
}

/// Applies operations to local database
fn apply_operations(conn: &Connection, ops: &[crdt_service::Operation]) -> Result<usize, AppError> {
    let tx = conn.unchecked_transaction()?;
    let mut applied = 0;

    for op in ops {
        // Check if operation already applied (idempotency)
        let already_applied: bool = tx
            .query_row(
                "SELECT 1 FROM op_log WHERE op_id = ?1",
                rusqlite::params![&op.op_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if already_applied {
            continue;
        }

        // Apply based on entity type
        match op.entity_type.as_str() {
            "quail" => apply_quail_op(&tx, op)?,
            "event" => apply_event_op(&tx, op)?,
            "photo" => apply_photo_op(&tx, op)?,
            "egg" => apply_egg_op(&tx, op)?,
            _ => {
                log::warn!("Unknown entity type: {}", op.entity_type);
                continue;
            }
        }

        // Record in op_log
        let op_kind_str = serde_json::to_string(&op.op)
            .map_err(|e| AppError::Other(format!("Serialize op_kind failed: {}", e)))?;
        tx.execute(
            "INSERT INTO op_log (
                op_id, entity_type, entity_id, ts, logical_counter, device_id, op_kind, payload
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &op.op_id,
                &op.entity_type,
                &op.entity_id,
                op.clock.ts,
                op.clock.logical_counter,
                &op.clock.device_id,
                op_kind_str,
                "" // payload unused for now
            ],
        )?;

        applied += 1;
    }

    tx.commit()?;

    Ok(applied)
}

/// Applies a quail operation (LWW-Register merge)
fn apply_quail_op(
    tx: &rusqlite::Transaction,
    op: &crdt_service::Operation,
) -> Result<(), AppError> {
    use crate::services::crdt_service::CrdtOp;

    match &op.op {
        CrdtOp::LwwSet { field, value } => {
            // Check current logical_clock
            let current: Option<(i64, i32)> = tx
                .query_row(
                    "SELECT logical_clock, deleted FROM quails WHERE uuid = ?1",
                    rusqlite::params![&op.entity_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();

            // Only apply if this is newer
            if let Some((current_clock, deleted)) = current {
                if current_clock >= op.clock.ts || deleted == 1 {
                    return Ok(()); // Skip older operation or deleted entity
                }
            }

            // Apply field update
            match field.as_str() {
                "name" => {
                    let name = value
                        .as_str()
                        .ok_or_else(|| AppError::Validation("Invalid name value".to_string()))?;
                    tx.execute(
                        "INSERT OR REPLACE INTO quails (uuid, name, gender, ring_color, profile_photo, rev, logical_clock, deleted)
                         SELECT ?1, ?2, COALESCE(gender, 'unknown'), ring_color, profile_photo, ?3, ?3, 0
                         FROM (SELECT NULL) LEFT JOIN quails ON uuid = ?1",
                        rusqlite::params![&op.entity_id, name, op.clock.ts],
                    )?;
                }
                "gender" => {
                    let gender = value
                        .as_str()
                        .ok_or_else(|| AppError::Validation("Invalid gender value".to_string()))?;
                    tx.execute(
                        "UPDATE quails SET gender = ?1, logical_clock = ?2 WHERE uuid = ?3",
                        rusqlite::params![gender, op.clock.ts, &op.entity_id],
                    )?;
                }
                "ring_color" => {
                    let color = value.as_str();
                    tx.execute(
                        "UPDATE quails SET ring_color = ?1, logical_clock = ?2 WHERE uuid = ?3",
                        rusqlite::params![color, op.clock.ts, &op.entity_id],
                    )?;
                }
                "profile_photo" => {
                    if let Some(photo_uuid) = value.as_str() {
                        let exists: bool = tx
                            .query_row(
                                "SELECT 1 FROM photos WHERE uuid = ?1",
                                rusqlite::params![photo_uuid],
                                |_| Ok(true),
                            )
                            .unwrap_or(false);
                        if !exists {
                            // Lege Platzhalter an – Pfad leer, wird durch späteres Photo-Op aufgefüllt
                            log::info!(
                                "CRDT: Erstelle Platzhalter für fehlendes Profilfoto {} (out-of-order merge)",
                                photo_uuid
                            );
                            tx.execute(
                                "INSERT OR IGNORE INTO photos (uuid, quail_id, event_id, path, relative_path, thumbnail_path, rev, logical_clock, deleted)
                                 VALUES (?1, NULL, NULL, '', NULL, NULL, 0, ?2, 0)",
                                rusqlite::params![photo_uuid, op.clock.ts],
                            )?;
                        }
                        // Versuche jetzt das Profilfoto zu setzen (FK greift nur wenn Foto nicht existiert)
                        if let Err(e) = tx.execute(
                            "UPDATE quails SET profile_photo = ?1, logical_clock = ?2 WHERE uuid = ?3",
                            rusqlite::params![photo_uuid, op.clock.ts, &op.entity_id],
                        ) {
                            log::error!("CRDT: Setzen von profile_photo {} fehlgeschlagen: {:?}", photo_uuid, e);
                        }
                    }
                }
                _ => {
                    log::warn!("Unknown quail field: {}", field);
                }
            }
        }
        CrdtOp::Delete => {
            tx.execute(
                "UPDATE quails SET deleted = 1, logical_clock = ?1 WHERE uuid = ?2",
                rusqlite::params![op.clock.ts, &op.entity_id],
            )?;
        }
        _ => {} // Other ops not applicable to quails
    }

    Ok(())
}

/// Applies an event operation
fn apply_event_op(
    tx: &rusqlite::Transaction,
    op: &crdt_service::Operation,
) -> Result<(), AppError> {
    use crate::services::crdt_service::CrdtOp;

    match &op.op {
        CrdtOp::LwwSet { field, value } => {
            let current: Option<(i64, i32)> = tx
                .query_row(
                    "SELECT logical_clock, deleted FROM quail_events WHERE uuid = ?1",
                    rusqlite::params![&op.entity_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();

            if let Some((current_clock, deleted)) = current {
                if current_clock >= op.clock.ts || deleted == 1 {
                    return Ok(());
                }
            }

            match field.as_str() {
                "quail_id" => {
                    let quail_id = value
                        .as_str()
                        .ok_or_else(|| AppError::Validation("Invalid quail_id".to_string()))?;

                    // Sicherstellen, dass die referenzierte Wachtel existiert (Platzhalter bei Out-of-Order Merge)
                    let quail_exists: bool = tx
                        .query_row(
                            "SELECT 1 FROM quails WHERE uuid = ?1",
                            rusqlite::params![quail_id],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if !quail_exists {
                        log::info!(
                            "CRDT: Erstelle Platzhalter-Wachtel {} für Event {} (out-of-order merge)",
                            quail_id,
                            &op.entity_id
                        );
                        // Minimal gültiger Platzhalter: name darf nicht NULL sein
                        if let Err(e) = tx.execute(
                            "INSERT OR IGNORE INTO quails (uuid, name, rev, logical_clock, deleted)
                             VALUES (?1, '', 0, ?2, 0)",
                            rusqlite::params![quail_id, op.clock.ts],
                        ) {
                            log::error!(
                                "CRDT: Anlage Platzhalter-Wachtel {} fehlgeschlagen: {:?}",
                                quail_id,
                                e
                            );
                        }
                    }

                    tx.execute(
                        "INSERT OR REPLACE INTO quail_events (uuid, quail_id, event_type, event_date, notes, rev, logical_clock, deleted)
                         SELECT ?1, ?2, COALESCE(event_type, 'alive'), COALESCE(event_date, date('now')), notes, ?3, ?3, 0
                         FROM (SELECT NULL) LEFT JOIN quail_events ON uuid = ?1",
                        rusqlite::params![&op.entity_id, quail_id, op.clock.ts],
                    )?;
                }
                "event_type" | "type" => {
                    let event_type = value
                        .as_str()
                        .ok_or_else(|| AppError::Validation("Invalid event_type".to_string()))?;
                    tx.execute(
                        "UPDATE quail_events SET event_type = ?1, logical_clock = ?2 WHERE uuid = ?3",
                        rusqlite::params![event_type, op.clock.ts, &op.entity_id],
                    )?;
                }
                "event_date" | "date" => {
                    let event_date = value
                        .as_str()
                        .ok_or_else(|| AppError::Validation("Invalid event_date".to_string()))?;
                    tx.execute(
                        "UPDATE quail_events SET event_date = ?1, logical_clock = ?2 WHERE uuid = ?3",
                        rusqlite::params![event_date, op.clock.ts, &op.entity_id],
                    )?;
                }
                "notes" => {
                    let notes = value.as_str();
                    tx.execute(
                        "UPDATE quail_events SET notes = ?1, logical_clock = ?2 WHERE uuid = ?3",
                        rusqlite::params![notes, op.clock.ts, &op.entity_id],
                    )?;
                }
                _ => {
                    log::warn!("Unknown event field: {}", field);
                }
            }
        }
        CrdtOp::Delete => {
            tx.execute(
                "UPDATE quail_events SET deleted = 1, logical_clock = ?1 WHERE uuid = ?2",
                rusqlite::params![op.clock.ts, &op.entity_id],
            )?;
        }
        _ => {}
    }

    Ok(())
}

/// Applies a photo operation
fn apply_photo_op(
    tx: &rusqlite::Transaction,
    op: &crdt_service::Operation,
) -> Result<(), AppError> {
    use crate::services::crdt_service::CrdtOp;

    match &op.op {
        CrdtOp::LwwSet { field, value } => {
            let current: Option<(i64, i32)> = tx
                .query_row(
                    "SELECT logical_clock, deleted FROM photos WHERE uuid = ?1",
                    rusqlite::params![&op.entity_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();

            if let Some((current_clock, deleted)) = current {
                if current_clock >= op.clock.ts || deleted == 1 {
                    return Ok(());
                }
            }

            match field.as_str() {
                "quail_id" => {
                    let quail_id = value.as_str();
                    // Stelle sicher dass Foto-Eintrag existiert
                    let exists: bool = tx
                        .query_row(
                            "SELECT 1 FROM photos WHERE uuid = ?1",
                            rusqlite::params![&op.entity_id],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if !exists {
                        // Lege Platzhalter an
                        tx.execute(
                            "INSERT INTO photos (uuid, quail_id, event_id, path, relative_path, thumbnail_path, rev, logical_clock, deleted)
                             VALUES (?1, ?2, NULL, '', NULL, NULL, 0, ?3, 0)",
                            rusqlite::params![&op.entity_id, quail_id, op.clock.ts],
                        )?;
                    } else {
                        tx.execute(
                            "UPDATE photos SET quail_id = ?1, logical_clock = ?2 WHERE uuid = ?3",
                            rusqlite::params![quail_id, op.clock.ts, &op.entity_id],
                        )?;
                    }
                }
                "event_id" => {
                    let event_id = value.as_str();
                    // Stelle sicher dass Foto-Eintrag existiert
                    let exists: bool = tx
                        .query_row(
                            "SELECT 1 FROM photos WHERE uuid = ?1",
                            rusqlite::params![&op.entity_id],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if !exists {
                        // Lege Platzhalter an
                        tx.execute(
                            "INSERT INTO photos (uuid, quail_id, event_id, path, relative_path, thumbnail_path, rev, logical_clock, deleted)
                             VALUES (?1, NULL, ?2, '', NULL, NULL, 0, ?3, 0)",
                            rusqlite::params![&op.entity_id, event_id, op.clock.ts],
                        )?;
                    } else {
                        tx.execute(
                            "UPDATE photos SET event_id = ?1, logical_clock = ?2 WHERE uuid = ?3",
                            rusqlite::params![event_id, op.clock.ts, &op.entity_id],
                        )?;
                    }
                }
                "relative_path" | "relative" => {
                    let path = value
                        .as_str()
                        .ok_or_else(|| AppError::Validation("Invalid path".to_string()))?;
                    // Lege Eintrag an falls nicht vorhanden (ohne quail_id/event_id zu überschreiben)
                    tx.execute(
                        "INSERT OR IGNORE INTO photos (uuid, path, relative_path, quail_id, event_id, thumbnail_path, rev, logical_clock, deleted)
                         VALUES (?1, '', ?2, NULL, NULL, NULL, 0, ?3, 0)",
                        rusqlite::params![&op.entity_id, path, op.clock.ts],
                    )?;
                    // Update nur relative_path und logical_clock, behalte quail_id/event_id
                    tx.execute(
                        "UPDATE photos SET relative_path = ?1, logical_clock = ?2 WHERE uuid = ?3",
                        rusqlite::params![path, op.clock.ts, &op.entity_id],
                    )?;
                }
                "relative_thumb" | "thumb" => {
                    let thumb = value.as_str();
                    // Lege Eintrag an falls nicht vorhanden
                    tx.execute(
                        "INSERT OR IGNORE INTO photos (uuid, path, relative_path, quail_id, event_id, thumbnail_path, rev, logical_clock, deleted)
                         VALUES (?1, '', NULL, NULL, NULL, ?2, 0, ?3, 0)",
                        rusqlite::params![&op.entity_id, thumb, op.clock.ts],
                    )?;
                    // Update thumbnail_path
                    tx.execute(
                        "UPDATE photos SET thumbnail_path = ?1, logical_clock = ?2 WHERE uuid = ?3",
                        rusqlite::params![thumb, op.clock.ts, &op.entity_id],
                    )?;
                }
                _ => {
                    log::warn!("Unknown photo field: {}", field);
                }
            }
        }
        CrdtOp::Delete => {
            tx.execute(
                "UPDATE photos SET deleted = 1, logical_clock = ?1 WHERE uuid = ?2",
                rusqlite::params![op.clock.ts, &op.entity_id],
            )?;
        }
        _ => {}
    }

    Ok(())
}

/// Applies an egg operation
fn apply_egg_op(tx: &rusqlite::Transaction, op: &crdt_service::Operation) -> Result<(), AppError> {
    use crate::services::crdt_service::CrdtOp;

    match &op.op {
        CrdtOp::LwwSet { field, value } => {
            // Prüfe ob Eintrag gelöscht ist
            let deleted: Option<i32> = tx
                .query_row(
                    "SELECT deleted FROM egg_records WHERE uuid = ?1",
                    rusqlite::params![&op.entity_id],
                    |row| row.get(0),
                )
                .ok();

            if deleted == Some(1) {
                return Ok(()); // Ignoriere Updates zu gelöschten Einträgen
            }

            match field.as_str() {
                "date" | "record_date" => {
                    let date = value
                        .as_str()
                        .ok_or_else(|| AppError::Validation("Invalid date".to_string()))?;
                    // Erst sicherstellen dass Eintrag existiert
                    tx.execute(
                        "INSERT OR IGNORE INTO egg_records (uuid, record_date, total_eggs, notes, rev, logical_clock, deleted)
                         VALUES (?1, ?2, 0, NULL, ?3, ?3, 0)",
                        rusqlite::params![&op.entity_id, date, op.clock.ts],
                    )?;
                    // Dann updaten (jede Operation hat jetzt unterschiedlichen ts wegen tick)
                    tx.execute(
                        "UPDATE egg_records SET record_date = ?1, logical_clock = ?2 
                         WHERE uuid = ?3",
                        rusqlite::params![date, op.clock.ts, &op.entity_id],
                    )?;
                }
                "count" | "total_eggs" => {
                    let count = value
                        .as_i64()
                        .ok_or_else(|| AppError::Validation("Invalid count".to_string()))?
                        as i32;
                    // Erst sicherstellen dass Eintrag existiert (mit Dummy-Datum wenn nötig)
                    tx.execute(
                        "INSERT OR IGNORE INTO egg_records (uuid, record_date, total_eggs, notes, rev, logical_clock, deleted)
                         VALUES (?1, date('now'), ?2, NULL, ?3, ?3, 0)",
                        rusqlite::params![&op.entity_id, count, op.clock.ts],
                    )?;
                    // Dann updaten (jede Operation hat jetzt unterschiedlichen ts wegen tick)
                    tx.execute(
                        "UPDATE egg_records SET total_eggs = ?1, logical_clock = ?2 
                         WHERE uuid = ?3",
                        rusqlite::params![count, op.clock.ts, &op.entity_id],
                    )?;
                }
                _ => {
                    log::warn!("Unknown egg field: {}", field);
                }
            }
        }
        CrdtOp::PnIncrement { field, delta } => {
            if field == "count" || field == "total_eggs" {
                // Stelle sicher dass Eintrag existiert
                tx.execute(
                    "INSERT OR IGNORE INTO egg_records (uuid, record_date, total_eggs, notes, rev, logical_clock, deleted)
                     VALUES (?1, date('now'), 0, NULL, ?2, ?2, 0)",
                    rusqlite::params![&op.entity_id, op.clock.ts],
                )?;
                // Increment
                tx.execute(
                    "UPDATE egg_records SET total_eggs = total_eggs + ?1, logical_clock = ?2 WHERE uuid = ?3",
                    rusqlite::params![delta, op.clock.ts, &op.entity_id],
                )?;
            }
        }
        CrdtOp::Delete => {
            tx.execute(
                "UPDATE egg_records SET deleted = 1, logical_clock = ?1 WHERE uuid = ?2",
                rusqlite::params![op.clock.ts, &op.entity_id],
            )?;
        }
        _ => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_roundtrip() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        let mut manifest = HashMap::new();
        manifest.insert(
            "sync/ops/device1/202412/01JGTEST.ndjson".to_string(),
            "\"abc123\"".to_string(),
        );
        manifest.insert(
            "sync/ops/device2/202412/01JGTEST2.ndjson".to_string(),
            "\"def456\"".to_string(),
        );

        save_manifest(&conn, &manifest).unwrap();
        let loaded = load_manifest(&conn).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(
            loaded.get("sync/ops/device1/202412/01JGTEST.ndjson"),
            Some(&"\"abc123\"".to_string())
        );
    }
}
