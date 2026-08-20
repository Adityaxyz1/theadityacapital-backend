use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use bson::{doc, oid::ObjectId};
use chrono::Utc;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    models::{Message, MessageResponse, MessageThreadSummary, SendMessageInput, User},
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<ObjectId> {
    ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

// Private DMs are deliberately NOT visibility-scoped or admin-bypassed the
// way business records (customers/policies/etc.) are — every handler below
// filters strictly to "sender_id == me OR recipient_id == me", full stop,
// even for an Admin caller. Staff messages aren't a business record admins
// have a standing right to read, and the server couldn't read the content
// anyway (it's end-to-end encrypted).
fn my_message_filter(user_id: ObjectId) -> bson::Document {
    doc! { "$or": [{ "sender_id": user_id }, { "recipient_id": user_id }] }
}

pub async fn send_message(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<SendMessageInput>,
) -> ApiResult<Json<MessageResponse>> {
    let recipient_id = parse_oid(&input.recipient_id)?;
    if recipient_id == auth.user_id {
        return Err(ApiError::BadRequest("cannot message yourself".into()));
    }
    let recipient_exists = state
        .db
        .collection::<User>("users")
        .find_one(doc! { "_id": recipient_id })
        .await?
        .is_some();
    if !recipient_exists {
        return Err(ApiError::BadRequest("recipient not found".into()));
    }

    let message = Message {
        id: None,
        sender_id: auth.user_id,
        recipient_id,
        ciphertext: input.ciphertext,
        iv: input.iv,
        read_at: None,
        created_at: Utc::now(),
    };
    let collection = state.db.collection::<Message>("messages");
    let result = collection.insert_one(&message).await?;
    let mut created = message;
    created.id = result.inserted_id.as_object_id();
    let response: MessageResponse = created.into();

    // Best-effort live push to the recipient — still just ciphertext over
    // the wire, same as the REST response; the recipient's browser decrypts
    // it locally exactly like a fetched one. No queued/offline store here:
    // if they're not connected they'll see it on their next thread fetch.
    #[derive(Serialize)]
    struct MessageEvent<'a> {
        #[serde(rename = "type")]
        kind: &'static str,
        message: &'a MessageResponse,
    }
    if let Ok(payload) = serde_json::to_string(&MessageEvent { kind: "message", message: &response }) {
        let senders = { state.ws_hub.lock().await.get(&recipient_id).cloned() };
        if let Some(senders) = senders {
            for sender in &senders {
                let _ = sender.send(payload.clone());
            }
        }
    }

    Ok(Json(response))
}

#[derive(Debug, Deserialize)]
pub struct ThreadQuery {
    pub page_size: Option<u64>,
}

pub async fn get_thread(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(partner_id): Path<String>,
    Query(page): Query<ThreadQuery>,
) -> ApiResult<Json<Vec<MessageResponse>>> {
    let partner_id = parse_oid(&partner_id)?;
    let filter = doc! {
        "$or": [
            { "sender_id": auth.user_id, "recipient_id": partner_id },
            { "sender_id": partner_id, "recipient_id": auth.user_id },
        ],
    };
    let limit = page.page_size.unwrap_or(100).clamp(1, 200) as i64;
    let collection = state.db.collection::<Message>("messages");
    let cursor = collection.find(filter).sort(doc! { "created_at": -1 }).limit(limit).await?;
    let mut messages: Vec<Message> = cursor.try_collect().await?;
    messages.reverse(); // oldest first for display
    Ok(Json(messages.into_iter().map(Into::into).collect()))
}

pub async fn mark_thread_read(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(partner_id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let partner_id = parse_oid(&partner_id)?;
    let collection = state.db.collection::<Message>("messages");
    collection
        .update_many(
            doc! { "sender_id": partner_id, "recipient_id": auth.user_id, "read_at": null },
            doc! { "$set": { "read_at": Utc::now() } },
        )
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn list_threads(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<MessageThreadSummary>>> {
    let collection = state.db.collection::<Message>("messages");
    let cursor = collection
        .find(my_message_filter(auth.user_id))
        .sort(doc! { "created_at": -1 })
        .await?;
    let messages: Vec<Message> = cursor.try_collect().await?;

    // Messages arrive newest-first, so the first time we see a given partner
    // it's necessarily their most recent message — `or_insert_with` captures
    // that as `last_message` once; every later (older) message for the same
    // partner only bumps `unread_count` via `and_modify`, never overwriting it.
    let mut by_partner: HashMap<ObjectId, (Message, u32)> = HashMap::new();
    for m in messages {
        let partner_id = if m.sender_id == auth.user_id { m.recipient_id } else { m.sender_id };
        let unread = if m.recipient_id == auth.user_id && m.read_at.is_none() { 1 } else { 0 };
        by_partner
            .entry(partner_id)
            .and_modify(|(_, count)| *count += unread)
            .or_insert_with(|| (m.clone(), unread));
    }

    let partner_ids: Vec<ObjectId> = by_partner.keys().copied().collect();
    let users: Vec<User> = state
        .db
        .collection::<User>("users")
        .find(doc! { "_id": { "$in": &partner_ids } })
        .await?
        .try_collect()
        .await?;
    let names: HashMap<ObjectId, String> = users.into_iter().filter_map(|u| u.id.map(|id| (id, u.name))).collect();

    let mut summaries: Vec<MessageThreadSummary> = by_partner
        .into_iter()
        .map(|(partner_id, (last_message, unread_count))| MessageThreadSummary {
            partner_id: partner_id.to_hex(),
            partner_name: names.get(&partner_id).cloned().unwrap_or_else(|| "Unknown".into()),
            last_message: last_message.into(),
            unread_count,
        })
        .collect();
    summaries.sort_by(|a, b| b.last_message.created_at.cmp(&a.last_message.created_at));

    Ok(Json(summaries))
}
