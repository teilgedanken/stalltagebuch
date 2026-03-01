use chrono::NaiveDate;
use rusqlite::types::Type;
use rusqlite::Row;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Event in the life of a quail (status change, birth, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuailEvent {
    pub uuid: Uuid,
    pub quail_id: Uuid,
    pub event_type: EventType,
    pub event_date: NaiveDate,
    pub notes: Option<String>,
    pub photos: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventType {
    Born,               // Birth
    Alive,              // Living (default state)
    Sick,               // Sick
    Healthy,            // Recovered/Healthy
    MarkedForSlaughter, // Marked for slaughter
    Slaughtered,        // Slaughtered
    Died,               // Died naturally
}

impl EventType {
    pub fn as_str(&self) -> &str {
        match self {
            EventType::Born => "born",
            EventType::Alive => "alive",
            EventType::Sick => "sick",
            EventType::Healthy => "healthy",
            EventType::MarkedForSlaughter => "marked_for_slaughter",
            EventType::Slaughtered => "slaughtered",
            EventType::Died => "died",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "born" | "geboren" => EventType::Born,
            "alive" | "am_leben" => EventType::Alive,
            "sick" | "krank" => EventType::Sick,
            "healthy" | "gesund" => EventType::Healthy,
            "marked_for_slaughter" | "markiert_zum_schlachten" => EventType::MarkedForSlaughter,
            "slaughtered" | "geschlachtet" => EventType::Slaughtered,
            "died" | "gestorben" => EventType::Died,
            _ => EventType::Alive,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            EventType::Born => "Geboren",
            EventType::Alive => "Am Leben",
            EventType::Sick => "Krank",
            EventType::Healthy => "Gesund",
            EventType::MarkedForSlaughter => "Markiert zum Schlachten",
            EventType::Slaughtered => "Geschlachtet",
            EventType::Died => "Gestorben",
        }
    }

    /// Returns true if this event type represents a final state (death)
    #[allow(dead_code)]
    pub fn is_final(&self) -> bool {
        matches!(self, EventType::Slaughtered | EventType::Died)
    }

    /// Returns true if this event type is a health-related status
    #[allow(dead_code)]
    pub fn is_health_status(&self) -> bool {
        matches!(self, EventType::Sick | EventType::Healthy)
    }
}

impl<'r> TryFrom<&Row<'r>> for QuailEvent {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'r>) -> Result<Self, Self::Error> {
        let uuid_str: String = row.get(0)?;
        let uuid = Uuid::parse_str(&uuid_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let quail_id_str: String = row.get(1)?;
        let quail_id = Uuid::parse_str(&quail_id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let event_type_str: String = row.get(2)?;
        let event_date_str: String = row.get(3)?;
        let notes: Option<String> = row.get(4)?;
        let photos_string: Option<String> = row.get(5)?;

        let event_date = NaiveDate::parse_from_str(&event_date_str, "%Y-%m-%d")
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, Type::Text, Box::new(e)))?;
        let photos: Option<Uuid> = photos_string.map(|s| Uuid::parse_str(&s).ok()).flatten();
        Ok(QuailEvent {
            uuid,
            quail_id,
            event_type: EventType::from_str(&event_type_str),
            event_date,
            notes,
            photos,
        })
    }
}
