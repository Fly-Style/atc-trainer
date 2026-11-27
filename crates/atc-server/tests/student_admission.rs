//! Student admission over HTTP — one invitation admits one student, the join
//! response carries the session's sector identity (never geometry), and the
//! issued credentials stay scoped to the session and the student role.

use atc_shared::ids::SectorId;
use atc_shared::protocol::ApiError;
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
async fn valid_join_returns_credentials_and_the_sector_identity_the_host_bound() {
    let (_s, mut c) = start_server().await;
    c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let session = c
        .create_session_with_sector("Ground training", &loaded_sector())
        .await
        .unwrap();
    let position = c
        .create_student_position(
            session.session.session_id.as_str(),
            StudentPositionType::Gnd,
        )
        .await
        .unwrap();
    let joined = c
        .join_student(session.session.session_hash.as_str())
        .await
        .unwrap();
    assert_eq!(joined.session_id, session.session.session_id);
    assert_eq!(joined.position_id, position.position_id);
    assert_eq!(joined.position_type, StudentPositionType::Gnd);
    assert_eq!(joined.sector_id, loaded_sector().sector_id);
    assert_eq!(joined.sector_name, "UKBU");
    let state = session_state(
        &c,
        joined.session_id.as_str(),
        joined.student_token.as_str(),
    )
    .await;
    assert_eq!(state.sector_id, loaded_sector().sector_id);
    assert_eq!(state.sector_name, "UKBU");
}

#[tokio::test]
async fn join_response_carries_credentials_and_identity_but_no_sector_geometry() {
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
    let value = serde_json::to_value(&joined).unwrap();
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut expected = vec![
        "student_token",
        "session_id",
        "position_id",
        "position_type",
        "sector_id",
        "sector_name",
    ];
    let mut actual = keys;
    expected.sort_unstable();
    actual.sort_unstable();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn invalid_invitation_string_is_rejected_as_an_invalid_session_hash() {
    let (_s, c) = start_server().await;
    let response = c.join_student_raw("join-NOPE").await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error.code, "invalid_session_hash");
}

#[tokio::test]
async fn second_admission_to_the_occupied_student_position_is_rejected() {
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
    let hash = session.session.session_hash.as_str();
    c.join_student(hash).await.unwrap();
    let response = c.join_student_raw(hash).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error.code, "student_position_occupied");
}

#[tokio::test]
async fn student_token_cannot_grant_instructor_access() {
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
    let student_token = joined.student_token.as_str();
    let session_id = session.session.session_id.as_str();

    let create_session = c
        .http
        .post(format!("{}/api/v1/sessions", c.http_base))
        .bearer_auth(student_token)
        .json(&serde_json::json!({ "name": "student takeover" }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_session.status(), StatusCode::UNAUTHORIZED);
    let error: ApiError = create_session.json().await.unwrap();
    assert_eq!(error.error.code, "unauthorized");

    let start_session = c
        .http
        .post(format!(
            "{}/api/v1/sessions/{}/start",
            c.http_base, session_id
        ))
        .bearer_auth(student_token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(start_session.status(), StatusCode::UNAUTHORIZED);
    let error: ApiError = start_session.json().await.unwrap();
    assert_eq!(error.error.code, "unauthorized");

    let create_position = c
        .http
        .post(format!(
            "{}/api/v1/sessions/{}/student-position",
            c.http_base, session_id
        ))
        .bearer_auth(student_token)
        .json(&serde_json::json!({ "position_type": "twr" }))
        .send()
        .await
        .unwrap();
    assert_eq!(create_position.status(), StatusCode::UNAUTHORIZED);
    let error: ApiError = create_position.json().await.unwrap();
    assert_eq!(error.error.code, "unauthorized");
}

#[tokio::test]
async fn student_credentials_are_scoped_to_the_intended_session() {
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
    let other_session = c.create_session("Other training").await.unwrap();
    let response = c
        .get_session_raw(
            other_session.session.session_id.as_str(),
            joined.student_token.as_str(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let error: ApiError = response.json().await.unwrap();
    assert_eq!(error.error.code, "forbidden");
}

#[tokio::test]
async fn join_without_a_loaded_sector_returns_the_builtin_identity() {
    let (_s, mut c) = start_server().await;
    c.trainer_login(TEST_TRAINER_HASH).await.unwrap();
    let session = c.create_session("Ground training").await.unwrap();
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
    assert_eq!(joined.sector_id.as_str(), "airport_v1");
    assert_eq!(joined.sector_name, "Training Airport");
}
