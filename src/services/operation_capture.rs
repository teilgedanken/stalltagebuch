/// Captures local changes and converts them into CRDT operations
use crate::error::AppError;
use crate::services::{crdt_service, upload_service};
use rusqlite::Connection;

/// Captures CREATE operation for a new quail
pub async fn capture_quail_create(
    conn: &Connection,
    quail_id: &str,
    name: &str,
    gender: &str,
    ring_color: Option<&str>,
    profile_photo: Option<&str>,
) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    // Create clock and tick for each operation to ensure different logical_clock values
    let mut clock = crdt_service::HybridLogicalClock::new(device_id.clone());
    let mut operations = Vec::new();

    // Operation 1: name
    clock.tick();
    operations.push(crdt_service::Operation {
        op_id: ulid::Ulid::new().to_string(),
        entity_type: "quail".to_string(),
        entity_id: quail_id.to_string(),
        clock: clock.clone(),
        op: crdt_service::CrdtOp::LwwSet {
            field: "name".to_string(),
            value: serde_json::Value::String(name.to_string()),
        },
    });

    // Operation 2: gender
    clock.tick();
    operations.push(crdt_service::Operation {
        op_id: ulid::Ulid::new().to_string(),
        entity_type: "quail".to_string(),
        entity_id: quail_id.to_string(),
        clock: clock.clone(),
        op: crdt_service::CrdtOp::LwwSet {
            field: "gender".to_string(),
            value: serde_json::Value::String(gender.to_string()),
        },
    });

    if let Some(color) = ring_color {
        clock.tick();
        operations.push(crdt_service::Operation {
            op_id: ulid::Ulid::new().to_string(),
            entity_type: "quail".to_string(),
            entity_id: quail_id.to_string(),
            clock: clock.clone(),
            op: crdt_service::CrdtOp::LwwSet {
                field: "ring_color".to_string(),
                value: serde_json::Value::String(color.to_string()),
            },
        });
    }

    if let Some(photo) = profile_photo {
        clock.tick();
        operations.push(crdt_service::Operation {
            op_id: ulid::Ulid::new().to_string(),
            entity_type: "quail".to_string(),
            entity_id: quail_id.to_string(),
            clock: clock.clone(),
            op: crdt_service::CrdtOp::LwwSet {
                field: "profile_photo".to_string(),
                value: serde_json::Value::String(photo.to_string()),
            },
        });
    }

    upload_service::upload_ops_batch(conn, operations).await?;

    Ok(())
}

/// Captures UPDATE operation for a quail field
pub async fn capture_quail_update(
    conn: &Connection,
    quail_id: &str,
    field: &str,
    value: serde_json::Value,
) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    let op = crdt_service::Operation::new(
        "quail".to_string(),
        quail_id.to_string(),
        device_id,
        crdt_service::CrdtOp::LwwSet {
            field: field.to_string(),
            value,
        },
    );

    upload_service::upload_ops_batch(conn, vec![op]).await?;

    Ok(())
}

/// Captures DELETE operation for a quail
pub async fn capture_quail_delete(conn: &Connection, quail_id: &str) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    let op = crdt_service::Operation::new(
        "quail".to_string(),
        quail_id.to_string(),
        device_id,
        crdt_service::CrdtOp::Delete,
    );

    upload_service::upload_ops_batch(conn, vec![op]).await?;

    Ok(())
}

/// Captures CREATE operation for a new event
pub async fn capture_event_create(
    conn: &Connection,
    event_id: &str,
    quail_id: &str,
    event_type: &str,
    event_date: &str,
    notes: Option<&str>,
) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    // Create clock and tick for each operation to ensure different logical_clock values
    let mut clock = crdt_service::HybridLogicalClock::new(device_id.clone());
    let mut operations = Vec::new();

    // Operation 1: quail_id
    clock.tick();
    operations.push(crdt_service::Operation {
        op_id: ulid::Ulid::new().to_string(),
        entity_type: "event".to_string(),
        entity_id: event_id.to_string(),
        clock: clock.clone(),
        op: crdt_service::CrdtOp::LwwSet {
            field: "quail_id".to_string(),
            value: serde_json::Value::String(quail_id.to_string()),
        },
    });

    // Operation 2: event_type
    clock.tick();
    operations.push(crdt_service::Operation {
        op_id: ulid::Ulid::new().to_string(),
        entity_type: "event".to_string(),
        entity_id: event_id.to_string(),
        clock: clock.clone(),
        op: crdt_service::CrdtOp::LwwSet {
            field: "event_type".to_string(),
            value: serde_json::Value::String(event_type.to_string()),
        },
    });

    // Operation 3: event_date
    clock.tick();
    operations.push(crdt_service::Operation {
        op_id: ulid::Ulid::new().to_string(),
        entity_type: "event".to_string(),
        entity_id: event_id.to_string(),
        clock: clock.clone(),
        op: crdt_service::CrdtOp::LwwSet {
            field: "event_date".to_string(),
            value: serde_json::Value::String(event_date.to_string()),
        },
    });

    if let Some(notes_text) = notes {
        clock.tick();
        operations.push(crdt_service::Operation {
            op_id: ulid::Ulid::new().to_string(),
            entity_type: "event".to_string(),
            entity_id: event_id.to_string(),
            clock: clock.clone(),
            op: crdt_service::CrdtOp::LwwSet {
                field: "notes".to_string(),
                value: serde_json::Value::String(notes_text.to_string()),
            },
        });
    }

    upload_service::upload_ops_batch(conn, operations).await?;

    Ok(())
}

/// Captures UPDATE operation for an event field
pub async fn capture_event_update(
    conn: &Connection,
    event_id: &str,
    field: &str,
    value: serde_json::Value,
) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    let op = crdt_service::Operation::new(
        "event".to_string(),
        event_id.to_string(),
        device_id,
        crdt_service::CrdtOp::LwwSet {
            field: field.to_string(),
            value,
        },
    );

    upload_service::upload_ops_batch(conn, vec![op]).await?;

    Ok(())
}

/// Captures DELETE operation for an event
pub async fn capture_event_delete(conn: &Connection, event_id: &str) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    let op = crdt_service::Operation::new(
        "event".to_string(),
        event_id.to_string(),
        device_id,
        crdt_service::CrdtOp::Delete,
    );

    upload_service::upload_ops_batch(conn, vec![op]).await?;

    Ok(())
}

/// Captures CREATE operation for a new photo
pub async fn capture_photo_create(
    conn: &Connection,
    photo_id: &str,
    quail_id: Option<&str>,
    event_id: Option<&str>,
    relative_path: &str,
    relative_thumb: Option<&str>,
) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    // Create clock and tick for each operation to ensure different logical_clock values
    let mut clock = crdt_service::HybridLogicalClock::new(device_id.clone());
    let mut operations = Vec::new();

    // Operation 1: relative_path
    clock.tick();
    operations.push(crdt_service::Operation {
        op_id: ulid::Ulid::new().to_string(),
        entity_type: "photo".to_string(),
        entity_id: photo_id.to_string(),
        clock: clock.clone(),
        op: crdt_service::CrdtOp::LwwSet {
            field: "relative_path".to_string(),
            value: serde_json::Value::String(relative_path.to_string()),
        },
    });

    if let Some(qid) = quail_id {
        clock.tick();
        operations.push(crdt_service::Operation {
            op_id: ulid::Ulid::new().to_string(),
            entity_type: "photo".to_string(),
            entity_id: photo_id.to_string(),
            clock: clock.clone(),
            op: crdt_service::CrdtOp::LwwSet {
                field: "quail_id".to_string(),
                value: serde_json::Value::String(qid.to_string()),
            },
        });
    }

    if let Some(eid) = event_id {
        clock.tick();
        operations.push(crdt_service::Operation {
            op_id: ulid::Ulid::new().to_string(),
            entity_type: "photo".to_string(),
            entity_id: photo_id.to_string(),
            clock: clock.clone(),
            op: crdt_service::CrdtOp::LwwSet {
                field: "event_id".to_string(),
                value: serde_json::Value::String(eid.to_string()),
            },
        });
    }

    if let Some(thumb) = relative_thumb {
        clock.tick();
        operations.push(crdt_service::Operation {
            op_id: ulid::Ulid::new().to_string(),
            entity_type: "photo".to_string(),
            entity_id: photo_id.to_string(),
            clock: clock.clone(),
            op: crdt_service::CrdtOp::LwwSet {
                field: "relative_thumb".to_string(),
                value: serde_json::Value::String(thumb.to_string()),
            },
        });
    }

    upload_service::upload_ops_batch(conn, operations).await?;

    Ok(())
}

/// Captures DELETE operation for a photo
pub async fn capture_photo_delete(conn: &Connection, photo_id: &str) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    let op = crdt_service::Operation::new(
        "photo".to_string(),
        photo_id.to_string(),
        device_id,
        crdt_service::CrdtOp::Delete,
    );

    upload_service::upload_ops_batch(conn, vec![op]).await?;

    Ok(())
}

/// Captures CREATE operation for a new egg record
pub async fn capture_egg_create(
    conn: &Connection,
    egg_id: &str,
    date: &str,
    count: i32,
) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    // Shared HLC für beide Operationen, damit sie unterschiedliche logical_counter haben
    let mut clock = crate::services::crdt_service::HybridLogicalClock::new(device_id.clone());
    clock.tick();
    let op1_clock = clock.clone();
    clock.tick();
    let op2_clock = clock.clone();

    let operations = vec![
        crdt_service::Operation {
            op_id: ulid::Ulid::new().to_string(),
            entity_type: "egg".to_string(),
            entity_id: egg_id.to_string(),
            clock: op1_clock,
            op: crdt_service::CrdtOp::LwwSet {
                field: "record_date".to_string(),
                value: serde_json::Value::String(date.to_string()),
            },
        },
        crdt_service::Operation {
            op_id: ulid::Ulid::new().to_string(),
            entity_type: "egg".to_string(),
            entity_id: egg_id.to_string(),
            clock: op2_clock,
            op: crdt_service::CrdtOp::LwwSet {
                field: "total_eggs".to_string(),
                value: serde_json::Value::Number(count.into()),
            },
        },
    ];

    upload_service::upload_ops_batch(conn, operations).await?;

    Ok(())
}

/// Captures UPDATE operation for an egg record
pub async fn capture_egg_update(
    conn: &Connection,
    egg_id: &str,
    count: i32,
) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    let op = crdt_service::Operation::new(
        "egg".to_string(),
        egg_id.to_string(),
        device_id,
        crdt_service::CrdtOp::LwwSet {
            field: "total_eggs".to_string(),
            value: serde_json::Value::Number(count.into()),
        },
    );

    upload_service::upload_ops_batch(conn, vec![op]).await?;

    Ok(())
}

/// Captures DELETE operation for an egg record
pub async fn capture_egg_delete(conn: &Connection, egg_id: &str) -> Result<(), AppError> {
    let device_id = upload_service::get_device_id(conn)?;

    let op = crdt_service::Operation::new(
        "egg".to_string(),
        egg_id.to_string(),
        device_id,
        crdt_service::CrdtOp::Delete,
    );

    upload_service::upload_ops_batch(conn, vec![op]).await?;

    Ok(())
}
