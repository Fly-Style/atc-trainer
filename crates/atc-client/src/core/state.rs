//! Client-side state: authentication, session cache, sector metadata, and the
//! local view transform (zoom, pan, focused aircraft).

use atc_shared::ids::{AircraftId, SessionId, TrainerId};
use atc_shared::protocol::ServerEvent;
use atc_shared::role::Role;
use atc_shared::sector::SectorMetadata;
use atc_shared::session::SessionState;

/// Server connection profile entered on the start screen.
#[derive(Debug, Clone)]
pub struct ServerProfile {
    pub label: String,
    pub http_base: String,
    pub ws_base: String,
}

impl ServerProfile {
    pub fn local_dev() -> Self {
        Self {
            label: "local".into(),
            http_base: "http://127.0.0.1:8080".into(),
            ws_base: "ws://127.0.0.1:8080".into(),
        }
    }
}

/// Authentication context. Trainers and students hold different tokens; the
/// session_id is bound to student tokens at login and to whichever session a
/// trainer most recently created.
#[derive(Debug, Clone)]
pub enum AuthState {
    Anonymous,
    Trainer { token: String, trainer_id: TrainerId },
    Student { token: String, session_id: SessionId },
}

impl AuthState {
    pub fn token(&self) -> Option<&str> {
        match self {
            AuthState::Trainer { token, .. } | AuthState::Student { token, .. } => Some(token.as_str()),
            AuthState::Anonymous => None,
        }
    }

    pub fn role(&self) -> Option<Role> {
        match self {
            AuthState::Trainer { .. } => Some(Role::Trainer),
            AuthState::Student { .. } => Some(Role::Student),
            AuthState::Anonymous => None,
        }
    }
}

/// Cached, locally-mutated mirror of the server's `SessionState`.
///
/// `apply_event` is the pure reducer used both by tests and by the runtime
/// when forwarding `ServerEvent`s from the WebSocket task.
#[derive(Debug, Clone)]
pub struct CachedSession {
    pub state: SessionState,
}

impl CachedSession {
    pub fn from_snapshot(state: SessionState) -> Self {
        Self { state }
    }

    /// Apply a server event to the cached state. Returns true when state
    /// was meaningfully updated.
    pub fn apply_event(&mut self, event: &ServerEvent) -> bool {
        match event {
            ServerEvent::Hello(_) => false,
            ServerEvent::SessionState(snap) => {
                self.state = (**snap).clone();
                true
            }
            ServerEvent::SessionUpdated(p) => {
                if let Some(s) = p.status {
                    self.state.status = s;
                }
                if let Some(rwy) = &p.active_runway {
                    self.state.active_runway = rwy.clone();
                }
                self.state.revision = p.revision;
                true
            }
            ServerEvent::AircraftCreated(p) => {
                self.state.aircraft.push(p.aircraft.clone());
                true
            }
            ServerEvent::AircraftUpdated(p) => {
                if let Some(slot) = self
                    .state
                    .aircraft
                    .iter_mut()
                    .find(|a| a.aircraft_id == p.aircraft.aircraft_id)
                {
                    *slot = p.aircraft.clone();
                    true
                } else {
                    self.state.aircraft.push(p.aircraft.clone());
                    true
                }
            }
            ServerEvent::AircraftRemoved(p) => {
                let before = self.state.aircraft.len();
                self.state.aircraft.retain(|a| a.aircraft_id != p.aircraft_id);
                self.state.revision = p.revision;
                self.state.aircraft.len() != before
            }
            ServerEvent::CommandRejected(_) | ServerEvent::SystemNotice(_) => false,
        }
    }
}

/// Sector view transform: world units (NM) ↔ screen pixels.
///
/// `zoom_px_per_nm` is pixels-per-NM. `pan_nm` is the world-space point shown
/// at the centre of the canvas (positive y is north).
#[derive(Debug, Clone, PartialEq)]
pub struct ViewState {
    pub zoom_px_per_nm: f32,
    pub pan_nm: (f32, f32),
    pub focused_aircraft: Option<AircraftId>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            zoom_px_per_nm: 16.0,
            pan_nm: (0.0, 0.0),
            focused_aircraft: None,
        }
    }
}

impl ViewState {
    /// Multiplicative zoom step. Clamped to a usable range so wheel/key
    /// presses can't take the canvas to a degenerate scale.
    pub fn zoom_in(&mut self) {
        self.zoom_px_per_nm = (self.zoom_px_per_nm * 1.4).min(20_000.0);
    }
    pub fn zoom_out(&mut self) {
        self.zoom_px_per_nm = (self.zoom_px_per_nm / 1.4).max(1.0);
    }
    pub fn pan_by_nm(&mut self, dx: f32, dy: f32) {
        self.pan_nm.0 += dx;
        self.pan_nm.1 += dy;
    }

    /// Map a world point (NM, north-up) into screen coordinates given the
    /// canvas size in pixels. Screen y grows downward.
    pub fn world_to_screen(&self, world: (f32, f32), canvas_size: (f32, f32)) -> (f32, f32) {
        let (cx, cy) = (canvas_size.0 * 0.5, canvas_size.1 * 0.5);
        let dx = (world.0 - self.pan_nm.0) * self.zoom_px_per_nm;
        let dy = (world.1 - self.pan_nm.1) * self.zoom_px_per_nm;
        (cx + dx, cy - dy)
    }

    /// Inverse of `world_to_screen`. Used for click/drag hit-testing.
    pub fn screen_to_world(&self, screen: (f32, f32), canvas_size: (f32, f32)) -> (f32, f32) {
        let (cx, cy) = (canvas_size.0 * 0.5, canvas_size.1 * 0.5);
        let x = (screen.0 - cx) / self.zoom_px_per_nm + self.pan_nm.0;
        let y = -(screen.1 - cy) / self.zoom_px_per_nm + self.pan_nm.1;
        (x, y)
    }
}

/// Top-level client state held by the iced application and consumed by tests.
#[derive(Debug, Clone)]
pub struct ClientState {
    pub server: ServerProfile,
    pub auth: AuthState,
    pub session: Option<CachedSession>,
    pub sector: Option<SectorMetadata>,
    pub view: ViewState,
    pub last_error: Option<String>,
}

impl Default for ClientState {
    fn default() -> Self {
        Self {
            server: ServerProfile::local_dev(),
            auth: AuthState::Anonymous,
            session: None,
            sector: None,
            view: ViewState::default(),
            last_error: None,
        }
    }
}

impl ClientState {
    pub fn current_session_id(&self) -> Option<&SessionId> {
        match &self.auth {
            AuthState::Student { session_id, .. } => Some(session_id),
            _ => self.session.as_ref().map(|s| &s.state.session_id),
        }
    }

    /// Pick the aircraft whose marker contains the given screen point, if
    /// any. Used by the canvas widget to drive focus selection.
    pub fn hit_test_aircraft(
        &self,
        screen: (f32, f32),
        canvas_size: (f32, f32),
        marker_radius_px: f32,
    ) -> Option<AircraftId> {
        let session = self.session.as_ref()?;
        let r2 = marker_radius_px * marker_radius_px;
        for ac in &session.state.aircraft {
            let (sx, sy) = self.view.world_to_screen((ac.x_nm, ac.y_nm), canvas_size);
            let (dx, dy) = (sx - screen.0, sy - screen.1);
            if dx * dx + dy * dy <= r2 {
                return Some(ac.aircraft_id.clone());
            }
        }
        None
    }
}
