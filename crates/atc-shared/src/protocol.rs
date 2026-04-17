use crate::aircraft::AircraftState;
use crate::ids::*;
use crate::role::{Role, StudentPositionType};
use crate::session::{SessionState, SessionStatus, SessionSummary};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

// ------------- HTTP DTOs -------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerLoginRequest {
    pub trainer_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerLoginResponse {
    pub trainer_token: TrainerToken,
    pub trainer_id: TrainerId,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionResponse {
    pub session: SessionSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStudentPositionRequest {
    pub position_type: StudentPositionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStudentPositionResponse {
    pub position_id: PositionId,
    pub position_type: StudentPositionType,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinStudentRequest {
    pub session_hash: SessionHash,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinStudentResponse {
    pub student_token: StudentToken,
    pub session_id: SessionId,
    pub position_id: PositionId,
    pub position_type: StudentPositionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartSessionRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartSessionResponse {
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}

// ------------- WebSocket envelope -------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEnvelope<T> {
    #[serde(rename = "type")]
    pub type_: String,
    pub message_id: String,
    pub session_id: SessionId,
    #[serde(with = "time::serde::rfc3339")]
    pub sent_at: OffsetDateTime,
    pub payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    Hello(HelloPayload),
    SessionState(Box<SessionState>),
    SessionUpdated(SessionUpdatedPayload),
    AircraftCreated(AircraftEventPayload),
    AircraftUpdated(AircraftEventPayload),
    AircraftRemoved(AircraftRemovedPayload),
    CommandRejected(CommandRejectedPayload),
    SystemNotice(SystemNoticePayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloPayload {
    pub connection_id: ConnectionId,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUpdatedPayload {
    pub status: Option<SessionStatus>,
    pub active_runway: Option<String>,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircraftEventPayload {
    pub aircraft: AircraftState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircraftRemovedPayload {
    pub aircraft_id: AircraftId,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRejectedPayload {
    pub correlation_id: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemNoticePayload {
    pub code: String,
    pub message: String,
}
