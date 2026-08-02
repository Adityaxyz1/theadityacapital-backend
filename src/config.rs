use std::env;

#[derive(Clone)]
pub struct Config {
    pub mongo_uri: String,
    pub mongo_db_name: String,
    pub jwt_secret: String,
    pub port: u16,
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
        }
    }
}
