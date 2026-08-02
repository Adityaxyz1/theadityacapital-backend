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
    models::{
        Notification, NotificationResponse, NotificationRule, NotificationRuleResponse,
        UpsertNotificationRuleInput,
    },
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

pub async fn list_my_notifications(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<NotificationResponse>>> {
    let collection = state.db.collection::<Notification>("notifications");
    let cursor = collection
        .find(doc! { "user_id": auth.user_id })
        .sort(doc! { "created_at": -1 })
        .limit(100)
        .await?;
    let notifications: Vec<Notification> = cursor.try_collect().await?;
    Ok(Json(notifications.into_iter().map(Into::into).collect()))
}

pub async fn mark_notification_read(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<NotificationResponse>> {
    let oid = parse_oid(&id)?;
    let collection = state.db.collection::<Notification>("notifications");

    let notification = collection
        .find_one_and_update(
            doc! { "_id": oid, "user_id": auth.user_id },
            doc! { "$set": { "read_at": Utc::now() } },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(notification.into()))
}

pub async fn list_notification_rules(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<NotificationRuleResponse>>> {
    require_admin(&auth)?;
    let collection = state.db.collection::<NotificationRule>("notification_rules");
    let cursor = collection.find(doc! {}).await?;
    let rules: Vec<NotificationRule> = cursor.try_collect().await?;
    Ok(Json(rules.into_iter().map(Into::into).collect()))
}

pub async fn upsert_notification_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<UpsertNotificationRuleInput>,
) -> ApiResult<Json<NotificationRuleResponse>> {
    require_admin(&auth)?;
    let collection = state.db.collection::<NotificationRule>("notification_rules");

    let policy_type_filter = match &input.policy_type {
        Some(v) => bson::Bson::String(v.clone()),
        None => bson::Bson::Null,
    };

    let rule = collection
        .find_one_and_update(
            doc! { "policy_type": policy_type_filter },
            doc! { "$set": {
                "policy_type": input.policy_type,
                "offset_days": &input.offset_days,
                "template": &input.template,
            } },
        )
        .upsert(true)
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::Internal(anyhow::anyhow!("upsert returned no document")))?;

    Ok(Json(rule.into()))
}
