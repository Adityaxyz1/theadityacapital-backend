use axum::{extract::State, Json};
use bson::doc;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{create_token, require_admin, AuthUser},
    auth::password::{hash_password, verify_password},
    error::{ApiError, ApiResult},
    models::{Role, User, UserPublic},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct RegisterInput {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub password: String,
    pub role: Option<Role>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
}

// Bootstraps the very first user as admin without auth; every subsequent
// registration requires an existing admin's token.
pub async fn register(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Json(input): Json<RegisterInput>,
) -> ApiResult<Json<UserPublic>> {
    let users = state.db.collection::<User>("users");
    let is_first_user = users.estimated_document_count().await? == 0;

    let role = if is_first_user {
        Role::Admin
    } else {
        let auth = auth.ok_or(ApiError::Unauthorized)?;
        require_admin(&auth)?;
        input.role.unwrap_or(Role::Agent)
    };

    let password_hash = hash_password(&input.password).map_err(ApiError::Internal)?;

    let user = User {
        id: None,
        name: input.name,
        email: input.email.to_lowercase(),
        phone: input.phone,
        password_hash,
        role,
        created_at: Utc::now(),
    };

    let result = users.insert_one(&user).await.map_err(|e| {
        if e.to_string().contains("duplicate key") {
            ApiError::Conflict("a user with this email already exists".into())
        } else {
            ApiError::Database(e)
        }
    })?;

    let mut created = user;
    created.id = result.inserted_id.as_object_id();
    Ok(Json(created.into()))
}

#[derive(Debug, Deserialize)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginInput>,
) -> ApiResult<Json<AuthResponse>> {
    let users = state.db.collection::<User>("users");
    let user = users
        .find_one(doc! { "email": input.email.to_lowercase() })
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if !verify_password(&input.password, &user.password_hash) {
        return Err(ApiError::Unauthorized);
    }

    let id = user.id.ok_or(ApiError::Internal(anyhow::anyhow!("user missing id")))?;
    let token = create_token(&id.to_hex(), user.role, &state.config.jwt_secret)
        .map_err(ApiError::Internal)?;

    Ok(Json(AuthResponse {
        token,
        user: user.into(),
    }))
}

pub async fn me(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<UserPublic>> {
    let users = state.db.collection::<User>("users");
    let user = users
        .find_one(doc! { "_id": auth.user_id })
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(user.into()))
}
