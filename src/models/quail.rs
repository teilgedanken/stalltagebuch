use rusqlite::Row;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quail {
    pub uuid: Uuid,
    pub name: String,
    pub gender: Gender,
    pub ring_color: Option<RingColor>,
    pub profile_photo: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RingColor {
    Lila,
    Rosa,
    Hellblau,
    Dunkelblau,
    Rot,
    Orange,
    Weiss, // Speicherung als weiss (ASCII) – Anzeige als Weiß
    Gelb,
    Schwarz,
    Gruen, // Speicherung als gruen (ASCII) – Anzeige als Grün
}

impl RingColor {
    pub fn as_str(&self) -> &str {
        match self {
            RingColor::Lila => "lila",
            RingColor::Rosa => "rosa",
            RingColor::Hellblau => "hellblau",
            RingColor::Dunkelblau => "dunkelblau",
            RingColor::Rot => "rot",
            RingColor::Orange => "orange",
            RingColor::Weiss => "weiss",
            RingColor::Gelb => "gelb",
            RingColor::Schwarz => "schwarz",
            RingColor::Gruen => "gruen",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "lila" => RingColor::Lila,
            "rosa" => RingColor::Rosa,
            "hellblau" => RingColor::Hellblau,
            "dunkelblau" => RingColor::Dunkelblau,
            "rot" => RingColor::Rot,
            "orange" => RingColor::Orange,
            "weiß" | "weiss" => RingColor::Weiss,
            "gelb" => RingColor::Gelb,
            "schwarz" => RingColor::Schwarz,
            "grün" | "gruen" => RingColor::Gruen,
            _ => RingColor::Lila, // Default fallback
        }
    }

    #[allow(dead_code)]
    pub fn display_name(&self) -> &str {
        match self {
            RingColor::Lila => "Lila",
            RingColor::Rosa => "Rosa",
            RingColor::Hellblau => "Hellblau",
            RingColor::Dunkelblau => "Dunkelblau",
            RingColor::Rot => "Rot",
            RingColor::Orange => "Orange",
            RingColor::Weiss => "Weiß",
            RingColor::Gelb => "Gelb",
            RingColor::Schwarz => "Schwarz",
            RingColor::Gruen => "Grün",
        }
    }

    #[allow(dead_code)]
    pub fn all() -> &'static [RingColor] {
        static ALL: [RingColor; 10] = [
            RingColor::Lila,
            RingColor::Rosa,
            RingColor::Hellblau,
            RingColor::Dunkelblau,
            RingColor::Rot,
            RingColor::Orange,
            RingColor::Weiss,
            RingColor::Gelb,
            RingColor::Schwarz,
            RingColor::Gruen,
        ];
        &ALL
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Gender {
    Male,
    Female,
    Unknown,
}

impl Gender {
    pub fn as_str(&self) -> &str {
        match self {
            Gender::Male => "male",
            Gender::Female => "female",
            Gender::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "male" => Gender::Male,
            "female" => Gender::Female,
            _ => Gender::Unknown,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Gender::Male => "Männlich",
            Gender::Female => "Weiblich",
            Gender::Unknown => "Unbekannt",
        }
    }
}

impl<'r> TryFrom<&Row<'r>> for Quail {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'r>) -> Result<Self, Self::Error> {
        let uuid_str: String = row.get(0)?;
        let uuid = Uuid::parse_str(&uuid_str).map_err(|_| rusqlite::Error::InvalidQuery)?;
        let name: String = row.get(1)?;
        let gender_str: String = row.get(2)?;
        let ring_color_opt: Option<String> = row.get(3)?;
        let profile_photo_str: Option<String> = row.get(4)?;
        let profile_photo = profile_photo_str
            .map(|s| Uuid::parse_str(&s).ok())
            .flatten();

        Ok(Quail {
            uuid,
            name,
            gender: Gender::from_str(&gender_str),
            ring_color: ring_color_opt.map(|s| RingColor::from_str(&s)),
            profile_photo,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gender_conversion() {
        assert_eq!(Gender::from_str("male"), Gender::Male);
        assert_eq!(Gender::from_str("female"), Gender::Female);
        assert_eq!(Gender::from_str("invalid"), Gender::Unknown);
    }
}
