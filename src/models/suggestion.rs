use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::workflow_rule::EntityType;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    Open,
    Dismissed,
    Actioned,
}

// Created by workflow.rs's `Suggest` action — a workflow rule whose action is
// "propose this to staff" instead of executing something directly. Never
// executes anything itself; "actioning" one is just staff clicking through to
// the normal CRUD action it points at (see PATCH /suggestions/{id}).
#[derive(Debug, Serialize, Deserialize)]
pub struct Suggestion {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub rule_id: ObjectId,
    pub entity_type: EntityType,
    pub entity_id: ObjectId,
    // Denormalized from the triggering record at creation time (mirroring
    // Renewal's own denormalization pattern) — without this, row-level
    // visibility (backend/src/visibility.rs) couldn't scope the suggestions
    // list at all, since the `suggestions` collection has no other way to
    // know who owns the underlying record. None for Customer-type
    // suggestions (no assigned_to concept there), which makes them
    // admin-only-visible by default — a deliberately safe default.
    pub assigned_to: Option<ObjectId>,
    pub message: String,
    pub status: SuggestionStatus,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime_optional")]
    pub dismissed_at: Option<DateTime<Utc>>,
    pub dismissed_by: Option<ObjectId>,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct SuggestionResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub rule_id: String,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub message: String,
    pub status: SuggestionStatus,
    pub created_at: DateTime<Utc>,
    pub dismissed_at: Option<DateTime<Utc>>,
    pub dismissed_by: Option<String>,
}

impl From<Suggestion> for SuggestionResponse {
    fn from(s: Suggestion) -> Self {
        SuggestionResponse {
            id: s.id.map(|i| i.to_hex()).unwrap_or_default(),
            rule_id: s.rule_id.to_hex(),
            entity_type: s.entity_type,
            entity_id: s.entity_id.to_hex(),
            message: s.message,
            status: s.status,
            created_at: s.created_at,
            dismissed_at: s.dismissed_at,
            dismissed_by: s.dismissed_by.map(|i| i.to_hex()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateSuggestionInput {
    pub status: SuggestionStatus,
}
