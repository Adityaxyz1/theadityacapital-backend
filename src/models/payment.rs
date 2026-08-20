use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Collected,
    Pending,
    Overdue,
    Expected,
}

// Denormalized customer_name/policy_type from the policy at creation time,
// same reasoning as Claim/Renewal (ARCHITECTURE.md §3).
#[derive(Debug, Serialize, Deserialize)]
pub struct Payment {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub policy_id: ObjectId,
    pub customer_id: ObjectId,
    pub customer_name: String,
    pub policy_type: String,
    pub receipt_number: String,
    pub amount: f64,
    pub status: PaymentStatus,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub due_date: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub paid_at: Option<DateTime<Utc>>,
    pub payment_method: Option<String>,
    pub assigned_to: ObjectId,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePaymentInput {
    pub policy_id: String,
    pub receipt_number: String,
    pub amount: f64,
    pub status: Option<PaymentStatus>,
    pub due_date: DateTime<Utc>,
    pub payment_method: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePaymentStatusInput {
    pub status: PaymentStatus,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct PaymentResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub policy_id: String,
    pub customer_id: String,
    pub customer_name: String,
    pub policy_type: String,
    pub receipt_number: String,
    pub amount: f64,
    pub status: PaymentStatus,
    pub due_date: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
    pub payment_method: Option<String>,
    pub assigned_to: String,
    pub created_at: DateTime<Utc>,
}

impl From<Payment> for PaymentResponse {
    fn from(p: Payment) -> Self {
        PaymentResponse {
            id: p.id.map(|i| i.to_hex()).unwrap_or_default(),
            policy_id: p.policy_id.to_hex(),
            customer_id: p.customer_id.to_hex(),
            customer_name: p.customer_name,
            policy_type: p.policy_type,
            receipt_number: p.receipt_number,
            amount: p.amount,
            status: p.status,
            due_date: p.due_date,
            paid_at: p.paid_at,
            payment_method: p.payment_method,
            assigned_to: p.assigned_to.to_hex(),
            created_at: p.created_at,
        }
    }
}
