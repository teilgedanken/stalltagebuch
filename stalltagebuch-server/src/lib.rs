//! SpacetimeDB server module for Stalltagebuch.
//!
//! This module defines the database tables and reducers that run on SpacetimeDB.
//! Deploy with: `spacetime publish --project-path stalltagebuch-server stalltagebuch-server`
//! Generate client bindings: `spacetime generate --lang rust --out-dir ../src/spacetime/module_bindings`
//! (or `--lang dioxus` when the Dioxus binding generator is available)

use spacetimedb::{reducer, ReducerContext, SpacetimeType, Table};

// ─── Tables ────────────────────────────────────────────────────────────────────

/// A quail in the flock.
#[spacetimedb::table(accessor = quails, public)]
pub struct Quail {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    pub id: u64,
    pub name: String,
    /// "male" | "female" | "unknown"
    pub gender: String,
    /// Optional leg ring colour (e.g. "lila", "rot", …)
    pub ring_color: Option<String>,
    /// UUID of the profile photo stored in the photo-gallery / Nextcloud.
    pub profile_photo: Option<String>,
    /// The SpacetimeDB identity of the user who owns this quail.
    pub owner: String,
}

/// A lifecycle event for a quail.
#[spacetimedb::table(accessor = quail_events, public)]
pub struct QuailEvent {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    pub id: u64,
    pub quail_uuid: String,
    /// "born" | "alive" | "sick" | "healthy" | "marked_for_slaughter" | "slaughtered" | "died"
    pub event_type: String,
    /// ISO 8601 date string YYYY-MM-DD.
    pub event_date: String,
    pub notes: Option<String>,
    /// UUID of an attached photo stored on Nextcloud.
    pub photos: Option<String>,
    pub owner: String,
}

/// A daily egg-production record.
#[spacetimedb::table(accessor = egg_records, public)]
pub struct EggRecord {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    pub id: u64,
    /// ISO 8601 date string YYYY-MM-DD.
    pub record_date: String,
    pub total_eggs: i32,
    pub notes: Option<String>,
    pub owner: String,
}

/// A collection of photos (for a quail or event).
#[spacetimedb::table(accessor = photo_collections, public)]
pub struct PhotoCollection {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    pub id: u64,
    /// UUID of the quail this collection belongs to (if applicable).
    pub quail_uuid: Option<String>,
    /// UUID of the event this collection belongs to (if applicable).
    pub event_uuid: Option<String>,
    /// UUID of the photo used as preview for this collection.
    pub preview_photo_uuid: Option<String>,
    /// Name/description of the collection.
    pub name: String,
    pub owner: String,
}

/// A photo in a collection.
#[spacetimedb::table(accessor = photos, public)]
pub struct Photo {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    pub id: u64,
    /// UUID of the collection this photo belongs to.
    pub collection_uuid: String,
    /// Relative path from the app's photos directory (e.g. "quail-123/photo.jpg").
    pub relative_path: String,
    /// Sync status regarding Nextcloud upload: "local_only" | "uploading" | "synced" | "download_pending" | "downloading" | "download_failed"
    pub sync_status: String,
    /// Error message if sync failed.
    pub sync_error: Option<String>,
    /// Timestamp of last sync attempt.
    pub last_sync_attempt: Option<i64>,
    /// Number of failed retry attempts.
    pub retry_count: i32,
    pub owner: String,
}

// ─── Reducer argument types ────────────────────────────────────────────────────

#[derive(SpacetimeType)]
pub struct CreateQuailArgs {
    pub uuid: String,
    pub name: String,
    pub gender: String,
    pub ring_color: Option<String>,
    pub profile_photo: Option<String>,
}

#[derive(SpacetimeType)]
pub struct UpdateQuailArgs {
    pub uuid: String,
    pub name: String,
    pub gender: String,
    pub ring_color: Option<String>,
    pub profile_photo: Option<String>,
}

#[derive(SpacetimeType)]
pub struct CreateEventArgs {
    pub uuid: String,
    pub quail_uuid: String,
    pub event_type: String,
    pub event_date: String,
    pub notes: Option<String>,
    pub photos: Option<String>,
}

#[derive(SpacetimeType)]
pub struct UpdateEventArgs {
    pub uuid: String,
    pub event_type: String,
    pub event_date: String,
    pub notes: Option<String>,
    pub photos: Option<String>,
}

#[derive(SpacetimeType)]
pub struct UpsertEggRecordArgs {
    pub uuid: String,
    pub record_date: String,
    pub total_eggs: i32,
    pub notes: Option<String>,
}

#[derive(SpacetimeType)]
pub struct CreatePhotoCollectionArgs {
    pub uuid: String,
    pub quail_uuid: Option<String>,
    pub event_uuid: Option<String>,
    pub name: String,
}

#[derive(SpacetimeType)]
pub struct UpdatePhotoCollectionArgs {
    pub uuid: String,
    pub preview_photo_uuid: Option<String>,
}

#[derive(SpacetimeType)]
pub struct CreatePhotoArgs {
    pub uuid: String,
    pub collection_uuid: String,
    pub relative_path: String,
}

#[derive(SpacetimeType)]
pub struct UpdatePhotoSyncStatusArgs {
    pub uuid: String,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub last_sync_attempt: Option<i64>,
    pub retry_count: i32,
}

// ─── Reducers ─────────────────────────────────────────────────────────────────

/// Create a new quail profile.
#[reducer]
pub fn create_quail(ctx: &ReducerContext, args: CreateQuailArgs) {
    ctx.db.quails().insert(Quail {
        uuid: args.uuid,
        id: 0,
        name: args.name,
        gender: args.gender,
        ring_color: args.ring_color,
        profile_photo: args.profile_photo,
        owner: ctx.sender().to_string(),
    });
}

/// Update an existing quail profile owned by the caller.
#[reducer]
pub fn update_quail(ctx: &ReducerContext, args: UpdateQuailArgs) {
    if let Some(mut existing) = ctx.db.quails().iter().find(|q| q.uuid == args.uuid) {
        if existing.owner != ctx.sender().to_string() {
            log::warn!("update_quail: caller is not the owner");
            return;
        }
        existing.name = args.name;
        existing.gender = args.gender;
        existing.ring_color = args.ring_color;
        existing.profile_photo = args.profile_photo;
        ctx.db.quails().uuid().update(existing);
    }
}

/// Delete a quail and all its events (owner-only).
#[reducer]
pub fn delete_quail(ctx: &ReducerContext, uuid: String) {
    if let Some(existing) = ctx.db.quails().iter().find(|q| q.uuid == uuid) {
        if existing.owner != ctx.sender().to_string() {
            log::warn!("delete_quail: caller is not the owner");
            return;
        }
        // Cascade-delete events belonging to this quail.
        let event_uuids: Vec<String> = ctx
            .db
            .quail_events()
            .iter()
            .filter(|e| e.quail_uuid == uuid && e.owner == ctx.sender().to_string())
            .map(|e| e.uuid.clone())
            .collect();
        for event_uuid in event_uuids {
            ctx.db.quail_events().uuid().delete(&event_uuid);
        }
        ctx.db.quails().uuid().delete(&uuid);
    }
}

/// Update only the profile photo of a quail (owner-only).
#[reducer]
pub fn set_quail_photo(ctx: &ReducerContext, quail_uuid: String, photo_uuid: Option<String>) {
    if let Some(mut existing) = ctx.db.quails().iter().find(|q| q.uuid == quail_uuid) {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        existing.profile_photo = photo_uuid;
        ctx.db.quails().uuid().update(existing);
    }
}

/// Create a lifecycle event for a quail.
#[reducer]
pub fn create_event(ctx: &ReducerContext, args: CreateEventArgs) {
    ctx.db.quail_events().insert(QuailEvent {
        uuid: args.uuid,
        id: 0,
        quail_uuid: args.quail_uuid,
        event_type: args.event_type,
        event_date: args.event_date,
        notes: args.notes,
        photos: args.photos,
        owner: ctx.sender().to_string(),
    });
}

/// Update a lifecycle event (owner-only).
#[reducer]
pub fn update_event(ctx: &ReducerContext, args: UpdateEventArgs) {
    if let Some(mut existing) = ctx.db.quail_events().iter().find(|e| e.uuid == args.uuid) {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        existing.event_type = args.event_type;
        existing.event_date = args.event_date;
        existing.notes = args.notes;
        existing.photos = args.photos;
        ctx.db.quail_events().uuid().update(existing);
    }
}

/// Delete a lifecycle event (owner-only).
#[reducer]
pub fn delete_event(ctx: &ReducerContext, uuid: String) {
    if let Some(existing) = ctx.db.quail_events().iter().find(|e| e.uuid == uuid) {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        ctx.db.quail_events().uuid().delete(&uuid);
    }
}

/// Insert or update an egg record for a given date (owner-only).
#[reducer]
pub fn upsert_egg_record(ctx: &ReducerContext, args: UpsertEggRecordArgs) {
    // Check whether an egg record already exists for this date + owner.
    let sender_str = ctx.sender().to_string();
    let existing = ctx
        .db
        .egg_records()
        .iter()
        .find(|r| r.record_date == args.record_date && r.owner == sender_str);

    if let Some(mut existing) = existing {
        existing.total_eggs = args.total_eggs;
        existing.notes = args.notes;
        ctx.db.egg_records().uuid().update(existing);
    } else {
        ctx.db.egg_records().insert(EggRecord {
            uuid: args.uuid,
            id: 0,
            record_date: args.record_date,
            total_eggs: args.total_eggs,
            notes: args.notes,
            owner: sender_str,
        });
    }
}

/// Delete an egg record (owner-only).
#[reducer]
pub fn delete_egg_record(ctx: &ReducerContext, uuid: String) {
    if let Some(existing) = ctx.db.egg_records().iter().find(|r| r.uuid == uuid) {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        ctx.db.egg_records().uuid().delete(&uuid);
    }
}

/// Create a photo collection.
#[reducer]
pub fn create_photo_collection(ctx: &ReducerContext, args: CreatePhotoCollectionArgs) {
    ctx.db.photo_collections().insert(PhotoCollection {
        uuid: args.uuid,
        id: 0,
        quail_uuid: args.quail_uuid,
        event_uuid: args.event_uuid,
        preview_photo_uuid: None,
        name: args.name,
        owner: ctx.sender().to_string(),
    });
}

/// Update photo collection preview (owner-only).
#[reducer]
pub fn update_photo_collection(ctx: &ReducerContext, args: UpdatePhotoCollectionArgs) {
    if let Some(mut existing) = ctx
        .db
        .photo_collections()
        .iter()
        .find(|c| c.uuid == args.uuid)
    {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        existing.preview_photo_uuid = args.preview_photo_uuid;
        ctx.db.photo_collections().uuid().update(existing);
    }
}

/// Delete a photo collection and all its photos (owner-only).
#[reducer]
pub fn delete_photo_collection(ctx: &ReducerContext, uuid: String) {
    if let Some(existing) = ctx.db.photo_collections().iter().find(|c| c.uuid == uuid) {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        // Cascade-delete photos in this collection.
        let photo_uuids: Vec<String> = ctx
            .db
            .photos()
            .iter()
            .filter(|p| p.collection_uuid == uuid && p.owner == ctx.sender().to_string())
            .map(|p| p.uuid.clone())
            .collect();
        for photo_uuid in photo_uuids {
            ctx.db.photos().uuid().delete(&photo_uuid);
        }
        ctx.db.photo_collections().uuid().delete(&uuid);
    }
}

/// Create a new photo in a collection.
#[reducer]
pub fn create_photo(ctx: &ReducerContext, args: CreatePhotoArgs) {
    ctx.db.photos().insert(Photo {
        uuid: args.uuid,
        id: 0,
        collection_uuid: args.collection_uuid,
        relative_path: args.relative_path,
        sync_status: "local_only".to_string(),
        sync_error: None,
        last_sync_attempt: None,
        retry_count: 0,
        owner: ctx.sender().to_string(),
    });
}

/// Update photo sync status (owner-only).
#[reducer]
pub fn update_photo_sync_status(ctx: &ReducerContext, args: UpdatePhotoSyncStatusArgs) {
    if let Some(mut existing) = ctx.db.photos().iter().find(|p| p.uuid == args.uuid) {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        existing.sync_status = args.sync_status;
        existing.sync_error = args.sync_error;
        existing.last_sync_attempt = args.last_sync_attempt;
        existing.retry_count = args.retry_count;
        ctx.db.photos().uuid().update(existing);
    }
}

/// Delete a photo (owner-only).
#[reducer]
pub fn delete_photo(ctx: &ReducerContext, uuid: String) {
    if let Some(existing) = ctx.db.photos().iter().find(|p| p.uuid == uuid) {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        ctx.db.photos().uuid().delete(&uuid);
    }
}
