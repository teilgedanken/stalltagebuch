CREATE TABLE schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
CREATE TABLE photos (
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
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1)), collection_id TEXT,
            CHECK( (quail_id IS NOT NULL AND event_id IS NOT NULL) OR (quail_id IS NULL OR event_id IS NULL) )
        );
CREATE INDEX idx_photos_quail ON photos(quail_id);
CREATE INDEX idx_photos_event ON photos(event_id);
CREATE TRIGGER update_photos_timestamp 
         AFTER UPDATE ON photos
         BEGIN
            UPDATE photos SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END;
CREATE TABLE quails (
            uuid TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            gender TEXT CHECK(gender IN ('male', 'female', 'unknown')) NOT NULL DEFAULT 'unknown',
            ring_color TEXT,
            profile_photo TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rev INTEGER NOT NULL DEFAULT 0,
            logical_clock INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1)), collection_id TEXT,
            FOREIGN KEY (profile_photo) REFERENCES photos(uuid) ON DELETE SET NULL
        );
CREATE INDEX idx_quails_name ON quails(name);
CREATE TRIGGER update_quails_timestamp 
         AFTER UPDATE ON quails
         BEGIN
            UPDATE quails SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END;
CREATE TABLE quail_events (
            uuid TEXT PRIMARY KEY,
            quail_id TEXT NOT NULL,
            event_type TEXT CHECK(event_type IN ('born', 'alive', 'sick', 'healthy', 'marked_for_slaughter', 'slaughtered', 'died')) NOT NULL,
            event_date TEXT NOT NULL,
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rev INTEGER NOT NULL DEFAULT 0,
            logical_clock INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1)), collection_id TEXT,
            FOREIGN KEY (quail_id) REFERENCES quails(uuid) ON DELETE CASCADE
        );
CREATE INDEX idx_quail_events_quail_id ON quail_events(quail_id);
CREATE INDEX idx_quail_events_date ON quail_events(event_date DESC);
CREATE INDEX idx_quail_events_type ON quail_events(event_type);
CREATE TRIGGER update_quail_events_timestamp 
         AFTER UPDATE ON quail_events
         BEGIN
            UPDATE quail_events SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END;
CREATE TABLE egg_records (
            uuid TEXT PRIMARY KEY,
            record_date TEXT NOT NULL UNIQUE,
            total_eggs INTEGER NOT NULL DEFAULT 0 CHECK(total_eggs >= 0),
            notes TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rev INTEGER NOT NULL DEFAULT 0,
            logical_clock INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1))
        );
CREATE INDEX idx_egg_records_date ON egg_records(record_date DESC);
CREATE TRIGGER update_egg_records_timestamp 
         AFTER UPDATE ON egg_records
         BEGIN
            UPDATE egg_records SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END;
CREATE TABLE sync_settings (
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
        , initial_upload_done INTEGER NOT NULL DEFAULT 0 CHECK(initial_upload_done IN (0,1)));
CREATE TABLE sqlite_sequence(name,seq);
CREATE TRIGGER update_sync_settings_timestamp 
         AFTER UPDATE ON sync_settings
         BEGIN
            UPDATE sync_settings SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
         END;
CREATE TABLE sync_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            photo_id TEXT NOT NULL,
            status TEXT CHECK(status IN ('pending', 'uploading', 'completed', 'failed')) NOT NULL DEFAULT 'pending',
            retry_count INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (photo_id) REFERENCES photos(uuid) ON DELETE CASCADE
        );
CREATE TABLE op_log (
            op_id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            ts INTEGER NOT NULL,
            logical_counter INTEGER NOT NULL,
            device_id TEXT NOT NULL,
            op_kind TEXT NOT NULL, -- upsert | delete | inc
            payload TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
CREATE INDEX idx_op_log_entity ON op_log(entity_id, ts);
CREATE INDEX idx_op_log_device ON op_log(device_id, ts);
CREATE TABLE sync_checkpoint (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            last_ts INTEGER NOT NULL DEFAULT 0,
            last_logical_counter INTEGER NOT NULL DEFAULT 0,
            last_device_id TEXT,
            snapshot_rev INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
CREATE TABLE device_state (
            id INTEGER PRIMARY KEY CHECK(id = 1),
            device_id TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
CREATE INDEX idx_sync_queue_status ON sync_queue(status);
CREATE INDEX idx_sync_queue_photo ON sync_queue(photo_id);
CREATE TRIGGER update_sync_queue_timestamp 
         AFTER UPDATE ON sync_queue
         BEGIN
            UPDATE sync_queue SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
         END;
CREATE TABLE photo_schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
CREATE TABLE photo_collections (
            uuid TEXT PRIMARY KEY,
            preview_photo_uuid TEXT,
            name TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rev INTEGER NOT NULL DEFAULT 0,
            logical_clock INTEGER NOT NULL DEFAULT 0,
            deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1))
        );
CREATE TRIGGER update_photo_collections_timestamp 
         AFTER UPDATE ON photo_collections
         BEGIN
            UPDATE photo_collections SET updated_at = CURRENT_TIMESTAMP WHERE uuid = NEW.uuid;
         END;
CREATE INDEX idx_photos_collection ON photos(collection_id);