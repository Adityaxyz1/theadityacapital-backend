use bson::{doc, oid::ObjectId, Document};
use mongodb::Database;

use crate::{auth::AuthUser, error::ApiResult, models::{Role, Team}};

// The Mongo filter fragment that scopes a policies/renewals query to what
// `auth` is allowed to see: admins see everything, agents see only records
// `assigned_to` them, managers see their team's records (their own team
// lookup is by `manager_id`, not `team_id`, so a manager doesn't need to be
// listed as a member of their own team). Merge this into every list AND
// single-record filter via `combine_filters` so an out-of-scope record 404s
// like any other missing record, rather than needing a separate 403 path.
//
// Customers are intentionally left unscoped for now — the current data model
// has no `assigned_to` on customers (they're a shared address book), only
// policies/renewals represent an agent's "book of business".
pub async fn visibility_filter(db: &Database, auth: &AuthUser) -> ApiResult<Document> {
    match auth.role {
        Role::Admin => Ok(doc! {}),
        Role::Agent => Ok(doc! { "assigned_to": auth.user_id }),
        Role::Manager => {
            let team = db
                .collection::<Team>("teams")
                .find_one(doc! { "manager_id": auth.user_id })
                .await?;
            let mut ids: Vec<ObjectId> = team.map(|t| t.member_ids).unwrap_or_default();
            ids.push(auth.user_id);
            Ok(doc! { "assigned_to": { "$in": ids } })
        }
    }
}

// Mongo filters are documents of top-level keys; naively `.extend()`-ing two
// filters that both happen to touch "assigned_to" (e.g. a manager viewing
// their team, further narrowed by an explicit `?assigned_to=` query param)
// would silently let the second overwrite the first. Wrapping in `$and`
// instead means both conditions are always honored together.
pub fn combine_filters(a: Document, b: Document) -> Document {
    if b.is_empty() {
        a
    } else if a.is_empty() {
        b
    } else {
        doc! { "$and": [a, b] }
    }
}
