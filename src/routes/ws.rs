use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::{auth::jwt::decode_token, state::AppState};

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    // Browsers can't set an Authorization header on the WS handshake, so the
    // JWT travels as a query param here instead.
    pub token: String,
    // Comma-separated topic names (e.g. "renewals-board") this connection
    // additionally wants broadcasts for, on top of its own per-user
    // notifications. Optional — most connections (e.g. the notification
    // bell) don't subscribe to any topic.
    pub topics: Option<String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
) -> Response {
    let Ok(claims) = decode_token(&query.token, &state.config.jwt_secret) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(user_id) = bson::oid::ObjectId::parse_str(&claims.sub) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let topics: Vec<String> = query
        .topics
        .as_deref()
        .map(|t| t.split(',').map(str::trim).filter(|t| !t.is_empty()).map(String::from).collect())
        .unwrap_or_default();

    ws.on_upgrade(move |socket| handle_socket(socket, state, user_id, topics))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, user_id: bson::oid::ObjectId, topics: Vec<String>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    {
        let mut hub = state.ws_hub.lock().await;
        hub.entry(user_id).or_default().push(tx.clone());
    }
    {
        let mut topic_hub = state.topic_hub.lock().await;
        for topic in &topics {
            topic_hub.entry(topic.clone()).or_default().push(tx.clone());
        }
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
            outgoing = rx.recv() => {
                match outgoing {
                    Some(msg) => {
                        if socket.send(Message::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    let mut hub = state.ws_hub.lock().await;
    if let Some(senders) = hub.get_mut(&user_id) {
        senders.retain(|s| !s.same_channel(&tx));
        if senders.is_empty() {
            hub.remove(&user_id);
        }
    }
    drop(hub);

    if !topics.is_empty() {
        let mut topic_hub = state.topic_hub.lock().await;
        for topic in &topics {
            if let Some(senders) = topic_hub.get_mut(topic) {
                senders.retain(|s| !s.same_channel(&tx));
                if senders.is_empty() {
                    topic_hub.remove(topic);
                }
            }
        }
    }
}
