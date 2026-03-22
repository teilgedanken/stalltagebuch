//! SpacetimeDB server module for Stalltagebuch.
//!
//! This module defines the database tables and reducers that run on SpacetimeDB.
//! Deploy with: `spacetime publish --project-path stalltagebuch-server stalltagebuch-server`
//! Generate client bindings: `spacetime generate --lang rust --out-dir ../src/spacetime/module_bindings`
//! (or `--lang dioxus` when the Dioxus binding generator is available)

#![allow(legacy_derive_helpers)]

use spacetimedb::{reducer, ReducerContext, SpacetimeType, Table, Timestamp};

// ─── Tables ────────────────────────────────────────────────────────────────────

/// A registered device that has connected to the database.
#[spacetimedb::table(accessor = devices, public)]
#[index(columns = [last_seen])]
pub struct Device {
    /// Unique device identifier (e.g., ANDROID_ID on Android).
    #[primary_key]
    pub device_id: String,
    /// User-friendly device name (e.g., "Mein Handy", "Tablet").
    pub name: Option<String>,
    /// Optional comment/description for this device.
    pub comment: Option<String>,
    /// Unix timestamp (seconds) when device first connected.
    pub first_seen: i64,
    /// Unix timestamp (seconds) when device last connected.
    pub last_seen: i64,
    /// The SpacetimeDB identity of the user who owns this device.
    pub owner: String,
}

/// A quail in the flock.
#[spacetimedb::table(accessor = quails, public)]
#[index(columns = [created_at])]
pub struct Quail {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    pub name: String,
    /// "male" | "female" | "unknown"
    pub gender: String,
    /// Optional left leg ring colour (e.g. "lila", "rot", …)
    pub ring_color_left: Option<String>,
    /// Optional right leg ring colour (e.g. "lila", "rot", …)
    pub ring_color_right: Option<String>,
    /// UUID of the profile photo stored in the photo-gallery / Nextcloud.
    pub profile_photo: Option<String>,
    /// ID of the device that created this quail.
    pub device_id: String,
    /// The SpacetimeDB identity of the user who owns this quail.
    pub owner: String,
    /// Unix timestamp (seconds) when quail was created.
    pub created_at: i64,
}

/// A lifecycle event for a quail.
#[spacetimedb::table(accessor = quail_events, public)]
#[index(columns = [event_date])]
pub struct QuailEvent {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    pub quail_uuid: String,
    /// "born" | "alive" | "sick" | "healthy" | "marked_for_slaughter" | "slaughtered" | "died"
    pub event_type: String,
    /// ISO 8601 date string YYYY-MM-DD.
    pub event_date: String,
    pub notes: Option<String>,
    /// UUID of an attached photo stored on Nextcloud.
    pub photos: Option<String>,
    /// ID of the device that created this event.
    pub device_id: String,
    pub owner: String,
}

/// A daily egg-production record.
#[spacetimedb::table(accessor = egg_records, public)]
#[index(columns = [record_date])]
pub struct EggRecord {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    /// Unix timestamp (seconds) for the record date.
    pub record_date: i64,
    pub total_eggs: i32,
    pub notes: Option<String>,
    /// ID of the device that created this record.
    pub device_id: String,
    pub owner: String,
}

/// A collection of photos (for a quail or event).
#[spacetimedb::table(accessor = photo_collections, public)]
#[index(columns = [created_at])]
pub struct PhotoCollection {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
    /// UUID of the quail this collection belongs to (if applicable).
    pub quail_uuid: Option<String>,
    /// UUID of the event this collection belongs to (if applicable).
    pub event_uuid: Option<String>,
    /// UUID of the photo used as preview for this collection.
    pub preview_photo_uuid: Option<String>,
    /// Name/description of the collection.
    pub name: String,
    /// ID of the device that created this collection.
    pub device_id: String,
    pub owner: String,
    /// Unix timestamp (seconds) when collection was created.
    pub created_at: i64,
    /// Unix timestamp (seconds) when collection was last modified.
    pub updated_at: i64,
}

/// A photo in a collection.
#[spacetimedb::table(accessor = photos, public)]
#[index(columns = [created_at])]
pub struct Photo {
    /// Client-generated UUID used as the stable cross-device identifier.
    #[primary_key]
    pub uuid: String,
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
    /// ID of the device that created this photo.
    pub device_id: String,
    pub owner: String,
    /// Unix timestamp (seconds) when photo was created.
    pub created_at: i64,
    /// Unix timestamp (seconds) when photo was last modified.
    pub updated_at: i64,
}

/// A tracked backup run and its final result.
#[spacetimedb::table(accessor = backups, public)]
#[index(columns = [created_at])]
pub struct Backup {
    #[primary_key]
    pub backup_id: String,
    /// "file" | "nextcloud"
    pub kind: String,
    /// "started" | "success" | "failed"
    pub status: String,
    pub include_images: bool,
    pub local_path: Option<String>,
    pub remote_filename: Option<String>,
    pub zip_size_bytes: Option<i64>,
    pub quails: i32,
    pub events: i32,
    pub egg_records: i32,
    pub photos_meta: i32,
    pub photos_files_included: i32,
    pub photos_files_missing: i32,
    pub error_message: Option<String>,
    pub owner: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// ─── Reducer argument types ────────────────────────────────────────────────────

#[derive(SpacetimeType)]
pub struct CreateQuailArgs {
    pub uuid: String,
    pub name: String,
    pub gender: String,
    pub ring_color_left: Option<String>,
    pub ring_color_right: Option<String>,
    pub profile_photo: Option<String>,
    pub device_id: String,
}

#[derive(SpacetimeType)]
pub struct UpdateQuailArgs {
    pub uuid: String,
    pub name: String,
    pub gender: String,
    pub ring_color_left: Option<String>,
    pub ring_color_right: Option<String>,
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
    pub device_id: String,
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
    pub record_date: i64,
    pub total_eggs: i32,
    pub notes: Option<String>,
    pub device_id: String,
}

#[derive(SpacetimeType)]
pub struct CreatePhotoCollectionArgs {
    pub uuid: String,
    pub quail_uuid: Option<String>,
    pub event_uuid: Option<String>,
    pub name: String,
    pub device_id: String,
}

#[derive(SpacetimeType)]
pub struct UpdatePhotoCollectionArgs {
    pub uuid: String,
    pub preview_photo_uuid: Option<String>,
    pub updated_at: i64,
}

#[derive(SpacetimeType)]
pub struct CreatePhotoArgs {
    pub uuid: String,
    pub collection_uuid: String,
    pub relative_path: String,
    pub device_id: String,
}

#[derive(SpacetimeType)]
pub struct UpdatePhotoSyncStatusArgs {
    pub uuid: String,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub last_sync_attempt: Option<i64>,
    pub retry_count: i32,
}

#[derive(SpacetimeType)]
pub struct CreateBackupStartedArgs {
    pub backup_id: String,
    pub kind: String,
    pub include_images: bool,
    pub local_path: Option<String>,
}

#[derive(SpacetimeType)]
pub struct FinishBackupArgs {
    pub backup_id: String,
    pub status: String,
    pub local_path: Option<String>,
    pub remote_filename: Option<String>,
    pub zip_size_bytes: Option<i64>,
    pub quails: i32,
    pub events: i32,
    pub egg_records: i32,
    pub photos_meta: i32,
    pub photos_files_included: i32,
    pub photos_files_missing: i32,
    pub error_message: Option<String>,
}

// ─── Reducers ─────────────────────────────────────────────────────────────────

/// Create a new quail profile.
#[reducer]
pub fn create_quail(ctx: &ReducerContext, args: CreateQuailArgs) {
    let now = ctx
        .timestamp
        .duration_since(Timestamp::UNIX_EPOCH)
        .expect("timestamp should be after UNIX_EPOCH")
        .as_secs() as i64;

    ctx.db.quails().insert(Quail {
        uuid: args.uuid,
        
        name: args.name,
        gender: args.gender,
        ring_color_left: args.ring_color_left,
        ring_color_right: args.ring_color_right,
        profile_photo: args.profile_photo,
        device_id: args.device_id,
        owner: ctx.sender().to_string(),
        created_at: now,
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
        existing.ring_color_left = args.ring_color_left;
        existing.ring_color_right = args.ring_color_right;
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
        
        quail_uuid: args.quail_uuid,
        event_type: args.event_type,
        event_date: args.event_date,
        notes: args.notes,
        photos: args.photos,
        device_id: args.device_id,
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
            
            record_date: args.record_date,
            total_eggs: args.total_eggs,
            notes: args.notes,
            device_id: args.device_id,
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
    let now = ctx
        .timestamp
        .duration_since(Timestamp::UNIX_EPOCH)
        .expect("timestamp should be after UNIX_EPOCH")
        .as_secs() as i64;

    ctx.db.photo_collections().insert(PhotoCollection {
        uuid: args.uuid,
        
        quail_uuid: args.quail_uuid,
        event_uuid: args.event_uuid,
        preview_photo_uuid: None,
        name: args.name,
        device_id: args.device_id,
        owner: ctx.sender().to_string(),
        created_at: now,
        updated_at: now,
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
        existing.updated_at = args.updated_at;
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
    let now = ctx
        .timestamp
        .duration_since(Timestamp::UNIX_EPOCH)
        .expect("timestamp should be after UNIX_EPOCH")
        .as_secs() as i64;

    ctx.db.photos().insert(Photo {
        uuid: args.uuid,
        collection_uuid: args.collection_uuid,
        relative_path: args.relative_path,
        sync_status: "local_only".to_string(),
        sync_error: None,
        last_sync_attempt: None,
        retry_count: 0,
        device_id: args.device_id,
        owner: ctx.sender().to_string(),
        created_at: now,
        updated_at: now,
    });
}

/// Create a backup tracking entry with status "started".
#[reducer]
pub fn create_backup_started(ctx: &ReducerContext, args: CreateBackupStartedArgs) {
    let now = ctx
        .timestamp
        .duration_since(Timestamp::UNIX_EPOCH)
        .expect("timestamp should be after UNIX_EPOCH")
        .as_secs() as i64;

    ctx.db.backups().insert(Backup {
        backup_id: args.backup_id,
        kind: args.kind,
        status: "started".to_string(),
        include_images: args.include_images,
        local_path: args.local_path,
        remote_filename: None,
        zip_size_bytes: None,
        quails: 0,
        events: 0,
        egg_records: 0,
        photos_meta: 0,
        photos_files_included: 0,
        photos_files_missing: 0,
        error_message: None,
        owner: ctx.sender().to_string(),
        created_at: now,
        updated_at: now,
    });
}

/// Finish a backup entry with status "success" or "failed" (owner-only).
#[reducer]
pub fn finish_backup(ctx: &ReducerContext, args: FinishBackupArgs) {
    if let Some(mut existing) = ctx.db.backups().iter().find(|b| b.backup_id == args.backup_id) {
        if existing.owner != ctx.sender().to_string() {
            return;
        }
        existing.status = args.status;
        existing.local_path = args.local_path;
        existing.remote_filename = args.remote_filename;
        existing.zip_size_bytes = args.zip_size_bytes;
        existing.quails = args.quails;
        existing.events = args.events;
        existing.egg_records = args.egg_records;
        existing.photos_meta = args.photos_meta;
        existing.photos_files_included = args.photos_files_included;
        existing.photos_files_missing = args.photos_files_missing;
        existing.error_message = args.error_message;
        let now = ctx
            .timestamp
            .duration_since(Timestamp::UNIX_EPOCH)
            .expect("timestamp should be after UNIX_EPOCH")
            .as_secs() as i64;
        existing.updated_at = now;
        ctx.db.backups().backup_id().update(existing);
    }
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
        let now = ctx.timestamp
            .duration_since(Timestamp::UNIX_EPOCH)
            .expect("timestamp should be after UNIX_EPOCH")
            .as_secs() as i64;
        existing.updated_at = now;
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

// ─── Device Management ────────────────────────────────────────────────────────

#[derive(SpacetimeType)]
pub struct RegisterDeviceArgs {
    pub device_id: String,
    pub name: Option<String>,
    pub comment: Option<String>,
}

#[derive(SpacetimeType)]
pub struct UpdateDeviceArgs {
    pub device_id: String,
    pub name: Option<String>,
    pub comment: Option<String>,
}

/// Register or update a device. Creates a new device if it doesn't exist,
/// or updates the last_seen timestamp if it does.
#[reducer]
pub fn register_device(ctx: &ReducerContext, args: RegisterDeviceArgs) {
    let sender_str = ctx.sender().to_string();
    // Use ctx.timestamp to get the current time in a WASM-compatible way
    let now = ctx.timestamp
        .duration_since(Timestamp::UNIX_EPOCH)
        .expect("timestamp should be after UNIX_EPOCH")
        .as_secs() as i64;

    log::info!(
        "register_device called: device_id={}, sender={}", 
        args.device_id, 
        sender_str
    );

    if let Some(mut existing) = ctx.db.devices().iter().find(|d| d.device_id == args.device_id) {
        // Update existing device
        log::info!(
            "register_device: found existing device, owner={}, current_sender={}",
            existing.owner,
            sender_str
        );
        if existing.owner != sender_str {
            log::warn!("register_device: device owned by different user");
            return;
        }
        existing.last_seen = now;
        if let Some(name) = args.name {
            existing.name = Some(name);
        }
        if let Some(comment) = args.comment {
            existing.comment = Some(comment);
        }
        ctx.db.devices().device_id().update(existing);
        log::info!("register_device: updated existing device {}", args.device_id);
    } else {
        // Create new device
        log::info!("register_device: creating new device {}", args.device_id);
        ctx.db.devices().insert(Device {
            device_id: args.device_id.clone(),
            
            name: args.name,
            comment: args.comment,
            first_seen: now,
            last_seen: now,
            owner: sender_str,
        });
        log::info!("register_device: created new device {}", args.device_id);
    }
}

/// Update device information (owner-only).
#[reducer]
pub fn update_device(ctx: &ReducerContext, args: UpdateDeviceArgs) {
    if let Some(mut existing) = ctx.db.devices().iter().find(|d| d.device_id == args.device_id) {
        if existing.owner != ctx.sender().to_string() {
            log::warn!("update_device: caller is not the owner");
            return;
        }
        existing.name = args.name;
        existing.comment = args.comment;
        ctx.db.devices().device_id().update(existing);
    }
}

/// Delete a device (owner-only).
#[reducer]
pub fn delete_device(ctx: &ReducerContext, device_id: String) {
    if let Some(existing) = ctx.db.devices().iter().find(|d| d.device_id == device_id) {
        if existing.owner != ctx.sender().to_string() {
            log::warn!("delete_device: caller is not the owner");
            return;
        }
        ctx.db.devices().device_id().delete(&device_id);
    }
}
