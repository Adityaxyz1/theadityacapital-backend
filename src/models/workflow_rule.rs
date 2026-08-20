use bson::{oid::ObjectId, Bson};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::policy::FollowUpType;

// Which collection a rule targets. Kept as a closed enum (not a free string)
// so a typo in `entity_type` fails validation immediately rather than
// silently matching nothing at scan time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Renewal,
    Policy,
    Customer,
    Lead,
    Claim,
}

// Generalizes NotificationRule's single implicit trigger (offset_days against
// a renewal's due_date) into three trigger shapes. `TimeOffset` is the only
// one evaluated on the periodic worker tick (see workflow.rs); the other two
// fire synchronously, in-process, right after the write that would trigger
// them commits (no polling, no missed transitions).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TriggerKind {
    TimeOffset { date_field: String, offset_days: Vec<i64> },
    FieldChanged { field: String, to_value: Option<Bson> },
    RecordCreated,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Gte,
    Lte,
    In,
    IsSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub op: ConditionOp,
    // Unused (but must still be present in the doc) for IsSet.
    #[serde(default)]
    pub value: Bson,
}

// `DraftAiMessage`/`Suggest` are accepted and stored so rule authoring
// doesn't need a schema migration once Phase 4 (AI assistant / next-best-
// action) lands — but `workflow.rs::execute_actions` only stubs them for now
// (logs and no-ops) since their real implementations depend on the AI
// assistant and `suggestions` collection built in that phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionKind {
    SendNotification { template: String },
    CreateFollowUp { follow_up_kind: FollowUpType, template: String },
    ReassignOwner { to_user_id: Option<ObjectId> },
    UpdateField { field: String, value: Bson },
    DraftAiMessage { hint: String },
    Suggest { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowRule {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub entity_type: EntityType,
    pub trigger: TriggerKind,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    pub actions: Vec<ActionKind>,
    pub enabled: bool,
    pub created_by: ObjectId,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkflowRuleInput {
    pub name: String,
    pub entity_type: EntityType,
    pub trigger: TriggerKind,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    pub actions: Vec<ActionKind>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkflowRuleInput {
    pub name: Option<String>,
    pub trigger: Option<TriggerKind>,
    pub conditions: Option<Vec<Condition>>,
    pub actions: Option<Vec<ActionKind>>,
    pub enabled: Option<bool>,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct WorkflowRuleResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub entity_type: EntityType,
    pub trigger: TriggerKind,
    pub conditions: Vec<Condition>,
    pub actions: Vec<ActionKind>,
    pub enabled: bool,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

impl From<WorkflowRule> for WorkflowRuleResponse {
    fn from(r: WorkflowRule) -> Self {
        WorkflowRuleResponse {
            id: r.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: r.name,
            entity_type: r.entity_type,
            trigger: r.trigger,
            conditions: r.conditions,
            actions: r.actions,
            enabled: r.enabled,
            created_by: r.created_by.to_hex(),
            created_at: r.created_at,
        }
    }
}

// Per-(rule, record) progress cursor for TimeOffset triggers — generalizes
// the single hardcoded `Renewal.reminder_stage` field into something that
// works for any rule against any entity, without adding a field per rule to
// every collection. Not exposed via any route; internal to workflow.rs.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowTriggerState {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub rule_id: ObjectId,
    pub entity_id: ObjectId,
    pub offset_reached: i64,
}
