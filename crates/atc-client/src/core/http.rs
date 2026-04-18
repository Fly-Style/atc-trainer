//! HTTP adapter functions used by the iced runtime to execute the
//! [`SideEffect::Http`] descriptors emitted by the command processor.
//!
//! Returning typed `Result`s here means the UI layer just spawns these as
//! `iced::Task::perform` futures and converts the outcome into a follow-up
//! message. Test coverage for the request *shape* lives at the command
//! processor level; these functions are integration-tested through
//! `atc-test-support` against the real server.

use crate::core::command::{HttpCorrelation, HttpRequestSpec};
use atc_shared::protocol::*;
use atc_shared::sector::SectorMetadata;
use serde::de::DeserializeOwned;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("transport: {0}")]
    Transport(String),
    #[error("status {status}: {body}")]
    Status { status: u16, body: String },
    #[error("parse: {0}")]
    Parse(String),
}

impl From<reqwest::Error> for HttpError {
    fn from(value: reqwest::Error) -> Self {
        HttpError::Transport(value.to_string())
    }
}

impl From<serde_json::Error> for HttpError {
    fn from(value: serde_json::Error) -> Self {
        HttpError::Parse(value.to_string())
    }
}

/// Outcome of an HTTP request, paired back to the originating UI command via
/// the correlation tag carried in the request spec.
#[derive(Debug, Clone)]
pub enum HttpOutcome {
    TrainerLogin(TrainerLoginResponse),
    CreateSession(CreateSessionResponse),
    CreateStudentPosition(CreateStudentPositionResponse),
    JoinStudent(JoinStudentResponse),
    StartSession(StartSessionResponse),
    LoadSector(SectorMetadata),
}

#[derive(Clone)]
pub struct HttpClient {
    pub base: String,
    pub bearer: Option<String>,
    inner: reqwest::Client,
}

impl HttpClient {
    pub fn new(base: String) -> Self {
        Self {
            base,
            bearer: None,
            inner: reqwest::Client::new(),
        }
    }

    pub fn with_bearer(mut self, token: Option<String>) -> Self {
        self.bearer = token;
        self
    }

    /// Execute a request spec produced by `core::command::process` and
    /// return the typed outcome.
    pub async fn perform(&self, spec: HttpRequestSpec) -> Result<HttpOutcome, HttpError> {
        match spec.correlation {
            HttpCorrelation::TrainerLogin => {
                let r: TrainerLoginResponse =
                    self.post_json("/api/v1/auth/trainer-login", &spec.body, false).await?;
                Ok(HttpOutcome::TrainerLogin(r))
            }
            HttpCorrelation::CreateSession => {
                let r: CreateSessionResponse =
                    self.post_json("/api/v1/sessions", &spec.body, spec.with_auth).await?;
                Ok(HttpOutcome::CreateSession(r))
            }
            HttpCorrelation::CreateStudentPosition { session_id } => {
                let path = format!("/api/v1/sessions/{}/student-position", session_id.as_str());
                let r: CreateStudentPositionResponse =
                    self.post_json(&path, &spec.body, spec.with_auth).await?;
                Ok(HttpOutcome::CreateStudentPosition(r))
            }
            HttpCorrelation::JoinStudent => {
                let r: JoinStudentResponse =
                    self.post_json("/api/v1/sessions/join-student", &spec.body, false).await?;
                Ok(HttpOutcome::JoinStudent(r))
            }
            HttpCorrelation::StartSession { session_id } => {
                let path = format!("/api/v1/sessions/{}/start", session_id.as_str());
                let r: StartSessionResponse =
                    self.post_json(&path, &spec.body, spec.with_auth).await?;
                Ok(HttpOutcome::StartSession(r))
            }
            HttpCorrelation::LoadSector { sector_id } => {
                let path = format!("/api/v1/sector/{}", sector_id.as_str());
                let r: SectorMetadata = self.get_json(&path, spec.with_auth).await?;
                Ok(HttpOutcome::LoadSector(r))
            }
        }
    }

    async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        with_auth: bool,
    ) -> Result<T, HttpError> {
        let mut req = self.inner.post(format!("{}{}", self.base, path)).json(body);
        if with_auth {
            if let Some(t) = &self.bearer {
                req = req.bearer_auth(t);
            }
        }
        decode(req.send().await?).await
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str, with_auth: bool) -> Result<T, HttpError> {
        let mut req = self.inner.get(format!("{}{}", self.base, path));
        if with_auth {
            if let Some(t) = &self.bearer {
                req = req.bearer_auth(t);
            }
        }
        decode(req.send().await?).await
    }
}

async fn decode<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, HttpError> {
    let status = resp.status();
    let bytes = resp.bytes().await?;
    if !status.is_success() {
        return Err(HttpError::Status {
            status: status.as_u16(),
            body: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }
    Ok(serde_json::from_slice(&bytes)?)
}
