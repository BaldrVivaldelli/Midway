use serde::{Deserialize, Serialize};

use super::{http::ResponseEnvelope, testing::AssertionReport};

pub const COLLECTION_RUN_PROGRESS_EVENT: &str = "collection-run-progress";

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CollectionRunPhase {
    Started,
    RequestStarted,
    RequestFinished,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRunProgressEvent {
    pub run_id: String,
    pub phase: CollectionRunPhase,
    pub collection_id: String,
    pub collection_name: String,
    pub total_requests: u64,
    pub processed_requests: u64,
    pub current_index: u64,
    pub completed_requests: u64,
    pub errored_requests: u64,
    pub passed_assertions: u64,
    pub failed_assertions: u64,
    pub request_id: Option<String>,
    pub request_name: Option<String>,
    pub environment_name: Option<String>,
    pub resolved_url: Option<String>,
    pub response_status: Option<u16>,
    pub duration_ms: Option<u64>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub emitted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestExecutionOutcome {
    pub response: ResponseEnvelope,
    pub assertion_report: AssertionReport,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequestResult {
    pub execution_id: String,
    pub canceled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCollectionInput {
    pub collection_id: String,
    pub environment_override_id: Option<String>,
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRunItem {
    pub request_id: String,
    pub request_name: String,
    pub environment_name: Option<String>,
    pub resolved_url: Option<String>,
    pub response_status: Option<u16>,
    pub duration_ms: Option<u64>,
    pub error_message: Option<String>,
    pub assertion_report: Option<AssertionReport>,
    pub executed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRunReport {
    pub collection_id: String,
    pub collection_name: String,
    pub started_at: String,
    pub finished_at: String,
    pub total_requests: u64,
    pub completed_requests: u64,
    pub errored_requests: u64,
    pub passed_assertions: u64,
    pub failed_assertions: u64,
    pub items: Vec<CollectionRunItem>,
}
