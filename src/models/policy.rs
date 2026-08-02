use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyStatus {
    Active,
    Renewed,
    Lapsed,
    Lost,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FollowUpType {
    Call,
    Whatsapp,
    Email,
    Note,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FollowUp {
    pub user_id: ObjectId,
    #[serde(rename = "type")]
    pub kind: FollowUpType,
    pub content: String,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Policy {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub customer_id: ObjectId,
    // Free-form, not a hardcoded enum: any insurer/policy type is valid (PRD 4.1).
    pub insurer_name: String,
    pub policy_type: String,
    pub policy_number: String,
    pub sum_insured: f64,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub start_date: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub end_date: DateTime<Utc>,
    pub previous_year_premium: Option<f64>,
    pub current_year_premium: f64,
    pub status: PolicyStatus,
    pub previous_policy_id: Option<ObjectId>,
    pub assigned_to: ObjectId,
    pub source_document_id: Option<ObjectId>,
    #[serde(default)]
    pub follow_ups: Vec<FollowUp>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicyInput {
    pub customer_id: String,
    pub insurer_name: String,
    pub policy_type: String,
    pub policy_number: String,
    pub sum_insured: f64,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub previous_year_premium: Option<f64>,
    pub current_year_premium: f64,
    pub previous_policy_id: Option<String>,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolicyInput {
    pub insurer_name: Option<String>,
    pub policy_type: Option<String>,
    pub policy_number: Option<String>,
    pub sum_insured: Option<f64>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub previous_year_premium: Option<f64>,
    pub current_year_premium: Option<f64>,
    pub status: Option<PolicyStatus>,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FollowUpResponse {
    pub user_id: String,
    #[serde(rename = "type")]
    pub kind: FollowUpType,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl From<FollowUp> for FollowUpResponse {
    fn from(f: FollowUp) -> Self {
        FollowUpResponse {
            user_id: f.user_id.to_hex(),
            kind: f.kind,
            content: f.content,
            created_at: f.created_at,
        }
    }
}

// See CustomerResponse for why API responses use a dedicated DTO instead of
// serializing the Mongo model (Policy) directly.
#[derive(Debug, Serialize)]
pub struct PolicyResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub customer_id: String,
    pub insurer_name: String,
    pub policy_type: String,
    pub policy_number: String,
    pub sum_insured: f64,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub previous_year_premium: Option<f64>,
    pub current_year_premium: f64,
    pub status: PolicyStatus,
    pub previous_policy_id: Option<String>,
    pub assigned_to: String,
    pub source_document_id: Option<String>,
    pub follow_ups: Vec<FollowUpResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Policy> for PolicyResponse {
    fn from(p: Policy) -> Self {
        PolicyResponse {
            id: p.id.map(|i| i.to_hex()).unwrap_or_default(),
            customer_id: p.customer_id.to_hex(),
            insurer_name: p.insurer_name,
            policy_type: p.policy_type,
            policy_number: p.policy_number,
            sum_insured: p.sum_insured,
            start_date: p.start_date,
            end_date: p.end_date,
            previous_year_premium: p.previous_year_premium,
            current_year_premium: p.current_year_premium,
            status: p.status,
            previous_policy_id: p.previous_policy_id.map(|i| i.to_hex()),
            assigned_to: p.assigned_to.to_hex(),
            source_document_id: p.source_document_id.map(|i| i.to_hex()),
            follow_ups: p.follow_ups.into_iter().map(Into::into).collect(),
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}
