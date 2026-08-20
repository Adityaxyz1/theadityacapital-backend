use std::time::Duration;

use bson::{doc, oid::ObjectId, Bson, Document};
use chrono::Utc;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::{
    config::SYSTEM_USER_ID,
    models::{
        ActionKind, Condition, ConditionOp, EntityType, FollowUp, Notification, NotificationChannel,
        NotificationStatus, Policy, Renewal, Suggestion, SuggestionStatus, TriggerKind, WorkflowRule,
        WorkflowTriggerState,
    },
    notify,
    state::AppState,
};

fn collection_name(entity_type: EntityType) -> &'static str {
    match entity_type {
        EntityType::Renewal => "renewals",
        EntityType::Policy => "policies",
        EntityType::Customer => "customers",
        EntityType::Lead => "leads",
        EntityType::Claim => "claims",
    }
}

async fn load_enabled_rules(state: &AppState, entity_type: EntityType) -> anyhow::Result<Vec<WorkflowRule>> {
    let filter = doc! { "enabled": true, "entity_type": bson::to_bson(&entity_type)? };
    let cursor = state.db.collection::<WorkflowRule>("workflow_rules").find(filter).await?;
    Ok(cursor.try_collect().await?)
}

// Call right after a customer/policy/renewal is inserted. Fires every enabled
// RecordCreated rule for that entity type whose conditions match the new doc.
pub async fn on_record_created(state: &AppState, entity_type: EntityType, entity_id: ObjectId, doc: &Document) {
    let rules = match load_enabled_rules(state, entity_type).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("workflow: failed to load rules for {entity_type:?}: {e}");
            return;
        }
    };
    for rule in &rules {
        if matches!(rule.trigger, TriggerKind::RecordCreated) && conditions_match(doc, &rule.conditions) {
            execute_actions(state, rule, entity_type, entity_id, doc).await;
        }
    }
}

// Call right after a field changes on an existing record (e.g. a renewal's
// status). `doc` should be the *post-update* record. Fires every enabled
// FieldChanged rule watching this field, whose `to_value` (if any) matches
// the new value and whose conditions match the rest of the record.
pub async fn on_field_changed(
    state: &AppState,
    entity_type: EntityType,
    entity_id: ObjectId,
    field: &str,
    new_value: &Bson,
    doc: &Document,
) {
    let rules = match load_enabled_rules(state, entity_type).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("workflow: failed to load rules for {entity_type:?}: {e}");
            return;
        }
    };
    for rule in &rules {
        let TriggerKind::FieldChanged { field: rule_field, to_value } = &rule.trigger else {
            continue;
        };
        if rule_field != field {
            continue;
        }
        let value_matches = to_value.as_ref().map(|v| v == new_value).unwrap_or(true);
        if value_matches && conditions_match(doc, &rule.conditions) {
            execute_actions(state, rule, entity_type, entity_id, doc).await;
        }
    }
}

// Periodic scan for TimeOffset triggers — the direct generalization of
// worker.rs's notification-rule scan. Scoped to Renewal entities only for
// now (the one type with a meaningful "days until X" date field today); the
// per-(rule, record) progress cursor in `workflow_trigger_state` generalizes
// what used to be the single hardcoded `Renewal.reminder_stage` field.
pub async fn evaluate_time_triggers(state: &AppState) -> anyhow::Result<()> {
    let rules: Vec<WorkflowRule> = state
        .db
        .collection::<WorkflowRule>("workflow_rules")
        .find(doc! { "enabled": true, "entity_type": "renewal" })
        .await?
        .try_collect()
        .await?;
    let time_rules: Vec<&WorkflowRule> = rules
        .iter()
        .filter(|r| matches!(r.trigger, TriggerKind::TimeOffset { .. }))
        .collect();
    if time_rules.is_empty() {
        return Ok(());
    }

    let renewals: Vec<Renewal> = state
        .db
        .collection::<Renewal>("renewals")
        .find(doc! { "status": { "$in": ["pending", "contacted"] } })
        .await?
        .try_collect()
        .await?;

    let progress_collection = state.db.collection::<WorkflowTriggerState>("workflow_trigger_state");
    let today = Utc::now().date_naive();

    for renewal in &renewals {
        let Some(renewal_id) = renewal.id else { continue };
        let doc = bson::to_document(renewal)?;

        for rule in &time_rules {
            let TriggerKind::TimeOffset { date_field, offset_days } = &rule.trigger else {
                continue;
            };
            let Some(date_value) = doc.get(date_field).and_then(Bson::as_datetime) else {
                continue;
            };
            if !conditions_match(&doc, &rule.conditions) {
                continue;
            }

            let mut offsets = offset_days.clone();
            offsets.sort_unstable_by(|a, b| b.cmp(a));
            let days_left = (date_value.to_chrono().date_naive() - today).num_days();

            let rule_id = rule.id.expect("persisted rule always has an id");
            let existing = progress_collection
                .find_one(doc! { "rule_id": rule_id, "entity_id": renewal_id })
                .await?;
            let mut stage = existing.map(|s| s.offset_reached.max(0) as usize).unwrap_or(0);
            let stage_before = stage;

            while stage < offsets.len() && days_left <= offsets[stage] {
                // A shallow copy with the crossed threshold's day count added
                // so a `{days}` token in a SendNotification template still
                // works, matching the old renderer's behavior.
                let mut render_doc = doc.clone();
                render_doc.insert("days", offsets[stage]);
                execute_actions(state, rule, EntityType::Renewal, renewal_id, &render_doc).await;
                stage += 1;
            }

            if stage != stage_before {
                progress_collection
                    .update_one(
                        doc! { "rule_id": rule_id, "entity_id": renewal_id },
                        doc! { "$set": { "offset_reached": stage as i64 } },
                    )
                    .upsert(true)
                    .await?;
            }
        }
    }

    Ok(())
}

pub fn spawn(state: AppState) {
    let interval_secs = state.config.notification_scan_interval_secs;
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            if let Err(e) = evaluate_time_triggers(&state).await {
                tracing::error!("workflow time-trigger scan failed: {e}");
            }
        }
    });
}

fn conditions_match(doc: &Document, conditions: &[Condition]) -> bool {
    conditions.iter().all(|c| condition_matches(doc, c))
}

fn condition_matches(doc: &Document, condition: &Condition) -> bool {
    let field_value = doc.get(&condition.field);
    match condition.op {
        ConditionOp::IsSet => field_value.is_some(),
        ConditionOp::Eq => field_value.map(|v| v == &condition.value).unwrap_or(false),
        ConditionOp::Ne => field_value.map(|v| v != &condition.value).unwrap_or(true),
        ConditionOp::In => match &condition.value {
            Bson::Array(arr) => field_value.map(|v| arr.contains(v)).unwrap_or(false),
            _ => false,
        },
        ConditionOp::Gt | ConditionOp::Lt | ConditionOp::Gte | ConditionOp::Lte => {
            match (field_value.and_then(bson_to_f64), bson_to_f64(&condition.value)) {
                (Some(a), Some(b)) => match condition.op {
                    ConditionOp::Gt => a > b,
                    ConditionOp::Lt => a < b,
                    ConditionOp::Gte => a >= b,
                    ConditionOp::Lte => a <= b,
                    _ => unreachable!(),
                },
                _ => false,
            }
        }
    }
}

fn bson_to_f64(b: &Bson) -> Option<f64> {
    match b {
        Bson::Double(d) => Some(*d),
        Bson::Int32(i) => Some(*i as f64),
        Bson::Int64(i) => Some(*i as f64),
        Bson::DateTime(dt) => Some(dt.timestamp_millis() as f64),
        _ => None,
    }
}

fn bson_to_display_string(b: &Bson) -> String {
    match b {
        Bson::String(s) => s.clone(),
        Bson::Int32(i) => i.to_string(),
        Bson::Int64(i) => i.to_string(),
        Bson::Double(d) => {
            if d.fract() == 0.0 {
                format!("{}", *d as i64)
            } else {
                format!("{d}")
            }
        }
        Bson::DateTime(dt) => dt.to_chrono().format("%d %b %Y").to_string(),
        Bson::Boolean(b) => b.to_string(),
        Bson::Null => String::new(),
        other => format!("{other:?}"),
    }
}

// Generalizes the old worker.rs::render_message naive chained `str::replace`
// into a data-driven token scan: any `{field}` in the template is looked up
// directly on the record's document, rather than a hardcoded list of
// supported tokens.
fn render_template(template: &str, doc: &Document) -> String {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            result.push(c);
            continue;
        }
        let mut token = String::new();
        let mut closed = false;
        while let Some(&next) = chars.peek() {
            if next == '}' {
                chars.next();
                closed = true;
                break;
            }
            token.push(next);
            chars.next();
        }
        if closed {
            let value = doc.get(&token).map(bson_to_display_string).unwrap_or_default();
            result.push_str(&value);
        } else {
            result.push('{');
            result.push_str(&token);
        }
    }
    result
}

// Same per-field conversion render_template uses for `{field}` tokens
// (bson_to_display_string), just applied to every field up front instead of
// one token at a time — so the record data the AI drafting call sees for
// DraftAiMessage matches, field for field, what a SendNotification/Suggest
// template's tokens would render for the same record.
fn document_field_map(doc: &Document) -> serde_json::Map<String, serde_json::Value> {
    doc.iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(bson_to_display_string(v))))
        .collect()
}

#[derive(Debug, Serialize)]
struct DraftMessageRequest<'a> {
    hint: &'a str,
    record_context: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct DraftMessageResponse {
    message: String,
}

// Calls ai-service's POST /draft-message (main.py) — a single plain chat
// completion, not the interactive assistant's tool-calling loop, since a
// workflow rule firing unattended has no user turn to gather more context in;
// everything the model gets is the hint plus the triggering record's own
// fields.
async fn call_draft_message_service(state: &AppState, hint: &str, doc: &Document) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/draft-message", state.config.ai_service_url))
        .timeout(Duration::from_secs(60))
        .json(&DraftMessageRequest { hint, record_context: document_field_map(doc) })
        .send()
        .await?
        .error_for_status()?
        .json::<DraftMessageResponse>()
        .await?;
    Ok(response.message)
}

async fn execute_actions(state: &AppState, rule: &WorkflowRule, entity_type: EntityType, entity_id: ObjectId, doc: &Document) {
    for action in &rule.actions {
        if let Err(e) = execute_action(state, rule, action, entity_type, entity_id, doc).await {
            tracing::error!("workflow rule '{}' action {action:?} failed: {e}", rule.name);
        }
    }
}

async fn execute_action(
    state: &AppState,
    rule: &WorkflowRule,
    action: &ActionKind,
    entity_type: EntityType,
    entity_id: ObjectId,
    doc: &Document,
) -> anyhow::Result<()> {
    match action {
        ActionKind::SendNotification { template } => {
            // Notification.renewal_id is renewal-specific today — a broader
            // Notification model (any entity, not just renewals) is out of
            // scope for this pass.
            // Notification.renewal_id is renewal-specific, so Policy/Customer/
            // Lead/Claim rules all land in this unsupported branch alongside
            // Renewal's existing non-matches.
            if entity_type != EntityType::Renewal {
                tracing::warn!("SendNotification only supports Renewal entities today, skipping for {entity_type:?}");
                return Ok(());
            }
            let user_id = doc
                .get_object_id("assigned_to")
                .map_err(|_| anyhow::anyhow!("record has no assigned_to field"))?;
            let mut notification = Notification {
                id: None,
                renewal_id: entity_id,
                customer_id: doc.get_object_id("customer_id").ok(),
                user_id,
                channel: NotificationChannel::InApp,
                status: NotificationStatus::Queued,
                message: render_template(template, doc),
                offset_days: doc.get_i32("days").map(|d| d as i64).unwrap_or(0),
                sent_at: None,
                read_at: None,
                created_at: Utc::now(),
            };
            let result = state.db.collection::<Notification>("notifications").insert_one(&notification).await?;
            notification.id = result.inserted_id.as_object_id();
            notify::dispatch(state, &mut notification).await;
        }
        ActionKind::CreateFollowUp { follow_up_kind, template } => {
            let policy_id = match entity_type {
                EntityType::Policy => entity_id,
                // Renewal and Claim both carry a policy_id pointing at the
                // real policy this follow-up should attach to.
                EntityType::Renewal | EntityType::Claim => doc
                    .get_object_id("policy_id")
                    .map_err(|_| anyhow::anyhow!("record has no policy_id field"))?,
                EntityType::Customer => anyhow::bail!("CreateFollowUp isn't supported for Customer entities"),
                EntityType::Lead => {
                    anyhow::bail!("CreateFollowUp isn't supported for Lead entities (no policy_id field)")
                }
            };
            let follow_up = FollowUp {
                // Attributed to the system user, same as the AI extraction
                // pipeline — this follow-up was authored by automation, not
                // a specific staff member.
                user_id: ObjectId::parse_str(SYSTEM_USER_ID)?,
                kind: *follow_up_kind,
                content: render_template(template, doc),
                created_at: Utc::now(),
            };
            let follow_up_bson = bson::to_bson(&follow_up)?;
            let now = Utc::now();
            let updated = state
                .db
                .collection::<Renewal>("renewals")
                .find_one_and_update(
                    doc! { "policy_id": policy_id, "status": "pending" },
                    doc! { "$set": { "status": "contacted", "last_contacted_at": now } },
                )
                .return_document(mongodb::options::ReturnDocument::After)
                .await
                .ok()
                .flatten();
            state
                .db
                .collection::<Policy>("policies")
                .update_one(
                    doc! { "_id": policy_id },
                    doc! { "$push": { "follow_ups": follow_up_bson }, "$set": { "updated_at": now } },
                )
                .await?;
            if let Some(updated) = updated {
                notify::broadcast_renewal_updated(state, &updated.into()).await;
            }
        }
        ActionKind::ReassignOwner { to_user_id } => {
            let Some(new_owner) = to_user_id else {
                anyhow::bail!("ReassignOwner requires to_user_id");
            };
            if entity_type == EntityType::Customer {
                anyhow::bail!("ReassignOwner isn't supported for Customer entities (no assigned_to field)");
            }
            state
                .db
                .collection::<Document>(collection_name(entity_type))
                .update_one(doc! { "_id": entity_id }, doc! { "$set": { "assigned_to": new_owner } })
                .await?;
        }
        ActionKind::UpdateField { field, value } => {
            state
                .db
                .collection::<Document>(collection_name(entity_type))
                .update_one(doc! { "_id": entity_id }, doc! { "$set": { field.as_str(): value.clone() } })
                .await?;
        }
        ActionKind::Suggest { message } => {
            let rule_id = rule.id.expect("persisted rule always has an id");
            let suggestions = state.db.collection::<Suggestion>("suggestions");
            // Dedup: don't pile up duplicate open suggestions for the same
            // rule+record on every re-evaluation (e.g. every scan tick while
            // a time-offset condition keeps matching).
            let existing = suggestions
                .find_one(doc! { "rule_id": rule_id, "entity_id": entity_id, "status": "open" })
                .await?;
            if existing.is_none() {
                let suggestion = Suggestion {
                    id: None,
                    rule_id,
                    entity_type,
                    entity_id,
                    assigned_to: doc.get_object_id("assigned_to").ok(),
                    message: render_template(message, doc),
                    status: SuggestionStatus::Open,
                    created_at: Utc::now(),
                    dismissed_at: None,
                    dismissed_by: None,
                };
                suggestions.insert_one(&suggestion).await?;
            }
        }
        ActionKind::DraftAiMessage { hint } => {
            // A drafting failure (ai-service down, no NVIDIA_API_KEY, model
            // error) must never abort the rest of this rule's actions or the
            // wider workflow evaluation pass — it's handled entirely inline
            // here (logged and swallowed) rather than bubbled up via `?`,
            // unlike every other arm in this match.
            match call_draft_message_service(state, hint, doc).await {
                Ok(drafted) => {
                    let rule_id = rule.id.expect("persisted rule always has an id");
                    let suggestions = state.db.collection::<Suggestion>("suggestions");
                    // Same dedup-by-open-status pattern as the Suggest arm
                    // above: don't pile up duplicate open suggestions for the
                    // same rule+record on every re-evaluation.
                    let existing = suggestions
                        .find_one(doc! { "rule_id": rule_id, "entity_id": entity_id, "status": "open" })
                        .await?;
                    if existing.is_none() {
                        let suggestion = Suggestion {
                            id: None,
                            rule_id,
                            entity_type,
                            entity_id,
                            assigned_to: doc.get_object_id("assigned_to").ok(),
                            // Prefixed so staff can tell at a glance this was
                            // AI-drafted (and thus needs a read-over before
                            // sending) rather than a static Suggest template.
                            message: format!("Drafted follow-up: {drafted}"),
                            status: SuggestionStatus::Open,
                            created_at: Utc::now(),
                            dismissed_at: None,
                            dismissed_by: None,
                        };
                        suggestions.insert_one(&suggestion).await?;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "workflow rule '{}' DraftAiMessage action could not reach ai-service, skipping: {e}",
                        rule.name
                    );
                }
            }
        }
    }
    Ok(())
}
