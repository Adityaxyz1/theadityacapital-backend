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
    models::{Customer, CustomerResponse, CreateCustomerInput, UpdateCustomerInput},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct ListCustomersQuery {
    pub q: Option<String>,
}

pub async fn list_customers(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<ListCustomersQuery>,
) -> ApiResult<Json<Vec<CustomerResponse>>> {
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

    let cursor = collection.find(filter).await?;
    let customers: Vec<Customer> = cursor.try_collect().await?;
    Ok(Json(customers.into_iter().map(Into::into).collect()))
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
        created_by: auth.user_id,
        created_at: now,
        updated_at: now,
    };

    let result = collection.insert_one(&customer).await?;
    let mut created = customer;
    created.id = result.inserted_id.as_object_id();
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
