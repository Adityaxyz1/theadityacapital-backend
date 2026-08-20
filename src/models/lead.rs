use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// A pre-customer pipeline entry (LeadsPage/LeadFunnelWidget) — deliberately
// separate from Customer, not a Customer with a "lead" flag, since a lead has
// no policies/address/notes yet and most of its fields (stage, probability,
// estimated_premium) have no meaning once it becomes a real customer. Winning
// a lead does not auto-create a Customer/Policy today — staff do that
// manually via the normal customer/policy flow once a deal closes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeadStage {
    New,
    Contacted,
    Qualified,
    Proposal,
    Negotiation,
    Won,
    Lost,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Lead {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub policy_type: String,
    pub estimated_premium: Option<f64>,
    pub stage: LeadStage,
    // 0-100, staff's own estimate of close likelihood — not derived from
    // stage; a "negotiation" lead the agent doubts can still be 30%.
    pub probability: i32,
    pub assigned_to: ObjectId,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLeadInput {
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub policy_type: String,
    pub estimated_premium: Option<f64>,
    #[serde(default)]
    pub probability: i32,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLeadInput {
    pub name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub policy_type: Option<String>,
    pub estimated_premium: Option<f64>,
    pub probability: Option<i32>,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLeadStageInput {
    pub stage: LeadStage,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct LeadResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub policy_type: String,
    pub estimated_premium: Option<f64>,
    pub stage: LeadStage,
    pub probability: i32,
    pub assigned_to: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Lead> for LeadResponse {
    fn from(l: Lead) -> Self {
        LeadResponse {
            id: l.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: l.name,
            phone: l.phone,
            email: l.email,
            policy_type: l.policy_type,
            estimated_premium: l.estimated_premium,
            stage: l.stage,
            probability: l.probability,
            assigned_to: l.assigned_to.to_hex(),
            created_at: l.created_at,
            updated_at: l.updated_at,
        }
    }
}
