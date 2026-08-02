pub mod auth;
pub mod customers;
pub mod policies;
pub mod dashboard;

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
        .route("/dashboard/monthly", get(dashboard::monthly))
}
