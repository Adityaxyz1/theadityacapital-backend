use axum::{extract::FromRequestParts, http::request::Parts};
use bson::oid::ObjectId;

use crate::{
    auth::jwt::decode_token,
    error::ApiError,
    models::Role,
    state::AppState,
};

pub struct AuthUser {
    pub user_id: ObjectId,
    pub role: Role,
}

fn extract_from_parts(parts: &Parts, state: &AppState) -> Option<AuthUser> {
    let header = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())?;
    let token = header.strip_prefix("Bearer ")?;
    let claims = decode_token(token, &state.config.jwt_secret).ok()?;
    let user_id = ObjectId::parse_str(&claims.sub).ok()?;
    Some(AuthUser {
        user_id,
        role: claims.role,
    })
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        extract_from_parts(parts, state).ok_or(ApiError::Unauthorized)
    }
}

// Lets handlers (e.g. bootstrap registration) treat auth as optional instead of
// hard-rejecting the request when no token is present.
impl FromRequestParts<AppState> for Option<AuthUser> {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        Ok(extract_from_parts(parts, state))
    }
}

pub fn require_admin(user: &AuthUser) -> Result<(), ApiError> {
    match user.role {
        Role::Admin => Ok(()),
        Role::Manager | Role::Agent => Err(ApiError::Forbidden),
    }
}
