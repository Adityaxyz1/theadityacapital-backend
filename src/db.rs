use mongodb::{options::ClientOptions, Client, Database};

use crate::{
    config::{Config, SYSTEM_USER_ID},
    models::{NotificationRule, Role, User},
};

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

    db.collection::<bson::Document>("teams")
        .create_index(IndexModel::builder().keys(doc! { "manager_id": 1 }).build())
        .await?;

    db.collection::<bson::Document>("saved_views")
        .create_index(IndexModel::builder().keys(doc! { "object": 1, "owner_id": 1 }).build())
        .await?;

    db.collection::<bson::Document>("workflow_rules")
        .create_index(IndexModel::builder().keys(doc! { "entity_type": 1, "enabled": 1 }).build())
        .await?;

    db.collection::<bson::Document>("workflow_trigger_state")
        .create_index(
            IndexModel::builder()
                .keys(doc! { "rule_id": 1, "entity_id": 1 })
                .options(mongodb::options::IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;

    db.collection::<bson::Document>("suggestions")
        .create_index(IndexModel::builder().keys(doc! { "assigned_to": 1, "status": 1 }).build())
        .await?;
    db.collection::<bson::Document>("suggestions")
        .create_index(IndexModel::builder().keys(doc! { "entity_type": 1, "entity_id": 1, "status": 1 }).build())
        .await?;

    db.collection::<bson::Document>("dashboard_widgets")
        .create_index(IndexModel::builder().keys(doc! { "user_id": 1, "position": 1 }).build())
        .await?;

    db.collection::<bson::Document>("leads")
        .create_index(IndexModel::builder().keys(doc! { "assigned_to": 1, "stage": 1 }).build())
        .await?;

    db.collection::<bson::Document>("claims")
        .create_index(IndexModel::builder().keys(doc! { "customer_id": 1 }).build())
        .await?;
    db.collection::<bson::Document>("claims")
        .create_index(IndexModel::builder().keys(doc! { "assigned_to": 1, "status": 1 }).build())
        .await?;

    db.collection::<bson::Document>("payments")
        .create_index(IndexModel::builder().keys(doc! { "customer_id": 1 }).build())
        .await?;
    db.collection::<bson::Document>("payments")
        .create_index(IndexModel::builder().keys(doc! { "assigned_to": 1, "status": 1 }).build())
        .await?;

    db.collection::<bson::Document>("activities")
        .create_index(IndexModel::builder().keys(doc! { "customer_id": 1 }).build())
        .await?;
    db.collection::<bson::Document>("activities")
        .create_index(IndexModel::builder().keys(doc! { "assigned_to": 1, "created_at": -1 }).build())
        .await?;

    db.collection::<bson::Document>("documents")
        .create_index(IndexModel::builder().keys(doc! { "customer_id": 1 }).build())
        .await?;

    db.collection::<bson::Document>("messages")
        .create_index(IndexModel::builder().keys(doc! { "sender_id": 1, "recipient_id": 1, "created_at": -1 }).build())
        .await?;
    db.collection::<bson::Document>("messages")
        .create_index(IndexModel::builder().keys(doc! { "recipient_id": 1, "sender_id": 1, "created_at": -1 }).build())
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

// Upserts a fixed-_id "system" user that the Python AI extraction service
// authenticates as (it self-mints a JWT with sub=SYSTEM_USER_ID using the
// shared JWT_SECRET — see ai-service/app/auth.py). Idempotent: safe to run on
// every startup. The password hash is a random, never-used placeholder since
// this account only ever authenticates via a self-minted JWT, never a login.
pub async fn seed_system_user(db: &Database) -> anyhow::Result<()> {
    use bson::doc;

    let oid = bson::oid::ObjectId::parse_str(SYSTEM_USER_ID)?;
    let collection = db.collection::<User>("users");

    if collection.find_one(doc! { "_id": oid }).await?.is_some() {
        return Ok(());
    }

    let unusable_password = uuid::Uuid::new_v4().to_string();
    let password_hash = crate::auth::password::hash_password(&unusable_password)?;

    let system_user = User {
        id: Some(oid),
        name: "AI Extraction Service".into(),
        email: "ai-extraction@internal.local".into(),
        phone: None,
        password_hash,
        role: Role::Admin,
        team_id: None,
        public_key: None,
        created_at: chrono::Utc::now(),
    };
    collection.insert_one(&system_user).await?;
    tracing::info!("seeded system user for AI extraction service");
    Ok(())
}

// Flags any existing database records where `insurer_name` was mistakenly set
// to the agency's own name ("The Aditya Capital") during bulk uploads or quick
// adds — a real extraction bug, now prevented at the source by explicit
// prompt instructions in ai-service (extraction.py never lets the model
// output the agency's own name as the insurer).
//
// This used to *silently overwrite* every match — across policies, renewals,
// and claims independently — with the hardcoded literal "ICICI Lombard",
// which is almost certainly not the real insurer for any given record. Worse,
// because it patched all three collections independently rather than through
// one edit, it could easily leave them agreeing with each other on a made-up
// value while being just as wrong as before, or — if only one of the three
// collections still had the bad name at the time it ran — leave that one
// collection out of step with the (already-correct) other two, exactly the
// renewal-vs-policy insurer_name mismatch this file was found chasing (see
// docs/PHASES.md / conversation history: Varun Tiwari's renewal had
// insurer_name "ICICI Lombard" while his policy correctly said "THE NEW INDIA
// ASSURANCE COMPANY LTD.").
//
// Now this only logs what's still wrong so a human corrects it via
// PATCH /policies/{id} (which cascades insurer_name/policy_type/policy_number
// to the linked renewal + claim automatically — see update_policy in
// routes/policies.rs) instead of fabricating a plausible-looking but false
// insurer name.
pub async fn clean_invalid_insurers(db: &Database) -> anyhow::Result<()> {
    use bson::doc;
    use futures_util::TryStreamExt;

    let bad_names = [
        "The Aditya Capital",
        "Aditya Capital",
        "The Aditya Capital Wealth & Policy CRM",
        "The Aditya Capital Wealth Management Firm",
        "Aditya Capital Wealth Management Firm",
        "aditya capital",
    ];
    let filter = doc! { "insurer_name": doc! { "$in": bad_names.to_vec() } };

    let mut cursor = db
        .collection::<bson::Document>("policies")
        .find(filter)
        .projection(doc! { "policy_number": 1 })
        .await?;
    let mut bad_policy_numbers = Vec::new();
    while let Some(doc) = cursor.try_next().await? {
        bad_policy_numbers.push(doc.get_str("policy_number").unwrap_or("<unknown>").to_string());
    }

    if !bad_policy_numbers.is_empty() {
        tracing::warn!(
            "{} policies still have the agency's own name as insurer_name — needs manual correction via PATCH /policies/{{id}}: {:?}",
            bad_policy_numbers.len(),
            bad_policy_numbers
        );
    }
    Ok(())
}
