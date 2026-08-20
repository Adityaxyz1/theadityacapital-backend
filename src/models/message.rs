use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// End-to-end encrypted: `ciphertext`/`iv` are opaque to the server. Both
// sides derive the same AES-GCM key via ECDH (their own private key, which
// never leaves the browser, + the other party's public key from
// UserDirectoryEntry) — see frontend/src/lib/crypto.ts. The server stores
// and relays this exactly as received; it has no key material to decrypt it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub sender_id: ObjectId,
    pub recipient_id: ObjectId,
    pub ciphertext: String,
    pub iv: String,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub read_at: Option<DateTime<Utc>>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageInput {
    pub recipient_id: String,
    pub ciphertext: String,
    pub iv: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub sender_id: String,
    pub recipient_id: String,
    pub ciphertext: String,
    pub iv: String,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<Message> for MessageResponse {
    fn from(m: Message) -> Self {
        MessageResponse {
            id: m.id.map(|i| i.to_hex()).unwrap_or_default(),
            sender_id: m.sender_id.to_hex(),
            recipient_id: m.recipient_id.to_hex(),
            ciphertext: m.ciphertext,
            iv: m.iv,
            read_at: m.read_at,
            created_at: m.created_at,
        }
    }
}

// One row per conversation partner, newest-active first — the thread-list
// view. `unread_count` only counts messages the partner sent to the caller;
// the caller's own sent messages are never "unread".
#[derive(Debug, Serialize)]
pub struct MessageThreadSummary {
    pub partner_id: String,
    pub partner_name: String,
    pub last_message: MessageResponse,
    pub unread_count: u32,
}
