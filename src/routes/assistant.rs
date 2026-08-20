use std::time::Duration;

use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    state::AppState,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssistantChatRequest {
    pub message: String,
    #[serde(default)]
    pub history: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssistantChatResponse {
    pub reply: String,
}

// Thin authenticated proxy into the ai-service's assistant. `_auth` gates
// this route to logged-in staff same as everything else; separately, the
// caller's *raw* bearer token is forwarded to ai-service as `X-User-Token` so
// the assistant's tool calls back into the Rust API run as that real staff
// member — never a minted system-user token — and therefore automatically
// inherit whatever row-level visibility applies to them (visibility.rs).
// Using a system token here would be a privilege-escalation hole: the
// assistant would see every record regardless of who asked.
pub async fn chat(
    State(state): State<AppState>,
    _auth: AuthUser,
    headers: HeaderMap,
    Json(input): Json<AssistantChatRequest>,
) -> ApiResult<Json<AssistantChatResponse>> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/assistant/chat", state.config.ai_service_url))
        .timeout(Duration::from_secs(60))
        .header("X-User-Token", token)
        .json(&input)
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    if !response.status().is_success() {
        // Surfaced as a normal chat reply, not an HTTP error — a friendly
        // inline message in the conversation is better UX than the frontend
        // having to special-case a failed request just for the assistant.
        let status = response.status();
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        let detail = body.get("detail").and_then(|v| v.as_str()).unwrap_or("please try again shortly");
        tracing::warn!("ai-service assistant error ({status}): {detail}");
        return Ok(Json(AssistantChatResponse {
            reply: format!("Sorry, I couldn't answer that — {detail}"),
        }));
    }

    let parsed: AssistantChatResponse = response.json().await.map_err(|e| ApiError::Internal(e.into()))?;
    Ok(Json(parsed))
}
