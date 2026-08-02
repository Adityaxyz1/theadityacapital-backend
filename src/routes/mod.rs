pub mod auth;
pub mod customers;
pub mod policies;
pub mod renewals;
pub mod notifications;
pub mod dashboard;
pub mod ws;

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
        .route("/renewals", get(renewals::list_renewals))
        .route("/renewals/{id}/status", axum::routing::patch(renewals::update_renewal_status))
        .route("/notifications", get(notifications::list_my_notifications))
        .route(
            "/notifications/{id}/read",
            axum::routing::patch(notifications::mark_notification_read),
        )
        .route(
            "/notification-rules",
            get(notifications::list_notification_rules).post(notifications::upsert_notification_rule),
        )
        .route("/dashboard/monthly", get(dashboard::monthly))
        .route("/ws", get(ws::ws_handler))
}
