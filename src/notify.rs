use bson::doc;
use chrono::Utc;

use crate::{
    models::{Notification, NotificationResponse, NotificationStatus},
    state::AppState,
};

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

    let Ok(payload) = serde_json::to_string(&NotificationResponse::from(notification.clone()))
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
