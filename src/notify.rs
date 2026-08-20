use bson::doc;
use chrono::Utc;
use serde::Serialize;

use crate::{
    models::{Notification, NotificationResponse, NotificationStatus, RenewalResponse},
    state::AppState,
};

pub const RENEWALS_BOARD_TOPIC: &str = "renewals-board";

// Pushes a notification over the user's live WebSocket connection(s), if any
// are open, and marks it delivered; otherwise it's left queued so the bell
// picks it up on the user's next GET /notifications (on page load/reconnect).
pub async fn dispatch(state: &AppState, notification: &mut Notification) {
    let senders = {
        let hub = state.ws_hub.lock().await;
        hub.get(&notification.user_id).cloned()
    };

    let Some(senders) = senders.filter(|s| !s.is_empty()) else {
        return;
    };

    // Mark as sent *before* building the payload so the pushed copy reflects
    // what the client is actually receiving (a live, delivered notification),
    // not its pre-dispatch "queued" state.
    let now = Utc::now();
    notification.status = NotificationStatus::Sent;
    notification.sent_at = Some(now);

    // Tagged with `type` so a connection that's also subscribed to a topic
    // (see `broadcast_topic` below) can tell a per-user notification apart
    // from a topic broadcast arriving on the same socket.
    #[derive(serde::Serialize)]
    struct NotificationEvent<'a> {
        #[serde(rename = "type")]
        kind: &'static str,
        #[serde(flatten)]
        notification: &'a NotificationResponse,
    }
    let response = NotificationResponse::from(notification.clone());
    let Ok(payload) = serde_json::to_string(&NotificationEvent { kind: "notification", notification: &response })
    else {
        return;
    };

    let delivered = senders.iter().any(|sender| sender.send(payload.clone()).is_ok());
    if !delivered {
        notification.status = NotificationStatus::Queued;
        notification.sent_at = None;
        return;
    }

    let _ = state
        .db
        .collection::<Notification>("notifications")
        .update_one(
            doc! { "_id": notification.id },
            doc! { "$set": { "status": "sent", "sent_at": now } },
        )
        .await;
}

// Called after any write that changes a renewal's status/fields — both a
// direct Kanban drag (renewals::update_renewal_status) and an indirect
// transition (policies::add_follow_up's implicit Pending->Contacted flip)
// go through this so the board updates live regardless of which screen
// triggered the change.
pub async fn broadcast_renewal_updated(state: &AppState, renewal: &RenewalResponse) {
    #[derive(serde::Serialize)]
    struct RenewalEvent<'a> {
        #[serde(rename = "type")]
        kind: &'static str,
        renewal: &'a RenewalResponse,
    }
    broadcast_topic(
        state,
        RENEWALS_BOARD_TOPIC,
        &RenewalEvent { kind: "renewal_updated", renewal },
    )
    .await;
}

// Best-effort push to every live connection subscribed to `topic` (see
// `TopicHub` in state.rs) — unlike per-user notifications, there's no
// Mongo-backed "queued" fallback here: a topic broadcast is a live-view
// refresh hint (e.g. "a renewal changed, refetch the board"), not a
// record staff need to see even if they were offline when it fired.
pub async fn broadcast_topic(state: &AppState, topic: &str, payload: &impl Serialize) {
    let Ok(payload) = serde_json::to_string(payload) else {
        return;
    };
    let senders = {
        let hub = state.topic_hub.lock().await;
        hub.get(topic).cloned()
    };
    let Some(senders) = senders else {
        return;
    };
    for sender in &senders {
        let _ = sender.send(payload.clone());
    }
}
