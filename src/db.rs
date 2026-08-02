use mongodb::{options::ClientOptions, Client, Database};

use crate::config::Config;

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

    Ok(())
}
