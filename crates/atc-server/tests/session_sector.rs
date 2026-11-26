//! Session sector binding — a session carries the identity of the sector the
//! instructor loaded, and falls back to the builtin airport without one.

use atc_shared::ids::SectorId;
use atc_shared::protocol::CreateSessionResponse;
use atc_shared::role::StudentPositionType;
use atc_shared::sector::SectorIdentity;
use atc_shared::session::SessionState;
use atc_test_support::client::TestClient;
use atc_test_support::fixtures::{base_config, TEST_TRAINER_HASH};
use atc_test_support::server::TestServer;
use reqwest::StatusCode;

fn loaded_sector() -> SectorIdentity {
    SectorIdentity {
        sector_id: SectorId::new("aurora-0123456789abcdef"),
        name: "UKBU".into(),
    }
}

async fn start_server() -> (TestServer, TestClient) {
    let server = TestServer::start(base_config()).await;
    let client = TestClient::new(server.http_base(), server.ws_base());
    (server, client)
}

async fn session_state(client: &TestClient, session_id: &str, token: &str) -> SessionState {
    let response = client.get_session_raw(session_id, token).await.unwrap();
    assert!(response.status().is_success());
    serde_json::from_slice(&response.bytes().await.unwrap()).unwrap()
}

#[tokio::test]
async fn create_session_with_identity_binds_the_loaded_sector() {
    let (_s, mut c) = start_server().await;
    let login = c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let session = c
        .create_session_with_sector("Ground training", &loaded_sector())
        .await
        .unwrap();
    let state = session_state(
        &c,
        session.session.session_id.as_str(),
        login.trainer_token.as_str(),
    )
    .await;
    assert_eq!(state.sector_id, loaded_sector().sector_id);
    assert_eq!(state.sector_name, "UKBU");
}

#[tokio::test]
async fn create_session_without_identity_defaults_to_the_builtin_sector() {
    let (_s, mut c) = start_server().await;
    let login = c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let session = c.create_session("Ground training").await.unwrap();
    let state = session_state(
        &c,
        session.session.session_id.as_str(),
        login.trainer_token.as_str(),
    )
    .await;
    assert_eq!(state.sector_id.as_str(), "airport_v1");
    assert_eq!(state.sector_name, "Training Airport");
}

#[tokio::test]
async fn partial_sector_identity_falls_back_to_the_builtin_sector() {
    let (_s, mut c) = start_server().await;
    let login = c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let response = c
        .http
        .post(format!("{}/api/v1/sessions", c.http_base))
        .bearer_auth(login.trainer_token.as_str())
        .json(&serde_json::json!({ "name": "s", "sector_id": "aurora-0123456789abcdef" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: CreateSessionResponse = response.json().await.unwrap();
    let state = session_state(
        &c,
        created.session.session_id.as_str(),
        login.trainer_token.as_str(),
    )
    .await;
    assert_eq!(state.sector_id.as_str(), "airport_v1");
}

#[tokio::test]
async fn student_can_join_a_session_bound_to_a_loaded_sector() {
    let (_s, mut c) = start_server().await;
    c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let session = c
        .create_session_with_sector("Ground training", &loaded_sector())
        .await
        .unwrap();
    c.create_student_position(
        session.session.session_id.as_str(),
        StudentPositionType::Gnd,
    )
    .await
    .unwrap();
    let joined = c
        .join_student(session.session.session_hash.as_str())
        .await
        .unwrap();
    assert_eq!(joined.position_type, StudentPositionType::Gnd);
    let state = session_state(
        &c,
        joined.session_id.as_str(),
        joined.student_token.as_str(),
    )
    .await;
    assert_eq!(state.sector_id, loaded_sector().sector_id);
}
