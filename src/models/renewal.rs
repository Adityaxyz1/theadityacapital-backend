use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RenewalStatus {
    Pending,
    Contacted,
    Renewed,
    Lapsed,
    Lost,
}

// Denormalized fields (customer_name/insurer_name/policy_type/premium_due) exist so
// dashboard/report aggregations avoid $lookup joins across collections (ARCHITECTURE.md §3).
#[derive(Debug, Serialize, Deserialize)]
pub struct Renewal {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub policy_id: ObjectId,
    pub customer_id: ObjectId,
    pub customer_name: String,
    pub insurer_name: String,
    pub policy_type: String,
    pub premium_due: f64,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub due_date: DateTime<Utc>,
    pub reminder_stage: i32,
    pub status: RenewalStatus,
    pub assigned_to: ObjectId,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub last_contacted_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct RenewalResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub policy_id: String,
    pub customer_id: String,
    pub customer_name: String,
    pub insurer_name: String,
    pub policy_type: String,
    pub premium_due: f64,
    pub due_date: DateTime<Utc>,
    pub reminder_stage: i32,
    pub status: RenewalStatus,
    pub assigned_to: String,
    pub last_contacted_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<Renewal> for RenewalResponse {
    fn from(r: Renewal) -> Self {
        RenewalResponse {
            id: r.id.map(|i| i.to_hex()).unwrap_or_default(),
            policy_id: r.policy_id.to_hex(),
            customer_id: r.customer_id.to_hex(),
            customer_name: r.customer_name,
            insurer_name: r.insurer_name,
            policy_type: r.policy_type,
            premium_due: r.premium_due,
            due_date: r.due_date,
            reminder_stage: r.reminder_stage,
            status: r.status,
            assigned_to: r.assigned_to.to_hex(),
            last_contacted_at: r.last_contacted_at,
            notes: r.notes,
            created_at: r.created_at,
        }
    }
}
