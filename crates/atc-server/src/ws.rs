use crate::app::AppState;
use crate::auth::TokenSubject;
use atc_shared::ids::{ConnectionId, SessionId};
use atc_shared::protocol::{HelloPayload, ServerEvent, WsEnvelope};
use atc_shared::role::Role;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String,
    pub session_id: String,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(q): Query<WsQuery>,
) -> Response {
    // Validate the token + session before upgrading.
    let subject = match state.tokens.lookup(&q.token) {
        Some(s) => s,
        None => return (StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
    };
    let session_id = SessionId::new(q.session_id);
    let arc = match state.registry.get(&session_id) {
        Some(a) => a,
        None => return (StatusCode::NOT_FOUND, "session_not_found").into_response(),
    };
    // Student tokens must be bound to this exact session.
    if let TokenSubject::Student { session_id: bound, .. } = &subject {
        if bound != &session_id {
            return (StatusCode::FORBIDDEN, "forbidden").into_response();
        }
    }
    let role = match subject {
        TokenSubject::Trainer { .. } => Role::Trainer,
        TokenSubject::Student { .. } => Role::Student,
    };

    let receiver = arc.lock().unwrap().broadcaster.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, arc, session_id, role, receiver))
}

async fn handle_socket(
    socket: WebSocket,
    arc: std::sync::Arc<std::sync::Mutex<crate::sessions::SessionRecord>>,
    session_id: SessionId,
    role: Role,
    mut receiver: tokio::sync::broadcast::Receiver<ServerEvent>,
) {
    let (mut sink, mut stream) = socket.split();

    let connection_id = ConnectionId::new(format!("conn_{}", crate::sessions::rand_suffix(8)));

    // Send hello.
    let hello = ServerEvent::Hello(HelloPayload {
        connection_id: connection_id.clone(),
        role,
    });
    if send_event(&mut sink, &session_id, &hello).await.is_err() {
        return;
    }

    // Send full session snapshot.
    let snapshot = {
        let rec = arc.lock().unwrap();
        ServerEvent::SessionState(Box::new(rec.to_state()))
    };
    if send_event(&mut sink, &session_id, &snapshot).await.is_err() {
        return;
    }

    // Pump: forward broadcast events to client, drop inbound client msgs (Phase 1 no commands).
    loop {
        tokio::select! {
            ev = receiver.recv() => {
                match ev {
                    Ok(ev) => {
                        if send_event(&mut sink, &session_id, &ev).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => continue,
                    Some(Err(_)) => break,
                }
            }
        }
    }
}

async fn send_event<S>(
    sink: &mut S,
    session_id: &SessionId,
    ev: &ServerEvent,
) -> Result<(), ()>
where
    S: SinkExt<Message> + Unpin,
    <S as futures_util::Sink<Message>>::Error: std::fmt::Debug,
{
    let type_name = server_event_type(ev);
    let envelope = WsEnvelope {
        type_: type_name.to_string(),
        message_id: format!("srv_{}", crate::sessions::rand_suffix(10)),
        session_id: session_id.clone(),
        sent_at: OffsetDateTime::now_utc(),
        payload: ev,
    };
    let text = serde_json::to_string(&envelope).map_err(|_| ())?;
    sink.send(Message::Text(text.into())).await.map_err(|_| ())
}

fn server_event_type(ev: &ServerEvent) -> &'static str {
    match ev {
        ServerEvent::Hello(_) => "hello",
        ServerEvent::SessionState(_) => "session_state",
        ServerEvent::SessionUpdated(_) => "session_updated",
        ServerEvent::AircraftCreated(_) => "aircraft_created",
        ServerEvent::AircraftUpdated(_) => "aircraft_updated",
        ServerEvent::AircraftRemoved(_) => "aircraft_removed",
        ServerEvent::CommandRejected(_) => "command_rejected",
        ServerEvent::SystemNotice(_) => "system_notice",
    }
}
