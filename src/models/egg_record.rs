use crate::error::AppError;
use chrono::NaiveDate;
use rusqlite::Row;
use rusqlite::types::Type;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EggRecord {
    pub uuid: Uuid,
    pub record_date: NaiveDate,
    pub total_eggs: i32,
    pub notes: Option<String>,
}

impl EggRecord {
    /// Creates a new egg record
    #[allow(dead_code)]
    pub fn new(record_date: NaiveDate, total_eggs: i32) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            record_date,
            total_eggs,
            notes: None,
        }
    }

    /// Validates the egg record
    #[allow(dead_code)]
    pub fn validate(&self) -> Result<(), AppError> {
        // Count must not be negative
        if self.total_eggs < 0 {
            return Err(AppError::Validation(
                "Egg count must not be negative".to_string(),
            ));
        }

        // Realistic upper limit (e.g. max 100 eggs per day)
        if self.total_eggs > 100 {
            return Err(AppError::Validation(
                "Egg count seems unrealistically high".to_string(),
            ));
        }

        // Date must not be in the future
        let today = chrono::Local::now().date_naive();
        if self.record_date > today {
            return Err(AppError::Validation(
                "Date must not be in the future".to_string(),
            ));
        }

        Ok(())
    }
}

impl<'r> TryFrom<&Row<'r>> for EggRecord {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'r>) -> Result<Self, Self::Error> {
        let uuid_str: String = row.get(0)?;
        let uuid = Uuid::parse_str(&uuid_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let date_str: String = row.get(1)?;
        let total_eggs: i32 = row.get(2)?;
        let notes: Option<String> = row.get(3)?;

        let record_date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, Type::Text, Box::new(e)))?;

        Ok(EggRecord {
            uuid,
            record_date,
            total_eggs,
            notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_egg_record() {
        let date = NaiveDate::from_ymd_opt(2025, 11, 9).unwrap();
        let record = EggRecord::new(date, 5);
        assert_eq!(record.total_eggs, 5);
        assert_eq!(record.record_date, date);
    }

    #[test]
    fn test_validate_negative_eggs() {
        let date = NaiveDate::from_ymd_opt(2025, 11, 9).unwrap();
        let mut record = EggRecord::new(date, 5);
        record.total_eggs = -1;
        assert!(record.validate().is_err());
    }

    #[test]
    fn test_validate_too_many_eggs() {
        let date = NaiveDate::from_ymd_opt(2025, 11, 9).unwrap();
        let record = EggRecord::new(date, 150);
        assert!(record.validate().is_err());
    }
}
