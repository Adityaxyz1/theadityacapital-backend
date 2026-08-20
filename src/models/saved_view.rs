use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Which list-view a saved view applies to. Kept as a closed enum (not a free
// string) so a typo can't silently create an unreachable saved view.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ListObject {
    Customers,
    Policies,
    Renewals,
}

// `query` stores the raw list-endpoint querystring (filters/sort) the user
// had built when they saved the view — the frontend just re-navigates with
// it. Simpler than a structured filter document and just as replayable,
// since the same whitelisted query params the endpoint already accepts are
// the only thing that can end up in here.
#[derive(Debug, Serialize, Deserialize)]
pub struct SavedView {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub object: ListObject,
    pub name: String,
    pub owner_id: ObjectId,
    pub is_shared: bool,
    pub query: String,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSavedViewInput {
    pub object: ListObject,
    pub name: String,
    pub is_shared: Option<bool>,
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct SavedViewResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub object: ListObject,
    pub name: String,
    pub owner_id: String,
    pub is_shared: bool,
    pub query: String,
    pub created_at: DateTime<Utc>,
}

impl From<SavedView> for SavedViewResponse {
    fn from(v: SavedView) -> Self {
        SavedViewResponse {
            id: v.id.map(|i| i.to_hex()).unwrap_or_default(),
            object: v.object,
            name: v.name,
            owner_id: v.owner_id.to_hex(),
            is_shared: v.is_shared,
            query: v.query,
            created_at: v.created_at,
        }
    }
}
