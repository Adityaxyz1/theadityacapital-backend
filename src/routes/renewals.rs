use axum::{
    extract::{Path, Query, State},
    Json,
};
use bson::doc;
use chrono::Utc;
use futures_util::TryStreamExt;

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    models::{ListRenewalsQuery, Policy, PolicyStatus, Renewal, RenewalResponse, RenewalStatus, UpdateRenewalStatusInput},
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

pub async fn list_renewals(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<ListRenewalsQuery>,
) -> ApiResult<Json<Vec<RenewalResponse>>> {
    let collection = state.db.collection::<Renewal>("renewals");

    let mut filter = doc! {};
    if let Some(status) = query.status {
        filter.insert("status", bson::to_bson(&status).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(insurer_name) = query.insurer_name {
        filter.insert("insurer_name", insurer_name);
    }
    if let Some(policy_type) = query.policy_type {
        filter.insert("policy_type", policy_type);
    }
    if let Some(assigned_to) = query.assigned_to {
        filter.insert("assigned_to", parse_oid(&assigned_to)?);
    }
    if let Some(days) = query.due_within_days {
        let cutoff = Utc::now() + chrono::Duration::days(days);
        filter.insert("due_date", doc! { "$lte": cutoff });
    }

    let cursor = collection.find(filter).await?;
    let renewals: Vec<Renewal> = cursor.try_collect().await?;
    Ok(Json(renewals.into_iter().map(Into::into).collect()))
}

pub async fn update_renewal_status(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateRenewalStatusInput>,
) -> ApiResult<Json<RenewalResponse>> {
    let oid = parse_oid(&id)?;
    let renewals = state.db.collection::<Renewal>("renewals");

    let mut set_doc = doc! {
        "status": bson::to_bson(&input.status).map_err(|e| ApiError::Internal(e.into()))?,
    };
    if input.status == RenewalStatus::Contacted {
        set_doc.insert("last_contacted_at", Utc::now());
    }
    if let Some(notes) = input.notes {
        set_doc.insert("notes", notes);
    }

    let renewal = renewals
        .find_one_and_update(doc! { "_id": oid }, doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Renewed/Lapsed/Lost are terminal outcomes for the underlying policy too
    // (PolicyStatus mirrors them); Pending/Contacted are renewal-pipeline-only
    // states that don't change the policy's own status.
    let policy_status = match input.status {
        RenewalStatus::Renewed => Some(PolicyStatus::Renewed),
        RenewalStatus::Lapsed => Some(PolicyStatus::Lapsed),
        RenewalStatus::Lost => Some(PolicyStatus::Lost),
        RenewalStatus::Pending | RenewalStatus::Contacted => None,
    };
    if let Some(policy_status) = policy_status {
        state
            .db
            .collection::<Policy>("policies")
            .update_one(
                doc! { "_id": renewal.policy_id },
                doc! { "$set": { "status": bson::to_bson(&policy_status).map_err(|e| ApiError::Internal(e.into()))?, "updated_at": Utc::now() } },
            )
            .await?;
    }

    Ok(Json(renewal.into()))
}
