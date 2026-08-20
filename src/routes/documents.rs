use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bson::doc;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{ApiError, ApiResult},
    listview::{paginate, PageParams, Paginated},
    models::{
        Document, DocumentExtraction, DocumentExtractionResponse, DocumentResponse,
        ExtractionStatus, FileType, Policy, Renewal,
    },
    state::AppState,
};

fn parse_oid(id: &str) -> ApiResult<bson::oid::ObjectId> {
    bson::oid::ObjectId::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

#[derive(Debug, Deserialize)]
pub struct ListDocumentsQuery {
    pub customer_id: Option<String>,
    pub policy_id: Option<String>,
}

// There was previously no way to list a customer's/policy's documents at
// all — only fetch one by its own id (GET /documents/{id}) — which meant
// finding a document to clean up (e.g. after deleting the policy it belongs
// to) required going around the API entirely. This closes that gap.
pub async fn list_documents(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<ListDocumentsQuery>,
    Query(page): Query<PageParams>,
) -> ApiResult<Json<Paginated<DocumentResponse>>> {
    let collection = state.db.collection::<Document>("documents");

    let mut filter = doc! {};
    if let Some(customer_id) = query.customer_id {
        filter.insert("customer_id", parse_oid(&customer_id)?);
    }
    if let Some(policy_id) = query.policy_id {
        filter.insert("policy_id", parse_oid(&policy_id)?);
    }
    let sort = doc! { "uploaded_at": -1 };

    Ok(Json(paginate(&collection, filter, sort, &page).await?))
}

// The AI extraction pipeline creates policies/renewals via its own system
// JWT (ai-service has no notion of "who uploaded this"), so a freshly
// extracted policy's `assigned_to` defaults to the system user — invisible
// to every real Manager/Agent (only Admin's unscoped view sees it). Whoever
// actually uploaded the document is the real owner of this book-of-business
// entry, so reassign both the policy and its paired renewal right after
// creation.
async fn reassign_policy_and_renewal(state: &AppState, policy_id: bson::oid::ObjectId, owner: bson::oid::ObjectId) {
    let _ = state
        .db
        .collection::<Policy>("policies")
        .update_one(doc! { "_id": policy_id }, doc! { "$set": { "assigned_to": owner } })
        .await;
    let _ = state
        .db
        .collection::<Renewal>("renewals")
        .update_one(doc! { "policy_id": policy_id }, doc! { "$set": { "assigned_to": owner } })
        .await;
}

fn infer_file_type(filename: &str) -> ApiResult<FileType> {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "pdf" => Ok(FileType::Pdf),
        "png" | "jpg" | "jpeg" | "webp" => Ok(FileType::Image),
        "xlsx" | "xls" => Ok(FileType::Xlsx),
        "html" | "htm" => Ok(FileType::Html),
        _ => Err(ApiError::BadRequest(format!("unsupported file type: .{ext}"))),
    }
}

#[derive(Debug, Serialize)]
struct ExtractRequest {
    document_id: String,
    file_path: String,
    file_type: FileType,
}

#[derive(Debug, Deserialize)]
struct ExtractResponse {
    status: ExtractionStatus,
    confidence: f64,
    extracted_fields: serde_json::Value,
    matched_customer_id: Option<String>,
    matched_policy_id: Option<String>,
    created_customer: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub document: DocumentResponse,
    pub extraction: DocumentExtractionResponse,
}

// Shared by upload_document (single pdf/image) and upload_bulk_document
// (xlsx/html): read the multipart "file" field, write it to disk, and insert
// its `documents` record. What happens with the stored file afterward (which
// extraction endpoint gets called, how many DocumentExtraction rows come
// back) differs enough between the two paths that they stay separate
// handlers rather than one handler branching midway.
async fn store_uploaded_file(
    state: &AppState,
    auth: &AuthUser,
    multipart: &mut Multipart,
) -> ApiResult<Document> {
    let mut original_filename = None;
    let mut bytes: Option<axum::body::Bytes> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid upload: {e}")))?
    {
        if field.name() == Some("file") {
            original_filename = field.file_name().map(|s| s.to_string());
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("invalid upload: {e}")))?,
            );
        }
    }

    let original_filename =
        original_filename.ok_or(ApiError::BadRequest("no file field in upload".into()))?;
    let bytes = bytes.ok_or(ApiError::BadRequest("no file field in upload".into()))?;
    let file_type = infer_file_type(&original_filename)?;

    tokio::fs::create_dir_all(&state.config.documents_dir)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let ext = original_filename.rsplit('.').next().unwrap_or("bin");
    let stored_name = format!("{}.{}", Uuid::new_v4(), ext);
    let storage_path = PathBuf::from(&state.config.documents_dir).join(&stored_name);
    tokio::fs::write(&storage_path, &bytes)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let absolute_path = storage_path
        .canonicalize()
        .map_err(|e| ApiError::Internal(e.into()))?;

    let document = Document {
        id: None,
        policy_id: None,
        customer_id: None,
        file_type,
        storage_path: absolute_path.to_string_lossy().to_string(),
        original_filename,
        uploaded_by: auth.user_id,
        uploaded_at: Utc::now(),
    };
    let documents = state.db.collection::<Document>("documents");
    let result = documents.insert_one(&document).await?;
    let document_id = result
        .inserted_id
        .as_object_id()
        .expect("insert_one always returns an ObjectId for a Mongo-generated _id");

    let mut document = document;
    document.id = Some(document_id);
    Ok(document)
}

pub async fn upload_document(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> ApiResult<Json<UploadResponse>> {
    let mut document = store_uploaded_file(&state, &auth, &mut multipart).await?;
    let document_id = document.id.expect("just inserted");
    let now = Utc::now();

    // The Python service is called synchronously here (per ARCHITECTURE.md
    // §2's "simple approach") — fine at this volume; move to a job queue if
    // extraction latency ever becomes a problem for the upload request.
    let extract_result = call_extraction_service(&state, document_id, &document).await;

    let extraction = match extract_result {
        Ok(resp) => DocumentExtraction {
            id: None,
            document_id,
            status: resp.status,
            extracted_fields: bson::to_bson(&resp.extracted_fields)
                .map_err(|e| ApiError::Internal(e.into()))?,
            matched_customer_id: resp.matched_customer_id.as_deref().map(parse_oid).transpose()?,
            matched_policy_id: resp.matched_policy_id.as_deref().map(parse_oid).transpose()?,
            created_customer: resp.created_customer,
            confidence: resp.confidence,
            error: resp.error,
            row_index: None,
            created_at: now,
        },
        Err(e) => {
            tracing::error!("extraction service call failed: {e}");
            DocumentExtraction {
                id: None,
                document_id,
                status: ExtractionStatus::Failed,
                extracted_fields: bson::Bson::Null,
                matched_customer_id: None,
                matched_policy_id: None,
                created_customer: false,
                confidence: 0.0,
                error: Some(format!("extraction service unavailable: {e}")),
                row_index: None,
                created_at: now,
            }
        }
    };

    let extractions = state.db.collection::<DocumentExtraction>("document_extractions");
    let ext_result = extractions.insert_one(&extraction).await?;
    let mut extraction = extraction;
    extraction.id = ext_result.inserted_id.as_object_id();

    if extraction.matched_customer_id.is_some() || extraction.matched_policy_id.is_some() {
        state
            .db
            .collection::<Document>("documents")
            .update_one(
                doc! { "_id": document_id },
                doc! { "$set": {
                    "customer_id": extraction.matched_customer_id,
                    "policy_id": extraction.matched_policy_id,
                } },
            )
            .await?;
        document.customer_id = extraction.matched_customer_id;
        document.policy_id = extraction.matched_policy_id;
    }
    if let Some(policy_id) = extraction.matched_policy_id {
        reassign_policy_and_renewal(&state, policy_id, auth.user_id).await;
    }

    Ok(Json(UploadResponse {
        document: document.into(),
        extraction: extraction.into(),
    }))
}

async fn call_extraction_service(
    state: &AppState,
    document_id: bson::oid::ObjectId,
    document: &Document,
) -> anyhow::Result<ExtractResponse> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/extract", state.config.ai_service_url))
        .timeout(std::time::Duration::from_secs(180))
        .json(&ExtractRequest {
            document_id: document_id.to_hex(),
            file_path: document.storage_path.clone(),
            file_type: document.file_type,
        })
        .send()
        .await?
        .error_for_status()?
        .json::<ExtractResponse>()
        .await?;
    Ok(response)
}

#[derive(Debug, Serialize)]
struct BulkExtractRequest {
    document_id: String,
    file_path: String,
    file_type: FileType,
}

#[derive(Debug, Deserialize)]
struct BulkRowResult {
    #[serde(default)]
    row_index: Option<i32>,
    status: ExtractionStatus,
    confidence: f64,
    extracted_fields: serde_json::Value,
    matched_customer_id: Option<String>,
    matched_policy_id: Option<String>,
    created_customer: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BulkExtractResponse {
    total_rows: i32,
    results: Vec<BulkRowResult>,
}

// One document, many policies (PRD 4.5/4.6) — a bulk upload gets its own
// route and response shape rather than branching inside upload_document,
// since the 1 document : N DocumentExtraction relationship here is
// fundamentally different from the single-doc path's 1:1.
#[derive(Debug, Serialize)]
pub struct BulkUploadResponse {
    pub document: DocumentResponse,
    pub total_rows: i32,
    pub extractions: Vec<DocumentExtractionResponse>,
}

pub async fn upload_bulk_document(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> ApiResult<Json<BulkUploadResponse>> {
    let mut document = store_uploaded_file(&state, &auth, &mut multipart).await?;
    if !matches!(document.file_type, FileType::Xlsx | FileType::Html) {
        return Err(ApiError::BadRequest("bulk upload only accepts .xlsx or .html files".into()));
    }
    let document_id = document.id.expect("just inserted");
    let now = Utc::now();

    let bulk_result = call_bulk_extraction_service(&state, document_id, &document).await;

    let (total_rows, row_results) = match bulk_result {
        Ok(resp) => (resp.total_rows, resp.results),
        Err(e) => {
            tracing::error!("bulk extraction service call failed: {e}");
            (
                0,
                vec![BulkRowResult {
                    // The whole service call failed before it could parse any
                    // rows, so there's no real row to point to.
                    row_index: None,
                    status: ExtractionStatus::Failed,
                    confidence: 0.0,
                    extracted_fields: serde_json::Value::Null,
                    matched_customer_id: None,
                    matched_policy_id: None,
                    created_customer: false,
                    error: Some(format!("extraction service unavailable: {e}")),
                }],
            )
        }
    };

    let extractions_collection = state.db.collection::<DocumentExtraction>("document_extractions");
    let mut extraction_responses = Vec::with_capacity(row_results.len());
    let mut last_matched_customer_id = None;

    for row in row_results {
        let extraction = DocumentExtraction {
            id: None,
            document_id,
            status: row.status,
            extracted_fields: bson::to_bson(&row.extracted_fields).map_err(|e| ApiError::Internal(e.into()))?,
            matched_customer_id: row.matched_customer_id.as_deref().map(parse_oid).transpose()?,
            matched_policy_id: row.matched_policy_id.as_deref().map(parse_oid).transpose()?,
            created_customer: row.created_customer,
            confidence: row.confidence,
            error: row.error,
            row_index: row.row_index,
            created_at: now,
        };
        let result = extractions_collection.insert_one(&extraction).await?;
        let mut created = extraction;
        created.id = result.inserted_id.as_object_id();
        if created.matched_customer_id.is_some() {
            last_matched_customer_id = created.matched_customer_id;
        }
        if let Some(policy_id) = created.matched_policy_id {
            reassign_policy_and_renewal(&state, policy_id, auth.user_id).await;
        }
        extraction_responses.push(created.into());
    }

    // A bulk document doesn't belong to one customer/policy the way a
    // single-document upload does — it's tagged with whichever customer the
    // last successfully-matched row touched, purely so the file has *some*
    // discoverable link back from a customer record; the authoritative trail
    // is each row's own DocumentExtraction.
    if last_matched_customer_id.is_some() {
        state
            .db
            .collection::<Document>("documents")
            .update_one(
                doc! { "_id": document_id },
                doc! { "$set": { "customer_id": last_matched_customer_id } },
            )
            .await?;
        document.customer_id = last_matched_customer_id;
    }

    Ok(Json(BulkUploadResponse {
        document: document.into(),
        total_rows,
        extractions: extraction_responses,
    }))
}

async fn call_bulk_extraction_service(
    state: &AppState,
    document_id: bson::oid::ObjectId,
    document: &Document,
) -> anyhow::Result<BulkExtractResponse> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/bulk-extract", state.config.ai_service_url))
        // ai-service retries each LLM-fallback row with backoff on a NIM
        // rate-limit response (up to ~60s of sleep per row) rather than
        // failing it outright, so a large batch that hits sustained rate
        // limiting legitimately needs more headroom than a single-row call.
        .timeout(std::time::Duration::from_secs(600))
        .json(&BulkExtractRequest {
            document_id: document_id.to_hex(),
            file_path: document.storage_path.clone(),
            file_type: document.file_type,
        })
        .send()
        .await?
        .error_for_status()?
        .json::<BulkExtractResponse>()
        .await?;
    Ok(response)
}

pub async fn get_document(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<DocumentResponse>> {
    let oid = parse_oid(&id)?;
    let document = state
        .db
        .collection::<Document>("documents")
        .find_one(doc! { "_id": oid })
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(document.into()))
}

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    // A plain `<a href target="_blank">` (the "View source document" links)
    // is a real browser navigation, not an axios request — it can't attach
    // an Authorization header. Accept the JWT as a query param too, same
    // constraint and same fix as routes/ws.rs's WsQuery::token.
    pub token: Option<String>,
}

pub async fn download_document(
    State(state): State<AppState>,
    auth: Option<AuthUser>,
    Query(query): Query<DownloadQuery>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    if auth.is_none() {
        let token = query.token.as_deref().ok_or(ApiError::Unauthorized)?;
        crate::auth::jwt::decode_token(token, &state.config.jwt_secret).map_err(|_| ApiError::Unauthorized)?;
    }

    let oid = parse_oid(&id)?;
    let document = state
        .db
        .collection::<Document>("documents")
        .find_one(doc! { "_id": oid })
        .await?
        .ok_or(ApiError::NotFound)?;

    let bytes = tokio::fs::read(&document.storage_path)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let content_type = match document.file_type {
        FileType::Pdf => "application/pdf",
        FileType::Image => "application/octet-stream",
        FileType::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        FileType::Html => "text/html",
    };

    let disposition = format!("inline; filename=\"{}\"", document.original_filename);
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        Body::from(bytes),
    )
        .into_response())
}

// Deletes the document artifact itself and its extraction audit trail
// (DocumentExtraction rows), but deliberately leaves any Policy/Renewal
// created from it alone — those are real business records independent of
// the source file, not something a document deletion should cascade into.
pub async fn delete_document(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let oid = parse_oid(&id)?;
    let documents = state.db.collection::<Document>("documents");
    let document = documents
        .find_one(doc! { "_id": oid })
        .await?
        .ok_or(ApiError::NotFound)?;

    // Best-effort: the file may already be gone from disk (e.g. manually
    // cleaned up), which shouldn't block deleting the metadata rows below.
    // Any other IO error (permissions, etc.) is a genuine failure though.
    if let Err(e) = tokio::fs::remove_file(&document.storage_path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(ApiError::Internal(e.into()));
        }
    }

    documents.delete_one(doc! { "_id": oid }).await?;
    state
        .db
        .collection::<DocumentExtraction>("document_extractions")
        .delete_many(doc! { "document_id": oid })
        .await?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn get_policy_extraction(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<UploadResponse>> {
    let oid = parse_oid(&id)?;
    let policy = state
        .db
        .collection::<Policy>("policies")
        .find_one(doc! { "_id": oid })
        .await?
        .ok_or(ApiError::NotFound)?;

    let document_id = policy.source_document_id.ok_or(ApiError::NotFound)?;

    let document = state
        .db
        .collection::<Document>("documents")
        .find_one(doc! { "_id": document_id })
        .await?
        .ok_or(ApiError::NotFound)?;

    let extraction = state
        .db
        .collection::<DocumentExtraction>("document_extractions")
        .find_one(doc! { "document_id": document_id })
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(UploadResponse {
        document: document.into(),
        extraction: extraction.into(),
    }))
}
