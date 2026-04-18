use crate::aircraft::{AircraftCategory, AircraftStatus, FlightRuleType, SquawkMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioFile {
    pub scenario_id: String,
    pub name: String,
    pub student_position_type: crate::role::StudentPositionType,
    pub initial_active_runway: String,
    pub metar: String,
    #[serde(default)]
    pub aircraft: Vec<ScenarioAircraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioAircraft {
    pub aircraft_id: String,
    pub template: AircraftCategory,
    pub callsign: String,
    pub flight_rules: FlightRuleType,
    pub departure_airfield: String,
    pub destination_airfield: String,
    pub route: String,
    pub initial_spawn: String,
    pub initial_status: AircraftStatus,
    pub squawk_mode: SquawkMode,
    #[serde(default)]
    pub assigned_sid: Option<String>,
    #[serde(default)]
    pub assigned_runway: Option<String>,
    #[serde(default)]
    pub assigned_altitude_ft: Option<i32>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScenarioLoadError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

impl ScenarioFile {
    pub fn from_toml_str(s: &str) -> Result<Self, ScenarioLoadError> {
        Ok(toml::from_str(s)?)
    }

    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, ScenarioLoadError> {
        let s = std::fs::read_to_string(path)?;
        Self::from_toml_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_scenario() {
        let scenario =
            ScenarioFile::from_toml_str(include_str!("../../../docs/scenario-gnd-36-medium.toml"))
                .expect("scenario should parse");
        assert_eq!(scenario.scenario_id, "gnd_medium_36");
        assert_eq!(scenario.initial_active_runway, "36");
        assert_eq!(scenario.aircraft.len(), 6);
        assert!(scenario.aircraft.iter().any(|a| a.callsign == "BTI201"));
    }
}
