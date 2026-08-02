use mongodb::{options::ClientOptions, Client, Database};

use crate::{config::Config, models::NotificationRule};

pub async fn connect(config: &Config) -> mongodb::error::Result<Database> {
    let mut options = ClientOptions::parse(&config.mongo_uri).await?;
    options.app_name = Some("aditya-crm-api".to_string());
    let client = Client::with_options(options)?;
    let db = client.database(&config.mongo_db_name);
    Ok(db)
}

pub async fn ensure_indexes(db: &Database) -> mongodb::error::Result<()> {
    use mongodb::IndexModel;
    use bson::doc;

    db.collection::<bson::Document>("users")
        .create_index(
            IndexModel::builder()
                .keys(doc! { "email": 1 })
                .options(mongodb::options::IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;

    db.collection::<bson::Document>("policies")
        .create_index(IndexModel::builder().keys(doc! { "customer_id": 1 }).build())
        .await?;
    db.collection::<bson::Document>("policies")
        .create_index(
            IndexModel::builder()
                .keys(doc! { "policy_number": 1 })
                .options(mongodb::options::IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;

    db.collection::<bson::Document>("renewals")
        .create_index(IndexModel::builder().keys(doc! { "due_date": 1 }).build())
        .await?;
    db.collection::<bson::Document>("renewals")
        .create_index(IndexModel::builder().keys(doc! { "status": 1 }).build())
        .await?;
    db.collection::<bson::Document>("renewals")
        .create_index(IndexModel::builder().keys(doc! { "assigned_to": 1 }).build())
        .await?;

    db.collection::<bson::Document>("documents")
        .create_index(IndexModel::builder().keys(doc! { "policy_id": 1 }).build())
        .await?;

    db.collection::<bson::Document>("notifications")
        .create_index(IndexModel::builder().keys(doc! { "user_id": 1, "status": 1 }).build())
        .await?;

    Ok(())
}

// Seeds a single global reminder rule (applies to every policy_type) the
// first time the app runs against an empty notification_rules collection, so
// the reminder worker has something to scan against out of the box. Staff can
// override it (or add per-policy-type rules) via the notification_rules API.
pub async fn seed_default_notification_rule(db: &Database) -> mongodb::error::Result<()> {
    let collection = db.collection::<NotificationRule>("notification_rules");
    if collection.estimated_document_count().await? > 0 {
        return Ok(());
    }

    let default_rule = NotificationRule {
        id: None,
        policy_type: None,
        offset_days: vec![30, 15, 7, 1, 0],
        template: "{customer_name}'s {policy_type} policy with {insurer_name} is due for renewal in {days} day(s).".into(),
    };
    collection.insert_one(&default_rule).await?;
    tracing::info!("seeded default notification rule");
    Ok(())
}
