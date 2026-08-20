use axum::{
    extract::{Path, State},
    Json,
};
use bson::doc;
use chrono::Utc;
use futures_util::TryStreamExt;

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    models::{CreateDashboardWidgetInput, DashboardWidget, DashboardWidgetResponse},
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

// Pinned widgets are personal — each staff member curates their own
// dashboard, so this is always scoped to the caller's own `user_id`, not the
// row-level visibility_filter used for policies/renewals (there's no
// "manager sees their team's pinned widgets" concept here).
pub async fn list_my_widgets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<DashboardWidgetResponse>>> {
    let collection = state.db.collection::<DashboardWidget>("dashboard_widgets");
    let cursor = collection
        .find(doc! { "user_id": auth.user_id })
        .sort(doc! { "position": 1 })
        .await?;
    let widgets: Vec<DashboardWidget> = cursor.try_collect().await?;
    Ok(Json(widgets.into_iter().map(Into::into).collect()))
}

pub async fn pin_widget(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateDashboardWidgetInput>,
) -> ApiResult<Json<DashboardWidgetResponse>> {
    let collection = state.db.collection::<DashboardWidget>("dashboard_widgets");
    let next_position = collection.count_documents(doc! { "user_id": auth.user_id }).await? as i32;

    let widget = DashboardWidget {
        id: None,
        user_id: auth.user_id,
        report_key: input.report_key,
        title: input.title,
        params: input.params,
        position: next_position,
        created_at: Utc::now(),
    };
    let result = collection.insert_one(&widget).await?;
    let mut created = widget;
    created.id = result.inserted_id.as_object_id();
    Ok(Json(created.into()))
}

pub async fn unpin_widget(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = parse_oid(&id)?;
    let collection = state.db.collection::<DashboardWidget>("dashboard_widgets");
    let result = collection.delete_one(doc! { "_id": oid, "user_id": auth.user_id }).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
