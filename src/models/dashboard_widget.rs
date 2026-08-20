use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::report::ReportKey;

// The same params shape a report's own query string takes (routes/reports.rs
// `ReportQuery`), just persisted — pinning a widget saves *which report with
// which filters*, never a data snapshot. Dates are stored as plain
// "YYYY-MM-DD" strings rather than a chrono type: this record is never
// range-queried itself (only read back and replayed as query params against
// /reports/{key}), so there's no need to round-trip through bson's
// DateTime-only date support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetParams {
    pub date_from: String,
    pub date_to: String,
    pub insurer_name: Option<String>,
    pub policy_type: Option<String>,
}

// Pinned widgets are personal (own `user_id`), not shared — each staff member
// curates their own dashboard, matching PRD 5's "the dashboard is the
// primary daily-use screen for staff" usability requirement.
#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardWidget {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: ObjectId,
    pub report_key: ReportKey,
    pub title: String,
    pub params: WidgetParams,
    pub position: i32,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

// See CustomerResponse (models/customer.rs) for why API responses use a
// dedicated DTO instead of serializing the Mongo model directly.
#[derive(Debug, Serialize)]
pub struct DashboardWidgetResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub report_key: ReportKey,
    pub title: String,
    pub params: WidgetParams,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

impl From<DashboardWidget> for DashboardWidgetResponse {
    fn from(w: DashboardWidget) -> Self {
        DashboardWidgetResponse {
            id: w.id.map(|i| i.to_hex()).unwrap_or_default(),
            report_key: w.report_key,
            title: w.title,
            params: w.params,
            position: w.position,
            created_at: w.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateDashboardWidgetInput {
    pub report_key: ReportKey,
    pub title: String,
    pub params: WidgetParams,
}
