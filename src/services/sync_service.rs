use crate::error::AppError;
use crate::models::SyncSettings;
use rusqlite::{Connection, Result};

/// Loads the synchronization settings from the database
pub fn load_sync_settings(conn: &Connection) -> Result<Option<SyncSettings>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, server_url, username, app_password, remote_path, enabled, last_sync, device_id, format_version, initial_upload_done, created_at, updated_at 
         FROM sync_settings 
         ORDER BY id DESC 
         LIMIT 1"
    )?;

    let result = stmt.query_row([], |row| {
        Ok(SyncSettings {
            id: row.get(0)?,
            server_url: row.get(1)?,
            username: row.get(2)?,
            app_password: row.get(3)?,
            remote_path: row.get(4)?,
            enabled: row.get(5)?,
            last_sync: row.get(6)?,
            device_id: row.get(7)?,
            format_version: row.get(8)?,
            initial_upload_done: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    });

    match result {
        Ok(settings) => Ok(Some(settings)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e)),
    }
}

/// Saves or updates the synchronization settings
pub fn save_sync_settings(conn: &Connection, settings: &SyncSettings) -> Result<i64, AppError> {
    // Check if settings already exist
    let existing = load_sync_settings(conn)?;

    if let Some(existing) = existing {
        // Update
        conn.execute(
            "UPDATE sync_settings 
             SET server_url = ?1, username = ?2, app_password = ?3, remote_path = ?4, enabled = ?5, device_id = ?6, format_version = ?7, initial_upload_done = ?8
             WHERE id = ?9",
            (
                &settings.server_url,
                &settings.username,
                &settings.app_password,
                &settings.remote_path,
                settings.enabled,
                &settings.device_id,
                settings.format_version,
                settings.initial_upload_done,
                existing.id,
            ),
        )?;
        Ok(existing.id)
    } else {
        // Insert
        conn.execute(
            "INSERT INTO sync_settings (server_url, username, app_password, remote_path, enabled, device_id, format_version, initial_upload_done)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            (
                &settings.server_url,
                &settings.username,
                &settings.app_password,
                &settings.remote_path,
                settings.enabled,
                &settings.device_id,
                settings.format_version,
                settings.initial_upload_done,
            ),
        )?;
        Ok(conn.last_insert_rowid())
    }
}

/// Deletes all synchronization settings
pub fn delete_sync_settings(conn: &Connection) -> Result<(), AppError> {
    conn.execute("DELETE FROM sync_settings", [])?;
    Ok(())
}

/// Marks initial upload as completed
pub fn set_initial_upload_done(conn: &Connection) -> Result<(), AppError> {
    conn.execute(
        "UPDATE sync_settings SET initial_upload_done = 1 WHERE id = (SELECT MAX(id) FROM sync_settings)",
        [],
    )?;
    Ok(())
}
