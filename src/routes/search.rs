use axum::{
    extract::{Query, State},
    Json,
};
use bson::doc;
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthUser,
    error::ApiResult,
    models::{Customer, Policy, Renewal},
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub object: &'static str,
    pub id: String,
    pub label: String,
    pub sublabel: String,
    // Neither policies nor renewals have their own detail page yet — the
    // frontend routes into the owning customer's page instead, so it needs
    // this even for non-customer results.
    pub customer_id: String,
}

const RESULTS_PER_OBJECT: i64 = 5;

// A single header search box across the whole app, fanning out to a
// small regex match per object (fine at this agency's data volume — no
// need for Atlas Search/a dedicated index). Policies/renewals are scoped by
// the same visibility rule as their list endpoints; customers stay unscoped
// (shared address book, see visibility.rs).
pub async fn search(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Json<Vec<SearchResult>>> {
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(Json(vec![]));
    }
    let regex = doc! { "$regex": q, "$options": "i" };
    let scope = visibility_filter(&state.db, &auth).await?;
    let mut results = Vec::new();

    let customers = state.db.collection::<Customer>("customers");
    let filter = doc! { "$or": [
        { "name": regex.clone() },
        { "phone": regex.clone() },
        { "email": regex.clone() },
    ] };
    let mut cursor = customers.find(filter).limit(RESULTS_PER_OBJECT).await?;
    while let Some(c) = cursor.try_next().await? {
        results.push(SearchResult {
            object: "customer",
            id: c.id.map(|i| i.to_hex()).unwrap_or_default(),
            customer_id: c.id.map(|i| i.to_hex()).unwrap_or_default(),
            label: c.name,
            sublabel: c.phone.or(c.email).unwrap_or_default(),
        });
    }

    let policies = state.db.collection::<Policy>("policies");
    let filter = combine_filters(
        doc! { "$or": [{ "policy_number": regex.clone() }, { "insurer_name": regex.clone() }] },
        scope.clone(),
    );
    let mut cursor = policies.find(filter).limit(RESULTS_PER_OBJECT).await?;
    while let Some(p) = cursor.try_next().await? {
        results.push(SearchResult {
            object: "policy",
            id: p.id.map(|i| i.to_hex()).unwrap_or_default(),
            customer_id: p.customer_id.to_hex(),
            label: p.policy_number,
            sublabel: format!("{} — {}", p.insurer_name, p.policy_type),
        });
    }

    let renewals = state.db.collection::<Renewal>("renewals");
    let filter = combine_filters(doc! { "customer_name": regex.clone() }, scope);
    let mut cursor = renewals.find(filter).limit(RESULTS_PER_OBJECT).await?;
    while let Some(r) = cursor.try_next().await? {
        results.push(SearchResult {
            object: "renewal",
            id: r.id.map(|i| i.to_hex()).unwrap_or_default(),
            customer_id: r.customer_id.to_hex(),
            label: format!("{} — {}", r.customer_name, r.policy_type),
            sublabel: format!("{} due {}", r.insurer_name, r.due_date.format("%d %b %Y")),
        });
    }

    Ok(Json(results))
}
