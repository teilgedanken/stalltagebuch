//! Photo metadata integration with SpacetimeDB.
//!
//! This module documents how to integrate photo metadata into SpacetimeDB
//! while keeping file operations with photo-gallery and Nextcloud sync unchanged.
//!
//! Components should migrate to calling Spacetime reducers directly:
//! - `use_reducer_create_photo_collection(args)` - Create collection
//! - `use_reducer_create_photo(args)` - Register photo
//! - `use_reducer_update_photo_sync_status(args)` - Update sync state
//! - `use_reducer_delete_photo(uuid)` - Delete photo
//!
//! The `photo_service` module continues to handle backward compatibility
//! with SQLite-based photo metadata via the local `photos` table.

// Placeholder for photo metadata integration helpers
// To be extended as components are migrated to Spacetime hooks
