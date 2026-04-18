use crate::app::AppState;
use crate::auth::TokenSubject;
use crate::error::AppError;
use crate::scenario::scenario_aircraft_to_state;
use crate::sessions::SessionRecord;
use atc_shared::aircraft::{
    AircraftOrigin, AircraftState, AircraftStatus, FlightPath, FlightRuleType, MovementMode,
    TargetAltitude,
};
use atc_shared::ids::{AircraftId, SessionId};
use atc_shared::protocol::{
    AircraftEventPayload, AircraftRemovedPayload, ClientEvent, ServerEvent, StudentAircraftChanges,
    StudentCommand, TrainerCommand,
};
use atc_shared::role::StudentPositionType;
use atc_shared::scenario::ScenarioAircraft;
use atc_shared::validation::{is_valid_squawk, paired_sid_for_runway, sid_outer_fix};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

pub async fn handle_client_event(
    state: AppState,
    session_id: SessionId,
    subject: TokenSubject,
    event: ClientEvent,
) -> Result<(), AppError> {
    let arc = state
        .registry
        .get(&session_id)
        .ok_or(AppError::SessionNotFound)?;
    match (subject, event) {
        (TokenSubject::Trainer { trainer_id }, ClientEvent::TrainerCommand(env)) => {
            handle_trainer_command(state, arc, trainer_id.as_str(), env.command).await
        }
        (TokenSubject::Student { .. }, ClientEvent::StudentCommand(env)) => {
            handle_student_command(arc, env.command)
        }
        (TokenSubject::Trainer { .. }, ClientEvent::StudentCommand(_))
        | (TokenSubject::Student { .. }, ClientEvent::TrainerCommand(_)) => Err(AppError::Forbidden),
    }
}

async fn handle_trainer_command(
    state: AppState,
    arc: Arc<Mutex<SessionRecord>>,
    actor: &str,
    command: TrainerCommand,
) -> Result<(), AppError> {
    match command {
        TrainerCommand::SpawnAircraft {
            template,
            callsign,
            squawk_mode,
            initial_x_nm,
            initial_y_nm,
            initial_state,
            initial_path,
        } => {
            let mut rec = arc.lock().unwrap();
            let aircraft_id = AircraftId::new(format!("ac_{}", crate::sessions::rand_suffix(6)));
            let route = initial_path
                .as_ref()
                .and_then(|path| path.points.last())
                .map(|_| "MANUAL".to_string())
                .unwrap_or_default();
            let scenario = ScenarioAircraft {
                aircraft_id: aircraft_id.as_str().to_string(),
                template,
                callsign: callsign.clone(),
                flight_rules: match template {
                    atc_shared::aircraft::AircraftCategory::VfrC172 => FlightRuleType::Vfr,
                    atc_shared::aircraft::AircraftCategory::IfrA320 => FlightRuleType::Ifr,
                },
                departure_airfield: "AAAA".to_string(),
                destination_airfield: "AAAA".to_string(),
                route,
                initial_spawn: "manual".to_string(),
                initial_status: initial_state,
                squawk_mode,
                assigned_sid: None,
                assigned_runway: None,
                assigned_altitude_ft: None,
            };
            let mut aircraft = scenario_aircraft_to_state(&scenario);
            aircraft.origin = AircraftOrigin::Manual;
            aircraft.x_nm = initial_x_nm;
            aircraft.y_nm = initial_y_nm;
            aircraft.draft_path = initial_path.clone();
            aircraft.active_path = None;
            aircraft.ground_speed_kt = 0.0;
            aircraft.air_speed_kt = None;
            let event = add_aircraft(&mut rec, aircraft.clone(), actor, "spawn_aircraft", "spawned");
            drop(rec);
            let _ = arc.lock().unwrap().broadcaster.send(event);
            if matches!(initial_state, AircraftStatus::Airborne) {
                if let Some(path) = initial_path {
                    launch_path_task(state, arc, aircraft.aircraft_id, path)?;
                }
            }
            Ok(())
        }
        TrainerCommand::RemoveAircraft { aircraft_id } => {
            let mut rec = arc.lock().unwrap();
            let idx = aircraft_index(&rec, &aircraft_id)?;
            rec.aircraft.remove(idx);
            rec.revision = rec.revision.saturating_add(1);
            rec.log.write(actor, Some(&aircraft_id), "remove_aircraft", "removed");
            let _ = rec.broadcaster.send(ServerEvent::AircraftRemoved(AircraftRemovedPayload {
                aircraft_id,
                revision: rec.revision,
            }));
            Ok(())
        }
        TrainerCommand::SetActiveRunway { runway } => {
            let mut rec = arc.lock().unwrap();
            rec.active_runway = runway.clone();
            for idx in 0..rec.aircraft.len() {
                if rec.aircraft[idx].status == AircraftStatus::Airborne {
                    continue;
                }
                rec.aircraft[idx].assigned_runway = Some(runway.clone());
                if let Some(sid) = rec.aircraft[idx].assigned_sid.clone() {
                    if let Some(paired) = paired_sid_for_runway(&sid, &runway) {
                        rec.aircraft[idx].assigned_sid = Some(paired.to_string());
                    }
                    rec.aircraft[idx].next_waypoint = rec.aircraft[idx]
                        .assigned_sid
                        .as_deref()
                        .and_then(sid_outer_fix)
                        .map(str::to_string);
                }
                emit_aircraft_updated(&mut rec, idx, actor, "set_active_runway", "runway reassigned");
            }
            rec.bump_and_broadcast(None, Some(runway));
            Ok(())
        }
        TrainerCommand::AssignRunwayInWork { aircraft_id, runway } => {
            let mut rec = arc.lock().unwrap();
            let idx = aircraft_index(&rec, &aircraft_id)?;
            rec.aircraft[idx].assigned_runway = runway;
            emit_aircraft_updated(&mut rec, idx, actor, "assign_runway_in_work", "runway updated");
            Ok(())
        }
        TrainerCommand::SetSpeed { aircraft_id, target_speed_kt } => {
            let mut rec = arc.lock().unwrap();
            let idx = aircraft_index(&rec, &aircraft_id)?;
            if rec.aircraft[idx].status == AircraftStatus::Airborne {
                rec.aircraft[idx].air_speed_kt = Some(target_speed_kt);
            }
            rec.aircraft[idx].ground_speed_kt = target_speed_kt;
            emit_aircraft_updated(&mut rec, idx, actor, "set_speed", &format!("{target_speed_kt} kt"));
            Ok(())
        }
        TrainerCommand::SetPath { aircraft_id, mut path } => {
            let mut rec = arc.lock().unwrap();
            let idx = aircraft_index(&rec, &aircraft_id)?;
            path.launched = false;
            rec.aircraft[idx].draft_path = Some(path.clone());
            emit_aircraft_updated(
                &mut rec,
                idx,
                actor,
                "set_path",
                &format!("{} points, launch pending", path.points.len()),
            );
            Ok(())
        }
        TrainerCommand::LaunchPath { aircraft_id } => {
            let path = {
                let mut rec = arc.lock().unwrap();
                let idx = aircraft_index(&rec, &aircraft_id)?;
                let mut path = rec.aircraft[idx]
                    .draft_path
                    .clone()
                    .ok_or_else(|| AppError::BadRequest("no draft path".into()))?;
                path.launched = true;
                rec.aircraft[idx].active_path = Some(path.clone());
                rec.aircraft[idx].draft_path = None;
                rec.aircraft[idx].path_run_id = rec.aircraft[idx].path_run_id.saturating_add(1);
                emit_aircraft_updated(&mut rec, idx, actor, "launch_path", "movement started");
                path
            };
            launch_path_task(state, arc, aircraft_id, path)
        }
        TrainerCommand::StopGroundAircraft { aircraft_id } => {
            let mut rec = arc.lock().unwrap();
            let idx = aircraft_index(&rec, &aircraft_id)?;
            if rec.aircraft[idx].status != AircraftStatus::Airborne {
                rec.aircraft[idx].active_path = None;
                rec.aircraft[idx].draft_path = None;
                rec.aircraft[idx].path_run_id = rec.aircraft[idx].path_run_id.saturating_add(1);
                rec.aircraft[idx].ground_speed_kt = 0.0;
                emit_aircraft_updated(&mut rec, idx, actor, "stop_ground_aircraft", "stopped");
            }
            Ok(())
        }
        TrainerCommand::TriggerGoAround { aircraft_id } => {
            let (path, runway) = {
                let rec = arc.lock().unwrap();
                let idx = aircraft_index(&rec, &aircraft_id)?;
                let path = go_around_path(&rec.aircraft[idx], &rec.active_runway);
                (path, rec.active_runway.clone())
            };
            {
                let mut rec = arc.lock().unwrap();
                let idx = aircraft_index(&rec, &aircraft_id)?;
                rec.aircraft[idx].active_path = Some(path.clone());
                rec.aircraft[idx].draft_path = None;
                rec.aircraft[idx].path_run_id = rec.aircraft[idx].path_run_id.saturating_add(1);
                rec.aircraft[idx].assigned_runway = Some(runway);
                emit_aircraft_updated(&mut rec, idx, actor, "trigger_go_around", "runway heading");
            }
            launch_path_task(state, arc, aircraft_id, path)
        }
        TrainerCommand::TriggerRejectedTakeoff { aircraft_id } => {
            let mut rec = arc.lock().unwrap();
            let idx = aircraft_index(&rec, &aircraft_id)?;
            rec.aircraft[idx].active_path = None;
            rec.aircraft[idx].draft_path = None;
            rec.aircraft[idx].path_run_id = rec.aircraft[idx].path_run_id.saturating_add(1);
            rec.aircraft[idx].ground_speed_kt = 0.0;
            rec.aircraft[idx].status = AircraftStatus::OnRunway;
            emit_aircraft_updated(&mut rec, idx, actor, "trigger_rejected_takeoff", "rejected");
            Ok(())
        }
        TrainerCommand::PauseSession => {
            let mut rec = arc.lock().unwrap();
            rec.log.write(actor, None, "pause_session", "paused");
            rec.bump_and_broadcast(Some(atc_shared::session::SessionStatus::Paused), None);
            Ok(())
        }
        TrainerCommand::ResumeSession => {
            let mut rec = arc.lock().unwrap();
            rec.log.write(actor, None, "resume_session", "running");
            rec.bump_and_broadcast(Some(atc_shared::session::SessionStatus::Running), None);
            Ok(())
        }
    }
}

fn handle_student_command(
    arc: Arc<Mutex<SessionRecord>>,
    command: StudentCommand,
) -> Result<(), AppError> {
    let mut rec = arc.lock().unwrap();
    let actor = match rec.student_position.as_ref().map(|p| p.position_type) {
        Some(StudentPositionType::Gnd) => "student:position_gnd",
        Some(StudentPositionType::Twr) => "student:position_twr",
        None => "student",
    };
    match command {
        StudentCommand::AssumeAircraft { aircraft_id } => {
            let idx = aircraft_index(&rec, &aircraft_id)?;
            rec.aircraft[idx].assumed_by_student = true;
            rec.aircraft[idx].handed_off = false;
            if matches!(
                rec.student_position.as_ref().map(|p| p.position_type),
                Some(StudentPositionType::Gnd)
            ) {
                rec.aircraft[idx].assigned_runway = None;
            }
            emit_aircraft_updated(&mut rec, idx, actor, "assume_aircraft", "assumed");
            Ok(())
        }
        StudentCommand::UpdateAircraft { aircraft_id, changes } => {
            let idx = aircraft_index(&rec, &aircraft_id)?;
            if !rec.aircraft[idx].assumed_by_student {
                return Err(AppError::Forbidden);
            }
            let position_type = rec.student_position.as_ref().map(|position| position.position_type);
            apply_student_changes(
                &mut rec.aircraft[idx],
                &changes,
                position_type,
            );
            emit_aircraft_updated(&mut rec, idx, actor, "update_aircraft", "updated");
            Ok(())
        }
        StudentCommand::HandoffAircraft { aircraft_id } => {
            let idx = aircraft_index(&rec, &aircraft_id)?;
            rec.aircraft[idx].assumed_by_student = false;
            rec.aircraft[idx].handed_off = true;
            emit_aircraft_updated(&mut rec, idx, actor, "handoff_aircraft", "handed off");
            Ok(())
        }
    }
}

fn apply_student_changes(
    aircraft: &mut AircraftState,
    changes: &StudentAircraftChanges,
    position_type: Option<StudentPositionType>,
) {
    if let Some(status) = changes.status {
        aircraft.status = status;
        aircraft.movement_mode = match status {
            AircraftStatus::New | AircraftStatus::Cleared | AircraftStatus::Startup => MovementMode::Parked,
            AircraftStatus::Push => MovementMode::Pushback,
            AircraftStatus::Taxi => MovementMode::Taxiing,
            AircraftStatus::OnRunway => MovementMode::Holding,
            AircraftStatus::Airborne => MovementMode::Airborne,
        };
        if matches!(position_type, Some(StudentPositionType::Twr)) && status == AircraftStatus::Taxi {
            aircraft.assigned_runway = None;
        }
    }
    if let Some(runway) = &changes.assigned_runway {
        aircraft.assigned_runway = runway.clone();
    }
    if let Some(sid) = &changes.assigned_sid {
        aircraft.assigned_sid = sid.clone();
        aircraft.next_waypoint = aircraft
            .assigned_sid
            .as_deref()
            .and_then(sid_outer_fix)
            .map(str::to_string);
    }
    if let Some(squawk) = &changes.assigned_squawk {
        aircraft.assigned_squawk = squawk.clone().filter(|code| is_valid_squawk(code));
    }
    if let Some(altitude) = changes.assigned_altitude_ft {
        aircraft.assigned_altitude_ft = altitude;
    }
}

fn aircraft_index(rec: &SessionRecord, aircraft_id: &AircraftId) -> Result<usize, AppError> {
    rec.aircraft
        .iter()
        .position(|aircraft| aircraft.aircraft_id == *aircraft_id)
        .ok_or(AppError::AircraftNotFound)
}

fn emit_aircraft_updated(
    rec: &mut SessionRecord,
    idx: usize,
    actor: &str,
    action: &str,
    result: &str,
) {
    rec.revision = rec.revision.saturating_add(1);
    rec.aircraft[idx].revision = rec.revision;
    let aircraft = rec.aircraft[idx].clone();
    rec.log.write(actor, Some(&aircraft.aircraft_id), action, result);
    let _ = rec
        .broadcaster
        .send(ServerEvent::AircraftUpdated(AircraftEventPayload { aircraft }));
}

fn add_aircraft(
    rec: &mut SessionRecord,
    mut aircraft: AircraftState,
    actor: &str,
    action: &str,
    result: &str,
) -> ServerEvent {
    rec.revision = rec.revision.saturating_add(1);
    aircraft.revision = rec.revision;
    rec.log.write(actor, Some(&aircraft.aircraft_id), action, result);
    rec.aircraft.push(aircraft.clone());
    ServerEvent::AircraftCreated(AircraftEventPayload { aircraft })
}

fn launch_path_task(
    _state: AppState,
    arc: Arc<Mutex<SessionRecord>>,
    aircraft_id: AircraftId,
    path: FlightPath,
) -> Result<(), AppError> {
    let run_id = {
        let rec = arc.lock().unwrap();
        let idx = aircraft_index(&rec, &aircraft_id)?;
        rec.aircraft[idx].path_run_id
    };
    tokio::spawn(async move {
        execute_path(arc, aircraft_id, run_id, path).await;
    });
    Ok(())
}

async fn execute_path(
    arc: Arc<Mutex<SessionRecord>>,
    aircraft_id: AircraftId,
    run_id: u64,
    path: FlightPath,
) {
    let mut previous = {
        let rec = arc.lock().unwrap();
        if let Ok(idx) = aircraft_index(&rec, &aircraft_id) {
            let aircraft = &rec.aircraft[idx];
            (aircraft.x_nm, aircraft.y_nm, aircraft.altitude_ft)
        } else {
            return;
        }
    };

    for point in path.points {
        let distance_nm = ((point.x_nm - previous.0).powi(2) + (point.y_nm - previous.1).powi(2)).sqrt();
        let speed_kt = point.target_speed_kt.max(1.0);
        let duration_s = if distance_nm > 0.01 {
            distance_nm / speed_kt * 3600.0
        } else {
            0.25
        };
        let steps = (duration_s / 0.2).ceil().max(1.0) as u32;

        for step in 1..=steps {
            loop {
                let paused = {
                    let rec = arc.lock().unwrap();
                    rec.status == atc_shared::session::SessionStatus::Paused
                };
                if paused {
                    sleep(Duration::from_millis(200)).await;
                } else {
                    break;
                }
            }

            {
                let mut rec = arc.lock().unwrap();
                let Ok(idx) = aircraft_index(&rec, &aircraft_id) else {
                    return;
                };
                if rec.aircraft[idx].path_run_id != run_id {
                    return;
                }
                let fraction = step as f32 / steps as f32;
                rec.aircraft[idx].x_nm = previous.0 + (point.x_nm - previous.0) * fraction;
                rec.aircraft[idx].y_nm = previous.1 + (point.y_nm - previous.1) * fraction;
                rec.aircraft[idx].ground_speed_kt = point.target_speed_kt;
                match point.target_altitude {
                    TargetAltitude::Gnd => {
                        rec.aircraft[idx].altitude_ft = 0.0;
                        rec.aircraft[idx].air_speed_kt = None;
                    }
                    TargetAltitude::MslFt { value_ft } => {
                        rec.aircraft[idx].status = AircraftStatus::Airborne;
                        rec.aircraft[idx].movement_mode = MovementMode::Airborne;
                        rec.aircraft[idx].altitude_ft =
                            previous.2 + (value_ft as f32 - previous.2) * fraction;
                        rec.aircraft[idx].air_speed_kt = Some(point.target_speed_kt);
                    }
                }
                emit_aircraft_updated(&mut rec, idx, "system:path", "path_step", "position updated");
            }
            sleep(Duration::from_millis(200)).await;
        }
        previous = (
            point.x_nm,
            point.y_nm,
            match point.target_altitude {
                TargetAltitude::Gnd => 0.0,
                TargetAltitude::MslFt { value_ft } => value_ft as f32,
            },
        );
    }

    let mut rec = arc.lock().unwrap();
    if let Ok(idx) = aircraft_index(&rec, &aircraft_id) {
        if rec.aircraft[idx].path_run_id == run_id {
            rec.aircraft[idx].active_path = None;
            emit_aircraft_updated(&mut rec, idx, "system:path", "path_complete", "completed");
        }
    }
}

fn go_around_path(aircraft: &AircraftState, active_runway: &str) -> FlightPath {
    let (dx, dy) = match active_runway {
        "18" => (0.0, -1.0),
        _ => (0.0, 1.0),
    };
    FlightPath {
        points: vec![atc_shared::aircraft::PathPoint {
            seq: 1,
            x_nm: aircraft.x_nm + dx * 5.0,
            y_nm: aircraft.y_nm + dy * 5.0,
            target_speed_kt: aircraft.air_speed_kt.unwrap_or(140.0),
            target_altitude: TargetAltitude::MslFt {
                value_ft: (aircraft.altitude_ft + 1000.0) as i32,
            },
        }],
        launched: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::auth::TokenStore;
    use crate::config::ServerConfig;
    use crate::logging::SessionLog;
    use crate::sessions::SessionRegistry;
    use atc_shared::aircraft::{
        AircraftCategory, AircraftOrigin, AircraftStatus, ControlledByPosition, FlightRuleType,
        MovementMode, SquawkMode,
    };
    use atc_shared::ids::{PositionId, SessionHash, SessionId, TrainerId};
    use atc_shared::role::StudentPositionType;
    use atc_shared::session::{ConnectionState, SessionStatus, StudentPosition, TrainerInfo};
    use std::sync::Arc;
    use tokio::sync::broadcast;

    fn app_state() -> AppState {
        AppState {
            config: Arc::new(ServerConfig::test_default()),
            registry: SessionRegistry::new(),
            tokens: TokenStore::new(),
        }
    }

    fn sample_record(position_type: StudentPositionType) -> Arc<Mutex<SessionRecord>> {
        let (broadcaster, _) = broadcast::channel(8);
        Arc::new(Mutex::new(SessionRecord {
            session_id: SessionId::new("sess_t"),
            session_hash: SessionHash::new("join-T"),
            name: "test".into(),
            status: SessionStatus::Running,
            student_position: Some(StudentPosition {
                position_id: PositionId::new("pos_t"),
                position_type,
                occupied: true,
                connection_state: ConnectionState::Connected,
            }),
            trainers: vec![TrainerInfo {
                trainer_id: TrainerId::new("tr_t"),
                connected: true,
            }],
            aircraft: vec![AircraftState {
                aircraft_id: AircraftId::new("ac_t"),
                origin: AircraftOrigin::Scenario,
                callsign: "BTI201".into(),
                departure_airfield: "AAAA".into(),
                destination_airfield: "AAAN".into(),
                category: AircraftCategory::IfrA320,
                aircraft_type: "A320".into(),
                flight_rules: FlightRuleType::Ifr,
                route: "NORTH1A".into(),
                status: AircraftStatus::Cleared,
                movement_mode: MovementMode::Parked,
                controlled_by_position: ControlledByPosition::Gnd,
                squawk_mode: SquawkMode::Standby,
                assigned_runway: Some("18".into()),
                assigned_sid: Some("north1a".into()),
                assigned_squawk: None,
                assigned_altitude_ft: Some(5000),
                current_node: None,
                target_node: None,
                scenario_placement: None,
                next_waypoint: Some("NORTH".into()),
                ground_speed_kt: 0.0,
                air_speed_kt: None,
                altitude_ft: 0.0,
                x_nm: 0.0,
                y_nm: 0.0,
                trainer_profile: None,
                assumed_by_student: false,
                handed_off: false,
                draft_path: None,
                active_path: None,
                path_run_id: 0,
                revision: 1,
            }],
            active_runway: "18".into(),
            metar: String::new(),
            revision: 1,
            broadcaster,
            log: Arc::new(SessionLog::new(SessionId::new("sess_t"), None).expect("log")),
        }))
    }

    #[test]
    fn gnd_assume_clears_assigned_runway() {
        let arc = sample_record(StudentPositionType::Gnd);
        handle_student_command(
            arc.clone(),
            StudentCommand::AssumeAircraft {
                aircraft_id: AircraftId::new("ac_t"),
            },
        )
        .expect("assume works");
        let rec = arc.lock().unwrap();
        let aircraft = &rec.aircraft[0];
        assert!(aircraft.assumed_by_student);
        assert_eq!(aircraft.assigned_runway, None);
    }

    #[tokio::test]
    async fn active_runway_switch_reassigns_ground_aircraft_and_sid() {
        let arc = sample_record(StudentPositionType::Twr);
        handle_trainer_command(
            app_state(),
            arc.clone(),
            "trainer:test",
            TrainerCommand::SetActiveRunway {
                runway: "36".into(),
            },
        )
        .await
        .expect("switch runway");
        let rec = arc.lock().unwrap();
        let aircraft = &rec.aircraft[0];
        assert_eq!(rec.active_runway, "36");
        assert_eq!(aircraft.assigned_runway.as_deref(), Some("36"));
        assert_eq!(aircraft.assigned_sid.as_deref(), Some("north1b"));
        assert_eq!(aircraft.next_waypoint.as_deref(), Some("NORTH"));
    }
}
