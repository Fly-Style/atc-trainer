//! Unit tests for the testable client core: state reducer, command processor,
//! view-transform math, hit-testing, and WebSocket envelope decoding.

use atc_client::core::*;
use atc_shared::ids::*;
use atc_shared::protocol::{
    AircraftEventPayload, AircraftRemovedPayload, ClientEvent, HelloPayload, ServerEvent,
    SessionUpdatedPayload, StudentCommand, TrainerCommand, WsEnvelope,
};
use atc_shared::role::{Role, StudentPositionType};
use atc_shared::session::{SessionState, SessionStatus};
use time::OffsetDateTime;

fn empty_snapshot() -> SessionState {
    SessionState {
        session_id: SessionId::new("sess_abc"),
        name: "test".into(),
        session_hash: SessionHash::new("join-X"),
        status: SessionStatus::Draft,
        sector_id: SectorId::new("airport_01"),
        student_position: None,
        trainers: vec![],
        aircraft: vec![],
        active_runway: "36".into(),
        metar: String::new(),
        server_time: OffsetDateTime::now_utc(),
        revision: 1,
    }
}

fn sample_aircraft(id: &str, x: f32, y: f32) -> atc_shared::aircraft::AircraftState {
    use atc_shared::aircraft::*;
    AircraftState {
        aircraft_id: AircraftId::new(id),
        origin: AircraftOrigin::Manual,
        callsign: id.into(),
        departure_airfield: "EVRA".into(),
        destination_airfield: "EETN".into(),
        category: AircraftCategory::IfrA320,
        aircraft_type: "A320".into(),
        flight_rules: FlightRuleType::Ifr,
        route: String::new(),
        status: AircraftStatus::Taxi,
        movement_mode: MovementMode::Taxiing,
        controlled_by_position: ControlledByPosition::Gnd,
        squawk_mode: SquawkMode::Standby,
        assigned_runway: Some("36".into()),
        assigned_sid: None,
        assigned_squawk: None,
        assigned_altitude_ft: None,
        current_node: None,
        target_node: None,
        scenario_placement: None,
        next_waypoint: None,
        ground_speed_kt: 0.0,
        air_speed_kt: None,
        altitude_ft: 0.0,
        x_nm: x,
        y_nm: y,
        trainer_profile: None,
        assumed_by_student: false,
        handed_off: false,
        draft_path: None,
        active_path: None,
        path_run_id: 0,
        revision: 1,
    }
}

#[test]
fn reducer_applies_session_state_snapshot() {
    let mut session = CachedSession::from_snapshot(empty_snapshot());
    let mut snap = empty_snapshot();
    snap.revision = 7;
    snap.active_runway = "18".into();
    let changed = session.apply_event(&ServerEvent::SessionState(Box::new(snap)));
    assert!(changed);
    assert_eq!(session.state.revision, 7);
    assert_eq!(session.state.active_runway, "18");
}

#[test]
fn reducer_applies_session_updated_partial() {
    let mut session = CachedSession::from_snapshot(empty_snapshot());
    let changed = session.apply_event(&ServerEvent::SessionUpdated(SessionUpdatedPayload {
        status: Some(SessionStatus::Running),
        active_runway: Some("18".into()),
        revision: 12,
    }));
    assert!(changed);
    assert_eq!(session.state.status, SessionStatus::Running);
    assert_eq!(session.state.active_runway, "18");
    assert_eq!(session.state.revision, 12);
}

#[test]
fn reducer_handles_aircraft_lifecycle() {
    let mut session = CachedSession::from_snapshot(empty_snapshot());
    session.apply_event(&ServerEvent::AircraftCreated(AircraftEventPayload {
        aircraft: sample_aircraft("ac1", 1.0, 2.0),
    }));
    assert_eq!(session.state.aircraft.len(), 1);

    let mut updated = sample_aircraft("ac1", 5.0, 6.0);
    updated.revision = 2;
    session.apply_event(&ServerEvent::AircraftUpdated(AircraftEventPayload { aircraft: updated }));
    assert_eq!(session.state.aircraft[0].x_nm, 5.0);
    assert_eq!(session.state.aircraft[0].revision, 2);

    session.apply_event(&ServerEvent::AircraftRemoved(AircraftRemovedPayload {
        aircraft_id: AircraftId::new("ac1"),
        revision: 3,
    }));
    assert!(session.state.aircraft.is_empty());
    assert_eq!(session.state.revision, 3);
}

#[test]
fn reducer_ignores_hello_and_notices() {
    let mut session = CachedSession::from_snapshot(empty_snapshot());
    let r1 = session.state.revision;
    let changed = session.apply_event(&ServerEvent::Hello(HelloPayload {
        connection_id: ConnectionId::new("c1"),
        role: Role::Trainer,
    }));
    assert!(!changed);
    assert_eq!(session.state.revision, r1);
}

// ---- View transform ----

#[test]
fn view_zoom_clamps_within_bounds() {
    let mut v = ViewState::default();
    for _ in 0..50 {
        v.zoom_in();
    }
    assert!(v.zoom_px_per_nm <= 800.0);
    for _ in 0..50 {
        v.zoom_out();
    }
    assert!(v.zoom_px_per_nm >= 2.0);
}

#[test]
fn world_screen_round_trip() {
    let v = ViewState {
        zoom_px_per_nm: 32.0,
        pan_nm: (1.0, -2.0),
        focused_aircraft: None,
    };
    let canvas = (640.0, 480.0);
    let world = (3.5, 4.0);
    let screen = v.world_to_screen(world, canvas);
    let back = v.screen_to_world(screen, canvas);
    assert!((back.0 - world.0).abs() < 1e-3);
    assert!((back.1 - world.1).abs() < 1e-3);
}

#[test]
fn hit_test_picks_marker_under_cursor() {
    let mut state = ClientState::default();
    let mut snap = empty_snapshot();
    snap.aircraft = vec![sample_aircraft("ac1", 0.0, 0.0), sample_aircraft("ac2", 5.0, 0.0)];
    state.session = Some(CachedSession::from_snapshot(snap));
    state.view.zoom_px_per_nm = 32.0;
    let canvas = (640.0, 480.0);
    let center = (canvas.0 * 0.5, canvas.1 * 0.5);
    let hit = state.hit_test_aircraft(center, canvas, 6.0);
    assert_eq!(hit.as_ref().map(|i| i.as_str()), Some("ac1"));
    let other = state.view.world_to_screen((5.0, 0.0), canvas);
    let hit = state.hit_test_aircraft(other, canvas, 6.0);
    assert_eq!(hit.as_ref().map(|i| i.as_str()), Some("ac2"));
    let miss = state.hit_test_aircraft((0.0, 0.0), canvas, 6.0);
    assert!(miss.is_none());
}

// ---- Command processor ----

#[test]
fn login_emits_unauthenticated_post() {
    let mut s = ClientState::default();
    let effects = process(&mut s, AppCommand::LoginAsTrainer { hash: "h".into() });
    assert_eq!(effects.len(), 1);
    match &effects[0] {
        SideEffect::Http(req) => {
            assert_eq!(req.correlation, HttpCorrelation::TrainerLogin);
            assert!(!req.with_auth);
            assert_eq!(req.body["trainer_hash"], "h");
        }
        e => panic!("unexpected {e:?}"),
    }
}

#[test]
fn create_session_requires_trainer_auth() {
    let mut s = ClientState::default();
    let effects = process(&mut s, AppCommand::CreateSession { name: "n".into() });
    assert!(matches!(effects[0], SideEffect::ReportError(_)));

    s.auth = AuthState::Trainer { token: "tk".into(), trainer_id: TrainerId::new("t1") };
    let effects = process(&mut s, AppCommand::CreateSession { name: "n".into() });
    match &effects[0] {
        SideEffect::Http(req) => {
            assert_eq!(req.correlation, HttpCorrelation::CreateSession);
            assert!(req.with_auth);
            assert_eq!(req.body["name"], "n");
        }
        e => panic!("unexpected {e:?}"),
    }
}

#[test]
fn create_student_position_includes_session_id() {
    let mut s = ClientState {
        auth: AuthState::Trainer { token: "tk".into(), trainer_id: TrainerId::new("t1") },
        ..ClientState::default()
    };
    let sid = SessionId::new("sess_xyz");
    let effects = process(
        &mut s,
        AppCommand::CreateStudentPosition {
            session_id: sid.clone(),
            position_type: StudentPositionType::Twr,
        },
    );
    match &effects[0] {
        SideEffect::Http(req) => {
            assert_eq!(req.correlation, HttpCorrelation::CreateStudentPosition { session_id: sid });
            assert_eq!(req.body["position_type"], "twr");
        }
        e => panic!("unexpected {e:?}"),
    }
}

#[test]
fn open_websocket_uses_token_when_present() {
    let mut s = ClientState::default();
    let sid = SessionId::new("sess_ws");
    let effects = process(&mut s, AppCommand::OpenWebSocket { session_id: sid.clone() });
    assert!(matches!(effects[0], SideEffect::ReportError(_)));

    s.auth = AuthState::Student { token: "stk".into(), session_id: sid.clone() };
    let effects = process(&mut s, AppCommand::OpenWebSocket { session_id: sid.clone() });
    match &effects[0] {
        SideEffect::OpenWebSocket { token, session_id } => {
            assert_eq!(token, "stk");
            assert_eq!(session_id, &sid);
        }
        e => panic!("unexpected {e:?}"),
    }
}

#[test]
fn view_only_commands_emit_no_side_effects() {
    let mut s = ClientState::default();
    let initial_zoom = s.view.zoom_px_per_nm;
    assert!(process(&mut s, AppCommand::ZoomIn).is_empty());
    assert!(s.view.zoom_px_per_nm > initial_zoom);
    assert!(process(&mut s, AppCommand::PanByNm { dx: 1.5, dy: -0.5 }).is_empty());
    assert_eq!(s.view.pan_nm, (1.5, -0.5));
    let id = AircraftId::new("ac9");
    process(&mut s, AppCommand::FocusAircraft(id.clone()));
    assert_eq!(s.view.focused_aircraft.as_ref(), Some(&id));
    process(&mut s, AppCommand::ClearFocus);
    assert!(s.view.focused_aircraft.is_none());
}

#[test]
fn student_assume_emits_ws_command() {
    let mut s = ClientState::default();
    let id = AircraftId::new("ac9");
    let effects = process(&mut s, AppCommand::AssumeAircraft { aircraft_id: id.clone() });
    match &effects[0] {
        SideEffect::SendWs(ClientEvent::StudentCommand(env)) => {
            assert!(matches!(
                &env.command,
                StudentCommand::AssumeAircraft { aircraft_id } if aircraft_id == &id
            ));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn trainer_set_path_emits_ws_command() {
    use atc_shared::aircraft::{FlightPath, PathPoint, TargetAltitude};

    let mut s = ClientState::default();
    let id = AircraftId::new("ac9");
    let path = FlightPath {
        points: vec![PathPoint {
            seq: 1,
            x_nm: 1.0,
            y_nm: 2.0,
            target_speed_kt: 120.0,
            target_altitude: TargetAltitude::Gnd,
        }],
        launched: false,
    };
    let effects = process(
        &mut s,
        AppCommand::SetPath {
            aircraft_id: id.clone(),
            path: path.clone(),
        },
    );
    match &effects[0] {
        SideEffect::SendWs(ClientEvent::TrainerCommand(env)) => {
            assert!(matches!(
                &env.command,
                TrainerCommand::SetPath { aircraft_id, path: sent }
                    if aircraft_id == &id && sent == &path
            ));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn disconnect_clears_session_and_emits_close() {
    let mut s = ClientState {
        session: Some(CachedSession::from_snapshot(empty_snapshot())),
        ..ClientState::default()
    };
    let effects = process(&mut s, AppCommand::Disconnect);
    assert!(s.session.is_none());
    assert!(matches!(effects[0], SideEffect::CloseWebSocket));
}

// ---- WS envelope decoder ----

#[test]
fn decode_envelope_round_trips_session_updated() {
    let env = WsEnvelope {
        type_: "session_updated".into(),
        message_id: "m1".into(),
        session_id: SessionId::new("sess_x"),
        sent_at: OffsetDateTime::now_utc(),
        payload: ServerEvent::SessionUpdated(SessionUpdatedPayload {
            status: Some(SessionStatus::Running),
            active_runway: Some("18".into()),
            revision: 5,
        }),
    };
    let json = serde_json::to_string(&env).unwrap();
    let ev = atc_client::core::ws::decode_envelope(&json).unwrap();
    match ev {
        ServerEvent::SessionUpdated(p) => {
            assert_eq!(p.revision, 5);
            assert_eq!(p.active_runway.as_deref(), Some("18"));
        }
        other => panic!("expected SessionUpdated, got {other:?}"),
    }
}
