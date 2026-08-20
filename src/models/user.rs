use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Manager sits between Admin (sees everything) and Agent (sees only their own
// assigned book) — a manager sees their team's book via `visibility.rs`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Manager,
    Agent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    // Never returned to clients: every handler maps User -> UserPublic before
    // sending a response, so this stays out of JSON without needing serde to
    // strip it (which would also strip it from the Mongo write).
    pub password_hash: String,
    pub role: Role,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<ObjectId>,
    // ECDH (P-256) public key, JWK JSON, uploaded by the user's own browser —
    // see routes/messages.rs. Public keys are not secret; the matching
    // private key never leaves the browser (localStorage), so this field is
    // the only key material the server ever sees. `None` until that user's
    // browser has generated a keypair at least once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub role: Role,
    pub team_id: Option<String>,
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        UserPublic {
            id: u.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: u.name,
            email: u.email,
            phone: u.phone,
            role: u.role,
            team_id: u.team_id.map(|i| i.to_hex()),
        }
    }
}

// Minimal, non-admin-safe staff directory entry for the messaging picker —
// deliberately excludes email/phone/role (UserPublic has those, but exposing
// them to every staff member for every colleague is more than a "who can I
// message" picker needs).
#[derive(Debug, Serialize)]
pub struct UserDirectoryEntry {
    pub id: String,
    pub name: String,
    pub public_key: Option<String>,
}

impl From<User> for UserDirectoryEntry {
    fn from(u: User) -> Self {
        UserDirectoryEntry {
            id: u.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: u.name,
            public_key: u.public_key,
        }
    }
}
