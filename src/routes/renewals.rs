use axum::{
    extract::{Path, Query, State},
    Json,
};
use bson::doc;
use chrono::{NaiveDate, TimeZone, Utc};

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    listview::{paginate, parse_sort, PageParams, Paginated},
    models::{ListRenewalsQuery, Policy, PolicyStatus, Renewal, RenewalResponse, RenewalStatus, UpdateRenewalStatusInput},
    notify::broadcast_renewal_updated,
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

// Mirrors reports.rs's day_bounds so the Reports page drill-down (which lists
// renewals for the same date_from/date_to) matches the report totals above it.
fn day_bounds(from: NaiveDate, to: NaiveDate) -> ApiResult<(chrono::DateTime<Utc>, chrono::DateTime<Utc>)> {
    if to < from {
        return Err(ApiError::BadRequest("date_to must be on or after date_from".into()));
    }
    let start = Utc.from_utc_datetime(&from.and_hms_opt(0, 0, 0).unwrap());
    let end_exclusive = Utc.from_utc_datetime(&(to + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap());
    Ok((start, end_exclusive))
}

const SORTABLE_FIELDS: &[&str] = &["due_date", "premium_due", "status", "created_at"];

// Drag-drop on the Kanban board and any future workflow "set status" action
// both call this handler, so this is the one place a transition is allowed
// or rejected — terminal states (Renewed/Lapsed/Lost) can move back to
// Pending (re-opening a renewal is a legitimate correction), but nothing can
// jump between two different terminal states without passing through an
// active one first.
fn is_valid_transition(from: RenewalStatus, to: RenewalStatus) -> bool {
    use RenewalStatus::*;
    if from == to {
        return true;
    }
    match from {
        Pending => matches!(to, Contacted | Renewed | Lapsed | Lost),
        Contacted => matches!(to, Pending | Renewed | Lapsed | Lost),
        Renewed | Lapsed | Lost => matches!(to, Pending),
    }
}

pub async fn list_renewals(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ListRenewalsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Paginated<RenewalResponse>>> {
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
    if let Some(customer_id) = query.customer_id {
        filter.insert("customer_id", parse_oid(&customer_id)?);
    }
    if let (Some(date_from), Some(date_to)) = (query.date_from, query.date_to) {
        let (start, end) = day_bounds(date_from, date_to)?;
        filter.insert("due_date", doc! { "$gte": start, "$lt": end });
    } else if let Some(days) = query.due_within_days {
        let cutoff = Utc::now() + chrono::Duration::days(days);
        filter.insert("due_date", doc! { "$lte": cutoff });
    }
    let filter = combine_filters(filter, visibility_filter(&state.db, &auth).await?);
    let sort = parse_sort(&page.sort, SORTABLE_FIELDS, "due_date")?;

    Ok(Json(paginate(&collection, filter, sort, &page).await?))
}

pub async fn update_renewal_status(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<UpdateRenewalStatusInput>,
) -> ApiResult<Json<RenewalResponse>> {
    let oid = parse_oid(&id)?;
    let scope = visibility_filter(&state.db, &auth).await?;
    let renewals = state.db.collection::<Renewal>("renewals");

    let current = renewals
        .find_one(combine_filters(doc! { "_id": oid }, scope.clone()))
        .await?
        .ok_or(ApiError::NotFound)?;
    if !is_valid_transition(current.status, input.status) {
        return Err(ApiError::BadRequest(format!(
            "cannot move a renewal from {:?} to {:?}",
            current.status, input.status
        )));
    }

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
        .find_one_and_update(combine_filters(doc! { "_id": oid }, scope), doc! { "$set": set_doc })
        .return_document(mongodb::options::ReturnDocument::After)
        .await?
        .ok_or(ApiError::NotFound)?;

    // Renewed/Lapsed/Lost are terminal outcomes for the underlying policy too
    // (PolicyStatus mirrors them); Pending/Contacted are renewal-pipeline-only
    // states that don't change the policy's own status.
    let policy_status = match input.status {
        RenewalStatus::Renewed => PolicyStatus::Renewed,
        RenewalStatus::Lapsed => PolicyStatus::Lapsed,
        RenewalStatus::Lost => PolicyStatus::Lost,
        RenewalStatus::Pending | RenewalStatus::Contacted => PolicyStatus::Active,
    };
    state
        .db
        .collection::<Policy>("policies")
        .update_one(
            doc! { "_id": renewal.policy_id },
            doc! { "$set": { "status": bson::to_bson(&policy_status).map_err(|e| ApiError::Internal(e.into()))?, "updated_at": Utc::now() } },
        )
        .await?;

    if current.status != input.status {
        if let Ok(renewal_doc) = bson::to_document(&renewal) {
            crate::workflow::on_field_changed(
                &state,
                crate::models::EntityType::Renewal,
                oid,
                "status",
                &bson::to_bson(&input.status).map_err(|e| ApiError::Internal(e.into()))?,
                &renewal_doc,
            )
            .await;
        }
    }

    if current.status != input.status {
        crate::activity::log(
            &state,
            crate::activity::NewActivity {
                user_id: auth.user_id,
                action: format!(
                    "moved {}'s {} renewal to {:?}",
                    renewal.customer_name, renewal.policy_type, input.status
                ),
                customer_id: Some(renewal.customer_id),
                customer_name: Some(renewal.customer_name.clone()),
                policy_type: Some(renewal.policy_type.clone()),
                icon_type: crate::models::ActivityIcon::Policy,
                assigned_to: Some(renewal.assigned_to),
            },
        )
        .await;
    }

    let response = RenewalResponse::from(renewal);
    broadcast_renewal_updated(&state, &response).await;
    Ok(Json(response))
}
