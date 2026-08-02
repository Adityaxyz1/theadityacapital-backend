use std::collections::BTreeMap;

use axum::{extract::{Query, State}, Json};
use bson::doc;
use chrono::{NaiveDate, TimeZone, Utc};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    models::{Renewal, RenewalResponse},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct MonthlyQuery {
    // "YYYY-MM"
    pub month: String,
}

#[derive(Debug, Serialize)]
pub struct MonthlyDashboard {
    pub month: String,
    pub counts_by_day: BTreeMap<String, u32>,
    pub renewals: Vec<RenewalResponse>,
}

pub async fn monthly(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<MonthlyQuery>,
) -> ApiResult<Json<MonthlyDashboard>> {
    let mut parts = query.month.splitn(2, '-');
    let year: i32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or(ApiError::BadRequest("month must be YYYY-MM".into()))?;
    let month: u32 = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or(ApiError::BadRequest("month must be YYYY-MM".into()))?;

    let start_naive = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or(ApiError::BadRequest("invalid month".into()))?;
    let end_naive = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .expect("computed next month is always valid");

    let start = Utc.from_utc_datetime(&start_naive.and_hms_opt(0, 0, 0).unwrap());
    let end = Utc.from_utc_datetime(&end_naive.and_hms_opt(0, 0, 0).unwrap());

    let collection = state.db.collection::<Renewal>("renewals");
    let cursor = collection
        .find(doc! { "due_date": { "$gte": start, "$lt": end } })
        .await?;
    let renewals: Vec<Renewal> = cursor.try_collect().await?;

    let mut counts_by_day: BTreeMap<String, u32> = BTreeMap::new();
    for renewal in &renewals {
        let key = renewal.due_date.format("%Y-%m-%d").to_string();
        *counts_by_day.entry(key).or_insert(0) += 1;
    }

    Ok(Json(MonthlyDashboard {
        month: query.month,
        counts_by_day,
        renewals: renewals.into_iter().map(Into::into).collect(),
    }))
}
