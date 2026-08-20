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
    models::{Customer, CustomerResponse, CreateCustomerInput, UpdateCustomerInput},
    state::AppState,
};

const SORTABLE_FIELDS: &[&str] = &["name", "created_at", "updated_at"];

#[derive(Debug, Deserialize)]
pub struct ListCustomersQuery {
    pub q: Option<String>,
}

// Customers are intentionally left unscoped by `visibility::visibility_filter`
// (see visibility.rs) — they're a shared address book, not an owned record
// like policies/renewals.
//
// Pagination/sort params are a *separate* `Query<PageParams>` extractor
// rather than `#[serde(flatten)]`-ed into `ListCustomersQuery`: axum's Query
// extractor (serde_urlencoded) has a known bug where flattening breaks
// non-string field types ("invalid type: string \"1\", expected u64"), since
// flatten requires a self-describing deserializer that urlencoded isn't.
// Two independent Query extractors both just re-read the same query string.
pub async fn list_customers(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<ListCustomersQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Paginated<CustomerResponse>>> {
    let collection = state.db.collection::<Customer>("customers");

    let filter = match query.q {
        Some(q) if !q.trim().is_empty() => doc! {
            "$or": [
                { "name": { "$regex": &q, "$options": "i" } },
                { "phone": { "$regex": &q, "$options": "i" } },
                { "email": { "$regex": &q, "$options": "i" } },
            ]
        },
        _ => doc! {},
    };
    let sort = parse_sort(&page.sort, SORTABLE_FIELDS, "name")?;

    Ok(Json(paginate(&collection, filter, sort, &page).await?))
}

pub async fn create_customer(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(input): Json<CreateCustomerInput>,
) -> ApiResult<Json<CustomerResponse>> {
    let collection = state.db.collection::<Customer>("customers");
    let now = Utc::now();

    let customer = Customer {
        id: None,
        name: input.name,
        phone: input.phone,
        email: input.email,
        address: input.address,
        notes: input.notes,
        tags: input.tags,
        created_by: auth.user_id,
        created_at: now,
        updated_at: now,
    };

    let result = collection.insert_one(&customer).await?;
    let mut created = customer;
    created.id = result.inserted_id.as_object_id();

    if let (Some(id), Ok(doc)) = (created.id, bson::to_document(&created)) {
        crate::workflow::on_record_created(&state, crate::models::EntityType::Customer, id, &doc).await;
    }

    crate::activity::log(
        &state,
        crate::activity::NewActivity {
            user_id: auth.user_id,
            action: format!("added a new customer record for {}", created.name),
            customer_id: created.id,
            customer_name: Some(created.name.clone()),
            policy_type: None,
            icon_type: crate::models::ActivityIcon::Lead,
            assigned_to: Some(auth.user_id),
        },
    )
    .await;

    Ok(Json(created.into()))
}

pub async fn get_customer(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<CustomerResponse>> {
    let oid = bson::oid::ObjectId::parse_str(&id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let collection = state.db.collection::<Customer>("customers");
    let customer = collection
        .find_one(doc! { "_id": oid })
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(customer.into()))
}

pub async fn update_customer(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateCustomerInput>,
) -> ApiResult<Json<CustomerResponse>> {
    let oid = bson::oid::ObjectId::parse_str(&id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let collection = state.db.collection::<Customer>("customers");

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
    if let Some(address) = input.address {
        set_doc.insert("address", address);
    }
    if let Some(notes) = input.notes {
        set_doc.insert("notes", notes);
    }
    if let Some(tags) = input.tags {
        set_doc.insert("tags", tags);
    }

    let customer = collection
        .find_one_and_update(doc! { "_id": oid }, doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(customer.into()))
}

pub async fn delete_customer(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = bson::oid::ObjectId::parse_str(&id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let collection = state.db.collection::<Customer>("customers");
    let result = collection.delete_one(doc! { "_id": oid }).await?;
    if result.deleted_count == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
