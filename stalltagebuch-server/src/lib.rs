//! SpacetimeDB server module for Stalltagebuch.
//!
//! This module defines the database tables and reducers that run on SpacetimeDB.
//! Deploy with: `spacetime publish --project-path stalltagebuch-server stalltagebuch-server`
//! Generate client bindings: `spacetime generate --lang rust --out-dir ../src/spacetime/module_bindings`
//! (or `--lang dioxus` when the Dioxus binding generator is available)

use spacetimedb::{reducer, table, Identity, ReducerContext, SpacetimeType, Table};

// ─── Tables ────────────────────────────────────────────────────────────────────

/// A quail in the flock.
#[table(name = quails, public)]
pub struct Quail {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    /// Client-generated UUID used as the stable cross-device identifier.
    #[unique]
    pub uuid: String,
    pub name: String,
    /// "male" | "female" | "unknown"
    pub gender: String,
    /// Optional leg ring colour (e.g. "lila", "rot", …)
    pub ring_color: Option<String>,
    /// UUID of the profile photo stored in the photo-gallery / Nextcloud.
    pub profile_photo: Option<String>,
    /// The SpacetimeDB identity of the user who owns this quail.
    pub owner: Identity,
}

/// A lifecycle event for a quail.
#[table(name = quail_events, public)]
pub struct QuailEvent {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub uuid: String,
    pub quail_uuid: String,
    /// "born" | "alive" | "sick" | "healthy" | "marked_for_slaughter" | "slaughtered" | "died"
    pub event_type: String,
    /// ISO 8601 date string YYYY-MM-DD.
    pub event_date: String,
    pub notes: Option<String>,
    /// UUID of an attached photo stored on Nextcloud.
    pub photos: Option<String>,
    pub owner: Identity,
}

/// A daily egg-production record.
#[table(name = egg_records, public)]
pub struct EggRecord {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    #[unique]
    pub uuid: String,
    /// ISO 8601 date string YYYY-MM-DD.
    pub record_date: String,
    pub total_eggs: i32,
    pub notes: Option<String>,
    pub owner: Identity,
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
        id: 0,
        uuid: args.uuid,
        name: args.name,
        gender: args.gender,
        ring_color: args.ring_color,
        profile_photo: args.profile_photo,
        owner: ctx.sender,
    });
}

/// Update an existing quail profile owned by the caller.
#[reducer]
pub fn update_quail(ctx: &ReducerContext, args: UpdateQuailArgs) {
    if let Some(existing) = ctx.db.quails().uuid().find(&args.uuid) {
        if existing.owner != ctx.sender {
            log::warn!("update_quail: caller is not the owner");
            return;
        }
        ctx.db.quails().uuid().update(Quail {
            id: existing.id,
            uuid: args.uuid,
            name: args.name,
            gender: args.gender,
            ring_color: args.ring_color,
            profile_photo: args.profile_photo,
            owner: existing.owner,
        });
    }
}

/// Delete a quail and all its events (owner-only).
#[reducer]
pub fn delete_quail(ctx: &ReducerContext, uuid: String) {
    if let Some(existing) = ctx.db.quails().uuid().find(&uuid) {
        if existing.owner != ctx.sender {
            log::warn!("delete_quail: caller is not the owner");
            return;
        }
        // Cascade-delete events belonging to this quail.
        let event_uuids: Vec<String> = ctx
            .db
            .quail_events()
            .iter()
            .filter(|e| e.quail_uuid == uuid && e.owner == ctx.sender)
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
    if let Some(existing) = ctx.db.quails().uuid().find(&quail_uuid) {
        if existing.owner != ctx.sender {
            return;
        }
        ctx.db.quails().uuid().update(Quail {
            profile_photo: photo_uuid,
            ..existing
        });
    }
}

/// Create a lifecycle event for a quail.
#[reducer]
pub fn create_event(ctx: &ReducerContext, args: CreateEventArgs) {
    ctx.db.quail_events().insert(QuailEvent {
        id: 0,
        uuid: args.uuid,
        quail_uuid: args.quail_uuid,
        event_type: args.event_type,
        event_date: args.event_date,
        notes: args.notes,
        photos: args.photos,
        owner: ctx.sender,
    });
}

/// Update a lifecycle event (owner-only).
#[reducer]
pub fn update_event(ctx: &ReducerContext, args: UpdateEventArgs) {
    if let Some(existing) = ctx.db.quail_events().uuid().find(&args.uuid) {
        if existing.owner != ctx.sender {
            return;
        }
        ctx.db.quail_events().uuid().update(QuailEvent {
            id: existing.id,
            uuid: args.uuid,
            quail_uuid: existing.quail_uuid,
            event_type: args.event_type,
            event_date: args.event_date,
            notes: args.notes,
            photos: args.photos,
            owner: existing.owner,
        });
    }
}

/// Delete a lifecycle event (owner-only).
#[reducer]
pub fn delete_event(ctx: &ReducerContext, uuid: String) {
    if let Some(existing) = ctx.db.quail_events().uuid().find(&uuid) {
        if existing.owner != ctx.sender {
            return;
        }
        ctx.db.quail_events().uuid().delete(&uuid);
    }
}

/// Insert or update an egg record for a given date (owner-only).
#[reducer]
pub fn upsert_egg_record(ctx: &ReducerContext, args: UpsertEggRecordArgs) {
    // Check whether an egg record already exists for this date + owner.
    let existing = ctx
        .db
        .egg_records()
        .iter()
        .find(|r| r.record_date == args.record_date && r.owner == ctx.sender);

    if let Some(existing) = existing {
        ctx.db.egg_records().uuid().update(EggRecord {
            id: existing.id,
            uuid: existing.uuid,
            record_date: args.record_date,
            total_eggs: args.total_eggs,
            notes: args.notes,
            owner: existing.owner,
        });
    } else {
        ctx.db.egg_records().insert(EggRecord {
            id: 0,
            uuid: args.uuid,
            record_date: args.record_date,
            total_eggs: args.total_eggs,
            notes: args.notes,
            owner: ctx.sender,
        });
    }
}

/// Delete an egg record (owner-only).
#[reducer]
pub fn delete_egg_record(ctx: &ReducerContext, uuid: String) {
    if let Some(existing) = ctx.db.egg_records().uuid().find(&uuid) {
        if existing.owner != ctx.sender {
            return;
        }
        ctx.db.egg_records().uuid().delete(&uuid);
    }
}
