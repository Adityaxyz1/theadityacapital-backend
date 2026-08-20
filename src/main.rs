mod activity;
mod auth;
mod categorize;
mod config;
mod db;
mod error;
mod listview;
mod models;
mod notify;
mod routes;
mod state;
mod visibility;
mod worker;
mod workflow;

use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_filename("env/.env").ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let config = Config::from_env();
    let db = db::connect(&config).await?;
    db::ensure_indexes(&db).await?;
    db::seed_default_notification_rule(&db).await?;
    db::seed_system_user(&db).await?;
    let _ = db::clean_invalid_insurers(&db).await;

    let state = AppState {
        db,
        config: config.clone(),
        ws_hub: Arc::new(Mutex::new(HashMap::new())),
        topic_hub: Arc::new(Mutex::new(HashMap::new())),
    };

    worker::spawn(state.clone());
    workflow::spawn(state.clone());

    let app = routes::router()
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("aditya-crm-api listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
