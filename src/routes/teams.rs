use axum::{
    extract::{Path, State},
    Json,
};
use bson::doc;
use futures_util::TryStreamExt;

use crate::{
    auth::{require_admin, AuthUser},
    error::{ApiError, ApiResult},
    models::{CreateTeamInput, Team, TeamResponse, UpdateTeamInput},
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

// Team management is admin-only (same guard already used for user
// registration and notification-rule CRUD) — assigning managers/members is a
// structural org-chart decision, not something an agent/manager self-serves.
pub async fn list_teams(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Vec<TeamResponse>>> {
    require_admin(&auth)?;
    let collection = state.db.collection::<Team>("teams");
    let cursor = collection.find(doc! {}).await?;
    let teams: Vec<Team> = cursor.try_collect().await?;
    Ok(Json(teams.into_iter().map(Into::into).collect()))
}

pub async fn create_team(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateTeamInput>,
) -> ApiResult<Json<TeamResponse>> {
    require_admin(&auth)?;
    let member_ids = input
        .member_ids
        .iter()
        .map(|id| parse_oid(id))
        .collect::<ApiResult<Vec<_>>>()?;

    let team = Team {
        id: None,
        name: input.name,
        manager_id: parse_oid(&input.manager_id)?,
        member_ids,
    };
    let collection = state.db.collection::<Team>("teams");
    let result = collection.insert_one(&team).await?;
    let mut created = team;
    created.id = result.inserted_id.as_object_id();
    Ok(Json(created.into()))
}

pub async fn update_team(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateTeamInput>,
) -> ApiResult<Json<TeamResponse>> {
    require_admin(&auth)?;
    let oid = parse_oid(&id)?;
    let collection = state.db.collection::<Team>("teams");

    let mut set_doc = doc! {};
    if let Some(name) = input.name {
        set_doc.insert("name", name);
    }
    if let Some(manager_id) = input.manager_id {
        set_doc.insert("manager_id", parse_oid(&manager_id)?);
    }
    if let Some(member_ids) = input.member_ids {
        let ids = member_ids
            .iter()
            .map(|id| parse_oid(id))
            .collect::<ApiResult<Vec<_>>>()?;
        set_doc.insert(
            "member_ids",
            bson::to_bson(&ids).map_err(|e| ApiError::Internal(e.into()))?,
        );
    }

    let team = collection
        .find_one_and_update(doc! { "_id": oid }, doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(team.into()))
}

pub async fn delete_team(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    require_admin(&auth)?;
    let oid = parse_oid(&id)?;
    let collection = state.db.collection::<Team>("teams");
    let result = collection.delete_one(doc! { "_id": oid }).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
