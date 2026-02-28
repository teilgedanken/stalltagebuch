# Remaining Work for Photo Gallery Refactoring

Updated: 2026-02-28

This document tracks what is still open after the latest photo-gallery and database integration work.

## Recently Completed ✅

1. Photo-gallery picker migration finished (`photo-gallery/src/picker.rs`, `src/camera.rs` wrapper).
2. UI components now load by `photo_uuid`/collection context (no more manual data URL loading in callers).
3. Main app now provides `PhotoGalleryContext` centrally in `src/main.rs`.
4. `stalltagebuch-database` crate introduced and wired into workspace.
5. `src/database/mod.rs` now runs app-specific schema extension (`src/database/schema.rs`) after core DB init.
6. Collection access simplified: quail/event UUID is used as collection UUID in `src/services/photo_service.rs`.
7. Android OpenSSL crash fixed by switching `nextcloud-auth` reqwest to `rustls-tls`.
8. Temporary OpenSSL copy logic removed from `build_android.sh`; Android build succeeds again.

## Remaining Tasks

### 1. Verify/repair legacy DBs that still miss `photos.collection_id` ❌

Recent runtime errors showed that some installed databases can still be on an older shape.

What is still needed:
1. Add a startup schema check specifically for `photos.collection_id`.
2. If missing, run a safe repair migration and log a clear message.
3. Add a regression test path for upgrading an old DB snapshot.

### 2. Finish upload/download refactor boundary ❌

Current state:
1. Core photo loading moved toward `photo-gallery`.
2. Upload/download orchestration still spans app services and gallery services.

What is still needed:
1. Define one stable API boundary between `src/services/upload_service.rs` and `photo-gallery/src/{upload,download}.rs`.
2. Ensure retry/error metadata is not duplicated between crates.
3. Keep CRDT capture app-side only.

### 3. Remove stale assumptions about `collection_id` on quails/events ❌

Current state:
1. Runtime logic now uses quail/event UUIDs as collection IDs.
2. Some docs/comments still describe FK-based `collection_id` linkage on `quails`/`quail_events`.

What is still needed:
1. Clean up outdated docs/comments to reflect UUID-as-collection model.
2. Confirm no code path still queries `quails.collection_id` or `quail_events.collection_id`.

### 4. Add focused migration and Android smoke tests ❌

What is still needed:
1. Migration test: old DB -> current schema -> add photo to collection.
2. Android smoke flow: install APK, create profile/event, add image, reopen app, verify image persists.
3. Optional sync smoke: verify one upload and one download still work after refactor.

## Notes

1. The previous broad refactor checklist has been trimmed; completed phases were removed.
2. Priority should be database upgrade safety first, then upload/download boundary cleanup.
