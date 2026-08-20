use bson::oid::ObjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Pdf,
    Image,
    Xlsx,
    Html,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub policy_id: Option<ObjectId>,
    pub customer_id: Option<ObjectId>,
    pub file_type: FileType,
    // Local filesystem path on the server (ARCHITECTURE.md §2) — never
    // web-exposed directly; downloads go through GET /documents/{id}/file.
    pub storage_path: String,
    pub original_filename: String,
    pub uploaded_by: ObjectId,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub uploaded_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DocumentResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub policy_id: Option<String>,
    pub customer_id: Option<String>,
    pub file_type: FileType,
    pub original_filename: String,
    pub uploaded_by: String,
    pub uploaded_at: DateTime<Utc>,
}

impl From<Document> for DocumentResponse {
    fn from(d: Document) -> Self {
        DocumentResponse {
            id: d.id.map(|i| i.to_hex()).unwrap_or_default(),
            policy_id: d.policy_id.map(|i| i.to_hex()),
            customer_id: d.customer_id.map(|i| i.to_hex()),
            file_type: d.file_type,
            original_filename: d.original_filename,
            uploaded_by: d.uploaded_by.to_hex(),
            uploaded_at: d.uploaded_at,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Processing,
    Applied,
    FlaggedLowConfidence,
    Failed,
}

// `extracted_fields` is the raw structured output from the AI service, kept
// verbatim (not remodeled per-field) so staff can trace exactly what the
// model returned, separate from what ended up written to the policy record.
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentExtraction {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub document_id: ObjectId,
    pub status: ExtractionStatus,
    pub extracted_fields: bson::Bson,
    pub matched_customer_id: Option<ObjectId>,
    pub matched_policy_id: Option<ObjectId>,
    pub created_customer: bool,
    pub confidence: f64,
    pub error: Option<String>,
    // Only set for rows that came from a bulk XLSX/HTML upload (0-based
    // position among that file's data rows, after the header and any blank
    // rows are dropped — see ai-service/app/parsing.py::_rows_from_grid).
    // None for a single-document upload, which has no notion of "a row".
    // `#[serde(default)]` so DocumentExtraction records written before this
    // field existed still deserialize (they simply come back as None).
    #[serde(default)]
    pub row_index: Option<i32>,
    #[serde(with = "bson::serde_helpers::chrono_datetime_as_bson_datetime")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DocumentExtractionResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub document_id: String,
    pub status: ExtractionStatus,
    pub extracted_fields: serde_json::Value,
    pub matched_customer_id: Option<String>,
    pub matched_policy_id: Option<String>,
    pub created_customer: bool,
    pub confidence: f64,
    pub error: Option<String>,
    pub row_index: Option<i32>,
    pub created_at: DateTime<Utc>,
}

impl From<DocumentExtraction> for DocumentExtractionResponse {
    fn from(e: DocumentExtraction) -> Self {
        DocumentExtractionResponse {
            id: e.id.map(|i| i.to_hex()).unwrap_or_default(),
            document_id: e.document_id.to_hex(),
            status: e.status,
            extracted_fields: bson::from_bson(e.extracted_fields).unwrap_or(serde_json::Value::Null),
            matched_customer_id: e.matched_customer_id.map(|i| i.to_hex()),
            matched_policy_id: e.matched_policy_id.map(|i| i.to_hex()),
            created_customer: e.created_customer,
            confidence: e.confidence,
            error: e.error,
            row_index: e.row_index,
            created_at: e.created_at,
        }
    }
}
