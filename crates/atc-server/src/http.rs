use crate::app::AppState;
use crate::auth::{BearerToken, TokenSubject};
use crate::error::AppError;
use atc_shared::ids::*;
use atc_shared::protocol::*;
use atc_shared::role::{Role, StudentPositionType};
use atc_shared::sector::builtin_sector;
use atc_shared::session::{ConnectionState, SessionStatus, StudentPosition};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

pub async fn trainer_login(
    State(state): State<AppState>,
    Json(req): Json<TrainerLoginRequest>,
) -> Result<Json<TrainerLoginResponse>, AppError> {
    if !state.config.trainer_hashes.iter().any(|h| h == &req.trainer_hash) {
        return Err(AppError::Unauthorized);
    }
    let trainer_id = TrainerId::new(format!("tr_{}", crate::sessions::rand_suffix(6)));
    let trainer_token = state.tokens.issue_trainer(trainer_id.clone());
    Ok(Json(TrainerLoginResponse {
        trainer_token,
        trainer_id,
        role: Role::Trainer,
    }))
}

fn require_trainer(
    state: &AppState,
    token: &BearerToken,
) -> Result<TrainerId, AppError> {
    match state.tokens.lookup(&token.0) {
        Some(TokenSubject::Trainer { trainer_id }) => Ok(trainer_id),
        _ => Err(AppError::Unauthorized),
    }
}

pub async fn create_session(
    State(state): State<AppState>,
    token: BearerToken,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<CreateSessionResponse>), AppError> {
    let trainer_id = require_trainer(&state, &token)?;
    let session_arc = state.registry.create(&state.config, req.name, trainer_id)?;
    let summary = session_arc.lock().unwrap().to_summary();
    Ok((StatusCode::CREATED, Json(CreateSessionResponse { session: summary })))
}

pub async fn create_student_position(
    State(state): State<AppState>,
    token: BearerToken,
    Path(session_id): Path<String>,
    Json(req): Json<CreateStudentPositionRequest>,
) -> Result<Json<CreateStudentPositionResponse>, AppError> {
    let _trainer_id = require_trainer(&state, &token)?;
    let sid = SessionId::new(session_id);
    let arc = state.registry.get(&sid).ok_or(AppError::SessionNotFound)?;
    let mut rec = arc.lock().unwrap();
    if matches!(rec.status, SessionStatus::Ended) {
        return Err(AppError::SessionEnded);
    }
    if rec.student_position.is_some() {
        return Err(AppError::StudentPositionAlreadyExists);
    }
    let position_id = PositionId::new(format!("pos_{}", crate::sessions::rand_suffix(6)));
    rec.student_position = Some(StudentPosition {
        position_id: position_id.clone(),
        position_type: req.position_type,
        occupied: false,
        connection_state: ConnectionState::Disconnected,
    });
    rec.status = SessionStatus::WaitingForStudent;
    rec.revision += 1;
    Ok(Json(CreateStudentPositionResponse {
        position_id,
        position_type: req.position_type,
        status: "open".into(),
    }))
}

pub async fn join_student(
    State(state): State<AppState>,
    Json(req): Json<JoinStudentRequest>,
) -> Result<Json<JoinStudentResponse>, AppError> {
    let arc = state
        .registry
        .find_by_hash(&req.session_hash)
        .ok_or(AppError::InvalidSessionHash)?;
    let mut rec = arc.lock().unwrap();
    let position = rec
        .student_position
        .as_mut()
        .ok_or(AppError::StudentPositionMissing)?;
    if position.occupied {
        return Err(AppError::StudentPositionOccupied);
    }
    position.occupied = true;
    let position_id = position.position_id.clone();
    let position_type = position.position_type;
    let session_id = rec.session_id.clone();
    let token = state.tokens.issue_student(session_id.clone(), position_id.clone());
    rec.revision += 1;
    Ok(Json(JoinStudentResponse {
        student_token: token,
        session_id,
        position_id,
        position_type,
    }))
}

pub async fn start_session(
    State(state): State<AppState>,
    token: BearerToken,
    Path(session_id): Path<String>,
    Json(_req): Json<StartSessionRequest>,
) -> Result<Json<StartSessionResponse>, AppError> {
    let _trainer_id = require_trainer(&state, &token)?;
    let sid = SessionId::new(session_id);
    let arc = state.registry.get(&sid).ok_or(AppError::SessionNotFound)?;
    let mut rec = arc.lock().unwrap();
    if rec.student_position.is_none() {
        return Err(AppError::StudentPositionMissing);
    }
    rec.status = SessionStatus::Running;
    rec.revision += 1;
    Ok(Json(StartSessionResponse { status: rec.status }))
}

pub async fn get_session(
    State(state): State<AppState>,
    token: BearerToken,
    Path(session_id): Path<String>,
) -> Result<Json<atc_shared::session::SessionState>, AppError> {
    let subject = state.tokens.lookup(&token.0).ok_or(AppError::Unauthorized)?;
    let sid = SessionId::new(session_id);
    let arc = state.registry.get(&sid).ok_or(AppError::SessionNotFound)?;
    let rec = arc.lock().unwrap();
    if let TokenSubject::Student { session_id: bound, .. } = &subject {
        if bound != &rec.session_id {
            return Err(AppError::Forbidden);
        }
    }
    Ok(Json(rec.to_state()))
}

pub async fn get_sector(Path(_sector_id): Path<String>) -> impl IntoResponse {
    Json(builtin_sector())
}

// Forwarder from protocol position_type default.
#[allow(dead_code)]
fn _assert_position_roundtrip(_pt: StudentPositionType) {}
