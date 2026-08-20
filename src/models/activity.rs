use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityIcon {
    Policy,
    Payment,
    Lead,
    Claim,
    Reminder,
    Note,
}

// A real, mostly-automatic audit trail (see activity.rs::log, called from
// every write worth surfacing — create_customer, create_policy, create_lead,
// create_claim, log_payment, add_follow_up, update_renewal_status) rather
// than a staff-curated feed. `POST /activities` (routes/activities.rs) also
// allows a manual entry for the "log a call/note" case that isn't tied to
// any other write (QuickAddModal's "Activity" tab).
#[derive(Debug, Serialize, Deserialize)]
pub struct Activity {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: ObjectId,
    pub action: String,
    pub customer_id: Option<ObjectId>,
    pub customer_name: Option<String>,
    pub policy_type: Option<String>,
    pub icon_type: ActivityIcon,
    // Denormalized like every other assigned-record (Renewal, Suggestion) so
    // visibility_filter can scope the feed without a $lookup — None means
    // visible only to admins (the safe default already established for
    // Customer-typed Suggestions, see models/suggestion.rs).
    pub assigned_to: Option<ObjectId>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateActivityInput {
    pub action: String,
    pub customer_id: Option<String>,
    pub customer_name: Option<String>,
    pub policy_type: Option<String>,
    pub icon_type: ActivityIcon,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct ActivityResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub user_id: String,
    pub action: String,
    pub customer_id: Option<String>,
    pub customer_name: Option<String>,
    pub policy_type: Option<String>,
    pub icon_type: ActivityIcon,
    pub created_at: DateTime<Utc>,
}

impl From<Activity> for ActivityResponse {
    fn from(a: Activity) -> Self {
        ActivityResponse {
            id: a.id.map(|i| i.to_hex()).unwrap_or_default(),
            user_id: a.user_id.to_hex(),
            action: a.action,
            customer_id: a.customer_id.map(|i| i.to_hex()),
            customer_name: a.customer_name,
            policy_type: a.policy_type,
            icon_type: a.icon_type,
            created_at: a.created_at,
        }
    }
}
