//! Database schema and migrations
//!
//! This module contains the core database schema for Stalltagebuch.
//! Photo-gallery specific schema is initialized separately via photo_gallery::init_photo_schema.

use rusqlite::{Connection, Result};

/// Initialize complete database schema for the Quail Diary app
pub fn init_schema(conn: &Connection) -> Result<()> {
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;

    // Schema version table for future migrations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Check if schema already exists
    let current_version: i32 = conn
        .query_row(
            "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current_version < 2 {
        create_schema(conn)?;
        conn.execute("INSERT INTO schema_version (version) VALUES (2)", [])?;
    }

    // Migration to version 3: Add relative_path column
    if current_version < 3 {
        migrate_to_v3(conn)?;
        conn.execute("INSERT INTO schema_version (version) VALUES (3)", [])?;
    }

    // Migration to version 4: Remove strict photo FK triggers to allow CRDT out-of-order merges
    if current_version < 4 {
        migrate_to_v4(conn)?;
        conn.execute("INSERT INTO schema_version (version) VALUES (4)", [])?;
    }

    // Migration to version 5: Add initial_upload_done flag to sync_settings
    if current_version < 5 {
        migrate_to_v5(conn)?;
        conn.execute("INSERT INTO schema_version (version) VALUES (5)", [])?;
    }

    // Note: Migration v6 (photo collections) is handled by photo_gallery crate
    // and called from the main app after init_schema

    Ok(())
}

/// Get current schema version
pub fn get_schema_version(conn: &Connection) -> i32 {
    conn.query_row(
        "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// Record a schema version as applied
pub fn set_schema_version(conn: &Connection, version: i32) -> Result<()> {
    conn.execute(
        "INSERT INTO schema_version (version) VALUES (?1)",
        [version],
    )?;
    Ok(())
}

/// Create the complete schema (version 2) - multi-master sync ready (CRDT fields)
fn create_schema(conn: &Connection) -> Result<()> {
    // Table: photos (now with rev/logical_clock/deleted fields for CRDT sync + multi-size thumbnails + sync status)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS photos (
            uuid TEXT PRIMARY KEY,
            quail_id TEXT,
            event_id TEXT,
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
            CHECK( (quail_id IS NOT NULL AND event_id IS NOT NULL) OR (quail_id IS NULL OR event_id IS NULL) )
        )",
        [],
    )?;

    // Indexes for photos
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_photos_quail ON photos(quail_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_photos_event ON photos(event_id)",
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

    // Table: quails (Quail profiles)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS quails (
            uuid TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            gender TEXT CHECK(gender IN ('male', 'female', 'unknown')) NOT NULL DEFAULT 'unknown',
            ring_color TEXT,
            profile_photo TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rev INTEGER NOT NULL DEFAULT 0,
            logical_clock INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1)),
            FOREIGN KEY (profile_photo) REFERENCES photos(uuid) ON DELETE SET NULL
        )",
        [],
    )?;

    // Indexes for quails
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_quails_name ON quails(name)",
        [],
    )?;

    // Trigger for updated_at in quails
    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS update_quails_timestamp 
         AFTER UPDATE ON quails
         BEGIN
            UPDATE quails SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END",
        [],
    )?;

    // Table: quail_events (Life events for quails)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS quail_events (
            uuid TEXT PRIMARY KEY,
            quail_id TEXT NOT NULL,
            event_type TEXT CHECK(event_type IN ('born', 'alive', 'sick', 'healthy', 'marked_for_slaughter', 'slaughtered', 'died')) NOT NULL,
            event_date TEXT NOT NULL,
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rev INTEGER NOT NULL DEFAULT 0,
            logical_clock INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1)),
            FOREIGN KEY (quail_id) REFERENCES quails(uuid) ON DELETE CASCADE
        )",
        [],
    )?;

    // Indexes for quail_events
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_quail_events_quail_id ON quail_events(quail_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_quail_events_date ON quail_events(event_date DESC)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_quail_events_type ON quail_events(event_type)",
        [],
    )?;

    // Trigger for updated_at in quail_events
    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS update_quail_events_timestamp 
         AFTER UPDATE ON quail_events
         BEGIN
            UPDATE quail_events SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END",
        [],
    )?;

    // Table: egg_records (Daily egg production tracking)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS egg_records (
            uuid TEXT PRIMARY KEY,
            record_date TEXT NOT NULL UNIQUE,
            total_eggs INTEGER NOT NULL DEFAULT 0 CHECK(total_eggs >= 0),
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rev INTEGER NOT NULL DEFAULT 0,
            logical_clock INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1))
        )",
        [],
    )?;

    // Indexes for egg_records
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_egg_records_date ON egg_records(record_date DESC)",
        [],
    )?;

    // Trigger for updated_at in egg_records
    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS update_egg_records_timestamp 
         AFTER UPDATE ON egg_records
         BEGIN
            UPDATE egg_records SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END",
        [],
    )?;

    // Table: sync_settings (Nextcloud WebDAV synchronization settings)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_url TEXT NOT NULL,
            username TEXT NOT NULL,
            app_password TEXT NOT NULL,
            remote_path TEXT NOT NULL DEFAULT '/Stalltagebuch',
            enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
            last_sync TEXT,
            device_id TEXT,
            format_version INTEGER NOT NULL DEFAULT 2,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Trigger for updated_at in sync_settings
    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS update_sync_settings_timestamp 
         AFTER UPDATE ON sync_settings
         BEGIN
            UPDATE sync_settings SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
         END",
        [],
    )?;

    // Table: sync_queue (Queue for pending photo uploads)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            photo_id TEXT NOT NULL,
            status TEXT CHECK(status IN ('pending', 'uploading', 'completed', 'failed')) NOT NULL DEFAULT 'pending',
            retry_count INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (photo_id) REFERENCES photos(uuid) ON DELETE CASCADE
        )",
        [],
    )?;

    // Operation Log for CRDT operations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS op_log (
            op_id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            ts INTEGER NOT NULL,
            logical_counter INTEGER NOT NULL,
            device_id TEXT NOT NULL,
            op_kind TEXT NOT NULL, -- upsert | delete | inc
            payload TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_op_log_entity ON op_log(entity_id, ts)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_op_log_device ON op_log(device_id, ts)",
        [],
    )?;

    // Sync checkpoint (tracks last applied clock/snapshot)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sync_checkpoint (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            last_ts INTEGER NOT NULL DEFAULT 0,
            last_logical_counter INTEGER NOT NULL DEFAULT 0,
            last_device_id TEXT,
            snapshot_rev INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Device state (stable device identifier)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS device_state (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            device_id TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Indexes for sync_queue
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sync_queue_status ON sync_queue(status)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sync_queue_photo ON sync_queue(photo_id)",
        [],
    )?;

    // Trigger for updated_at in sync_queue
    conn.execute(
        "CREATE TRIGGER IF NOT EXISTS update_sync_queue_timestamp 
         AFTER UPDATE ON sync_queue
         BEGIN
            UPDATE sync_queue SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
         END",
        [],
    )?;

    Ok(())
}

/// Migration to version 3: Add relative_path column to photos
fn migrate_to_v3(conn: &Connection) -> Result<()> {
    // Check if column already exists
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('photos') WHERE name='relative_path'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        )
        .unwrap_or(false);

    if !has_column {
        log::info!("Migrating to schema version 3: adding relative_path column");
        conn.execute("ALTER TABLE photos ADD COLUMN relative_path TEXT", [])?;

        // Migrate existing data: copy path to relative_path for existing photos
        conn.execute(
            "UPDATE photos SET relative_path = path WHERE relative_path IS NULL",
            [],
        )?;
        log::info!("Migration to v3 complete");
    }

    Ok(())
}

/// Migration to version 4: Drop strict photo FK triggers to avoid aborts during CRDT merges
fn migrate_to_v4(conn: &Connection) -> Result<()> {
    // Best-effort drop if they exist in older databases
    let _ = conn.execute("DROP TRIGGER IF EXISTS photos_quail_fk_check", []);
    let _ = conn.execute("DROP TRIGGER IF EXISTS photos_event_fk_check", []);
    Ok(())
}

/// Migration to version 5: Add initial_upload_done flag to sync_settings
fn migrate_to_v5(conn: &Connection) -> Result<()> {
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('sync_settings') WHERE name='initial_upload_done'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        )
        .unwrap_or(false);

    if !has_column {
        log::info!("Migrating to schema version 5: adding initial_upload_done column");
        conn.execute(
            "ALTER TABLE sync_settings ADD COLUMN initial_upload_done INTEGER NOT NULL DEFAULT 0 CHECK(initial_upload_done IN (0,1))",
            [],
        )?;
        log::info!("Migration to v5 complete");
    }

    Ok(())
}
