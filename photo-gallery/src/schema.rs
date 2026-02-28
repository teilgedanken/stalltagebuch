use rusqlite::{Connection, Result};

/// Initialize photo gallery database schema
pub fn init_photo_schema(conn: &Connection) -> Result<()> {
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Schema version table for photo gallery
    conn.execute(
        "CREATE TABLE IF NOT EXISTS photo_schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Check current photo schema version
    let current_version: i32 = conn
        .query_row(
            "SELECT version FROM photo_schema_version ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current_version < 1 {
        create_photo_schema_v1(conn)?;
        conn.execute("INSERT INTO photo_schema_version (version) VALUES (1)", [])?;
    }

    Ok(())
}

/// Create photo gallery schema version 1
fn create_photo_schema_v1(conn: &Connection) -> Result<()> {
    // Table: photo_collections - groups of photos
    conn.execute(
        "CREATE TABLE IF NOT EXISTS photo_collections (
            uuid TEXT PRIMARY KEY,
            preview_photo_uuid TEXT,
            name TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rev INTEGER NOT NULL DEFAULT 0,
            logical_clock INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1))
        )",
        [],
    )?;

    // Trigger for updated_at in photo_collections
    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS update_photo_collections_timestamp 
         AFTER UPDATE ON photo_collections
         BEGIN
            UPDATE photo_collections SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END",
        [],
    )?;

    // Check if photos table already exists (from main crate schema)
    let photos_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='photos'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if photos_exists {
        // Photos table already exists from main crate - just add collection_id column if needed
        let has_collection_id: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('photos') WHERE name='collection_id'",
                [],
                |row| row.get::<_, i32>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        if !has_collection_id {
            conn.execute("ALTER TABLE photos ADD COLUMN collection_id TEXT", [])?;
        }

        // Create index for collection_id if it doesn't exist
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photos_collection ON photos(collection_id)",
            [],
        )?;
    } else {
        // Photos table doesn't exist yet - create it (standalone photo-gallery usage)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS photos (
                uuid TEXT PRIMARY KEY,
                collection_id TEXT,
                path TEXT NOT NULL,
                relative_path TEXT,
                thumbnail_path TEXT,
                thumbnail_small_path TEXT,
                thumbnail_medium_path TEXT,
                sync_status TEXT DEFAULT 'local_only' CHECK(sync_status IN ('local_only', 'uploading', 'synced', 'download_pending', 'downloading', 'download_failed')),
                sync_error TEXT,
                last_sync_attempt INTEGER,
                retry_count INTEGER DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                rev INTEGER NOT NULL DEFAULT 0,
                logical_clock INTEGER NOT NULL DEFAULT 0,
                deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1)),
                FOREIGN KEY (collection_id) REFERENCES photo_collections(uuid) ON DELETE CASCADE
            )",
            [],
        )?;

        // Index for photos by collection
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photos_collection ON photos(collection_id)",
            [],
        )?;

        // Index for photos sync status
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_photos_sync_status ON photos(sync_status)",
            [],
        )?;

        // Trigger for updated_at in photos
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS update_photos_timestamp 
             AFTER UPDATE ON photos
             BEGIN
                UPDATE photos SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
             END",
            [],
        )?;
    }

    Ok(())
}

/// Migrate existing photos from quail/event schema to collections
pub fn migrate_existing_photos_to_collections(conn: &Connection) -> Result<usize> {
    // Check if old schema exists (quail_id/event_id columns in photos)
    let has_old_schema: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('photos') WHERE name IN ('quail_id', 'event_id')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !has_old_schema {
        return Ok(0);
    }

    let mut migrated = 0;

    // Get all photos with quail_id
    let mut stmt = conn.prepare(
        "SELECT uuid, quail_id, path, relative_path, thumbnail_path, thumbnail_small_path, thumbnail_medium_path, 
                sync_status, sync_error, last_sync_attempt, retry_count, created_at, updated_at, rev, logical_clock, deleted
         FROM photos WHERE quail_id IS NOT NULL AND deleted = 0"
    )?;

    let quail_photos: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i32>,
        String,
        String,
        i32,
        i64,
        i32,
    )> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,  // uuid
                row.get(1)?,  // quail_id
                row.get(2)?,  // path
                row.get(3)?,  // relative_path
                row.get(4)?,  // thumbnail_path
                row.get(5)?,  // thumbnail_small_path
                row.get(6)?,  // thumbnail_medium_path
                row.get(7)?,  // sync_status
                row.get(8)?,  // sync_error
                row.get(9)?,  // last_sync_attempt
                row.get(10)?, // retry_count
                row.get(11)?, // created_at
                row.get(12)?, // updated_at
                row.get(13)?, // rev
                row.get(14)?, // logical_clock
                row.get(15)?, // deleted
            ))
        })?
        .collect::<Result<Vec<_>>>()?;

    // Group photos by quail_id and create collections
    let mut collections: std::collections::HashMap<String, Vec<_>> =
        std::collections::HashMap::new();
    for photo in quail_photos {
        collections
            .entry(photo.1.clone())
            .or_insert_with(Vec::new)
            .push(photo);
    }

    for (quail_id, photos) in collections {
        // Create a collection for this quail
        let collection_uuid = uuid::Uuid::new_v4().to_string();
        let preview_photo = photos.first().map(|p| p.0.clone());

        conn.execute(
            "INSERT INTO photo_collections (uuid, preview_photo_uuid, name) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                collection_uuid,
                preview_photo,
                format!("Quail {}", quail_id)
            ],
        )?;

        // Update photos to reference this collection
        for photo in photos {
            conn.execute(
                "UPDATE photos SET collection_id = ?1 WHERE uuid = ?2",
                rusqlite::params![collection_uuid, photo.0],
            )?;
            migrated += 1;
        }
    }

    // Similar for event photos
    let mut stmt = conn.prepare(
        "SELECT uuid, event_id, path, relative_path, thumbnail_path, thumbnail_small_path, thumbnail_medium_path, 
                sync_status, sync_error, last_sync_attempt, retry_count, created_at, updated_at, rev, logical_clock, deleted
         FROM photos WHERE event_id IS NOT NULL AND deleted = 0"
    )?;

    let event_photos: Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i32>,
        String,
        String,
        i32,
        i64,
        i32,
    )> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,  // uuid
                row.get(1)?,  // event_id
                row.get(2)?,  // path
                row.get(3)?,  // relative_path
                row.get(4)?,  // thumbnail_path
                row.get(5)?,  // thumbnail_small_path
                row.get(6)?,  // thumbnail_medium_path
                row.get(7)?,  // sync_status
                row.get(8)?,  // sync_error
                row.get(9)?,  // last_sync_attempt
                row.get(10)?, // retry_count
                row.get(11)?, // created_at
                row.get(12)?, // updated_at
                row.get(13)?, // rev
                row.get(14)?, // logical_clock
                row.get(15)?, // deleted
            ))
        })?
        .collect::<Result<Vec<_>>>()?;

    let mut collections: std::collections::HashMap<String, Vec<_>> =
        std::collections::HashMap::new();
    for photo in event_photos {
        collections
            .entry(photo.1.clone())
            .or_insert_with(Vec::new)
            .push(photo);
    }

    for (event_id, photos) in collections {
        let collection_uuid = uuid::Uuid::new_v4().to_string();
        let preview_photo = photos.first().map(|p| p.0.clone());

        conn.execute(
            "INSERT INTO photo_collections (uuid, preview_photo_uuid, name) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                collection_uuid,
                preview_photo,
                format!("Event {}", event_id)
            ],
        )?;

        for photo in photos {
            conn.execute(
                "UPDATE photos SET collection_id = ?1 WHERE uuid = ?2",
                rusqlite::params![collection_uuid, photo.0],
            )?;
            migrated += 1;
        }
    }

    Ok(migrated)
}
