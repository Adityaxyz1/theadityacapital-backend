use bson::{doc, Document};
use futures_util::TryStreamExt;
use mongodb::Collection;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};

const DEFAULT_PAGE_SIZE: u64 = 25;
const MAX_PAGE_SIZE: u64 = 200;

// Flatten this into any `ListXQuery` struct (`#[serde(flatten)] pub page: PageParams`)
// to add real pagination/sort to a list endpoint without touching its existing
// filter fields.
#[derive(Debug, Deserialize)]
pub struct PageParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub sort: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Paginated<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

// `sort` is `field` (ascending) or `-field` (descending). `allowed` is the
// whitelist of sortable fields for this object — anything else is a 400, not
// a silently-ignored no-op, so a typo'd sort param surfaces immediately.
pub fn parse_sort(sort: &Option<String>, allowed: &[&str], default_field: &str) -> ApiResult<Document> {
    let (field, dir) = match sort {
        Some(s) if s.starts_with('-') => (&s[1..], -1),
        Some(s) => (s.as_str(), 1),
        None => (default_field, 1),
    };
    if !allowed.contains(&field) {
        return Err(ApiError::BadRequest(format!("cannot sort by '{field}'")));
    }
    Ok(doc! { field: dir })
}

pub async fn paginate<M, R>(
    collection: &Collection<M>,
    filter: Document,
    sort: Document,
    page: &PageParams,
) -> ApiResult<Paginated<R>>
where
    M: DeserializeOwned + Unpin + Send + Sync,
    R: From<M>,
{
    let page_num = page.page.unwrap_or(1).max(1);
    let page_size = page.page_size.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    let skip = (page_num - 1) * page_size;

    let total = collection.count_documents(filter.clone()).await?;
    let cursor = collection
        .find(filter)
        .sort(sort)
        .skip(skip)
        .limit(page_size as i64)
        .await?;
    let items: Vec<M> = cursor.try_collect().await?;

    Ok(Paginated {
        data: items.into_iter().map(Into::into).collect(),
        total,
        page: page_num,
        page_size,
    })
}
