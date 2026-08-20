use axum::{
    extract::{Path, State},
    Json,
};
use bson::doc;
use futures_util::TryStreamExt;
use serde::Deserialize;

use crate::{
    auth::{require_admin, AuthUser},
    error::{ApiError, ApiResult},
    models::{Role, User, UserDirectoryEntry, UserPublic},
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

// Admin-only: staff directory, needed for team-member pickers and role/team
// assignment. Registration itself (routes/auth.rs::register) already creates
// the User; this is the admin-facing management surface for existing users.
pub async fn list_users(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Vec<UserPublic>>> {
    require_admin(&auth)?;
    let collection = state.db.collection::<User>("users");
    let cursor = collection.find(doc! {}).await?;
    let users: Vec<User> = cursor.try_collect().await?;
    Ok(Json(users.into_iter().map(Into::into).collect()))
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserInput {
    pub role: Option<Role>,
    // "" clears the team assignment; omitted leaves it unchanged.
    pub team_id: Option<String>,
}

pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateUserInput>,
) -> ApiResult<Json<UserPublic>> {
    require_admin(&auth)?;
    let oid = parse_oid(&id)?;
    let collection = state.db.collection::<User>("users");

    let mut set_doc = doc! {};
    if let Some(role) = input.role {
        set_doc.insert("role", bson::to_bson(&role).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(team_id) = input.team_id {
        if team_id.is_empty() {
            set_doc.insert("team_id", bson::Bson::Null);
        } else {
            set_doc.insert("team_id", parse_oid(&team_id)?);
        }
    }

    let user = collection
        .find_one_and_update(doc! { "_id": oid }, doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(user.into()))
}

// Not admin-gated, unlike list_users above — this is the "who can I message"
// picker for the messaging feature (routes/messages.rs), so every
// authenticated staff member needs it, not just admins. Deliberately returns
// UserDirectoryEntry (id/name/public_key only), never the full UserPublic
// (email/phone/role), so it can't be used as a back-door staff directory.
pub async fn list_directory(State(state): State<AppState>, _auth: AuthUser) -> ApiResult<Json<Vec<UserDirectoryEntry>>> {
    let collection = state.db.collection::<User>("users");
    let cursor = collection.find(doc! {}).await?;
    let users: Vec<User> = cursor.try_collect().await?;
    Ok(Json(users.into_iter().map(Into::into).collect()))
}

#[derive(Debug, Deserialize)]
pub struct SetPublicKeyInput {
    pub public_key: String,
}

// Self-service only (no :id in the path — always the caller's own record):
// a user's public key is uploaded by their own browser right after it
// generates (or loads) that browser's ECDH keypair. See lib/crypto.ts.
pub async fn set_my_public_key(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<SetPublicKeyInput>,
) -> ApiResult<Json<UserPublic>> {
    let collection = state.db.collection::<User>("users");
    let user = collection
        .find_one_and_update(
            doc! { "_id": auth.user_id },
            doc! { "$set": { "public_key": input.public_key } },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(user.into()))
}
