use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Team {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub manager_id: ObjectId,
    #[serde(default)]
    pub member_ids: Vec<ObjectId>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTeamInput {
    pub name: String,
    pub manager_id: String,
    #[serde(default)]
    pub member_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTeamInput {
    pub name: Option<String>,
    pub manager_id: Option<String>,
    pub member_ids: Option<Vec<String>>,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct TeamResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub manager_id: String,
    pub member_ids: Vec<String>,
}

impl From<Team> for TeamResponse {
    fn from(t: Team) -> Self {
        TeamResponse {
            id: t.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: t.name,
            manager_id: t.manager_id.to_hex(),
            member_ids: t.member_ids.into_iter().map(|i| i.to_hex()).collect(),
        }
    }
}
