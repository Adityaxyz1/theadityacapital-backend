use mongodb::Database;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Config,
}
