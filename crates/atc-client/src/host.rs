//! In-process session hosting for the instructor app: the Axum router runs on
//! a dedicated thread inside the desktop process, no separate server binary.

use std::io;
use std::net::TcpListener;

use atc_server::error::AppError;
use atc_server::sessions::rand_suffix;
use atc_server::{build_router, ServerConfig};
use atc_shared::ids::{PositionId, TrainerId};
use atc_shared::role::StudentPositionType;
use atc_shared::sector::SectorIdentity;
use atc_shared::session::{ConnectionState, SessionStatus, StudentPosition};
use axum::Router;
use thiserror::Error;
use tokio::sync::oneshot;

/// Connection details shown to the instructor once hosting is up.
#[derive(Debug)]
pub struct HostInfo {
    pub address: String,
    pub join_string: String,
}

#[derive(Debug, Error)]
pub enum HostError {
    #[error("cannot bind {address}: {source}")]
    Bind { address: String, source: io::Error },
    #[error("cannot prepare the listener: {0}")]
    Listener(#[source] io::Error),
    #[error("cannot start the server runtime: {0}")]
    Runtime(#[source] io::Error),
    #[error("cannot start the host thread: {0}")]
    Thread(#[source] io::Error),
    #[error(transparent)]
    Session(#[from] AppError),
}

#[derive(Debug)]
pub struct HostStartup {
    pub info: HostInfo,
    pub listener: TcpListener,
    pub router: Router,
}

/// Bind the listener and create the session in-process so any failure is
/// reported before an invitation is shown.
pub fn prepare(bind_address: &str, identity: &SectorIdentity) -> Result<HostStartup, HostError> {
    let listener = TcpListener::bind(bind_address).map_err(|source| HostError::Bind {
        address: bind_address.to_owned(),
        source,
    })?;
    let addr = listener.local_addr().map_err(HostError::Listener)?;
    // tokio refuses to register a blocking socket with its runtime.
    listener
        .set_nonblocking(true)
        .map_err(HostError::Listener)?;
    let config = ServerConfig {
        bind_address: addr.to_string(),
        trainer_hashes: Vec::new(),
        log_dir: None,
        builtin_scenario_toml: None,
    };
    let (router, state) = build_router(config);
    let trainer_id = TrainerId::new(format!("tr_{}", rand_suffix(6)));
    state.tokens.issue_trainer(trainer_id.clone());
    let session = state.registry.create(
        &state.config,
        identity.name.clone(),
        trainer_id,
        Some(identity.clone()),
    )?;
    let join_string = {
        let mut record = session.lock().unwrap();
        // Mirrors http::create_student_position without a self-HTTP round trip.
        record.student_position = Some(StudentPosition {
            position_id: PositionId::new(format!("pos_{}", rand_suffix(6))),
            position_type: StudentPositionType::Gnd,
            occupied: false,
            connection_state: ConnectionState::Disconnected,
        });
        record.status = SessionStatus::WaitingForStudent;
        record.revision += 1;
        record.session_hash.as_str().to_owned()
    };
    Ok(HostStartup {
        info: HostInfo {
            address: format!("http://{addr}"),
            join_string,
        },
        listener,
        router,
    })
}

/// Serve for the process lifetime on a thread owning its runtime. The startup
/// outcome is reported through the returned receiver before accepting traffic.
pub fn spawn(
    identity: SectorIdentity,
) -> Result<oneshot::Receiver<Result<HostInfo, HostError>>, HostError> {
    let (ready, receiver) = oneshot::channel();
    std::thread::Builder::new()
        .name("atc-host".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready.send(Err(HostError::Runtime(error)));
                    return;
                }
            };
            runtime.block_on(async move {
                let startup = match prepare("127.0.0.1:0", &identity) {
                    Ok(startup) => startup,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let listener = match tokio::net::TcpListener::from_std(startup.listener) {
                    Ok(listener) => listener,
                    Err(error) => {
                        let _ = ready.send(Err(HostError::Listener(error)));
                        return;
                    }
                };
                let _ = ready.send(Ok(startup.info));
                let _ = axum::serve(listener, startup.router).await;
            });
        })
        .map_err(HostError::Thread)?;
    Ok(receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use atc_shared::ids::{SectorId, SessionHash};
    use atc_shared::protocol::{JoinStudentRequest, JoinStudentResponse};
    use atc_shared::session::SessionState;

    fn identity() -> SectorIdentity {
        SectorIdentity {
            sector_id: SectorId::new("aurora-0123456789abcdef"),
            name: "UKBU".into(),
        }
    }

    async fn start_hosted() -> HostInfo {
        spawn(identity()).unwrap().await.unwrap().unwrap()
    }

    async fn join_student(info: &HostInfo) -> JoinStudentResponse {
        reqwest::Client::new()
            .post(format!("{}/api/v1/sessions/join-student", info.address))
            .json(&JoinStudentRequest {
                session_hash: SessionHash::new(info.join_string.clone()),
            })
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    #[test]
    fn startup_fails_when_the_bind_address_is_invalid() {
        let error = prepare("not-an-address", &identity()).unwrap_err();
        assert!(matches!(error, HostError::Bind { .. }), "{error}");
        assert!(error.to_string().contains("not-an-address"), "{error}");
    }

    #[test]
    fn startup_reports_a_loopback_address_and_join_string() {
        let startup = prepare("127.0.0.1:0", &identity()).unwrap();
        assert!(
            startup.info.address.starts_with("http://127.0.0.1:"),
            "{}",
            startup.info.address
        );
        assert!(
            startup.info.join_string.starts_with("join-"),
            "{}",
            startup.info.join_string
        );
    }

    #[tokio::test]
    async fn hosted_session_accepts_a_student_join_over_http() {
        let info = start_hosted().await;
        let joined = join_student(&info).await;
        assert_eq!(joined.position_type, StudentPositionType::Gnd);
    }

    #[tokio::test]
    async fn hosted_session_reports_the_loaded_sector_identity() {
        let info = start_hosted().await;
        let joined = join_student(&info).await;
        let state: SessionState = reqwest::Client::new()
            .get(format!(
                "{}/api/v1/sessions/{}",
                info.address, joined.session_id
            ))
            .bearer_auth(joined.student_token.as_str())
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(state.sector_id, identity().sector_id);
        assert_eq!(state.sector_name, "UKBU");
    }
}
