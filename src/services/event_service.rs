use crate::error::AppError;
use crate::models::{EventType, QuailEvent};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

/// Creates a new event for a quail
pub async fn create_event(
    conn: &Connection,
    quail_id: Uuid,
    event_type: EventType,
    event_date: NaiveDate,
    notes: Option<String>,
) -> Result<Uuid, AppError> {
    let mut event = QuailEvent::new(quail_id, event_type, event_date);
    event.notes = notes.clone();

    event.validate()?;

    conn.execute(
        "INSERT INTO quail_events (uuid, quail_id, event_type, event_date, notes)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            event.uuid.to_string(),
            event.quail_id.to_string(),
            event.event_type.as_str(),
            event.event_date.to_string(),
            event.notes,
        ],
    )?;

    // Capture CRDT operation
    crate::services::operation_capture::capture_event_create(
        conn,
        &event.uuid.to_string(),
        &event.quail_id.to_string(),
        event.event_type.as_str(),
        &event.event_date.to_string(),
        notes.as_deref(),
    )
    .await?;

    Ok(event.uuid)
}

/// Returns all events for a specific quail
pub fn get_events_for_quail(
    conn: &Connection,
    quail_uuid: &Uuid,
) -> Result<Vec<QuailEvent>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT uuid, quail_id, event_type, event_date, notes
         FROM quail_events
         WHERE quail_id = ?1
         ORDER BY event_date DESC",
    )?;

    let events = stmt
        .query_map(params![quail_uuid.to_string()], |row| {
            QuailEvent::try_from(row)
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(events)
}

/// Returns the latest event for a quail
#[allow(dead_code)]
pub fn get_latest_event(
    conn: &Connection,
    quail_uuid: &Uuid,
) -> Result<Option<QuailEvent>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT uuid, quail_id, event_type, event_date, notes
         FROM quail_events
         WHERE quail_id = ?1
         ORDER BY event_date DESC
         LIMIT 1",
    )?;

    let event = stmt
        .query_row(params![quail_uuid.to_string()], |row| {
            QuailEvent::try_from(row)
        })
        .optional()?;

    Ok(event)
}

/// Returns the birth date of a quail (from the "born" event)
#[allow(dead_code)]
pub fn get_birth_date(conn: &Connection, quail_uuid: &Uuid) -> Result<Option<NaiveDate>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT event_date
         FROM quail_events
         WHERE quail_id = ?1 AND event_type = 'born'
         LIMIT 1",
    )?;

    let date_str: Option<String> = stmt
        .query_row(params![quail_uuid.to_string()], |row| row.get(0))
        .optional()?;

    if let Some(date_str) = &date_str {
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|_| AppError::Database(rusqlite::Error::InvalidQuery))?;
        Ok(Some(date))
    } else {
        Ok(None)
    }
}
/// Updates an existing event
#[allow(dead_code)]
pub fn update_event(
    conn: &Connection,
    event_uuid: &Uuid,
    notes: Option<String>,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE quail_events 
         SET notes = ?1
         WHERE uuid = ?2",
        params![notes, event_uuid.to_string()],
    )?;

    Ok(())
}

/// Deletes an event
pub async fn delete_event(conn: &Connection, event_uuid: &Uuid) -> Result<(), AppError> {
    let mut stmt = conn.prepare("SELECT quail_id, event_type FROM quail_events WHERE uuid = ?1")?;
    let (_quail_id, _event_type_str): (String, String) = stmt
        .query_row(params![event_uuid.to_string()], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    conn.execute(
        "DELETE FROM quail_events WHERE uuid = ?1",
        params![event_uuid.to_string()],
    )?;

    // Capture CRDT deletion
    crate::services::operation_capture::capture_event_delete(conn, &event_uuid.to_string()).await?;

    Ok(())
}

/// Gets a single event by UUID
pub fn get_event_by_id(
    conn: &Connection,
    event_uuid: &Uuid,
) -> Result<Option<QuailEvent>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT uuid, quail_id, event_type, event_date, notes FROM quail_events WHERE uuid = ?1",
    )?;
    let evt = stmt
        .query_row(params![event_uuid.to_string()], |row| {
            QuailEvent::try_from(row)
        })
        .optional()?;
    Ok(evt)
}

/// Full update of an event (type, date, notes)
pub async fn update_event_full(
    conn: &Connection,
    event_uuid: &Uuid,
    event_type: EventType,
    event_date: NaiveDate,
    notes: Option<String>,
) -> Result<(), AppError> {
    let existing = get_event_by_id(conn, event_uuid)?
        .ok_or_else(|| AppError::NotFound("Event not found".to_string()))?;
    let candidate = QuailEvent {
        uuid: existing.uuid,
        quail_id: existing.quail_id,
        event_type: event_type.clone(),
        event_date,
        notes: notes.clone(),
        photos: existing.photos,
    };
    candidate.validate()?;
    conn.execute(
        "UPDATE quail_events SET event_type = ?1, event_date = ?2, notes = ?3 WHERE uuid = ?4",
        params![
            event_type.as_str(),
            event_date.to_string(),
            &notes,
            event_uuid.to_string()
        ],
    )?;

    // Capture CRDT operations
    let event_id = event_uuid.to_string();
    crate::services::operation_capture::capture_event_update(
        conn,
        &event_id,
        "event_type",
        serde_json::Value::String(event_type.as_str().to_string()),
    )
    .await?;
    crate::services::operation_capture::capture_event_update(
        conn,
        &event_id,
        "event_date",
        serde_json::Value::String(event_date.to_string()),
    )
    .await?;
    if let Some(notes_text) = notes {
        crate::services::operation_capture::capture_event_update(
            conn,
            &event_id,
            "notes",
            serde_json::Value::String(notes_text),
        )
        .await?;
    }

    Ok(())
}
