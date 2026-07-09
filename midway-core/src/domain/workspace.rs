use serde::{Deserialize, Serialize};

use super::http::{HttpMethod, KeyValueRow, RequestDraft};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSummary {
    pub id: String,
    pub name: String,
    pub request_count: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedRequestRecord {
    pub id: String,
    pub collection_id: String,
    pub name: String,
    pub draft: RequestDraft,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionWithRequests {
    pub collection: CollectionSummary,
    pub requests: Vec<SavedRequestRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequestInput {
    pub request_id: Option<String>,
    pub collection_id: String,
    pub draft: RequestDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentRecord {
    pub id: String,
    pub name: String,
    pub variables: Vec<KeyValueRow>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEnvironmentInput {
    pub environment_id: Option<String>,
    pub name: String,
    pub variables: Vec<KeyValueRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMetadata {
    pub alias: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSecretInput {
    pub alias: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub request_name: String,
    pub method: HttpMethod,
    pub url: String,
    pub environment_name: Option<String>,
    pub response_status: Option<u16>,
    pub duration_ms: Option<u64>,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub collections: Vec<CollectionWithRequests>,
    pub environments: Vec<EnvironmentRecord>,
    pub history: Vec<HistoryEntry>,
    pub secrets: Vec<SecretMetadata>,
}
