use serde::{Deserialize, Serialize};

// A fixed, code-defined catalog rather than a user-authored query builder —
// same "pragmatic middle ground, not a full metadata framework" stance as
// workflow_rule.rs/saved_view.rs. Adding a report means adding a variant here
// plus a match arm in routes/reports.rs, not a schema staff can edit.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportKey {
    RenewalPipeline,
    PremiumByInsurer,
    NewBusinessByType,
}

#[derive(Debug, Serialize)]
pub struct ReportCatalogEntry {
    pub key: ReportKey,
    pub name: String,
    pub description: String,
}

// One row per category (a renewal status, an insurer, a policy type) — kept
// deliberately generic so every report template renders through the same
// frontend chart component instead of a bespoke shape per report.
#[derive(Debug, Serialize, Clone)]
pub struct ReportRow {
    pub label: String,
    pub count: u32,
    pub value: f64,
}

#[derive(Debug, Serialize)]
pub struct ReportResult {
    pub key: ReportKey,
    pub rows: Vec<ReportRow>,
}

// Year-over-year premium comparison (PRD 4.7) — a distinct shape from
// ReportRow because the job here is "before vs after per category", not a
// single magnitude, so it renders as a dumbbell, not a bar. Kept out of the
// pinnable-widget system (routes/dashboard_widgets.rs) for now: its params
// (year + group_by) don't fit WidgetParams' date-range shape, and PRD 4.7
// doesn't ask for it to be pinned, just viewed.
#[derive(Debug, Serialize, Clone)]
pub struct YearlyComparisonRow {
    pub label: String,
    pub previous_value: f64,
    pub current_value: f64,
}

#[derive(Debug, Serialize)]
pub struct YearlyComparisonResult {
    pub year: i32,
    pub rows: Vec<YearlyComparisonRow>,
}

// Backs the dashboard's "Top Performers" leaderboard widget. Computed
// in-process from policies/renewals already scoped by visibility_filter
// (an agent's "leaderboard" is just themselves; a manager's is their team;
// an admin's is everyone) rather than a stored/cached collection, since the
// underlying data changes on every policy/renewal write.
#[derive(Debug, Serialize, Clone)]
pub struct TopPerformerRow {
    pub user_id: String,
    pub user_name: String,
    pub policies_sold: u32,
    pub premium_generated: f64,
    pub renewal_rate: f64,
}

// Backs the dashboard's "Premium Growth Trend" chart. One point per bucket
// (day/week/month/year depending on the requested granularity), oldest
// first — the frontend takes the last two points as "current" vs "previous"
// rather than the backend baking in that framing, so any number of points
// (not just two) can be plotted as a real trend line.
#[derive(Debug, Serialize, Clone)]
pub struct PremiumTrendPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Serialize)]
pub struct PremiumTrendResult {
    pub granularity: String,
    pub points: Vec<PremiumTrendPoint>,
}
