//! Phase 2 integration tests — multi-client sync.
//!
//! Covers cases listed in the Testing Strategy § Phase 2 block of the TDD:
//! two WebSocket clients on one session both receive the initial snapshot,
//! and a server-side state change broadcasts identical events to both clients.

use atc_shared::ids::SessionId;
use atc_shared::protocol::{ServerEvent, WsEnvelope};
use atc_shared::role::StudentPositionType;
use atc_test_support::client::{next_ws_text, TestClient, WsStream};
use atc_test_support::fixtures::{base_config, TEST_TRAINER_HASH};
use atc_test_support::server::TestServer;

async fn drain_initial(stream: &mut WsStream) -> (ServerEvent, ServerEvent) {
    let hello: WsEnvelope<ServerEvent> =
        serde_json::from_str(&next_ws_text(stream).await.expect("hello")).unwrap();
    let snap: WsEnvelope<ServerEvent> =
        serde_json::from_str(&next_ws_text(stream).await.expect("snapshot")).unwrap();
    (hello.payload, snap.payload)
}

#[tokio::test]
async fn two_clients_receive_initial_snapshot() {
    let server = TestServer::start(base_config()).await;
    let mut a = TestClient::new(server.http_base(), server.ws_base());
    let mut b = TestClient::new(server.http_base(), server.ws_base());

    let trainer = a.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let sess = a.create_session("two-clients").await.unwrap();
    let sid = sess.session.session_id.as_str().to_string();
    a.create_student_position(&sid, StudentPositionType::Gnd)
        .await
        .unwrap();
    let joined = b.join_student(sess.session.session_hash.as_str()).await.unwrap();

    let mut ws_a = a.connect_ws(trainer.trainer_token.as_str(), &sid).await.unwrap();
    let mut ws_b = b.connect_ws(joined.student_token.as_str(), &sid).await.unwrap();

    let (_hello_a, snap_a) = drain_initial(&mut ws_a).await;
    let (_hello_b, snap_b) = drain_initial(&mut ws_b).await;

    let (state_a, state_b) = match (snap_a, snap_b) {
        (ServerEvent::SessionState(a), ServerEvent::SessionState(b)) => (a, b),
        other => panic!("expected snapshots, got {:?}", other),
    };
    assert_eq!(state_a.session_id.as_str(), sid);
    assert_eq!(state_b.session_id.as_str(), sid);
    assert_eq!(state_a.revision, state_b.revision);
    assert_eq!(state_a.aircraft.len(), state_b.aircraft.len());
    assert_eq!(state_a.active_runway, state_b.active_runway);
    assert_eq!(state_a.status, state_b.status);
}

#[tokio::test]
async fn server_state_change_broadcasts_identical_event() {
    let server = TestServer::start(base_config()).await;
    let mut a = TestClient::new(server.http_base(), server.ws_base());
    let mut b = TestClient::new(server.http_base(), server.ws_base());

    let trainer = a.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let sess = a.create_session("broadcast-eq").await.unwrap();
    let sid = sess.session.session_id.as_str().to_string();
    a.create_student_position(&sid, StudentPositionType::Twr).await.unwrap();
    let joined = b.join_student(sess.session.session_hash.as_str()).await.unwrap();

    let mut ws_a = a.connect_ws(trainer.trainer_token.as_str(), &sid).await.unwrap();
    let mut ws_b = b.connect_ws(joined.student_token.as_str(), &sid).await.unwrap();

    // Drain Hello + initial snapshot on both.
    let _ = drain_initial(&mut ws_a).await;
    let _ = drain_initial(&mut ws_b).await;

    // Trigger a server-side state change via the registry.
    let arc = server
        .state
        .registry
        .get(&SessionId::new(sid.clone()))
        .expect("session exists");
    let new_revision = {
        let mut rec = arc.lock().unwrap();
        rec.bump_and_broadcast(None, Some("18".to_string()))
    };

    // Both clients receive the same SessionUpdated event.
    let env_a: WsEnvelope<ServerEvent> =
        serde_json::from_str(&next_ws_text(&mut ws_a).await.expect("a recv")).unwrap();
    let env_b: WsEnvelope<ServerEvent> =
        serde_json::from_str(&next_ws_text(&mut ws_b).await.expect("b recv")).unwrap();
    assert_eq!(env_a.type_, "session_updated");
    assert_eq!(env_b.type_, "session_updated");
    let (a_payload, b_payload) = match (env_a.payload, env_b.payload) {
        (ServerEvent::SessionUpdated(a), ServerEvent::SessionUpdated(b)) => (a, b),
        other => panic!("expected session_updated, got {:?}", other),
    };
    assert_eq!(a_payload.revision, new_revision);
    assert_eq!(b_payload.revision, new_revision);
    assert_eq!(a_payload.active_runway.as_deref(), Some("18"));
    assert_eq!(b_payload.active_runway.as_deref(), Some("18"));
    assert_eq!(a_payload.status, b_payload.status);
}
