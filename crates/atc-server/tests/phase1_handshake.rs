//! Phase 1 integration tests — HTTP and WS handshake.
//!
//! Covers cases listed in the Testing Strategy § Phase 1 block of the TDD:
//! trainer auth, session create, student position create for GND and TWR,
//! join by hash (valid/invalid), unknown session 404, status transitions
//! draft → waiting_for_student → running, WS connect with/without auth,
//! and scenario TOML populating the initial aircraft collection.

use atc_shared::protocol::{ServerEvent, WsEnvelope};
use atc_shared::role::StudentPositionType;
use atc_shared::session::SessionStatus;
use atc_test_support::client::{next_ws_text, TestClient};
use atc_test_support::fixtures::{base_config, TEST_TRAINER_HASH};
use atc_test_support::server::TestServer;
use reqwest::StatusCode;

async fn start_server() -> (TestServer, TestClient) {
    let server = TestServer::start(base_config()).await;
    let client = TestClient::new(server.http_base(), server.ws_base());
    (server, client)
}

#[tokio::test]
async fn trainer_login_valid_returns_200_and_token() {
    let (_s, mut c) = start_server().await;
    let resp = c.trainer_login(TEST_TRAINER_HASH).await.expect("login ok");
    assert!(!resp.trainer_token.as_str().is_empty());
    assert!(!resp.trainer_id.as_str().is_empty());
}

#[tokio::test]
async fn trainer_login_invalid_returns_401() {
    let (_s, c) = start_server().await;
    let resp = c.trainer_login_raw("wrong-hash").await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_session_returns_ids() {
    let (_s, mut c) = start_server().await;
    c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let sess = c.create_session("Evening training").await.unwrap();
    assert!(!sess.session.session_id.as_str().is_empty());
    assert!(!sess.session.session_hash.as_str().is_empty());
    assert_eq!(sess.session.status, SessionStatus::Draft);
}

#[tokio::test]
async fn create_student_position_supports_gnd_and_twr() {
    let (_s, mut c) = start_server().await;
    c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let gnd_session = c.create_session("g").await.unwrap();
    let gnd = c
        .create_student_position(gnd_session.session.session_id.as_str(), StudentPositionType::Gnd)
        .await
        .unwrap();
    assert_eq!(gnd.position_type, StudentPositionType::Gnd);

    let twr_session = c.create_session("t").await.unwrap();
    let twr = c
        .create_student_position(twr_session.session.session_id.as_str(), StudentPositionType::Twr)
        .await
        .unwrap();
    assert_eq!(twr.position_type, StudentPositionType::Twr);
}

#[tokio::test]
async fn join_valid_hash_accepted() {
    let (_s, mut c) = start_server().await;
    c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let sess = c.create_session("s").await.unwrap();
    c.create_student_position(sess.session.session_id.as_str(), StudentPositionType::Gnd)
        .await
        .unwrap();
    let joined = c.join_student(sess.session.session_hash.as_str()).await.unwrap();
    assert_eq!(joined.session_id, sess.session.session_id);
    assert_eq!(joined.position_type, StudentPositionType::Gnd);
}

#[tokio::test]
async fn join_wrong_hash_rejected() {
    let (_s, c) = start_server().await;
    let resp = c.join_student_raw("join-DOES_NOT_EXIST").await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_unknown_session_returns_404() {
    let (_s, mut c) = start_server().await;
    let login = c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let resp = c
        .get_session_raw("sess_unknown", login.trainer_token.as_str())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn session_status_transitions_draft_to_running() {
    let (_s, mut c) = start_server().await;
    let login = c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let sess = c.create_session("s").await.unwrap();
    let sid = sess.session.session_id.as_str().to_string();

    // After create: draft.
    let state: atc_shared::session::SessionState = serde_json::from_slice(
        &c.get_session_raw(&sid, login.trainer_token.as_str())
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.status, SessionStatus::Draft);

    // After creating the student position: waiting_for_student.
    c.create_student_position(&sid, StudentPositionType::Gnd).await.unwrap();
    let state: atc_shared::session::SessionState = serde_json::from_slice(
        &c.get_session_raw(&sid, login.trainer_token.as_str())
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(state.status, SessionStatus::WaitingForStudent);

    // After start: running.
    let started = c.start_session(&sid).await.unwrap();
    assert_eq!(started.status, SessionStatus::Running);
}

#[tokio::test]
async fn ws_connect_with_valid_auth_delivers_hello_and_snapshot() {
    let (_s, mut c) = start_server().await;
    let login = c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let sess = c.create_session("s").await.unwrap();
    let sid = sess.session.session_id.as_str().to_string();
    c.create_student_position(&sid, StudentPositionType::Gnd).await.unwrap();

    let mut ws = c
        .connect_ws(login.trainer_token.as_str(), &sid)
        .await
        .expect("ws connect");

    let hello_text = next_ws_text(&mut ws).await.expect("hello frame");
    let hello: WsEnvelope<ServerEvent> = serde_json::from_str(&hello_text).unwrap();
    assert!(matches!(hello.payload, ServerEvent::Hello(_)));

    let snap_text = next_ws_text(&mut ws).await.expect("snapshot frame");
    let snap: WsEnvelope<ServerEvent> = serde_json::from_str(&snap_text).unwrap();
    match snap.payload {
        ServerEvent::SessionState(state) => {
            assert_eq!(state.session_id.as_str(), sid);
            assert!(!state.aircraft.is_empty(), "scenario aircraft should be present");
        }
        other => panic!("expected session_state, got {:?}", other),
    }
}

#[tokio::test]
async fn ws_connect_without_auth_rejected() {
    let (_s, c) = start_server().await;
    let err = c.connect_ws_raw("token=bad&session_id=sess_missing").await;
    assert!(err.is_err(), "connect without valid token must be rejected");
}

#[tokio::test]
async fn scenario_toml_populates_initial_aircraft() {
    let (_s, mut c) = start_server().await;
    let login = c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let sess = c.create_session("s").await.unwrap();
    let state: atc_shared::session::SessionState = serde_json::from_slice(
        &c.get_session_raw(sess.session.session_id.as_str(), login.trainer_token.as_str())
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    // The bundled scenario ships 7 aircraft; assert on a few canonical callsigns.
    assert!(!state.aircraft.is_empty());
    assert!(state.aircraft.iter().any(|a| a.callsign == "YL-VFR"));
    assert!(state.aircraft.iter().any(|a| a.callsign == "BTI201"));
    assert_eq!(state.active_runway, "36");
}
