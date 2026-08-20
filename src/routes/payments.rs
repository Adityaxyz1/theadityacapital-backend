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
    models::{ActivityIcon, CreatePaymentInput, Payment, PaymentResponse, PaymentStatus, Policy, UpdatePaymentStatusInput},
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

const SORTABLE_FIELDS: &[&str] = &["due_date", "amount", "created_at"];

#[derive(Debug, Deserialize)]
pub struct ListPaymentsQuery {
    pub status: Option<PaymentStatus>,
    pub customer_id: Option<String>,
    pub policy_type: Option<String>,
    // Keyword-bucketed Life/Health/Motor/Other, distinct from the exact-match
    // `policy_type` above — see categorize.rs.
    pub category: Option<String>,
}

pub async fn list_payments(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListPaymentsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Paginated<PaymentResponse>>> {
    let collection = state.db.collection::<Payment>("payments");

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
    let sort = parse_sort(&page.sort, SORTABLE_FIELDS, "due_date")?;

    Ok(Json(paginate(&collection, filter, sort, &page).await?))
}

pub async fn create_payment(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreatePaymentInput>,
) -> ApiResult<Json<PaymentResponse>> {
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

    let status = input.status.unwrap_or(PaymentStatus::Pending);
    let payment = Payment {
        id: None,
        policy_id,
        customer_id: policy.customer_id,
        customer_name: customer.name.clone(),
        policy_type: policy.policy_type,
        receipt_number: input.receipt_number,
        amount: input.amount,
        status,
        due_date: input.due_date,
        paid_at: if status == PaymentStatus::Collected { Some(Utc::now()) } else { None },
        payment_method: input.payment_method,
        assigned_to: policy.assigned_to,
        created_at: Utc::now(),
    };

    let collection = state.db.collection::<Payment>("payments");
    let result = collection.insert_one(&payment).await?;
    let mut created = payment;
    created.id = result.inserted_id.as_object_id();

    activity::log(
        &state,
        NewActivity {
            user_id: auth.user_id,
            action: format!("logged payment receipt {} for {}", created.receipt_number, customer.name),
            customer_id: Some(policy.customer_id),
            customer_name: Some(customer.name),
            policy_type: Some(created.policy_type.clone()),
            icon_type: ActivityIcon::Payment,
            assigned_to: Some(created.assigned_to),
        },
    )
    .await;

    Ok(Json(created.into()))
}

pub async fn get_payment(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<PaymentResponse>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);
    let payment = state
        .db
        .collection::<Payment>("payments")
        .find_one(filter)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(payment.into()))
}

pub async fn update_payment_status(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdatePaymentStatusInput>,
) -> ApiResult<Json<PaymentResponse>> {
    let oid = parse_oid(&id)?;
    let scope = visibility_filter(&state.db, &auth).await?;
    let collection = state.db.collection::<Payment>("payments");

    let mut set_doc = doc! {
        "status": bson::to_bson(&input.status).map_err(|e| ApiError::Internal(e.into()))?,
    };
    if input.status == PaymentStatus::Collected {
        set_doc.insert("paid_at", Utc::now());
    }

    let payment = collection
        .find_one_and_update(combine_filters(doc! { "_id": oid }, scope), doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(payment.into()))
}

pub async fn delete_payment(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = parse_oid(&id)?;
    let filter = combine_filters(doc! { "_id": oid }, visibility_filter(&state.db, &auth).await?);
    let result = state.db.collection::<Payment>("payments").delete_one(filter).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
