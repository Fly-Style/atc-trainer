use crate::aircraft::AircraftState;
use crate::ids::*;
use crate::role::{StudentPositionType, TrainerKind};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Draft,
    WaitingForStudent,
    Running,
    Paused,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudentPosition {
    pub position_id: PositionId,
    pub position_type: StudentPositionType,
    pub occupied: bool,
    pub connection_state: ConnectionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerInfo {
    pub trainer_id: TrainerId,
    pub trainer_kind: TrainerKind,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub name: String,
    pub session_hash: SessionHash,
    pub status: SessionStatus,
    pub student_position_type: Option<StudentPositionType>,
    pub student_connected: bool,
    pub lead_trainer_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: SessionId,
    pub name: String,
    pub session_hash: SessionHash,
    pub status: SessionStatus,
    pub sector_id: SectorId,
    pub student_position: Option<StudentPosition>,
    pub trainers: Vec<TrainerInfo>,
    pub aircraft: Vec<AircraftState>,
    pub active_runway: String,
    pub metar: String,
    #[serde(with = "time::serde::rfc3339")]
    pub server_time: OffsetDateTime,
    pub revision: u64,
}
