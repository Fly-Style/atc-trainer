//! WebSocket adapter: connect to `/api/v1/ws`, decode envelopes into
//! [`ServerEvent`]s, and forward them on a Tokio channel that the iced
//! subscription drains. The pure decoding helper is unit-testable without a
//! live socket.

use atc_shared::ids::SessionId;
use atc_shared::protocol::{ClientEvent, ServerEvent, WsEnvelope};
use futures_util::{SinkExt, StreamExt};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;

#[derive(Debug, Error)]
pub enum WsClientError {
    #[error("connect: {0}")]
    Connect(String),
    #[error("transport: {0}")]
    Transport(String),
    #[error("decode: {0}")]
    Decode(String),
}

#[derive(Clone, Debug)]
pub struct WsCommandSender {
    tx: mpsc::UnboundedSender<ClientEvent>,
}

impl WsCommandSender {
    pub fn send(&self, event: ClientEvent) -> Result<(), WsClientError> {
        self.tx
            .send(event)
            .map_err(|_| WsClientError::Transport("websocket command channel closed".into()))
    }
}

/// A live WebSocket session: incoming `ServerEvent`s arrive on `events`, and
/// dropping `_close` closes the socket via the cancellation channel.
pub struct WsSession {
    pub events: mpsc::UnboundedReceiver<ServerEvent>,
    pub commands: WsCommandSender,
    pub _close: mpsc::Sender<()>,
}

/// Open a WebSocket and spawn the pump task. Returns once the upgrade has
/// succeeded; events arrive asynchronously on the returned receiver.
pub async fn open(
    ws_base: &str,
    token: &str,
    session_id: &SessionId,
) -> Result<WsSession, WsClientError> {
    let url = format!(
        "{}/api/v1/ws?token={}&session_id={}",
        ws_base,
        token,
        session_id.as_str()
    );
    let (stream, _resp) = tokio_tungstenite::connect_async(&url)
        .await
        .map_err(|e| WsClientError::Connect(e.to_string()))?;

    let (tx, rx) = mpsc::unbounded_channel();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel();
    let (close_tx, mut close_rx) = mpsc::channel::<()>(1);
    let session_id = session_id.clone();
    tokio::spawn(async move {
        let (mut sink, mut s) = stream.split();
        loop {
            tokio::select! {
                _ = close_rx.recv() => {
                    let _ = sink.send(WsMessage::Close(None)).await;
                    break;
                }
                outbound = cmd_rx.recv() => {
                    let Some(outbound) = outbound else { break };
                    let envelope = WsEnvelope {
                        type_: client_event_type(&outbound).to_string(),
                        message_id: format!("cli_{}", uuid::Uuid::new_v4().simple()),
                        session_id: session_id.clone(),
                        sent_at: time::OffsetDateTime::now_utc(),
                        payload: outbound,
                    };
                    let text = match serde_json::to_string(&envelope) {
                        Ok(text) => text,
                        Err(_) => continue,
                    };
                    if sink.send(WsMessage::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                msg = s.next() => {
                    let Some(msg) = msg else { break };
                    let Ok(msg) = msg else { break };
                    match msg {
                        WsMessage::Text(t) => match decode_envelope(&t) {
                            Ok(ev) => { if tx.send(ev).is_err() { break; } }
                            Err(_) => continue,
                        },
                        WsMessage::Close(_) => break,
                        _ => continue,
                    }
                }
            }
        }
    });
    Ok(WsSession {
        events: rx,
        commands: WsCommandSender { tx: cmd_tx },
        _close: close_tx,
    })
}

/// Pure decoder used by the pump task and by the unit tests.
pub fn decode_envelope(text: &str) -> Result<ServerEvent, WsClientError> {
    let env: WsEnvelope<ServerEvent> =
        serde_json::from_str(text).map_err(|e| WsClientError::Decode(e.to_string()))?;
    Ok(env.payload)
}

fn client_event_type(event: &ClientEvent) -> &'static str {
    match event {
        ClientEvent::TrainerCommand(_) => "trainer_command",
        ClientEvent::StudentCommand(_) => "student_command",
    }
}
