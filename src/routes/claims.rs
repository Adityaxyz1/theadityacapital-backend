use axum::{
    extract::{Path, Query, State},
    Json,
};
use bson::doc;
use chrono::Utc;
use serde::Deserialize;

use crate::{
    activity::{self, NewActivity},
    auth::AuthUser,
    error::{ApiError, ApiResult},
    listview::{paginate, parse_sort, PageParams, Paginated},
    models::{ActivityIcon, Claim, ClaimResponse, ClaimStatus, CreateClaimInput, Policy, UpdateClaimStatusInput},
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

const SORTABLE_FIELDS: &[&str] = &["filed_date", "claim_amount", "created_at"];

#[derive(Debug, Deserialize)]
pub struct ListClaimsQuery {
    pub status: Option<ClaimStatus>,
    pub customer_id: Option<String>,
    pub policy_type: Option<String>,
    // Keyword-bucketed Life/Health/Motor/Other, distinct from the exact-match
    // `policy_type` above — see categorize.rs.
    pub category: Option<String>,
}

pub async fn list_claims(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListClaimsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Paginated<ClaimResponse>>> {
    let collection = state.db.collection::<Claim>("claims");

    let mut filter = doc! {};
    if let Some(status) = query.status {
        filter.insert("status", bson::to_bson(&status).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(customer_id) = query.customer_id {
        filter.insert("customer_id", parse_oid(&customer_id)?);
    }
    if let Some(policy_type) = query.policy_type {
        filter.insert("policy_type", policy_type);
    }
    if let Some(category) = query.category {
        if let Some(cat_filter) = crate::categorize::category_filter(&category) {
            filter = combine_filters(filter, cat_filter);
        }
    }
    let filter = combine_filters(filter, visibility_filter(&state.db, &auth).await?);
    let sort = parse_sort(&page.sort, SORTABLE_FIELDS, "filed_date")?;

    Ok(Json(paginate(&collection, filter, sort, &page).await?))
}

// Denormalizes customer_name/policy_number/insurer_name from the policy at
// creation time (same reasoning as Renewal, ARCHITECTURE.md §3) — the policy
// must exist and already carries the customer_id/assigned_to this claim
// inherits, so there's no separate customer_id input field.
pub async fn create_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateClaimInput>,
) -> ApiResult<Json<ClaimResponse>> {
    let policy_id = parse_oid(&input.policy_id)?;
    let policy = state
        .db
        .collection::<Policy>("policies")
        .find_one(combine_filters(
            doc! { "_id": policy_id },
            visibility_filter(&state.db, &auth).await?,
        ))
        .await?
        .ok_or(ApiError::BadRequest("policy not found".into()))?;

    let customer = state
        .db
        .collection::<crate::models::Customer>("customers")
        .find_one(doc! { "_id": policy.customer_id })
        .await?
        .ok_or(ApiError::Internal(anyhow::anyhow!("policy references a missing customer")))?;

    let claim = Claim {
        id: None,
        policy_id,
        customer_id: policy.customer_id,
        customer_name: customer.name.clone(),
        policy_number: policy.policy_number,
        insurer_name: policy.insurer_name,
        policy_type: policy.policy_type,
        claim_number: input.claim_number,
        claim_amount: input.claim_amount,
        status: ClaimStatus::Pending,
        filed_date: input.filed_date,
        settled_at: None,
        assigned_to: policy.assigned_to,
        created_at: Utc::now(),
    };

    let collection = state.db.collection::<Claim>("claims");
    let result = collection.insert_one(&claim).await?;
    let mut created = claim;
    created.id = result.inserted_id.as_object_id();

    if let (Some(id), Ok(doc)) = (created.id, bson::to_document(&created)) {
        crate::workflow::on_record_created(&state, crate::models::EntityType::Claim, id, &doc).await;
    }

    activity::log(
        &state,
        NewActivity {
            user_id: auth.user_id,
            action: format!("filed claim {} for {}", created.claim_number, customer.name),
            customer_id: Some(policy.customer_id),
            customer_name: Some(customer.name),
            policy_type: Some(created.claim_number.clone()),
            icon_type: ActivityIcon::Claim,
            assigned_to: Some(created.assigned_to),
        },
    )
    .await;

    Ok(Json(created.into()))
}

pub async fn get_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<ClaimResponse>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);
    let claim = state
        .db
        .collection::<Claim>("claims")
        .find_one(filter)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(claim.into()))
}

pub async fn update_claim_status(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateClaimStatusInput>,
) -> ApiResult<Json<ClaimResponse>> {
    let oid = parse_oid(&id)?;
    let scope = visibility_filter(&state.db, &auth).await?;
    let collection = state.db.collection::<Claim>("claims");

    let mut set_doc = doc! {
        "status": bson::to_bson(&input.status).map_err(|e| ApiError::Internal(e.into()))?,
    };
    if input.status == ClaimStatus::Settled {
        set_doc.insert("settled_at", Utc::now());
    }

    let claim = collection
        .find_one_and_update(combine_filters(doc! { "_id": oid }, scope), doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(claim.into()))
}

pub async fn delete_claim(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);
    let result = state.db.collection::<Claim>("claims").delete_one(filter).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
