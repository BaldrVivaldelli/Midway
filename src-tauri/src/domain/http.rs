use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::testing::ResponseAssertion;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    HEAD,
    OPTIONS,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KeyValueRow {
    pub id: String,
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BodyMode {
    None,
    Json,
    Text,
    FormData,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FormDataFieldKind {
    Text,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormDataRow {
    pub id: String,
    pub key: String,
    pub value: String,
    pub enabled: bool,
    pub kind: FormDataFieldKind,
    #[serde(default)]
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestBodyDraft {
    pub mode: BodyMode,
    pub value: String,
    #[serde(default)]
    pub form_data: Vec<FormDataRow>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApiKeyPlacement {
    Header,
    Query,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AuthConfig {
    None,
    Bearer { token: String },
    Basic { username: String, password: String },
    ApiKey {
        key: String,
        value: String,
        placement: ApiKeyPlacement,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestDraft {
    pub id: Option<String>,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub query: Vec<KeyValueRow>,
    pub headers: Vec<KeyValueRow>,
    pub auth: AuthConfig,
    pub body: RequestBodyDraft,
    pub timeout_ms: u64,
    pub environment_id: Option<String>,
    #[serde(default)]
    pub response_tests: Vec<ResponseAssertion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPair {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub enum ResolvedFormDataField {
    Text { key: String, value: String },
    File {
        key: String,
        path: String,
        file_name: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum ResolvedBody {
    None,
    Json {
        text: String,
        value: serde_json::Value,
    },
    Text {
        text: String,
    },
    FormData {
        fields: Vec<ResolvedFormDataField>,
    },
}

#[derive(Debug, Clone)]
pub struct ResolvedRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<ResolvedPair>,
    pub body: ResolvedBody,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct Resolution {
    pub request: ResolvedRequest,
    pub applied_environment: BTreeMap<String, String>,
    pub environment_name: Option<String>,
    pub used_secret_aliases: Vec<String>,
    pub missing_secret_aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPreview {
    pub method: HttpMethod,
    pub resolved_url: String,
    pub headers: Vec<ResolvedPair>,
    pub body_text: Option<String>,
    pub curl_command: String,
    pub environment_name: Option<String>,
    pub used_secret_aliases: Vec<String>,
    pub missing_secret_aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseEnvelope {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<ResolvedPair>,
    pub body_text: String,
    pub duration_ms: u64,
    pub size_bytes: u64,
    pub final_url: String,
    pub received_at: String,
}
