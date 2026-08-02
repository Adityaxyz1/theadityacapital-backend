use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
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
}

impl From<User> for UserPublic {
    fn from(u: User) -> Self {
        UserPublic {
            id: u.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: u.name,
            email: u.email,
            phone: u.phone,
            role: u.role,
        }
    }
}
