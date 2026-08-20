use axum::{
    extract::{Query, State},
    Json,
};
use bson::doc;
use serde::Deserialize;

use crate::{
    activity::{self, NewActivity},
    auth::AuthUser,
    error::{ApiError, ApiResult},
    listview::{paginate, PageParams},
    models::{Activity, ActivityResponse, CreateActivityInput},
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

#[derive(Debug, Deserialize)]
pub struct ListActivitiesQuery {
    pub customer_id: Option<String>,
}

// The Activities feed / customer-drawer "Timeline" tab — real trace of what
// happened, not the 5 frozen mock rows the frontend audit found. Scoped the
// same way Suggestions are: visibility_filter over the record's own
// `assigned_to`, so an agent only sees activity for their own book.
pub async fn list_activities(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListActivitiesQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<crate::listview::Paginated<ActivityResponse>>> {
    let collection = state.db.collection::<Activity>("activities");

    let mut filter = doc! {};
    if let Some(customer_id) = query.customer_id {
        filter.insert("customer_id", parse_oid(&customer_id)?);
    }
    let filter = combine_filters(filter, visibility_filter(&state.db, &auth).await?);
    let sort = doc! { "created_at": -1 };

    Ok(Json(paginate(&collection, filter, sort, &page).await?))
}

// Manual entry — backs QuickAddModal's "Activity" tab (a call/note that
// isn't the side effect of some other write, so there's no natural handler
// to hang activity::log off of).
pub async fn create_activity(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateActivityInput>,
) -> ApiResult<Json<ActivityResponse>> {
    let customer_id = input.customer_id.as_deref().map(parse_oid).transpose()?;

    activity::log(
        &state,
        NewActivity {
            user_id: auth.user_id,
            action: input.action,
            customer_id,
            customer_name: input.customer_name,
            policy_type: input.policy_type,
            icon_type: input.icon_type,
            assigned_to: Some(auth.user_id),
        },
    )
    .await;

    let latest = state
        .db
        .collection::<Activity>("activities")
        .find_one(doc! { "user_id": auth.user_id })
        .sort(doc! { "created_at": -1 })
        .await?
        .ok_or(ApiError::Internal(anyhow::anyhow!("activity insert did not persist")))?;

    Ok(Json(latest.into()))
}
