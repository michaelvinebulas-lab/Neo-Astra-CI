//! Language edition selected by a project manifest or compiler request.
//!
//! Editions select syntax behavior. The `1.1.0` cleanup introduces the `0.2`
//! edition; the default remains `0.1` so every existing source and caller keeps
//! its current behavior.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Language edition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edition {
    /// The v0.1 language. This is the default.
    #[default]
    #[serde(rename = "0.1")]
    V0_1,
    /// The cleaned v0.2 language selected by `edition = "0.2"`.
    #[serde(rename = "0.2")]
    V0_2,
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Edition::V0_1 => f.write_str("0.1"),
            Edition::V0_2 => f.write_str("0.2"),
        }
    }
}

impl FromStr for Edition {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "0.1" => Ok(Edition::V0_1),
            "0.2" => Ok(Edition::V0_2),
            other => Err(format!("unsupported edition `{other}`; expected `0.1` or `0.2`")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Edition;
    use std::str::FromStr;

    #[test]
    fn edition_defaults_to_v0_1() {
        assert_eq!(Edition::default(), Edition::V0_1);
    }

    #[test]
    fn edition_parses_both_values() {
        assert_eq!(Edition::from_str("0.1"), Ok(Edition::V0_1));
        assert_eq!(Edition::from_str("0.2"), Ok(Edition::V0_2));
    }

    #[test]
    fn edition_rejects_unknown_values() {
        let error = Edition::from_str("2026").unwrap_err();
        assert!(error.contains("expected `0.1` or `0.2`"), "{error}");
    }

    #[test]
    fn edition_displays_as_manifest_value() {
        assert_eq!(Edition::V0_1.to_string(), "0.1");
        assert_eq!(Edition::V0_2.to_string(), "0.2");
    }
}
