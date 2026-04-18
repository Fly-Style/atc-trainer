//! Top-level iced application: holds [`ClientState`], runs the command
//! dispatcher, performs HTTP via [`HttpClient`], and forwards WebSocket
//! events through a per-session [`Subscription`].

use crate::core::command::{self, AppCommand, HttpRequestSpec, SideEffect};
use crate::core::http::{HttpClient, HttpError, HttpOutcome};
use crate::core::state::{AuthState, CachedSession, ClientState};
use crate::core::ws::{self, WsClientError, WsCommandSender};
use crate::ui::session_screen;
use crate::ui::start_screen;
use atc_shared::aircraft::{AircraftCategory, AircraftState, AircraftStatus, FlightPath, PathPoint, SquawkMode, TargetAltitude};
use atc_shared::ids::{SectorId, SessionId};
use atc_shared::protocol::{ServerEvent, StudentAircraftChanges};
use iced::futures::SinkExt;
use iced::{stream, Element, Subscription, Task};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Message {
    Command(AppCommand),
    RequestConfirmation(ConfirmationKind),
    ConfirmAction,
    CancelConfirmation,
    HttpDone(Result<HttpOutcome, String>),
    WsEvent(Box<ServerEvent>),
    WsReady(WsCommandSender),
    WsDisconnected(String),
    StartFieldChanged(StartField, String),
    SessionFieldChanged(SessionField, String),
    TrainerFieldChanged(TrainerField, String),
    BeginDraftPath,
    CanvasDraftPointAdded { x_nm: f32, y_nm: f32 },
    UndoDraftPoint,
    FinishPathing,
    ClearDraftPath,
    DraftPointFieldChanged {
        index: usize,
        field: DraftPointField,
        value: String,
    },
    ZoomAtCursor {
        screen_x: f32,
        screen_y: f32,
        canvas_width: f32,
        canvas_height: f32,
        zoom_in: bool,
    },
}

#[derive(Debug, Clone)]
pub enum ConfirmationKind {
    RemoveAircraft { aircraft_id: atc_shared::ids::AircraftId },
}

#[derive(Debug, Clone, Copy)]
pub enum StartField {
    HttpBase,
    WsBase,
    TrainerHash,
    StudentHash,
    SessionName,
}

#[derive(Debug, Clone, Copy)]
pub enum SessionField {
    AssignedRunway,
    AssignedSid,
    AssignedSquawk,
    AssignedAltitude,
    Status,
}

#[derive(Debug, Clone, Copy)]
pub enum TrainerField {
    Callsign,
    Speed,
    SpawnX,
    SpawnY,
    Runway,
    Template,
    SquawkMode,
    InitialState,
}

#[derive(Debug, Clone, Copy)]
pub enum DraftPointField {
    Speed,
    Altitude,
}

#[derive(Default)]
pub struct StartFormState {
    pub trainer_hash: String,
    pub student_hash: String,
    pub session_name: String,
}

pub struct AtcApp {
    pub state: ClientState,
    pub form: StartFormState,
    pub ws_subscription_key: Option<(String, String)>,
    pub ws_commands: Option<WsCommandSender>,
    pub session_form: SessionFormState,
    pub trainer_form: TrainerFormState,
    pub pending_confirmation: Option<ConfirmationKind>,
}

impl Default for AtcApp {
    fn default() -> Self {
        Self {
            state: ClientState::default(),
            form: StartFormState {
                session_name: "Training".into(),
                ..Default::default()
            },
            ws_subscription_key: None,
            ws_commands: None,
            session_form: SessionFormState::default(),
            trainer_form: TrainerFormState::default(),
            pending_confirmation: None,
        }
    }
}

#[derive(Default)]
pub struct SessionFormState {
    pub assigned_runway: String,
    pub assigned_sid: String,
    pub assigned_squawk: String,
    pub assigned_altitude: String,
    pub status: String,
}

pub struct TrainerFormState {
    pub callsign: String,
    pub speed: String,
    pub spawn_x: String,
    pub spawn_y: String,
    pub runway: String,
    pub template: String,
    pub squawk_mode: String,
    pub initial_state: String,
    pub draft_points: Vec<PathPoint>,
    pub draft_mode_active: bool,
    pub path_geometry_finished: bool,
}

impl Default for TrainerFormState {
    fn default() -> Self {
        Self {
            callsign: String::new(),
            speed: String::new(),
            spawn_x: "0.0".into(),
            spawn_y: "0.0".into(),
            runway: "36".into(),
            template: "ifr_a320".into(),
            squawk_mode: "standby".into(),
            initial_state: "new".into(),
            draft_points: Vec::new(),
            draft_mode_active: false,
            path_geometry_finished: false,
        }
    }
}

impl AtcApp {
    pub fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    pub fn title(&self) -> String {
        "ATC Trainer".into()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RequestConfirmation(kind) => {
                self.pending_confirmation = Some(kind);
                Task::none()
            }
            Message::ConfirmAction => {
                let Some(kind) = self.pending_confirmation.take() else {
                    return Task::none();
                };
                match kind {
                    ConfirmationKind::RemoveAircraft { aircraft_id } => {
                        self.dispatch(AppCommand::RemoveAircraft { aircraft_id })
                    }
                }
            }
            Message::CancelConfirmation => {
                self.pending_confirmation = None;
                Task::none()
            }
            Message::StartFieldChanged(field, value) => {
                match field {
                    StartField::HttpBase => self.state.server.http_base = value,
                    StartField::WsBase => self.state.server.ws_base = value,
                    StartField::TrainerHash => self.form.trainer_hash = value,
                    StartField::StudentHash => self.form.student_hash = value,
                    StartField::SessionName => self.form.session_name = value,
                }
                Task::none()
            }
            Message::SessionFieldChanged(field, value) => {
                match field {
                    SessionField::AssignedRunway => self.session_form.assigned_runway = value,
                    SessionField::AssignedSid => self.session_form.assigned_sid = value,
                    SessionField::AssignedSquawk => self.session_form.assigned_squawk = value,
                    SessionField::AssignedAltitude => self.session_form.assigned_altitude = value,
                    SessionField::Status => self.session_form.status = value,
                }
                Task::none()
            }
            Message::TrainerFieldChanged(field, value) => {
                match field {
                    TrainerField::Callsign => self.trainer_form.callsign = value,
                    TrainerField::Speed => self.trainer_form.speed = value,
                    TrainerField::SpawnX => self.trainer_form.spawn_x = value,
                    TrainerField::SpawnY => self.trainer_form.spawn_y = value,
                    TrainerField::Runway => self.trainer_form.runway = value,
                    TrainerField::Template => self.trainer_form.template = value,
                    TrainerField::SquawkMode => self.trainer_form.squawk_mode = value,
                    TrainerField::InitialState => self.trainer_form.initial_state = value,
                }
                Task::none()
            }
            Message::BeginDraftPath => {
                self.trainer_form.draft_mode_active = true;
                self.trainer_form.path_geometry_finished = false;
                self.trainer_form.draft_points.clear();
                Task::none()
            }
            Message::CanvasDraftPointAdded { x_nm, y_nm } => {
                self.add_draft_point(x_nm, y_nm);
                Task::none()
            }
            Message::UndoDraftPoint => {
                self.trainer_form.draft_points.pop();
                Task::none()
            }
            Message::FinishPathing => {
                self.trainer_form.draft_mode_active = false;
                self.trainer_form.path_geometry_finished = true;
                Task::none()
            }
            Message::ClearDraftPath => {
                self.trainer_form.draft_points.clear();
                self.trainer_form.draft_mode_active = false;
                self.trainer_form.path_geometry_finished = false;
                Task::none()
            }
            Message::DraftPointFieldChanged { index, field, value } => {
                match field {
                    DraftPointField::Speed => {
                        if let Some(point) = self.trainer_form.draft_points.get_mut(index) {
                            point.target_speed_kt = value.trim().parse::<f32>().unwrap_or_default();
                        }
                    }
                    DraftPointField::Altitude => {
                        if let Some(point) = self.trainer_form.draft_points.get_mut(index) {
                            point.target_altitude = match value.trim() {
                                "" | "gnd" | "GND" => TargetAltitude::Gnd,
                                other => TargetAltitude::MslFt {
                                    value_ft: other.parse::<i32>().unwrap_or_default(),
                                },
                            };
                        }
                    }
                }
                Task::none()
            }
            Message::ZoomAtCursor {
                screen_x,
                screen_y,
                canvas_width,
                canvas_height,
                zoom_in,
            } => {
                let canvas_size = (canvas_width, canvas_height);
                let world_before = self
                    .state
                    .view
                    .screen_to_world((screen_x, screen_y), canvas_size);
                if zoom_in {
                    self.state.view.zoom_in();
                } else {
                    self.state.view.zoom_out();
                }
                let world_after = self
                    .state
                    .view
                    .screen_to_world((screen_x, screen_y), canvas_size);
                self.state
                    .view
                    .pan_by_nm(world_before.0 - world_after.0, world_before.1 - world_after.1);
                Task::none()
            }
            Message::Command(cmd) => {
                let effects = command::process(&mut self.state, cmd);
                self.run_effects(effects)
            }
            Message::HttpDone(Err(e)) => {
                self.state.last_error = Some(e);
                Task::none()
            }
            Message::HttpDone(Ok(outcome)) => self.apply_http_outcome(outcome),
            Message::WsEvent(ev) => {
                if let Some(session) = self.state.session.as_mut() {
                    session.apply_event(&ev);
                } else if let ServerEvent::SessionState(snap) = ev.as_ref() {
                    self.state.session = Some(CachedSession::from_snapshot((**snap).clone()));
                }
                self.sync_forms_from_focus();
                Task::none()
            }
            Message::WsReady(sender) => {
                self.ws_commands = Some(sender);
                Task::none()
            }
            Message::WsDisconnected(e) => {
                self.state.last_error = Some(format!("ws: {e}"));
                self.ws_subscription_key = None;
                self.ws_commands = None;
                Task::none()
            }
        }
    }

    fn run_effects(&mut self, effects: Vec<SideEffect>) -> Task<Message> {
        let mut tasks = Vec::new();
        for ef in effects {
            match ef {
                SideEffect::Http(spec) => {
                    let client = HttpClient::new(self.state.server.http_base.clone())
                        .with_bearer(self.state.auth.token().map(|s| s.to_string()));
                    tasks.push(Task::perform(perform_http(client, spec), Message::HttpDone));
                }
                SideEffect::OpenWebSocket { token, session_id } => {
                    self.ws_subscription_key = Some((token, session_id.as_str().to_string()));
                }
                SideEffect::CloseWebSocket => {
                    self.ws_subscription_key = None;
                    self.ws_commands = None;
                }
                SideEffect::SendWs(event) => {
                    if let Some(sender) = &self.ws_commands {
                        if let Err(e) = sender.send(event) {
                            self.state.last_error = Some(e.to_string());
                        }
                    } else {
                        self.state.last_error = Some("websocket not connected".into());
                    }
                }
                SideEffect::ReportError(e) => {
                    self.state.last_error = Some(e);
                }
            }
        }
        Task::batch(tasks)
    }

    fn apply_http_outcome(&mut self, outcome: HttpOutcome) -> Task<Message> {
        match outcome {
            HttpOutcome::TrainerLogin(r) => {
                self.state.auth = AuthState::Trainer {
                    token: r.trainer_token.as_str().to_string(),
                    trainer_id: r.trainer_id,
                };
                Task::none()
            }
            HttpOutcome::CreateSession(r) => {
                let sid = r.session.session_id.clone();
                // Auto-create a GND student position so the session can be started + joined.
                self.dispatch(AppCommand::CreateStudentPosition {
                    session_id: sid.clone(),
                    position_type: atc_shared::role::StudentPositionType::Gnd,
                })
                .chain(self.dispatch(AppCommand::LoadSector {
                    sector_id: SectorId::new("airport_01"),
                }))
                .chain(self.dispatch(AppCommand::OpenWebSocket { session_id: sid }))
            }
            HttpOutcome::CreateStudentPosition(_) => Task::none(),
            HttpOutcome::JoinStudent(r) => {
                self.state.auth = AuthState::Student {
                    token: r.student_token.as_str().to_string(),
                    session_id: r.session_id.clone(),
                };
                self.dispatch(AppCommand::LoadSector {
                    sector_id: SectorId::new("airport_01"),
                })
                .chain(self.dispatch(AppCommand::OpenWebSocket { session_id: r.session_id }))
            }
            HttpOutcome::StartSession(r) => {
                if let Some(s) = self.state.session.as_mut() {
                    s.state.status = r.status;
                }
                Task::none()
            }
            HttpOutcome::LoadSector(meta) => {
                self.state.sector = Some(meta);
                Task::none()
            }
        }
    }

    fn dispatch(&mut self, cmd: AppCommand) -> Task<Message> {
        let effects = command::process(&mut self.state, cmd);
        self.run_effects(effects)
    }

    pub fn view(&self) -> Element<'_, Message> {
        if self.state.session.is_some() && self.state.sector.is_some() {
            session_screen::view(self)
        } else {
            start_screen::view(self)
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let Some((token, session_id_str)) = &self.ws_subscription_key else {
            return Subscription::none();
        };
        let id = format!("ws-{}-{}", session_id_str, token);
        let token = Arc::new(token.clone());
        let session_id = Arc::new(SessionId::new(session_id_str.clone()));
        let ws_base = Arc::new(self.state.server.ws_base.clone());

        Subscription::run_with_id(id, stream::channel(64, move |mut output| {
            let token = token.clone();
            let session_id = session_id.clone();
            let ws_base = ws_base.clone();
            async move {
                match ws::open(&ws_base, &token, &session_id).await {
                    Ok(mut sess) => {
                        let _ = output.send(Message::WsReady(sess.commands.clone())).await;
                        while let Some(ev) = sess.events.recv().await {
                            if output.send(Message::WsEvent(Box::new(ev))).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(WsClientError::Connect(e))
                    | Err(WsClientError::Transport(e))
                    | Err(WsClientError::Decode(e)) => {
                        let _ = output.send(Message::WsDisconnected(e)).await;
                    }
                }
            }
        }))
    }
}

async fn perform_http(client: HttpClient, spec: HttpRequestSpec) -> Result<HttpOutcome, String> {
    client.perform(spec).await.map_err(|e: HttpError| e.to_string())
}

impl AtcApp {
    pub(crate) fn focused_aircraft(&self) -> Option<&AircraftState> {
        let focused = self.state.view.focused_aircraft.as_ref()?;
        self.state
            .session
            .as_ref()?
            .state
            .aircraft
            .iter()
            .find(|aircraft| &aircraft.aircraft_id == focused)
    }

    fn sync_forms_from_focus(&mut self) {
        if let Some(aircraft) = self.focused_aircraft().cloned() {
            self.session_form.assigned_runway = aircraft.assigned_runway.clone().unwrap_or_default();
            self.session_form.assigned_sid = aircraft.assigned_sid.unwrap_or_default();
            self.session_form.assigned_squawk = aircraft.assigned_squawk.unwrap_or_default();
            self.session_form.assigned_altitude =
                aircraft.assigned_altitude_ft.map(|value| value.to_string()).unwrap_or_default();
            self.session_form.status = format!("{:?}", aircraft.status).to_lowercase();
            self.trainer_form.speed = if aircraft.status == AircraftStatus::Airborne {
                aircraft.air_speed_kt.unwrap_or(aircraft.ground_speed_kt).round().to_string()
            } else {
                aircraft.ground_speed_kt.round().to_string()
            };
            self.trainer_form.runway = aircraft.assigned_runway.unwrap_or_default();
        }
    }

    fn add_draft_point(&mut self, x_nm: f32, y_nm: f32) {
        let seq = self.trainer_form.draft_points.len() as u32 + 1;
        self.trainer_form.draft_points.push(PathPoint {
            seq,
            x_nm,
            y_nm,
            target_speed_kt: 0.0,
            target_altitude: TargetAltitude::Gnd,
        });
    }

    pub fn build_student_changes(&self) -> StudentAircraftChanges {
        StudentAircraftChanges {
            status: parse_status(&self.session_form.status),
            assigned_runway: Some(parse_optional_string(&self.session_form.assigned_runway)),
            assigned_sid: Some(parse_optional_string(&self.session_form.assigned_sid)),
            assigned_squawk: Some(parse_optional_string(&self.session_form.assigned_squawk)),
            assigned_altitude_ft: Some(
                self.session_form
                    .assigned_altitude
                    .trim()
                    .parse::<i32>()
                    .ok(),
            ),
        }
    }

    pub fn build_spawn_template(&self) -> AircraftCategory {
        match self.trainer_form.template.as_str() {
            "vfr_c172" => AircraftCategory::VfrC172,
            _ => AircraftCategory::IfrA320,
        }
    }

    pub fn build_spawn_squawk_mode(&self) -> SquawkMode {
        match self.trainer_form.squawk_mode.as_str() {
            "off" => SquawkMode::Off,
            "charlie" => SquawkMode::Charlie,
            _ => SquawkMode::Standby,
        }
    }

    pub fn build_initial_state(&self) -> AircraftStatus {
        parse_status(&self.trainer_form.initial_state).unwrap_or(AircraftStatus::New)
    }

    pub fn build_draft_path(&self) -> FlightPath {
        FlightPath {
            points: self.trainer_form.draft_points.clone(),
            launched: false,
        }
    }
}

fn parse_status(value: &str) -> Option<AircraftStatus> {
    match value.trim().to_ascii_lowercase().as_str() {
        "new" => Some(AircraftStatus::New),
        "cleared" => Some(AircraftStatus::Cleared),
        "push" => Some(AircraftStatus::Push),
        "startup" => Some(AircraftStatus::Startup),
        "taxi" => Some(AircraftStatus::Taxi),
        "on_runway" | "on runway" => Some(AircraftStatus::OnRunway),
        "airborne" => Some(AircraftStatus::Airborne),
        _ => None,
    }
}

fn parse_optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
