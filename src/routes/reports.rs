use std::collections::BTreeMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use bson::{doc, Document};
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc};
use futures_util::TryStreamExt;
use serde::Deserialize;

use crate::{
    auth::AuthUser,
    categorize::classify,
    error::{ApiError, ApiResult},
    models::{
        Policy, PremiumTrendPoint, PremiumTrendResult, ReportCatalogEntry, ReportKey,
        ReportResult, ReportRow, Renewal, RenewalStatus, TopPerformerRow, User,
        YearlyComparisonResult, YearlyComparisonRow,
    },
    state::AppState,
    visibility::{combine_filters, visibility_filter},
};

fn parse_report_key(s: &str) -> ApiResult<ReportKey> {
    match s {
        "renewal_pipeline" => Ok(ReportKey::RenewalPipeline),
        "premium_by_insurer" => Ok(ReportKey::PremiumByInsurer),
        "new_business_by_type" => Ok(ReportKey::NewBusinessByType),
        _ => Err(ApiError::BadRequest("unknown report".into())),
    }
}

// Curated, not user-authored (PRD Phase 5) — the catalog just describes the
// fixed set of report templates below so the frontend can render a picker
// without duplicating names/descriptions.
pub async fn catalog() -> Json<Vec<ReportCatalogEntry>> {
    Json(vec![
        ReportCatalogEntry {
            key: ReportKey::RenewalPipeline,
            name: "Renewal pipeline".into(),
            description: "Renewals due in the selected window, broken down by status.".into(),
        },
        ReportCatalogEntry {
            key: ReportKey::PremiumByInsurer,
            name: "Premium by insurer".into(),
            description: "Premium due in the selected window, by insurer.".into(),
        },
        ReportCatalogEntry {
            key: ReportKey::NewBusinessByType,
            name: "New business by type".into(),
            description: "New policies sold in the selected window, by policy type.".into(),
        },
    ])
}

#[derive(Debug, Deserialize)]
pub struct ReportQuery {
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub insurer_name: Option<String>,
    pub policy_type: Option<String>,
}

fn day_bounds(from: NaiveDate, to: NaiveDate) -> ApiResult<(chrono::DateTime<Utc>, chrono::DateTime<Utc>)> {
    if to < from {
        return Err(ApiError::BadRequest("date_to must be on or after date_from".into()));
    }
    let start = Utc.from_utc_datetime(&from.and_hms_opt(0, 0, 0).unwrap());
    let end_exclusive = Utc.from_utc_datetime(&(to + Duration::days(1)).and_hms_opt(0, 0, 0).unwrap());
    Ok((start, end_exclusive))
}

fn status_key(s: RenewalStatus) -> &'static str {
    match s {
        RenewalStatus::Pending => "pending",
        RenewalStatus::Contacted => "contacted",
        RenewalStatus::Renewed => "renewed",
        RenewalStatus::Lapsed => "lapsed",
        RenewalStatus::Lost => "lost",
    }
}

pub async fn run(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(report_key): Path<String>,
    Query(query): Query<ReportQuery>,
) -> ApiResult<Json<ReportResult>> {
    let key = parse_report_key(&report_key)?;
    let (start, end) = day_bounds(query.date_from, query.date_to)?;
    let scope = visibility_filter(&state.db, &auth).await?;

    let rows = match key {
        ReportKey::RenewalPipeline => renewal_pipeline(&state, &scope, start, end, &query).await?,
        ReportKey::PremiumByInsurer => premium_by_insurer(&state, &scope, start, end, &query).await?,
        ReportKey::NewBusinessByType => new_business_by_type(&state, &scope, start, end, &query).await?,
    };

    Ok(Json(ReportResult { key, rows }))
}

// Fixed pipeline order carries the funnel's own sequence meaning, so rows are
// emitted in that order rather than sorted by value (see dataviz guidance:
// this is a status breakdown, not a magnitude ranking).
async fn renewal_pipeline(
    state: &AppState,
    scope: &Document,
    start: chrono::DateTime<Utc>,
    end: chrono::DateTime<Utc>,
    query: &ReportQuery,
) -> ApiResult<Vec<ReportRow>> {
    let mut filter = doc! { "due_date": { "$gte": start, "$lt": end } };
    if let Some(insurer) = &query.insurer_name {
        filter.insert("insurer_name", insurer.clone());
    }
    if let Some(policy_type) = &query.policy_type {
        filter.insert("policy_type", policy_type.clone());
    }
    let filter = combine_filters(filter, scope.clone());

    let cursor = state.db.collection::<Renewal>("renewals").find(filter).await?;
    let renewals: Vec<Renewal> = cursor.try_collect().await?;

    const ORDER: [RenewalStatus; 5] = [
        RenewalStatus::Pending,
        RenewalStatus::Contacted,
        RenewalStatus::Renewed,
        RenewalStatus::Lapsed,
        RenewalStatus::Lost,
    ];
    let mut totals = [(0u32, 0.0f64); 5];
    for r in &renewals {
        if let Some(idx) = ORDER.iter().position(|s| *s == r.status) {
            totals[idx].0 += 1;
            totals[idx].1 += r.premium_due;
        }
    }

    Ok(ORDER
        .iter()
        .zip(totals.iter())
        .map(|(status, (count, value))| ReportRow {
            label: status_key(*status).to_string(),
            count: *count,
            value: *value,
        })
        .collect())
}

async fn premium_by_insurer(
    state: &AppState,
    scope: &Document,
    start: chrono::DateTime<Utc>,
    end: chrono::DateTime<Utc>,
    query: &ReportQuery,
) -> ApiResult<Vec<ReportRow>> {
    let mut filter = doc! { "due_date": { "$gte": start, "$lt": end } };
    if let Some(policy_type) = &query.policy_type {
        filter.insert("policy_type", policy_type.clone());
    }
    let filter = combine_filters(filter, scope.clone());

    let cursor = state.db.collection::<Renewal>("renewals").find(filter).await?;
    let renewals: Vec<Renewal> = cursor.try_collect().await?;

    let mut agg: BTreeMap<String, (u32, f64)> = BTreeMap::new();
    for r in &renewals {
        let entry = agg.entry(r.insurer_name.clone()).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += r.premium_due;
    }

    let mut rows: Vec<ReportRow> = agg
        .into_iter()
        .map(|(label, (count, value))| ReportRow { label, count, value })
        .collect();
    rows.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    Ok(rows)
}

// Mirrors dashboard.rs's own new-business definition (a policy with no
// previous_policy_id, created in the window) so this report and the
// dashboard's "New business this month" tile never disagree on what counts.
async fn new_business_by_type(
    state: &AppState,
    scope: &Document,
    start: chrono::DateTime<Utc>,
    end: chrono::DateTime<Utc>,
    query: &ReportQuery,
) -> ApiResult<Vec<ReportRow>> {
    let mut filter = doc! {
        "previous_policy_id": null,
        "created_at": { "$gte": start, "$lt": end },
    };
    if let Some(insurer) = &query.insurer_name {
        filter.insert("insurer_name", insurer.clone());
    }
    let filter = combine_filters(filter, scope.clone());

    let cursor = state.db.collection::<Policy>("policies").find(filter).await?;
    let policies: Vec<Policy> = cursor.try_collect().await?;

    let mut agg: BTreeMap<String, (u32, f64)> = BTreeMap::new();
    for p in &policies {
        let entry = agg.entry(p.policy_type.clone()).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += p.current_year_premium;
    }

    let mut rows: Vec<ReportRow> = agg
        .into_iter()
        .map(|(label, (count, value))| ReportRow { label, count, value })
        .collect();
    rows.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(rows)
}

#[derive(Debug, Deserialize)]
pub struct YearlyComparisonQuery {
    pub year: i32,
    pub group_by: String,
}

// Uses each policy's own previous_year_premium/current_year_premium (already
// captured at extraction/renewal time, see ARCHITECTURE.md §4) rather than
// comparing two separate date ranges — the same fields the customer detail
// page's policy form and the AI extraction pipeline both already populate.
pub async fn yearly_comparison(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<YearlyComparisonQuery>,
) -> ApiResult<Json<YearlyComparisonResult>> {
    if query.group_by != "policy_type" && query.group_by != "insurer_name" {
        return Err(ApiError::BadRequest("group_by must be 'policy_type' or 'insurer_name'".into()));
    }

    let start = Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(query.year, 1, 1)
            .ok_or(ApiError::BadRequest("invalid year".into()))?
            .and_hms_opt(0, 0, 0)
            .unwrap(),
    );
    let end = Utc.from_utc_datetime(
        &NaiveDate::from_ymd_opt(query.year + 1, 1, 1)
            .ok_or(ApiError::BadRequest("invalid year".into()))?
            .and_hms_opt(0, 0, 0)
            .unwrap(),
    );

    let filter = combine_filters(
        doc! { "end_date": { "$gte": start, "$lt": end } },
        visibility_filter(&state.db, &auth).await?,
    );

    let cursor = state.db.collection::<Policy>("policies").find(filter).await?;
    let policies: Vec<Policy> = cursor.try_collect().await?;

    let mut agg: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for p in &policies {
        let key = if query.group_by == "policy_type" { &p.policy_type } else { &p.insurer_name };
        let entry = agg.entry(key.clone()).or_insert((0.0, 0.0));
        entry.0 += p.previous_year_premium.unwrap_or(0.0);
        entry.1 += p.current_year_premium;
    }

    let mut rows: Vec<YearlyComparisonRow> = agg
        .into_iter()
        .map(|(label, (previous_value, current_value))| YearlyComparisonRow {
            label,
            previous_value,
            current_value,
        })
        .collect();
    rows.sort_by(|a, b| b.current_value.partial_cmp(&a.current_value).unwrap_or(std::cmp::Ordering::Equal));

    Ok(Json(YearlyComparisonResult { year: query.year, rows }))
}

// All-time, in-process aggregation (mirrors premium_by_insurer's approach)
// rather than a Mongo aggregation pipeline — the policies/renewals collection
// sizes here don't warrant one, and this keeps the grouping logic in one
// place readers can follow without knowing Mongo's pipeline stage syntax.
pub async fn top_performers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<TopPerformerRow>>> {
    let scope = visibility_filter(&state.db, &auth).await?;

    let policies: Vec<Policy> = state
        .db
        .collection::<Policy>("policies")
        .find(scope.clone())
        .await?
        .try_collect()
        .await?;
    let renewals: Vec<Renewal> = state
        .db
        .collection::<Renewal>("renewals")
        .find(scope)
        .await?
        .try_collect()
        .await?;

    #[derive(Default)]
    struct Agg {
        policies_sold: u32,
        premium_generated: f64,
        renewed: u32,
        lost: u32,
    }

    let mut agg: BTreeMap<bson::oid::ObjectId, Agg> = BTreeMap::new();
    for p in &policies {
        let entry = agg.entry(p.assigned_to).or_default();
        entry.policies_sold += 1;
        entry.premium_generated += p.current_year_premium;
    }
    for r in &renewals {
        match r.status {
            RenewalStatus::Renewed => agg.entry(r.assigned_to).or_default().renewed += 1,
            RenewalStatus::Lost => agg.entry(r.assigned_to).or_default().lost += 1,
            _ => {}
        }
    }

    let user_ids: Vec<bson::oid::ObjectId> = agg.keys().copied().collect();
    let users: Vec<User> = state
        .db
        .collection::<User>("users")
        .find(doc! { "_id": { "$in": &user_ids } })
        .await?
        .try_collect()
        .await?;
    let names: std::collections::HashMap<bson::oid::ObjectId, String> =
        users.into_iter().filter_map(|u| u.id.map(|id| (id, u.name))).collect();

    let mut rows: Vec<TopPerformerRow> = agg
        .into_iter()
        .map(|(user_id, a)| {
            let total = a.renewed + a.lost;
            let renewal_rate = if total > 0 { (a.renewed as f64 / total as f64) * 100.0 } else { 0.0 };
            TopPerformerRow {
                user_id: user_id.to_hex(),
                user_name: names.get(&user_id).cloned().unwrap_or_else(|| "Unknown".into()),
                policies_sold: a.policies_sold,
                premium_generated: a.premium_generated,
                renewal_rate: (renewal_rate * 10.0).round() / 10.0,
            }
        })
        .collect();
    rows.sort_by(|a, b| b.premium_generated.partial_cmp(&a.premium_generated).unwrap_or(std::cmp::Ordering::Equal));
    rows.truncate(10);

    Ok(Json(rows))
}

// Backs the dashboard's "Premium by Type" donut. All-time book of business
// (no date filter, mirrors top_performers' definition of "premium" so the
// two widgets never disagree), grouped into the same Life/Health/Motor/Other
// buckets as the dashboard's category pills rather than raw free-text
// policy_type values, which would otherwise fragment into dozens of near-
// duplicate labels ("health", "Health Insurance", "Family Floater Health"...).
pub async fn premium_by_type(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<ReportRow>>> {
    let scope = visibility_filter(&state.db, &auth).await?;
    let policies: Vec<Policy> = state
        .db
        .collection::<Policy>("policies")
        .find(scope)
        .await?
        .try_collect()
        .await?;

    let mut agg: BTreeMap<&'static str, (u32, f64)> = BTreeMap::new();
    for p in &policies {
        let entry = agg.entry(classify(&p.policy_type)).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += p.current_year_premium;
    }

    let mut rows: Vec<ReportRow> = agg
        .into_iter()
        .map(|(label, (count, value))| ReportRow { label: label.to_string(), count, value })
        .collect();
    rows.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    Ok(Json(rows))
}

fn add_months(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let total = year * 12 + (month as i32 - 1) + delta;
    let y = total.div_euclid(12);
    let m = total.rem_euclid(12) + 1;
    (y, m as u32)
}

// Oldest-to-newest bucket boundaries for the requested granularity, each
// ending at (or containing) "now" — calendar-aligned for month/year so
// labels read naturally ("Aug 2026"), rolling for day/week since those have
// no natural single alignment point worth enforcing.
fn trend_periods(
    granularity: &str,
    now: chrono::DateTime<Utc>,
) -> ApiResult<Vec<(chrono::DateTime<Utc>, chrono::DateTime<Utc>, String)>> {
    const MONTH_ERR: &str = "add_months always returns a month in 1..=12, which from_ymd_opt always accepts";
    let today = now.date_naive();
    let mut out = Vec::new();

    match granularity {
        "daily" => {
            for i in (0..14).rev() {
                let day = today - Duration::days(i);
                let start = Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap());
                let end = start + Duration::days(1);
                out.push((start, end, day.format("%b %d").to_string()));
            }
        }
        "weekly" => {
            let weekday = today.weekday().num_days_from_monday() as i64;
            let this_week_start = today - Duration::days(weekday);
            for i in (0..8).rev() {
                let week_start = this_week_start - Duration::days(7 * i);
                let start = Utc.from_utc_datetime(&week_start.and_hms_opt(0, 0, 0).unwrap());
                let end = start + Duration::days(7);
                out.push((start, end, format!("Wk of {}", week_start.format("%b %d"))));
            }
        }
        "monthly" => {
            for i in (0..6).rev() {
                let (y, m) = add_months(today.year(), today.month(), -i);
                let start_naive = NaiveDate::from_ymd_opt(y, m, 1).expect(MONTH_ERR);
                let (ny, nm) = add_months(y, m, 1);
                let end_naive = NaiveDate::from_ymd_opt(ny, nm, 1).expect(MONTH_ERR);
                let start = Utc.from_utc_datetime(&start_naive.and_hms_opt(0, 0, 0).unwrap());
                let end = Utc.from_utc_datetime(&end_naive.and_hms_opt(0, 0, 0).unwrap());
                out.push((start, end, start_naive.format("%b %Y").to_string()));
            }
        }
        "yearly" => {
            for i in (0..4).rev() {
                let y = today.year() - i;
                let start_naive = NaiveDate::from_ymd_opt(y, 1, 1).expect("Jan 1 of any i32 year is always valid");
                let end_naive = NaiveDate::from_ymd_opt(y + 1, 1, 1).expect("Jan 1 of any i32 year is always valid");
                let start = Utc.from_utc_datetime(&start_naive.and_hms_opt(0, 0, 0).unwrap());
                let end = Utc.from_utc_datetime(&end_naive.and_hms_opt(0, 0, 0).unwrap());
                out.push((start, end, y.to_string()));
            }
        }
        _ => return Err(ApiError::BadRequest("granularity must be daily, weekly, monthly, or yearly".into())),
    }

    Ok(out)
}

#[derive(Debug, Deserialize)]
pub struct PremiumTrendQuery {
    pub granularity: String,
}

// Backs the dashboard's "Premium Growth Trend" chart — real renewals.premium_due
// summed per bucket (same metric MonthlyDashboard.total_premium_due already
// uses), scoped by visibility so an agent's trend is their own book, not the
// whole company's.
pub async fn premium_trend(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<PremiumTrendQuery>,
) -> ApiResult<Json<PremiumTrendResult>> {
    let periods = trend_periods(&query.granularity, Utc::now())?;
    let earliest = periods.first().expect("trend_periods always returns at least one period").0;

    let filter = combine_filters(
        doc! { "due_date": { "$gte": earliest } },
        visibility_filter(&state.db, &auth).await?,
    );
    let cursor = state.db.collection::<Renewal>("renewals").find(filter).await?;
    let renewals: Vec<Renewal> = cursor.try_collect().await?;

    let points: Vec<PremiumTrendPoint> = periods
        .iter()
        .map(|(start, end, label)| {
            let value: f64 = renewals
                .iter()
                .filter(|r| r.due_date >= *start && r.due_date < *end)
                .map(|r| r.premium_due)
                .sum();
            // f64::sum() over an empty/all-cancelling iterator can yield -0.0,
            // which serde_json faithfully serializes as "-0.0" — a nonsense
            // "negative premium" from the API consumer's point of view.
            // Adding 0.0 normalizes -0.0 to +0.0 without altering any other value.
            PremiumTrendPoint { label: label.clone(), value: value + 0.0 }
        })
        .collect();

    Ok(Json(PremiumTrendResult { granularity: query.granularity, points }))
}
