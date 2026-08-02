use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationChannel {
    InApp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationStatus {
    Queued,
    Sent,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub renewal_id: ObjectId,
    pub user_id: ObjectId,
    pub channel: NotificationChannel,
    pub status: NotificationStatus,
    pub message: String,
    pub offset_days: i64,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub sent_at: Option<DateTime<Utc>>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub read_at: Option<DateTime<Utc>>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct NotificationResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub renewal_id: String,
    pub channel: NotificationChannel,
    pub status: NotificationStatus,
    pub message: String,
    pub offset_days: i64,
    pub sent_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<Notification> for NotificationResponse {
    fn from(n: Notification) -> Self {
        NotificationResponse {
            id: n.id.map(|i| i.to_hex()).unwrap_or_default(),
            renewal_id: n.renewal_id.to_hex(),
            channel: n.channel,
            status: n.status,
            message: n.message,
            offset_days: n.offset_days,
            sent_at: n.sent_at,
            read_at: n.read_at,
            created_at: n.created_at,
        }
    }
}

// policy_type: None = applies to all policy types (ARCHITECTURE.md §3). offset_days
// are "days before due_date" thresholds, checked descending (30/15/7/1/0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRule {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub policy_type: Option<String>,
    pub offset_days: Vec<i64>,
    pub template: String,
}

#[derive(Debug, Serialize)]
pub struct NotificationRuleResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub policy_type: Option<String>,
    pub offset_days: Vec<i64>,
    pub template: String,
}

impl From<NotificationRule> for NotificationRuleResponse {
    fn from(r: NotificationRule) -> Self {
        NotificationRuleResponse {
            id: r.id.map(|i| i.to_hex()).unwrap_or_default(),
            policy_type: r.policy_type,
            offset_days: r.offset_days,
            template: r.template,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpsertNotificationRuleInput {
    // None = applies to all policy types.
    pub policy_type: Option<String>,
    pub offset_days: Vec<i64>,
    pub template: String,
}
