use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Customer {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    // Staff-editable labels (VIP, High Value, etc.) shown as chips on the
    // customer profile drawer — never computed/derived, so staff can apply
    // their own judgment rather than the backend guessing "VIP" from premium.
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_by: ObjectId,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCustomerInput {
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCustomerInput {
    pub name: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub tags: Option<Vec<String>>,
}

// bson::oid::ObjectId serializes to `{"$oid": "..."}` extended JSON, not a plain
// string, so API responses go through this DTO instead of serializing Customer
// directly (same reasoning as User -> UserPublic).
#[derive(Debug, Serialize)]
pub struct CustomerResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Customer> for CustomerResponse {
    fn from(c: Customer) -> Self {
        CustomerResponse {
            id: c.id.map(|i| i.to_hex()).unwrap_or_default(),
            name: c.name,
            phone: c.phone,
            email: c.email,
            address: c.address,
            notes: c.notes,
            tags: c.tags,
            created_by: c.created_by.to_hex(),
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}
