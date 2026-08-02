use std::{collections::HashMap, sync::Arc};

use bson::oid::ObjectId;
use mongodb::Database;
use tokio::sync::{mpsc::UnboundedSender, Mutex};

use crate::config::Config;

// Per-user list of live WebSocket outbound channels (a user can have more than
// one tab/device connected at once). Guarded by an async Mutex since it's only
// ever held briefly to push/register/remove a sender.
pub type WsHub = Arc<Mutex<HashMap<ObjectId, Vec<UnboundedSender<String>>>>>;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Config,
    pub ws_hub: WsHub,
}
