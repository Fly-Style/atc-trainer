use crate::ids::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AircraftCategory {
    VfrC172,
    IfrA320,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlightRuleType {
    Vfr,
    Ifr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AircraftStatus {
    New,
    Cleared,
    Push,
    Startup,
    Taxi,
    OnRunway,
    Airborne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementMode {
    Parked,
    Pushback,
    Taxiing,
    Holding,
    Lineup,
    TakeoffRoll,
    Airborne,
    LandingRoll,
    Vacating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SquawkMode {
    Off,
    Standby,
    Charlie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlledByPosition {
    Gnd,
    Twr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AircraftOrigin {
    Scenario,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathPoint {
    pub seq: u32,
    pub x_nm: f32,
    pub y_nm: f32,
    pub target_speed_kt: f32,
    pub target_altitude: TargetAltitude,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TargetAltitude {
    Gnd,
    MslFt { value_ft: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlightPath {
    pub points: Vec<PathPoint>,
    pub launched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircraftState {
    pub aircraft_id: AircraftId,
    pub origin: AircraftOrigin,
    pub callsign: String,
    pub departure_airfield: String,
    pub destination_airfield: String,
    pub category: AircraftCategory,
    pub aircraft_type: String,
    pub flight_rules: FlightRuleType,
    pub route: String,
    pub status: AircraftStatus,
    pub movement_mode: MovementMode,
    pub controlled_by_position: ControlledByPosition,
    pub squawk_mode: SquawkMode,
    pub assigned_runway: Option<String>,
    pub assigned_sid: Option<String>,
    pub assigned_squawk: Option<String>,
    pub assigned_altitude_ft: Option<i32>,
    pub current_node: Option<String>,
    pub target_node: Option<String>,
    pub scenario_placement: Option<String>,
    pub next_waypoint: Option<String>,
    pub ground_speed_kt: f32,
    pub air_speed_kt: Option<f32>,
    pub altitude_ft: f32,
    pub x_nm: f32,
    pub y_nm: f32,
    pub trainer_profile: Option<String>,
    pub assumed_by_student: bool,
    pub handed_off: bool,
    pub draft_path: Option<FlightPath>,
    pub active_path: Option<FlightPath>,
    pub path_run_id: u64,
    pub revision: u64,
}
