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
    models::{EntityType, Suggestion, SuggestionResponse, SuggestionStatus, UpdateSuggestionInput},
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

fn parse_entity_type(s: &str) -> ApiResult<EntityType> {
    match s {
        "renewal" => Ok(EntityType::Renewal),
        "policy" => Ok(EntityType::Policy),
        "customer" => Ok(EntityType::Customer),
        _ => Err(ApiError::BadRequest("unknown entity type".into())),
    }
}

#[derive(Debug, Deserialize)]
pub struct ListSuggestionsQuery {
    pub status: Option<SuggestionStatus>,
}

// Suggestions are scoped by the same visibility rule as the records they're
// about (they carry a denormalized `assigned_to` — see models/suggestion.rs)
// — a "start my day" triage list of what's Open and relevant to the caller.
pub async fn list_suggestions(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListSuggestionsQuery>,
) -> ApiResult<Json<Vec<SuggestionResponse>>> {
    let mut filter = doc! {};
    filter.insert(
        "status",
        bson::to_bson(&query.status.unwrap_or(SuggestionStatus::Open)).map_err(|e| ApiError::Internal(e.into()))?,
    );
    let filter = combine_filters(filter, visibility_filter(&state.db, &auth).await?);

    let collection = state.db.collection::<Suggestion>("suggestions");
    let cursor = collection.find(filter).sort(doc! { "created_at": -1 }).limit(100).await?;
    let suggestions: Vec<Suggestion> = cursor.try_collect().await?;
    Ok(Json(suggestions.into_iter().map(Into::into).collect()))
}

// Powers the inline suggestion cards on a record's own detail page.
pub async fn list_suggestions_for_record(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((entity_type, entity_id)): Path<(String, String)>,
) -> ApiResult<Json<Vec<SuggestionResponse>>> {
    let entity_type = parse_entity_type(&entity_type)?;
    let oid = parse_oid(&entity_id)?;
    let filter = combine_filters(
        doc! {
            "entity_type": bson::to_bson(&entity_type).map_err(|e| ApiError::Internal(e.into()))?,
            "entity_id": oid,
            "status": "open",
        },
        visibility_filter(&state.db, &auth).await?,
    );
    let collection = state.db.collection::<Suggestion>("suggestions");
    let cursor = collection.find(filter).await?;
    let suggestions: Vec<Suggestion> = cursor.try_collect().await?;
    Ok(Json(suggestions.into_iter().map(Into::into).collect()))
}

// Dismiss or mark actioned. There's no separate "action" endpoint — actioning
// a suggestion is just the frontend calling the normal CRUD action it points
// at (e.g. POST /policies/{id}/follow-ups), then patching this to record
// that it was acted on. No approval gate, no automation executes anything.
pub async fn update_suggestion(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateSuggestionInput>,
) -> ApiResult<Json<SuggestionResponse>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);

    let mut set_doc = doc! {
        "status": bson::to_bson(&input.status).map_err(|e| ApiError::Internal(e.into()))?,
    };
    if input.status != SuggestionStatus::Open {
        set_doc.insert("dismissed_at", Utc::now());
        set_doc.insert("dismissed_by", auth.user_id);
    }

    let collection = state.db.collection::<Suggestion>("suggestions");
    let suggestion = collection
        .find_one_and_update(filter, doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(suggestion.into()))
}
