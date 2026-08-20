use std::time::Duration;

use bson::doc;
use chrono::Utc;
use futures_util::TryStreamExt;

use crate::{
    models::{Notification, NotificationChannel, NotificationRule, NotificationStatus, Renewal},
    notify,
    state::AppState,
};

// Scans for renewals crossing a configured reminder threshold and creates/
// dispatches notifications for them, on a fixed interval (ARCHITECTURE.md §6).
pub fn spawn(state: AppState) {
    let interval_secs = state.config.notification_scan_interval_secs;
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            if let Err(e) = scan_once(&state).await {
                tracing::error!("notification scan failed: {e}");
            }
        }
    });
}

async fn scan_once(state: &AppState) -> anyhow::Result<()> {
    let rules: Vec<NotificationRule> = state
        .db
        .collection::<NotificationRule>("notification_rules")
        .find(doc! {})
        .await?
        .try_collect()
        .await?;
    let global_rule = rules.iter().find(|r| r.policy_type.is_none()).cloned();

    let renewals_collection = state.db.collection::<Renewal>("renewals");
    let renewals: Vec<Renewal> = renewals_collection
        .find(doc! { "status": { "$in": ["pending", "contacted"] } })
        .await?
        .try_collect()
        .await?;

    let now = Utc::now();
    let today = now.date_naive();

    for renewal in renewals {
        let Some(rule) = rules
            .iter()
            .find(|r| r.policy_type.as_deref() == Some(renewal.policy_type.as_str()))
            .or(global_rule.as_ref())
        else {
            continue;
        };

        let mut offsets = rule.offset_days.clone();
        offsets.sort_unstable_by(|a, b| b.cmp(a));

        let days_left = (renewal.due_date.date_naive() - today).num_days();
        let mut stage = renewal.reminder_stage.max(0) as usize;
        let stage_before = stage;

        while stage < offsets.len() && days_left <= offsets[stage] {
            let mut notification = Notification {
                id: None,
                renewal_id: renewal.id.expect("renewal always has an id once persisted"),
                customer_id: Some(renewal.customer_id),
                user_id: renewal.assigned_to,
                channel: NotificationChannel::InApp,
                status: NotificationStatus::Queued,
                message: render_message(&rule.template, &renewal, offsets[stage]),
                offset_days: offsets[stage],
                sent_at: None,
                read_at: None,
                created_at: now,
            };

            let result = state
                .db
                .collection::<Notification>("notifications")
                .insert_one(&notification)
                .await?;
            notification.id = result.inserted_id.as_object_id();

            notify::dispatch(state, &mut notification).await;

            stage += 1;
        }

        if stage != stage_before {
            renewals_collection
                .update_one(
                    doc! { "_id": renewal.id },
                    doc! { "$set": { "reminder_stage": stage as i32 } },
                )
                .await?;
        }
    }

    Ok(())
}

fn render_message(template: &str, renewal: &Renewal, offset_days: i64) -> String {
    template
        .replace("{customer_name}", &renewal.customer_name)
        .replace("{policy_type}", &renewal.policy_type)
        .replace("{insurer_name}", &renewal.insurer_name)
        .replace("{days}", &offset_days.to_string())
}
