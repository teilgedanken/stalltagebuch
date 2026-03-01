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
