use axum::{
    extract::{Path, Query, State},
    Json,
};
use bson::{doc, Document};
use chrono::Utc;
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    listview::{paginate, parse_sort, PageParams, Paginated},
    models::{
        AddFollowUpInput, Claim, CreatePolicyInput, FollowUp, Policy, PolicyResponse, PolicyStatus,
        Renewal, RenewalStatus, UpdatePolicyInput,
    },
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

const SORTABLE_FIELDS: &[&str] = &["created_at", "updated_at", "end_date", "current_year_premium", "policy_number"];

#[derive(Debug, Deserialize)]
pub struct ListPoliciesQuery {
    pub customer_id: Option<String>,
    pub insurer_name: Option<String>,
    pub policy_type: Option<String>,
    // Keyword-bucketed Life/Health/Motor/Other, distinct from the exact-match
    // `policy_type` above — see categorize.rs.
    pub category: Option<String>,
    pub assigned_to: Option<String>,
    pub status: Option<PolicyStatus>,
}

// See customers.rs::list_customers for why pagination is a separate `Query`
// extractor rather than `#[serde(flatten)]`-ed in.
pub async fn list_policies(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListPoliciesQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Paginated<PolicyResponse>>> {
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
    if let Some(category) = query.category {
        if let Some(cat_filter) = crate::categorize::category_filter(&category) {
            filter = combine_filters(filter, cat_filter);
        }
    }
    let filter = combine_filters(filter, visibility_filter(&state.db, &auth).await?);
    let sort = parse_sort(&page.sort, SORTABLE_FIELDS, "created_at")?;

    Ok(Json(paginate(&collection, filter, sort, &page).await?))
}

pub async fn create_policy(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreatePolicyInput>,
) -> ApiResult<Json<PolicyResponse>> {
    let customer_id = parse_oid(&input.customer_id)?;
    let previous_policy_id = input.previous_policy_id.as_deref().map(parse_oid).transpose()?;
    let source_document_id = input.source_document_id.as_deref().map(parse_oid).transpose()?;
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
        source_document_id,
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
        policy_number: created.policy_number.clone(),
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
    let renewal_result = state
        .db
        .collection::<Renewal>("renewals")
        .insert_one(&renewal)
        .await?;

    if let Ok(policy_doc) = bson::to_document(&created) {
        crate::workflow::on_record_created(&state, crate::models::EntityType::Policy, policy_id, &policy_doc).await;
    }
    if let Some(renewal_id) = renewal_result.inserted_id.as_object_id() {
        if let Ok(mut renewal_doc) = bson::to_document(&renewal) {
            renewal_doc.insert("_id", renewal_id);
            crate::workflow::on_record_created(&state, crate::models::EntityType::Renewal, renewal_id, &renewal_doc).await;
        }
    }

    crate::activity::log(
        &state,
        crate::activity::NewActivity {
            user_id: auth.user_id,
            action: format!(
                "added a new {} policy for {}",
                created.policy_type, renewal.customer_name
            ),
            customer_id: Some(customer_id),
            customer_name: Some(renewal.customer_name.clone()),
            policy_type: Some(created.policy_type.clone()),
            icon_type: crate::models::ActivityIcon::Policy,
            assigned_to: Some(assigned_to),
        },
    )
    .await;

    Ok(Json(created.into()))
}

pub async fn get_policy(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<PolicyResponse>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);
    let policy = state
        .db
        .collection::<Policy>("policies")
        .find_one(filter)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(policy.into()))
}

pub async fn update_policy(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdatePolicyInput>,
) -> ApiResult<Json<PolicyResponse>> {
    let oid = parse_oid(&id)?;
    let scope = visibility_filter(&state.db, &auth).await?;
    let policies = state.db.collection::<Policy>("policies");

    // Renewal/Claim denormalize several of these fields off the policy at
    // creation time (ARCHITECTURE.md §3) so list/report reads skip a $lookup.
    // Editing a policy here used to only ever touch the `policies` collection,
    // so any of these fields changing (e.g. correcting a misextracted
    // insurer_name) silently left the linked renewal/claim showing the old
    // value forever — exactly the kind of drift that made Varun Tiwari's
    // renewal disagree with his policy on insurer_name. Building the same
    // $set doc for renewals/claims here, restricted to the fields each of
    // them actually denormalizes, keeps all three in sync on every edit.
    let mut set_doc = doc! { "updated_at": Utc::now() };
    let mut renewal_set = Document::new();
    let mut claim_set = Document::new();

    if let Some(v) = input.insurer_name {
        set_doc.insert("insurer_name", v.clone());
        renewal_set.insert("insurer_name", v.clone());
        claim_set.insert("insurer_name", v);
    }
    if let Some(v) = input.policy_type {
        set_doc.insert("policy_type", v.clone());
        renewal_set.insert("policy_type", v.clone());
        claim_set.insert("policy_type", v);
    }
    if let Some(v) = input.policy_number {
        set_doc.insert("policy_number", v.clone());
        renewal_set.insert("policy_number", v.clone());
        claim_set.insert("policy_number", v);
    }
    if let Some(v) = input.sum_insured {
        set_doc.insert("sum_insured", v);
    }
    if let Some(v) = input.start_date {
        set_doc.insert("start_date", v);
    }
    if let Some(v) = input.end_date {
        set_doc.insert("end_date", v);
        renewal_set.insert("due_date", v);
    }
    if let Some(v) = input.previous_year_premium {
        set_doc.insert("previous_year_premium", v);
    }
    if let Some(v) = input.current_year_premium {
        set_doc.insert("current_year_premium", v);
        renewal_set.insert("premium_due", v);
    }
    if let Some(v) = input.status {
        set_doc.insert("status", bson::to_bson(&v).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(v) = input.assigned_to {
        let assigned_oid = parse_oid(&v)?;
        set_doc.insert("assigned_to", assigned_oid);
        renewal_set.insert("assigned_to", assigned_oid);
    }

    let policy = policies
        .find_one_and_update(combine_filters(doc! { "_id": oid }, scope), doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;

    if !renewal_set.is_empty() {
        state
            .db
            .collection::<Renewal>("renewals")
            .update_many(doc! { "policy_id": oid }, doc! { "$set": renewal_set })
            .await?;
    }
    if !claim_set.is_empty() {
        state
            .db
            .collection::<Claim>("claims")
            .update_many(doc! { "policy_id": oid }, doc! { "$set": claim_set })
            .await?;
    }

    Ok(Json(policy.into()))
}

pub async fn delete_policy(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);
    let policies = state.db.collection::<Policy>("policies");
    let result = policies.delete_one(filter).await?;
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
    let scope = visibility_filter(&state.db, &auth).await?;

    let policies = state.db.collection::<Policy>("policies");
    let policy = policies
        .find_one_and_update(
            combine_filters(doc! { "_id": oid }, scope),
            doc! { "$push": { "follow_ups": follow_up_bson }, "$set": { "updated_at": now } },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Logging a follow-up on a still-pending renewal is, by definition, what
    // moves it into the "contacted" stage of the pipeline (PRD 4.3). Uses
    // find_one_and_update (not update_one) so the updated doc is available to
    // broadcast to the Kanban board — a follow-up logged from the customer
    // page should move that renewal's card live even if nobody dragged it.
    if let Some(updated_renewal) = state
        .db
        .collection::<Renewal>("renewals")
        .find_one_and_update(
            doc! { "policy_id": oid, "status": bson::to_bson(&RenewalStatus::Pending).unwrap() },
            doc! { "$set": {
                "status": bson::to_bson(&RenewalStatus::Contacted).unwrap(),
                "last_contacted_at": now,
            } },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
    {
        crate::notify::broadcast_renewal_updated(&state, &updated_renewal.into()).await;
    }

    let customer_name = state
        .db
        .collection::<crate::models::Customer>("customers")
        .find_one(doc! { "_id": policy.customer_id })
        .await?
        .map(|c| c.name);
    crate::activity::log(
        &state,
        crate::activity::NewActivity {
            user_id: auth.user_id,
            action: format!(
                "logged a {:?} follow-up on {}",
                follow_up.kind,
                customer_name.clone().unwrap_or_default()
            ),
            customer_id: Some(policy.customer_id),
            customer_name,
            policy_type: Some(policy.policy_type.clone()),
            icon_type: crate::models::ActivityIcon::Reminder,
            assigned_to: Some(policy.assigned_to),
        },
    )
    .await;

    Ok(Json(policy.into()))
}
