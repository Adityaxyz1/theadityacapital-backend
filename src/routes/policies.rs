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
    models::{
        AddFollowUpInput, CreatePolicyInput, FollowUp, Policy, PolicyResponse, PolicyStatus,
        Renewal, RenewalStatus, UpdatePolicyInput,
    },
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

#[derive(Debug, Deserialize)]
pub struct ListPoliciesQuery {
    pub customer_id: Option<String>,
    pub insurer_name: Option<String>,
    pub policy_type: Option<String>,
    pub assigned_to: Option<String>,
    pub status: Option<PolicyStatus>,
}

pub async fn list_policies(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<ListPoliciesQuery>,
) -> ApiResult<Json<Vec<PolicyResponse>>> {
    let collection = state.db.collection::<Policy>("policies");

    let mut filter = doc! {};
    if let Some(customer_id) = query.customer_id {
        filter.insert("customer_id", parse_oid(&customer_id)?);
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
    if let Some(status) = query.status {
        filter.insert("status", bson::to_bson(&status).map_err(|e| ApiError::Internal(e.into()))?);
    }

    let cursor = collection.find(filter).await?;
    let policies: Vec<Policy> = cursor.try_collect().await?;
    Ok(Json(policies.into_iter().map(Into::into).collect()))
}

pub async fn create_policy(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreatePolicyInput>,
) -> ApiResult<Json<PolicyResponse>> {
    let customer_id = parse_oid(&input.customer_id)?;
    let previous_policy_id = input.previous_policy_id.as_deref().map(parse_oid).transpose()?;
    let assigned_to = match input.assigned_to.as_deref() {
        Some(id) => parse_oid(id)?,
        None => auth.user_id,
    };

    let customers = state.db.collection::<crate::models::Customer>("customers");
    let customer = customers
        .find_one(doc! { "_id": customer_id })
        .await?
        .ok_or(ApiError::BadRequest("customer not found".into()))?;

    let now = Utc::now();
    let policy = Policy {
        id: None,
        customer_id,
        insurer_name: input.insurer_name,
        policy_type: input.policy_type,
        policy_number: input.policy_number,
        sum_insured: input.sum_insured,
        start_date: input.start_date,
        end_date: input.end_date,
        previous_year_premium: input.previous_year_premium,
        current_year_premium: input.current_year_premium,
        status: PolicyStatus::Active,
        previous_policy_id,
        assigned_to,
        source_document_id: None,
        follow_ups: vec![],
        created_at: now,
        updated_at: now,
    };

    let policies = state.db.collection::<Policy>("policies");
    let result = policies.insert_one(&policy).await.map_err(|e| {
        if e.to_string().contains("duplicate key") {
            ApiError::Conflict("a policy with this policy number already exists".into())
        } else {
            ApiError::Database(e)
        }
    })?;

    let mut created = policy;
    created.id = result.inserted_id.as_object_id();
    let policy_id = created.id.expect("just inserted");

    // Every policy needs a corresponding renewals entry so it shows up on the
    // date-wise dashboard/calendar (PRD 4.3) — this is base wiring, not the
    // Phase 1 status-pipeline/notification behavior.
    let renewal = Renewal {
        id: None,
        policy_id,
        customer_id,
        customer_name: customer.name,
        insurer_name: created.insurer_name.clone(),
        policy_type: created.policy_type.clone(),
        premium_due: created.current_year_premium,
        due_date: created.end_date,
        reminder_stage: 0,
        status: RenewalStatus::Pending,
        assigned_to,
        last_contacted_at: None,
        notes: None,
        created_at: now,
    };
    state
        .db
        .collection::<Renewal>("renewals")
        .insert_one(&renewal)
        .await?;

    Ok(Json(created.into()))
}

pub async fn get_policy(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<PolicyResponse>> {
    let oid = parse_oid(&id)?;
    let policy = state
        .db
        .collection::<Policy>("policies")
        .find_one(doc! { "_id": oid })
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(policy.into()))
}

pub async fn update_policy(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdatePolicyInput>,
) -> ApiResult<Json<PolicyResponse>> {
    let oid = parse_oid(&id)?;
    let policies = state.db.collection::<Policy>("policies");

    let mut set_doc = doc! { "updated_at": Utc::now() };
    if let Some(v) = input.insurer_name {
        set_doc.insert("insurer_name", v);
    }
    if let Some(v) = input.policy_type {
        set_doc.insert("policy_type", v);
    }
    if let Some(v) = input.policy_number {
        set_doc.insert("policy_number", v);
    }
    if let Some(v) = input.sum_insured {
        set_doc.insert("sum_insured", v);
    }
    if let Some(v) = input.start_date {
        set_doc.insert("start_date", v);
    }
    if let Some(v) = input.end_date {
        set_doc.insert("end_date", v);
    }
    if let Some(v) = input.previous_year_premium {
        set_doc.insert("previous_year_premium", v);
    }
    if let Some(v) = input.current_year_premium {
        set_doc.insert("current_year_premium", v);
    }
    if let Some(v) = input.status {
        set_doc.insert("status", bson::to_bson(&v).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(v) = input.assigned_to {
        set_doc.insert("assigned_to", parse_oid(&v)?);
    }

    let policy = policies
        .find_one_and_update(doc! { "_id": oid }, doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(policy.into()))
}

pub async fn delete_policy(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = parse_oid(&id)?;
    let policies = state.db.collection::<Policy>("policies");
    let result = policies.delete_one(doc! { "_id": oid }).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    state
        .db
        .collection::<Renewal>("renewals")
        .delete_many(doc! { "policy_id": oid })
        .await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn add_follow_up(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<AddFollowUpInput>,
) -> ApiResult<Json<PolicyResponse>> {
    let oid = parse_oid(&id)?;
    let now = Utc::now();

    let follow_up = FollowUp {
        user_id: auth.user_id,
        kind: input.kind,
        content: input.content,
        created_at: now,
    };
    let follow_up_bson = bson::to_bson(&follow_up).map_err(|e| ApiError::Internal(e.into()))?;

    let policies = state.db.collection::<Policy>("policies");
    let policy = policies
        .find_one_and_update(
            doc! { "_id": oid },
            doc! { "$push": { "follow_ups": follow_up_bson }, "$set": { "updated_at": now } },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Logging a follow-up on a still-pending renewal is, by definition, what
    // moves it into the "contacted" stage of the pipeline (PRD 4.3).
    state
        .db
        .collection::<Renewal>("renewals")
        .update_one(
            doc! { "policy_id": oid, "status": bson::to_bson(&RenewalStatus::Pending).unwrap() },
            doc! { "$set": {
                "status": bson::to_bson(&RenewalStatus::Contacted).unwrap(),
                "last_contacted_at": now,
            } },
        )
        .await?;

    Ok(Json(policy.into()))
}
