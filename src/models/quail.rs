use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quail {
    pub uuid: Uuid,
    pub name: String,
    pub gender: Gender,
    pub ring_color_left: Option<RingColor>,
    pub ring_color_right: Option<RingColor>,
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

pub fn normalize_ring_color_code(value: &str) -> Option<String> {
    match value.trim().to_lowercase().as_str() {
        "" => None,
        "lila" => Some("lila".to_string()),
        "rosa" => Some("rosa".to_string()),
        "hellblau" => Some("hellblau".to_string()),
        "dunkelblau" => Some("dunkelblau".to_string()),
        "rot" => Some("rot".to_string()),
        "orange" => Some("orange".to_string()),
        "weiß" | "weiss" => Some("weiss".to_string()),
        "gelb" => Some("gelb".to_string()),
        "schwarz" => Some("schwarz".to_string()),
        "grün" | "gruen" => Some("gruen".to_string()),
        other => Some(other.to_string()),
    }
}

fn normalize_ring_color_option(value: Option<&str>) -> Option<String> {
    value.and_then(normalize_ring_color_code)
}

pub fn ring_color_combination_conflicts(
    candidate_left: Option<&str>,
    candidate_right: Option<&str>,
    existing_left: Option<&str>,
    existing_right: Option<&str>,
) -> bool {
    let candidate_left = normalize_ring_color_option(candidate_left);
    let candidate_right = normalize_ring_color_option(candidate_right);

    if candidate_left.is_none() && candidate_right.is_none() {
        return false;
    }

    candidate_left == normalize_ring_color_option(existing_left)
        && candidate_right == normalize_ring_color_option(existing_right)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gender_conversion() {
        assert_eq!(Gender::from_str("male"), Gender::Male);
        assert_eq!(Gender::from_str("female"), Gender::Female);
        assert_eq!(Gender::from_str("invalid"), Gender::Unknown);
    }

    #[test]
    fn test_ring_color_combination_allows_unringed_duplicates() {
        assert!(!ring_color_combination_conflicts(None, None, None, None));
        assert!(!ring_color_combination_conflicts(
            None,
            None,
            Some("rot"),
            None
        ));
    }

    #[test]
    fn test_ring_color_combination_is_side_specific() {
        assert!(ring_color_combination_conflicts(
            Some("hellblau"),
            Some("rot"),
            Some("hellblau"),
            Some("rot")
        ));
        assert!(!ring_color_combination_conflicts(
            Some("hellblau"),
            Some("rot"),
            Some("rot"),
            Some("hellblau")
        ));
    }

    #[test]
    fn test_ring_color_combination_distinguishes_partial_pairs() {
        assert!(ring_color_combination_conflicts(
            Some("hellblau"),
            None,
            Some("hellblau"),
            None
        ));
        assert!(!ring_color_combination_conflicts(
            Some("hellblau"),
            None,
            Some("hellblau"),
            Some("rot")
        ));
    }

    #[test]
    fn test_ring_color_normalization_uses_canonical_ascii_codes() {
        assert_eq!(
            normalize_ring_color_code(" Weiß "),
            Some("weiss".to_string())
        );
        assert_eq!(normalize_ring_color_code("grün"), Some("gruen".to_string()));
    }
}
