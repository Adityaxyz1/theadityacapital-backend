use axum::{
    extract::{Path, Query, State},
    Json,
};
use bson::doc;
use chrono::Utc;
use futures_util::TryStreamExt;
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    models::{CreateSavedViewInput, ListObject, SavedView, SavedViewResponse},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListSavedViewsQuery {
    pub object: ListObject,
}

pub async fn list_saved_views(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListSavedViewsQuery>,
) -> ApiResult<Json<Vec<SavedViewResponse>>> {
    let collection = state.db.collection::<SavedView>("saved_views");
    let filter = doc! {
        "object": bson::to_bson(&query.object).map_err(|e| ApiError::Internal(e.into()))?,
        "$or": [{ "owner_id": auth.user_id }, { "is_shared": true }],
    };
    let cursor = collection.find(filter).await?;
    let views: Vec<SavedView> = cursor.try_collect().await?;
    Ok(Json(views.into_iter().map(Into::into).collect()))
}

pub async fn create_saved_view(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateSavedViewInput>,
) -> ApiResult<Json<SavedViewResponse>> {
    let collection = state.db.collection::<SavedView>("saved_views");
    let view = SavedView {
        id: None,
        object: input.object,
        name: input.name,
        owner_id: auth.user_id,
        is_shared: input.is_shared.unwrap_or(false),
        query: input.query,
        created_at: Utc::now(),
    };
    let result = collection.insert_one(&view).await?;
    let mut created = view;
    created.id = result.inserted_id.as_object_id();
    Ok(Json(created.into()))
}

pub async fn delete_saved_view(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = bson::oid::ObjectId::parse_str(&id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let collection = state.db.collection::<SavedView>("saved_views");
    // Scoped to the caller's own views — a shared view can be used by anyone
    // but only deleted by whoever created it.
    let result = collection
        .delete_one(doc! { "_id": oid, "owner_id": auth.user_id })
        .await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
