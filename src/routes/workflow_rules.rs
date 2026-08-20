use axum::{
    extract::{Path, State},
    Json,
};
use bson::doc;
use chrono::Utc;
use futures_util::TryStreamExt;

use crate::{
    auth::{require_admin, AuthUser},
    error::{ApiError, ApiResult},
    models::{CreateWorkflowRuleInput, UpdateWorkflowRuleInput, WorkflowRule, WorkflowRuleResponse},
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

// Same admin-only guard already used for teams/notification-rules — staff
// configure automation, they don't self-serve it per PRD's staff/admin split.
pub async fn list_workflow_rules(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Vec<WorkflowRuleResponse>>> {
    require_admin(&auth)?;
    let collection = state.db.collection::<WorkflowRule>("workflow_rules");
    let cursor = collection.find(doc! {}).await?;
    let rules: Vec<WorkflowRule> = cursor.try_collect().await?;
    Ok(Json(rules.into_iter().map(Into::into).collect()))
}

pub async fn create_workflow_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateWorkflowRuleInput>,
) -> ApiResult<Json<WorkflowRuleResponse>> {
    require_admin(&auth)?;
    let rule = WorkflowRule {
        id: None,
        name: input.name,
        entity_type: input.entity_type,
        trigger: input.trigger,
        conditions: input.conditions,
        actions: input.actions,
        enabled: input.enabled,
        created_by: auth.user_id,
        created_at: Utc::now(),
    };
    let collection = state.db.collection::<WorkflowRule>("workflow_rules");
    let result = collection.insert_one(&rule).await?;
    let mut created = rule;
    created.id = result.inserted_id.as_object_id();
    Ok(Json(created.into()))
}

pub async fn update_workflow_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateWorkflowRuleInput>,
) -> ApiResult<Json<WorkflowRuleResponse>> {
    require_admin(&auth)?;
    let oid = parse_oid(&id)?;
    let collection = state.db.collection::<WorkflowRule>("workflow_rules");

    let mut set_doc = doc! {};
    if let Some(name) = input.name {
        set_doc.insert("name", name);
    }
    if let Some(trigger) = input.trigger {
        set_doc.insert("trigger", bson::to_bson(&trigger).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(conditions) = input.conditions {
        set_doc.insert("conditions", bson::to_bson(&conditions).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(actions) = input.actions {
        set_doc.insert("actions", bson::to_bson(&actions).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(enabled) = input.enabled {
        set_doc.insert("enabled", enabled);
    }

    let rule = collection
        .find_one_and_update(doc! { "_id": oid }, doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(rule.into()))
}

pub async fn delete_workflow_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&auth)?;
    let oid = parse_oid(&id)?;
    let collection = state.db.collection::<WorkflowRule>("workflow_rules");
    let result = collection.delete_one(doc! { "_id": oid }).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
