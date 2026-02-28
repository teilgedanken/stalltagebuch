//! Application-specific schema extensions
//!
//! This module handles migrations that depend on application crates (like photo-gallery).
//! Core schema is in stalltagebuch-database crate.

use rusqlite::{Connection, Result};

/// Initialize the complete schema including app-specific migrations
pub fn init_schema(conn: &Connection) -> Result<()> {
    // First, run core schema from database crate
    stalltagebuch_database::schema::init_schema(conn)?;

    // Then run app-specific migrations that depend on photo-gallery
    let current_version = stalltagebuch_database::schema::get_schema_version(conn);
    log::info!("Current schema version: {}", current_version);
    // Migration to version 6: Migrate photos to collections and add collection FK to quails/events
    if current_version < 6 {
        log::info!("Applying migration to schema version 6");
        migrate_to_v6(conn)?;
        stalltagebuch_database::schema::set_schema_version(conn, 6)?;
    }

    Ok(())
}

/// Migration to version 6: Migrate photos to collections architecture
fn migrate_to_v6(conn: &Connection) -> Result<()> {
    log::info!("Migrating to schema version 6: photo collections");

    // Initialize photo gallery schema (creates photo_collections table)
    photo_gallery::init_photo_schema(conn)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // Migrate existing photos to collections
    let migrated = photo_gallery::migrate_existing_photos_to_collections(conn)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    log::info!("Migrated {} photos to collections", migrated);

    // Add collection_id column to quails table if it doesn't exist
    let has_quail_collection_id: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('quails') WHERE name='collection_id'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        )
        .unwrap_or(false);

    if !has_quail_collection_id {
        conn.execute("ALTER TABLE quails ADD COLUMN collection_id TEXT", [])?;
    }

    // Add collection_id column to quail_events table if it doesn't exist
    let has_event_collection_id: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('quail_events') WHERE name='collection_id'",
            [],
            |row| row.get::<_, i32>(0).map(|c| c > 0),
        )
        .unwrap_or(false);

    if !has_event_collection_id {
        conn.execute("ALTER TABLE quail_events ADD COLUMN collection_id TEXT", [])?;
    }

    // Create mapping: for each quail, find its collection and set FK
    conn.execute(
        "UPDATE quails SET collection_id = (
            SELECT pc.uuid 
            FROM photo_collections pc 
            WHERE pc.name = 'Quail ' || quails.uuid
            LIMIT 1
        ) WHERE collection_id IS NULL",
        [],
    )?;

    // Create mapping: for each event, find its collection and set FK
    conn.execute(
        "UPDATE quail_events SET collection_id = (
            SELECT pc.uuid 
            FROM photo_collections pc 
            WHERE pc.name = 'Event ' || quail_events.uuid
            LIMIT 1
        ) WHERE collection_id IS NULL",
        [],
    )?;

    log::info!("Migration to v6 complete");
    Ok(())
}
