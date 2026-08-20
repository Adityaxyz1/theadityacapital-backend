use std::env;

// Fixed system-user id shared with the Python AI extraction service (see
// ai-service/env/.env.example: SYSTEM_USER_ID). Both sides must agree on this
// value so the service's self-minted JWT resolves to a real `users` document.
pub const SYSTEM_USER_ID: &str = "000000000000000000000001";

#[derive(Clone)]
pub struct Config {
    pub mongo_uri: String,
    pub mongo_db_name: String,
    pub jwt_secret: String,
    pub port: u16,
    pub notification_scan_interval_secs: u64,
    pub documents_dir: String,
    pub ai_service_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            mongo_uri: env::var("MONGO_URI").unwrap_or_else(|_| "mongodb://localhost:27017".into()),
            mongo_db_name: env::var("MONGO_DB_NAME").unwrap_or_else(|_| "aditya_crm".into()),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            notification_scan_interval_secs: env::var("NOTIFICATION_SCAN_INTERVAL_SECS")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3600),
            documents_dir: env::var("DOCUMENTS_DIR").unwrap_or_else(|_| "./data/documents".into()),
            ai_service_url: env::var("AI_SERVICE_URL").unwrap_or_else(|_| "http://localhost:8000".into()),
        }
    }
}
