use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Pending,
    UnderReview,
    Approved,
    Rejected,
    Settled,
}

// Denormalized customer_name/policy_number/insurer_name for the same reason
// Renewal denormalizes them (ARCHITECTURE.md §3) — fast list/report reads
// with no $lookup. `settlement_days` is deliberately NOT stored here: it's
// `today - filed_date` (or `settled_at - filed_date` once settled), computed
// by the frontend so it can never go stale.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claim {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub policy_id: ObjectId,
    pub customer_id: ObjectId,
    pub customer_name: String,
    pub policy_number: String,
    pub insurer_name: String,
    // Denormalized from Policy at creation time like the fields above — lets
    // the dashboard's category filter (Life/Health/Motor/Other) scope the
    // Pending Claims tile without a $lookup. Older documents predating this
    // field default to "" rather than failing to deserialize.
    #[serde(default)]
    pub policy_type: String,
    pub claim_number: String,
    pub claim_amount: f64,
    pub status: ClaimStatus,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub filed_date: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub settled_at: Option<DateTime<Utc>>,
    pub assigned_to: ObjectId,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateClaimInput {
    pub policy_id: String,
    pub claim_number: String,
    pub claim_amount: f64,
    pub filed_date: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateClaimStatusInput {
    pub status: ClaimStatus,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct ClaimResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub policy_id: String,
    pub customer_id: String,
    pub customer_name: String,
    pub policy_number: String,
    pub insurer_name: String,
    pub policy_type: String,
    pub claim_number: String,
    pub claim_amount: f64,
    pub status: ClaimStatus,
    pub filed_date: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
    pub assigned_to: String,
    pub created_at: DateTime<Utc>,
}

impl From<Claim> for ClaimResponse {
    fn from(c: Claim) -> Self {
        ClaimResponse {
            id: c.id.map(|i| i.to_hex()).unwrap_or_default(),
            policy_id: c.policy_id.to_hex(),
            customer_id: c.customer_id.to_hex(),
            customer_name: c.customer_name,
            policy_number: c.policy_number,
            insurer_name: c.insurer_name,
            policy_type: c.policy_type,
            claim_number: c.claim_number,
            claim_amount: c.claim_amount,
            status: c.status,
            filed_date: c.filed_date,
            settled_at: c.settled_at,
            assigned_to: c.assigned_to.to_hex(),
            created_at: c.created_at,
        }
    }
}
