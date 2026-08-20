pub mod auth;
pub mod customers;
pub mod policies;
pub mod renewals;
pub mod notifications;
pub mod dashboard;
pub mod ws;
pub mod documents;
pub mod teams;
pub mod users;
pub mod saved_views;
pub mod search;
pub mod workflow_rules;
pub mod suggestions;
pub mod assistant;
pub mod reports;
pub mod dashboard_widgets;
pub mod leads;
pub mod claims;
pub mod payments;
pub mod activities;
pub mod messages;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        .route("/search", get(search::search))
        .route("/users", get(users::list_users))
        .route("/users/directory", get(users::list_directory))
        .route("/users/me/public-key", axum::routing::put(users::set_my_public_key))
        .route("/users/{id}", axum::routing::patch(users::update_user))
        .route("/teams", get(teams::list_teams).post(teams::create_team))
        .route(
            "/teams/{id}",
            axum::routing::put(teams::update_team).delete(teams::delete_team),
        )
        .route(
            "/saved-views",
            get(saved_views::list_saved_views).post(saved_views::create_saved_view),
        )
        .route("/saved-views/{id}", axum::routing::delete(saved_views::delete_saved_view))
        .route(
            "/workflow-rules",
            get(workflow_rules::list_workflow_rules).post(workflow_rules::create_workflow_rule),
        )
        .route(
            "/workflow-rules/{id}",
            axum::routing::put(workflow_rules::update_workflow_rule).delete(workflow_rules::delete_workflow_rule),
        )
        .route("/suggestions", get(suggestions::list_suggestions))
        .route("/suggestions/{id}", axum::routing::patch(suggestions::update_suggestion))
        .route(
            "/records/{entity_type}/{entity_id}/suggestions",
            get(suggestions::list_suggestions_for_record),
        )
        .route("/assistant/chat", post(assistant::chat))
        .route("/reports", get(reports::catalog))
        .route("/reports/yearly-comparison", get(reports::yearly_comparison))
        .route("/reports/top-performers", get(reports::top_performers))
        .route("/reports/premium-by-type", get(reports::premium_by_type))
        .route("/reports/premium-trend", get(reports::premium_trend))
        .route("/reports/{report_key}", get(reports::run))
        .route(
            "/dashboard-widgets",
            get(dashboard_widgets::list_my_widgets).post(dashboard_widgets::pin_widget),
        )
        .route("/dashboard-widgets/{id}", axum::routing::delete(dashboard_widgets::unpin_widget))
        .route("/customers", get(customers::list_customers).post(customers::create_customer))
        .route(
            "/customers/{id}",
            get(customers::get_customer)
                .put(customers::update_customer)
                .delete(customers::delete_customer),
        )
        .route("/policies", get(policies::list_policies).post(policies::create_policy))
        .route(
            "/policies/{id}",
            get(policies::get_policy)
                .put(policies::update_policy)
                .delete(policies::delete_policy),
        )
        .route("/policies/{id}/follow-ups", post(policies::add_follow_up))
        .route("/policies/{id}/extraction", get(documents::get_policy_extraction))
        .route("/renewals", get(renewals::list_renewals))
        .route("/renewals/{id}/status", axum::routing::patch(renewals::update_renewal_status))
        .route(
            "/notifications",
            get(notifications::list_my_notifications).delete(notifications::clear_my_notifications),
        )
        .route(
            "/notifications/{id}/read",
            axum::routing::patch(notifications::mark_notification_read),
        )
        .route(
            "/notification-rules",
            get(notifications::list_notification_rules).post(notifications::upsert_notification_rule),
        )
        .route("/documents", get(documents::list_documents).post(documents::upload_document))
        .route("/documents/bulk", post(documents::upload_bulk_document))
        .route("/documents/{id}", get(documents::get_document).delete(documents::delete_document))
        .route("/documents/{id}/file", get(documents::download_document))
        .route("/dashboard/monthly", get(dashboard::monthly))
        .route("/leads", get(leads::list_leads).post(leads::create_lead))
        .route(
            "/leads/{id}",
            get(leads::get_lead).put(leads::update_lead).delete(leads::delete_lead),
        )
        .route("/leads/{id}/stage", axum::routing::patch(leads::update_lead_stage))
        .route("/claims", get(claims::list_claims).post(claims::create_claim))
        .route("/claims/{id}", get(claims::get_claim).delete(claims::delete_claim))
        .route("/claims/{id}/status", axum::routing::patch(claims::update_claim_status))
        .route("/payments", get(payments::list_payments).post(payments::create_payment))
        .route("/payments/{id}", get(payments::get_payment).delete(payments::delete_payment))
        .route("/payments/{id}/status", axum::routing::patch(payments::update_payment_status))
        .route("/activities", get(activities::list_activities).post(activities::create_activity))
        .route("/messages", post(messages::send_message))
        .route("/messages/threads", get(messages::list_threads))
        .route("/messages/thread/{partner_id}", get(messages::get_thread))
        .route("/messages/thread/{partner_id}/read", axum::routing::patch(messages::mark_thread_read))
        .route("/ws", get(ws::ws_handler))
}
