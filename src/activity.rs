use bson::oid::ObjectId;
use chrono::Utc;

use crate::{
    models::{Activity, ActivityIcon},
    state::AppState,
};

pub struct NewActivity {
    pub user_id: ObjectId,
    pub action: String,
    pub customer_id: Option<ObjectId>,
    pub customer_name: Option<String>,
    pub policy_type: Option<String>,
    pub icon_type: ActivityIcon,
    pub assigned_to: Option<ObjectId>,
}

// Fire-and-forget, same style as notify.rs::dispatch and
// workflow.rs::on_record_created — a failed activity-log write should never
// fail the real write it's describing, so this only logs on error.
pub async fn log(state: &AppState, new: NewActivity) {
    let activity = Activity {
        id: None,
        user_id: new.user_id,
        action: new.action,
        customer_id: new.customer_id,
        customer_name: new.customer_name,
        policy_type: new.policy_type,
        icon_type: new.icon_type,
        assigned_to: new.assigned_to,
        created_at: Utc::now(),
    };
    if let Err(e) = state.db.collection::<Activity>("activities").insert_one(&activity).await {
        tracing::error!("failed to log activity: {e}");
    }
}
