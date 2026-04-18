//! Command pattern: every UI gesture is dispatched as an [`AppCommand`].
//!
//! [`process`] is a pure function: it mutates [`ClientState`] for view-only
//! changes (zoom, pan, focus) and returns a list of [`SideEffect`]s that the
//! runtime is responsible for executing (HTTP calls, opening a WebSocket).
//! Tests can drive commands and assert on both the new state and the side
//! effects without performing any real I/O.

use crate::core::state::{AuthState, ClientState};
use atc_shared::aircraft::{AircraftCategory, AircraftStatus, FlightPath, SquawkMode};
use atc_shared::ids::{AircraftId, SectorId, SessionId};
use atc_shared::protocol::{
    ClientEvent, StudentAircraftChanges, StudentCommand, StudentCommandEnvelope, TrainerCommand,
    TrainerCommandEnvelope,
};
use atc_shared::role::StudentPositionType;

#[derive(Debug, Clone)]
pub enum AppCommand {
    // Auth + session management.
    SetServerProfile { http_base: String, ws_base: String },
    LoginAsTrainer { hash: String },
    CreateSession { name: String },
    CreateStudentPosition { session_id: SessionId, position_type: StudentPositionType },
    JoinAsStudent { session_hash: String },
    StartSession { session_id: SessionId },
    LoadSector { sector_id: SectorId },

    // WebSocket.
    OpenWebSocket { session_id: SessionId },
    Disconnect,

    // View-only.
    ZoomIn,
    ZoomOut,
    PanByNm { dx: f32, dy: f32 },
    FocusAircraft(AircraftId),
    ClearFocus,

    // Student operational workflow.
    AssumeAircraft { aircraft_id: AircraftId },
    UpdateAircraft {
        aircraft_id: AircraftId,
        changes: StudentAircraftChanges,
    },
    HandoffAircraft { aircraft_id: AircraftId },

    // Trainer operational workflow.
    SpawnAircraft {
        template: AircraftCategory,
        callsign: String,
        squawk_mode: SquawkMode,
        initial_x_nm: f32,
        initial_y_nm: f32,
        initial_state: AircraftStatus,
        initial_path: Option<FlightPath>,
    },
    RemoveAircraft { aircraft_id: AircraftId },
    SetActiveRunway { runway: String },
    AssignRunwayInWork { aircraft_id: AircraftId, runway: Option<String> },
    SetSpeed { aircraft_id: AircraftId, target_speed_kt: f32 },
    SetPath { aircraft_id: AircraftId, path: FlightPath },
    LaunchPath { aircraft_id: AircraftId },
    StopGroundAircraft { aircraft_id: AircraftId },
    TriggerGoAround { aircraft_id: AircraftId },
    TriggerRejectedTakeoff { aircraft_id: AircraftId },
    PauseSession,
    ResumeSession,

    // Misc.
    ClearError,
}

/// What kind of HTTP request the runtime should perform, and how to map the
/// response back into the state machine. The runtime crate (UI layer) wires
/// each correlation to the appropriate `core::http` helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpCorrelation {
    TrainerLogin,
    CreateSession,
    CreateStudentPosition { session_id: SessionId },
    JoinStudent,
    StartSession { session_id: SessionId },
    LoadSector { sector_id: SectorId },
}

#[derive(Debug, Clone)]
pub struct HttpRequestSpec {
    pub correlation: HttpCorrelation,
    /// Body to JSON-serialize. Stored as `serde_json::Value` so the spec is
    /// transport-agnostic and easy to assert against in tests.
    pub body: serde_json::Value,
    /// Whether to send the current bearer token (if any).
    pub with_auth: bool,
}

#[derive(Debug, Clone)]
pub enum SideEffect {
    Http(HttpRequestSpec),
    OpenWebSocket { token: String, session_id: SessionId },
    CloseWebSocket,
    SendWs(ClientEvent),
    ReportError(String),
}

/// Apply a command to the client state and emit side effects for the runtime.
pub fn process(state: &mut ClientState, command: AppCommand) -> Vec<SideEffect> {
    match command {
        AppCommand::SetServerProfile { http_base, ws_base } => {
            state.server.http_base = http_base;
            state.server.ws_base = ws_base;
            vec![]
        }
        AppCommand::LoginAsTrainer { hash } => vec![SideEffect::Http(HttpRequestSpec {
            correlation: HttpCorrelation::TrainerLogin,
            body: serde_json::json!({ "trainer_hash": hash }),
            with_auth: false,
        })],
        AppCommand::CreateSession { name } => match state.auth {
            AuthState::Trainer { .. } => vec![SideEffect::Http(HttpRequestSpec {
                correlation: HttpCorrelation::CreateSession,
                body: serde_json::json!({ "name": name }),
                with_auth: true,
            })],
            _ => vec![SideEffect::ReportError("trainer login required".into())],
        },
        AppCommand::CreateStudentPosition { session_id, position_type } => {
            vec![SideEffect::Http(HttpRequestSpec {
                correlation: HttpCorrelation::CreateStudentPosition { session_id },
                body: serde_json::json!({ "position_type": position_type }),
                with_auth: true,
            })]
        }
        AppCommand::JoinAsStudent { session_hash } => vec![SideEffect::Http(HttpRequestSpec {
            correlation: HttpCorrelation::JoinStudent,
            body: serde_json::json!({ "session_hash": session_hash }),
            with_auth: false,
        })],
        AppCommand::StartSession { session_id } => vec![SideEffect::Http(HttpRequestSpec {
            correlation: HttpCorrelation::StartSession { session_id },
            body: serde_json::json!({}),
            with_auth: true,
        })],
        AppCommand::LoadSector { sector_id } => vec![SideEffect::Http(HttpRequestSpec {
            correlation: HttpCorrelation::LoadSector { sector_id },
            body: serde_json::Value::Null,
            with_auth: true,
        })],
        AppCommand::OpenWebSocket { session_id } => match state.auth.token() {
            Some(token) => vec![SideEffect::OpenWebSocket {
                token: token.to_string(),
                session_id,
            }],
            None => vec![SideEffect::ReportError("no auth token".into())],
        },
        AppCommand::Disconnect => {
            state.session = None;
            state.view = Default::default();
            vec![SideEffect::CloseWebSocket]
        }
        AppCommand::ZoomIn => {
            state.view.zoom_in();
            vec![]
        }
        AppCommand::ZoomOut => {
            state.view.zoom_out();
            vec![]
        }
        AppCommand::PanByNm { dx, dy } => {
            state.view.pan_by_nm(dx, dy);
            vec![]
        }
        AppCommand::FocusAircraft(id) => {
            state.view.focused_aircraft = Some(id);
            vec![]
        }
        AppCommand::ClearFocus => {
            state.view.focused_aircraft = None;
            vec![]
        }
        AppCommand::AssumeAircraft { aircraft_id } => vec![SideEffect::SendWs(ClientEvent::StudentCommand(
            StudentCommandEnvelope {
                correlation_id: correlation_id(),
                command: StudentCommand::AssumeAircraft { aircraft_id },
            },
        ))],
        AppCommand::UpdateAircraft { aircraft_id, changes } => vec![SideEffect::SendWs(
            ClientEvent::StudentCommand(StudentCommandEnvelope {
                correlation_id: correlation_id(),
                command: StudentCommand::UpdateAircraft { aircraft_id, changes },
            }),
        )],
        AppCommand::HandoffAircraft { aircraft_id } => vec![SideEffect::SendWs(
            ClientEvent::StudentCommand(StudentCommandEnvelope {
                correlation_id: correlation_id(),
                command: StudentCommand::HandoffAircraft { aircraft_id },
            }),
        )],
        AppCommand::SpawnAircraft {
            template,
            callsign,
            squawk_mode,
            initial_x_nm,
            initial_y_nm,
            initial_state,
            initial_path,
        } => vec![SideEffect::SendWs(ClientEvent::TrainerCommand(TrainerCommandEnvelope {
            correlation_id: correlation_id(),
            command: TrainerCommand::SpawnAircraft {
                template,
                callsign,
                squawk_mode,
                initial_x_nm,
                initial_y_nm,
                initial_state,
                initial_path,
            },
        }))],
        AppCommand::RemoveAircraft { aircraft_id } => vec![SideEffect::SendWs(
            ClientEvent::TrainerCommand(TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::RemoveAircraft { aircraft_id },
            }),
        )],
        AppCommand::SetActiveRunway { runway } => vec![SideEffect::SendWs(
            ClientEvent::TrainerCommand(TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::SetActiveRunway { runway },
            }),
        )],
        AppCommand::AssignRunwayInWork { aircraft_id, runway } => vec![SideEffect::SendWs(
            ClientEvent::TrainerCommand(TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::AssignRunwayInWork { aircraft_id, runway },
            }),
        )],
        AppCommand::SetSpeed {
            aircraft_id,
            target_speed_kt,
        } => vec![SideEffect::SendWs(ClientEvent::TrainerCommand(TrainerCommandEnvelope {
            correlation_id: correlation_id(),
            command: TrainerCommand::SetSpeed {
                aircraft_id,
                target_speed_kt,
            },
        }))],
        AppCommand::SetPath { aircraft_id, path } => vec![SideEffect::SendWs(
            ClientEvent::TrainerCommand(TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::SetPath { aircraft_id, path },
            }),
        )],
        AppCommand::LaunchPath { aircraft_id } => vec![SideEffect::SendWs(
            ClientEvent::TrainerCommand(TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::LaunchPath { aircraft_id },
            }),
        )],
        AppCommand::StopGroundAircraft { aircraft_id } => vec![SideEffect::SendWs(
            ClientEvent::TrainerCommand(TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::StopGroundAircraft { aircraft_id },
            }),
        )],
        AppCommand::TriggerGoAround { aircraft_id } => vec![SideEffect::SendWs(
            ClientEvent::TrainerCommand(TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::TriggerGoAround { aircraft_id },
            }),
        )],
        AppCommand::TriggerRejectedTakeoff { aircraft_id } => vec![SideEffect::SendWs(
            ClientEvent::TrainerCommand(TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::TriggerRejectedTakeoff { aircraft_id },
            }),
        )],
        AppCommand::PauseSession => vec![SideEffect::SendWs(ClientEvent::TrainerCommand(
            TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::PauseSession,
            },
        ))],
        AppCommand::ResumeSession => vec![SideEffect::SendWs(ClientEvent::TrainerCommand(
            TrainerCommandEnvelope {
                correlation_id: correlation_id(),
                command: TrainerCommand::ResumeSession,
            },
        ))],
        AppCommand::ClearError => {
            state.last_error = None;
            vec![]
        }
    }
}

fn correlation_id() -> String {
    format!("cli_{}", uuid::Uuid::new_v4().simple())
}
