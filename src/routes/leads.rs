use axum::{
    extract::{Path, Query, State},
    Json,
};
use bson::doc;
use chrono::Utc;
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    listview::{paginate, parse_sort, PageParams, Paginated},
    models::{CreateLeadInput, Lead, LeadResponse, LeadStage, UpdateLeadInput, UpdateLeadStageInput},
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

const SORTABLE_FIELDS: &[&str] = &["created_at", "updated_at", "estimated_premium", "probability"];

#[derive(Debug, Deserialize)]
pub struct ListLeadsQuery {
    pub stage: Option<LeadStage>,
    pub assigned_to: Option<String>,
}

pub async fn list_leads(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListLeadsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Paginated<LeadResponse>>> {
    let collection = state.db.collection::<Lead>("leads");

    let mut filter = doc! {};
    if let Some(stage) = query.stage {
        filter.insert("stage", bson::to_bson(&stage).map_err(|e| ApiError::Internal(e.into()))?);
    }
    if let Some(assigned_to) = query.assigned_to {
        filter.insert("assigned_to", parse_oid(&assigned_to)?);
    }
    let filter = combine_filters(filter, visibility_filter(&state.db, &auth).await?);
    let sort = parse_sort(&page.sort, SORTABLE_FIELDS, "created_at")?;

    Ok(Json(paginate(&collection, filter, sort, &page).await?))
}

pub async fn create_lead(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateLeadInput>,
) -> ApiResult<Json<LeadResponse>> {
    let assigned_to = match input.assigned_to.as_deref() {
        Some(id) => parse_oid(id)?,
        None => auth.user_id,
    };
    let now = Utc::now();

    let lead = Lead {
        id: None,
        name: input.name,
        phone: input.phone,
        email: input.email,
        policy_type: input.policy_type,
        estimated_premium: input.estimated_premium,
        stage: LeadStage::New,
        probability: input.probability,
        assigned_to,
        created_at: now,
        updated_at: now,
    };

    let collection = state.db.collection::<Lead>("leads");
    let result = collection.insert_one(&lead).await?;
    let mut created = lead;
    created.id = result.inserted_id.as_object_id();

    if let (Some(id), Ok(doc)) = (created.id, bson::to_document(&created)) {
        crate::workflow::on_record_created(&state, crate::models::EntityType::Lead, id, &doc).await;
    }

    crate::activity::log(
        &state,
        crate::activity::NewActivity {
            user_id: auth.user_id,
            action: format!("added a new lead for {}", created.name),
            customer_id: None,
            customer_name: Some(created.name.clone()),
            policy_type: Some(created.policy_type.clone()),
            icon_type: crate::models::ActivityIcon::Lead,
            assigned_to: Some(assigned_to),
        },
    )
    .await;

    Ok(Json(created.into()))
}

pub async fn get_lead(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<LeadResponse>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);
    let lead = state
        .db
        .collection::<Lead>("leads")
        .find_one(filter)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(lead.into()))
}

pub async fn update_lead(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateLeadInput>,
) -> ApiResult<Json<LeadResponse>> {
    let oid = parse_oid(&id)?;
    let scope = visibility_filter(&state.db, &auth).await?;
    let collection = state.db.collection::<Lead>("leads");

    let mut set_doc = doc! { "updated_at": Utc::now() };
    if let Some(name) = input.name {
        set_doc.insert("name", name);
    }
    if let Some(phone) = input.phone {
        set_doc.insert("phone", phone);
    }
    if let Some(email) = input.email {
        set_doc.insert("email", email);
    }
    if let Some(policy_type) = input.policy_type {
        set_doc.insert("policy_type", policy_type);
    }
    if let Some(estimated_premium) = input.estimated_premium {
        set_doc.insert("estimated_premium", estimated_premium);
    }
    if let Some(probability) = input.probability {
        set_doc.insert("probability", probability);
    }
    if let Some(assigned_to) = input.assigned_to {
        set_doc.insert("assigned_to", parse_oid(&assigned_to)?);
    }

    let lead = collection
        .find_one_and_update(combine_filters(doc! { "_id": oid }, scope), doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(lead.into()))
}

// A sales pipeline moves freely both directions (a "negotiation" lead can
// slip back to "contacted"), unlike the renewal pipeline's terminal-state
// guard (renewals.rs::is_valid_transition) — so no transition validation here.
pub async fn update_lead_stage(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateLeadStageInput>,
) -> ApiResult<Json<LeadResponse>> {
    let oid = parse_oid(&id)?;
    let scope = visibility_filter(&state.db, &auth).await?;
    let collection = state.db.collection::<Lead>("leads");

    let lead = collection
        .find_one_and_update(
            combine_filters(doc! { "_id": oid }, scope),
            doc! { "$set": {
                "stage": bson::to_bson(&input.stage).map_err(|e| ApiError::Internal(e.into()))?,
                "updated_at": Utc::now(),
            } },
        )
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(lead.into()))
}

pub async fn delete_lead(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);
    let result = state.db.collection::<Lead>("leads").delete_one(filter).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
