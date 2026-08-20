use std::{collections::HashMap, sync::Arc};

use bson::oid::ObjectId;
use mongodb::Database;
use tokio::sync::{mpsc::UnboundedSender, Mutex};

use crate::config::Config;

// Per-user list of live WebSocket outbound channels (a user can have more than
// one tab/device connected at once). Guarded by an async Mutex since it's only
// ever held briefly to push/register/remove a sender.
pub type WsHub = Arc<Mutex<HashMap<ObjectId, Vec<UnboundedSender<String>>>>>;

// A second hub alongside `WsHub`, keyed by topic (e.g. "renewals-board")
// instead of user id — for broadcasts that need to reach every connection
// subscribed to a shared view (a Kanban board, a live dashboard), not just
// one user's own notifications. A connection can be registered in both hubs
// at once (see routes/ws.rs).
pub type TopicHub = Arc<Mutex<HashMap<String, Vec<UnboundedSender<String>>>>>;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Config,
    pub ws_hub: WsHub,
    pub topic_hub: TopicHub,
}
