use crate::config::ServerConfig;
use crate::error::AppError;
use crate::logging::SessionLog;
use crate::scenario::initial_aircraft;
use atc_shared::aircraft::AircraftState;
use atc_shared::ids::*;
use atc_shared::protocol::{HelloPayload, ServerEvent};
use atc_shared::role::{Role, TrainerKind};
use atc_shared::scenario::ScenarioFile;
use atc_shared::sector::builtin_sector;
use atc_shared::session::*;
use rand::distributions::{Alphanumeric, DistString};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;
use tokio::sync::broadcast;

/// Capacity for per-session broadcast channels (v1 = 2 trainers + 1 student).
const BROADCAST_CAPACITY: usize = 64;

pub struct SessionRecord {
    pub session_id: SessionId,
    pub session_hash: SessionHash,
    pub name: String,
    pub status: SessionStatus,
    pub student_position: Option<StudentPosition>,
    pub trainers: Vec<TrainerInfo>,
    pub aircraft: Vec<AircraftState>,
    pub active_runway: String,
    pub metar: String,
    pub revision: u64,
    pub broadcaster: broadcast::Sender<ServerEvent>,
    pub log: Arc<SessionLog>,
}

impl SessionRecord {
    pub fn to_state(&self) -> SessionState {
        SessionState {
            session_id: self.session_id.clone(),
            name: self.name.clone(),
            session_hash: self.session_hash.clone(),
            status: self.status,
            sector_id: builtin_sector().sector_id,
            student_position: self.student_position.clone(),
            trainers: self.trainers.clone(),
            aircraft: self.aircraft.clone(),
            active_runway: self.active_runway.clone(),
            metar: self.metar.clone(),
            server_time: OffsetDateTime::now_utc(),
            revision: self.revision,
        }
    }

    pub fn to_summary(&self) -> SessionSummary {
        SessionSummary {
            session_id: self.session_id.clone(),
            name: self.name.clone(),
            session_hash: self.session_hash.clone(),
            status: self.status,
            student_position_type: self.student_position.as_ref().map(|p| p.position_type),
            student_connected: self
                .student_position
                .as_ref()
                .map(|p| matches!(p.connection_state, ConnectionState::Connected))
                .unwrap_or(false),
            lead_trainer_connected: self.trainers.iter().any(|t| {
                matches!(t.trainer_kind, TrainerKind::LeadTrainer) && t.connected
            }),
        }
    }
}

#[derive(Default)]
pub struct SessionRegistry {
    sessions: Mutex<HashMap<SessionId, Arc<Mutex<SessionRecord>>>>,
}

impl SessionRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn create(
        &self,
        cfg: &ServerConfig,
        name: String,
        lead_trainer_id: TrainerId,
    ) -> Result<Arc<Mutex<SessionRecord>>, AppError> {
        let session_id = SessionId::new(format!("sess_{}", rand_suffix(8)));
        let session_hash = SessionHash::new(format!("join-{}", rand_suffix(6).to_uppercase()));

        let (broadcaster, _) = broadcast::channel(BROADCAST_CAPACITY);
        let log = SessionLog::new(session_id.clone(), cfg.log_dir.as_deref())
            .map_err(|e| AppError::Internal(format!("log init: {e}")))?;

        // Load the optional builtin scenario for initial aircraft + metar + runway.
        let (aircraft, active_runway, metar) = if let Some(toml) = &cfg.builtin_scenario_toml {
            let scenario = ScenarioFile::from_toml_str(toml)
                .map_err(|e| AppError::Internal(format!("scenario parse: {e}")))?;
            (initial_aircraft(&scenario), scenario.initial_active_runway, scenario.metar)
        } else {
            (Vec::new(), "18".to_string(), String::new())
        };

        let record = SessionRecord {
            session_id: session_id.clone(),
            session_hash,
            name,
            status: SessionStatus::Draft,
            student_position: None,
            trainers: vec![TrainerInfo {
                trainer_id: lead_trainer_id,
                trainer_kind: TrainerKind::LeadTrainer,
                connected: true,
            }],
            aircraft,
            active_runway,
            metar,
            revision: 1,
            broadcaster,
            log: Arc::new(log),
        };
        let arc = Arc::new(Mutex::new(record));
        self.sessions.lock().unwrap().insert(session_id, arc.clone());
        Ok(arc)
    }

    pub fn get(&self, id: &SessionId) -> Option<Arc<Mutex<SessionRecord>>> {
        self.sessions.lock().unwrap().get(id).cloned()
    }

    pub fn find_by_hash(&self, hash: &SessionHash) -> Option<Arc<Mutex<SessionRecord>>> {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .find(|s| s.lock().unwrap().session_hash == *hash)
            .cloned()
    }
}

pub fn rand_suffix(n: usize) -> String {
    let mut rng = rand::thread_rng();
    Alphanumeric.sample_string(&mut rng, n)
}

pub fn initial_hello(role: Role, connection_id: ConnectionId) -> ServerEvent {
    ServerEvent::Hello(HelloPayload { connection_id, role })
}
