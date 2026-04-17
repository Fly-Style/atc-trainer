use atc_shared::protocol::*;
use futures_util::StreamExt;
use reqwest::StatusCode;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tokio_tungstenite::MaybeTlsStream;

pub type WsStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

pub struct TestClient {
    pub http_base: String,
    pub ws_base: String,
    pub http: reqwest::Client,
    pub trainer_token: Option<String>,
    pub student_token: Option<String>,
}

impl TestClient {
    pub fn new(http_base: String, ws_base: String) -> Self {
        Self {
            http_base,
            ws_base,
            http: reqwest::Client::new(),
            trainer_token: None,
            student_token: None,
        }
    }

    pub async fn trainer_login(&mut self, hash: &str) -> Result<TrainerLoginResponse, HttpError> {
        let url = format!("{}/api/v1/auth/trainer-login", self.http_base);
        let resp = self
            .http
            .post(url)
            .json(&TrainerLoginRequest { trainer_hash: hash.to_string() })
            .send()
            .await?;
        let status = resp.status();
        let bytes = resp.bytes().await?;
        if !status.is_success() {
            return Err(HttpError::Status { status, body: String::from_utf8_lossy(&bytes).into_owned() });
        }
        let parsed: TrainerLoginResponse = serde_json::from_slice(&bytes)?;
        self.trainer_token = Some(parsed.trainer_token.as_str().to_string());
        Ok(parsed)
    }

    pub async fn trainer_login_raw(&self, hash: &str) -> Result<reqwest::Response, reqwest::Error> {
        let url = format!("{}/api/v1/auth/trainer-login", self.http_base);
        self.http
            .post(url)
            .json(&TrainerLoginRequest { trainer_hash: hash.to_string() })
            .send()
            .await
    }

    pub async fn create_session(&self, name: &str) -> Result<CreateSessionResponse, HttpError> {
        let url = format!("{}/api/v1/sessions", self.http_base);
        let resp = self
            .http
            .post(url)
            .bearer_auth(self.trainer_token.as_ref().expect("trainer login first"))
            .json(&CreateSessionRequest { name: name.to_string() })
            .send()
            .await?;
        parse_json(resp).await
    }

    pub async fn create_student_position(
        &self,
        session_id: &str,
        position_type: atc_shared::role::StudentPositionType,
    ) -> Result<CreateStudentPositionResponse, HttpError> {
        let url = format!("{}/api/v1/sessions/{}/student-position", self.http_base, session_id);
        let resp = self
            .http
            .post(url)
            .bearer_auth(self.trainer_token.as_ref().expect("trainer token"))
            .json(&CreateStudentPositionRequest { position_type })
            .send()
            .await?;
        parse_json(resp).await
    }

    pub async fn start_session(
        &self,
        session_id: &str,
    ) -> Result<StartSessionResponse, HttpError> {
        let url = format!("{}/api/v1/sessions/{}/start", self.http_base, session_id);
        let resp = self
            .http
            .post(url)
            .bearer_auth(self.trainer_token.as_ref().expect("trainer token"))
            .json(&StartSessionRequest {})
            .send()
            .await?;
        parse_json(resp).await
    }

    pub async fn get_session_raw(
        &self,
        session_id: &str,
        token: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let url = format!("{}/api/v1/sessions/{}", self.http_base, session_id);
        self.http.get(url).bearer_auth(token).send().await
    }

    pub async fn join_student(
        &mut self,
        session_hash: &str,
    ) -> Result<JoinStudentResponse, HttpError> {
        let url = format!("{}/api/v1/sessions/join-student", self.http_base);
        let resp = self
            .http
            .post(url)
            .json(&JoinStudentRequest {
                session_hash: atc_shared::ids::SessionHash::new(session_hash),
            })
            .send()
            .await?;
        let parsed: JoinStudentResponse = parse_json(resp).await?;
        self.student_token = Some(parsed.student_token.as_str().to_string());
        Ok(parsed)
    }

    pub async fn join_student_raw(
        &self,
        session_hash: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let url = format!("{}/api/v1/sessions/join-student", self.http_base);
        self.http
            .post(url)
            .json(&JoinStudentRequest {
                session_hash: atc_shared::ids::SessionHash::new(session_hash),
            })
            .send()
            .await
    }

    pub async fn connect_ws(
        &self,
        token: &str,
        session_id: &str,
    ) -> Result<WsStream, WsError> {
        let url = format!("{}/api/v1/ws?token={}&session_id={}", self.ws_base, token, session_id);
        let (stream, _resp) = tokio_tungstenite::connect_async(url).await?;
        Ok(stream)
    }

    pub async fn connect_ws_raw(
        &self,
        query: &str,
    ) -> Result<WsStream, WsError> {
        let url = format!("{}/api/v1/ws?{}", self.ws_base, query);
        let (stream, _resp) = tokio_tungstenite::connect_async(url).await?;
        Ok(stream)
    }
}

async fn parse_json<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T, HttpError> {
    let status = resp.status();
    let bytes = resp.bytes().await?;
    if !status.is_success() {
        return Err(HttpError::Status { status, body: String::from_utf8_lossy(&bytes).into_owned() });
    }
    Ok(serde_json::from_slice(&bytes)?)
}

pub async fn next_ws_text(stream: &mut WsStream) -> Option<String> {
    while let Some(msg) = stream.next().await {
        match msg.ok()? {
            WsMessage::Text(t) => return Some(t.to_string()),
            WsMessage::Binary(_) | WsMessage::Ping(_) | WsMessage::Pong(_) | WsMessage::Frame(_) => continue,
            WsMessage::Close(_) => return None,
        }
    }
    None
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("http status {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error(transparent)]
    Transport(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum WsError {
    #[error(transparent)]
    Tungstenite(#[from] tokio_tungstenite::tungstenite::Error),
}
