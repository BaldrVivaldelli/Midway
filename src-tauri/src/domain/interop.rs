use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app::errors::{AppError, AppResult},
    domain::{
        http::{
            ApiKeyPlacement, AuthConfig, BodyMode, FormDataFieldKind, FormDataRow, KeyValueRow,
            RequestBodyDraft, RequestDraft,
        },
        workspace::{CollectionWithRequests, WorkspaceSnapshot},
    },
};

pub const NATIVE_WORKSPACE_FORMAT_ID: &str = "rust-centric-api-client.workspace";
pub const POSTMAN_COLLECTION_SCHEMA_DRAFT_04: &str =
    "https://schema.getpostman.com/collection/json/v2.1.0/draft-04/collection.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceExportFormat {
    NativeWorkspaceV1,
    PostmanCollectionV21,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceImportFormat {
    Auto,
    NativeWorkspaceV1,
    PostmanCollectionV21,
    OpenApiV3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportWorkspaceInput {
    pub path: String,
    pub format: WorkspaceExportFormat,
    pub collection_id: Option<String>,
    pub include_history: bool,
    pub include_secret_metadata: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportWorkspaceResult {
    pub path: String,
    pub format: WorkspaceExportFormat,
    pub collections_exported: u64,
    pub requests_exported: u64,
    pub environments_exported: u64,
    pub history_exported: u64,
    pub secret_metadata_exported: u64,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWorkspaceInput {
    pub path: String,
    pub format: WorkspaceImportFormat,
    pub merge: bool,
    pub collection_name_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWorkspacePayloadInput {
    pub payload: String,
    pub format: WorkspaceImportFormat,
    pub merge: bool,
    pub collection_name_override: Option<String>,
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWorkspaceResult {
    pub path: String,
    pub detected_format: WorkspaceImportFormat,
    pub collections_imported: u64,
    pub requests_imported: u64,
    pub environments_imported: u64,
    pub history_imported: u64,
    pub secret_metadata_imported: u64,
    pub collection_ids: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeWorkspaceBundle {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub snapshot: WorkspaceSnapshot,
}

#[derive(Debug, Clone)]
pub struct ParsedPostmanCollection {
    pub collection_name: String,
    pub variables: Vec<KeyValueRow>,
    pub requests: Vec<RequestDraft>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedOpenApiCollection {
    pub collection_name: String,
    pub variables: Vec<KeyValueRow>,
    pub requests: Vec<RequestDraft>,
    pub warnings: Vec<String>,
}

pub fn make_native_bundle(
    mut snapshot: WorkspaceSnapshot,
    include_history: bool,
    include_secret_metadata: bool,
) -> NativeWorkspaceBundle {
    if !include_history {
        snapshot.history.clear();
    }

    if !include_secret_metadata {
        snapshot.secrets.clear();
    }

    NativeWorkspaceBundle {
        format: NATIVE_WORKSPACE_FORMAT_ID.to_string(),
        version: 1,
        exported_at: Utc::now().to_rfc3339(),
        snapshot,
    }
}

pub fn parse_native_bundle(value: Value) -> AppResult<NativeWorkspaceBundle> {
    let bundle: NativeWorkspaceBundle = serde_json::from_value(value)
        .map_err(|error| AppError::InvalidJson(error.to_string()))?;

    if bundle.format != NATIVE_WORKSPACE_FORMAT_ID {
        return Err(AppError::Validation(
            "El JSON no corresponde al export nativo de esta app.".to_string(),
        ));
    }

    if bundle.version != 1 {
        return Err(AppError::Validation(format!(
            "Versión de export nativo no soportada: {}",
            bundle.version
        )));
    }

    Ok(bundle)
}

pub fn detect_import_format(
    value: &Value,
    requested: WorkspaceImportFormat,
) -> AppResult<WorkspaceImportFormat> {
    match requested {
        WorkspaceImportFormat::Auto => {
            if is_native_workspace_bundle(value) {
                Ok(WorkspaceImportFormat::NativeWorkspaceV1)
            } else if is_postman_collection(value) {
                Ok(WorkspaceImportFormat::PostmanCollectionV21)
            } else if is_openapi_document(value) {
                Ok(WorkspaceImportFormat::OpenApiV3)
            } else {
                Err(AppError::Validation(
                    "No pude detectar un formato de import compatible. Probé nativo, Postman v2.1 y OpenAPI v3.".to_string(),
                ))
            }
        }
        explicit => Ok(explicit),
    }
}

pub fn is_native_workspace_bundle(value: &Value) -> bool {
    value
        .get("format")
        .and_then(Value::as_str)
        .map(|format| format == NATIVE_WORKSPACE_FORMAT_ID)
        .unwrap_or(false)
}


pub fn is_openapi_document(value: &Value) -> bool {
    value
        .get("openapi")
        .and_then(Value::as_str)
        .map(|version| version.trim_start().starts_with("3."))
        .unwrap_or(false)
}

pub fn parse_json_or_yaml_payload(payload: &str) -> AppResult<Value> {
    if payload.trim().is_empty() {
        return Err(AppError::Validation(
            "Necesitás pegar o leer un payload antes de importar.".to_string(),
        ));
    }

    match serde_json::from_str::<Value>(payload) {
        Ok(value) => Ok(value),
        Err(json_error) => serde_yaml::from_str::<Value>(payload).map_err(|yaml_error| {
            AppError::InvalidJson(format!(
                "No pude parsear el payload ni como JSON ni como YAML. JSON: {} · YAML: {}",
                json_error, yaml_error
            ))
        }),
    }
}

pub fn is_postman_collection(value: &Value) -> bool {
    let info = value.get("info").and_then(Value::as_object);
    let item = value.get("item").and_then(Value::as_array);

    match (info, item) {
        (Some(info), Some(items)) if !items.is_empty() => {
            info.get("schema")
                .and_then(Value::as_str)
                .map(|schema| schema.contains("schema.getpostman.com/collection") || schema.contains("schema.postman.com/collection"))
                .unwrap_or(true)
        }
        _ => false,
    }
}

pub fn export_postman_collection(collection: &CollectionWithRequests) -> Value {
    let items = collection
        .requests
        .iter()
        .map(|record| {
            let raw_url = build_raw_url(&record.draft.url, &record.draft.query);
            let header = record
                .draft
                .headers
                .iter()
                .filter(|row| !row.key.trim().is_empty())
                .map(|row| {
                    json!({
                        "key": row.key,
                        "value": row.value,
                        "disabled": !row.enabled
                    })
                })
                .collect::<Vec<_>>();

            let mut request = json!({
                "method": http_method_text(&record.draft.method),
                "header": header,
                "url": {
                    "raw": raw_url,
                    "query": export_postman_query(&record.draft.query)
                },
                "auth": export_postman_auth(&record.draft.auth)
            });

            if let Some(body) = export_postman_body(&record.draft.body) {
                if let Some(map) = request.as_object_mut() {
                    map.insert("body".to_string(), body);
                }
            }

            json!({
                "name": record.name,
                "request": request
            })
        })
        .collect::<Vec<_>>();

    json!({
        "info": {
            "name": collection.collection.name,
            "_postman_id": collection.collection.id,
            "schema": POSTMAN_COLLECTION_SCHEMA_DRAFT_04
        },
        "item": items
    })
}

pub fn import_postman_collection(value: &Value) -> AppResult<ParsedPostmanCollection> {
    if !is_postman_collection(value) {
        return Err(AppError::Validation(
            "El archivo no parece una collection Postman v2.1.".to_string(),
        ));
    }

    let collection_name = value
        .get("info")
        .and_then(|info| info.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Imported Postman Collection")
        .to_string();

    let variables = value
        .get("variable")
        .and_then(Value::as_array)
        .map(|items| parse_postman_variables(items))
        .unwrap_or_default();

    let mut warnings = Vec::new();
    let mut requests = Vec::new();

    if let Some(items) = value.get("item").and_then(Value::as_array) {
        collect_postman_items(items, &mut Vec::new(), &mut requests, &mut warnings)?;
    }

    if requests.is_empty() {
        warnings.push(
            "La collection se importó pero no encontré requests compatibles para convertir."
                .to_string(),
        );
    }

    Ok(ParsedPostmanCollection {
        collection_name,
        variables,
        requests,
        warnings,
    })
}

fn collect_postman_items(
    items: &[Value],
    path: &mut Vec<String>,
    requests: &mut Vec<RequestDraft>,
    warnings: &mut Vec<String>,
) -> AppResult<()> {
    for item in items {
        if let Some(children) = item.get("item").and_then(Value::as_array) {
            let folder_name = item
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or("Folder")
                .to_string();
            path.push(folder_name);
            collect_postman_items(children, path, requests, warnings)?;
            let _ = path.pop();
            continue;
        }

        if item.get("request").is_some() {
            requests.push(parse_postman_request_item(item, path, warnings)?);
        }
    }

    Ok(())
}

fn parse_postman_request_item(
    item: &Value,
    path: &[String],
    warnings: &mut Vec<String>,
) -> AppResult<RequestDraft> {
    let request = item
        .get("request")
        .ok_or_else(|| AppError::Validation("El item Postman no tiene request.".to_string()))?;

    let item_name = item
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Imported request")
        .to_string();

    let full_name = if path.is_empty() {
        item_name
    } else {
        format!("{} / {}", path.join(" / "), item_name)
    };

    let method = request
        .get("method")
        .and_then(Value::as_str)
        .map(parse_http_method)
        .unwrap_or(crate::domain::http::HttpMethod::GET);

    let (url, query) = extract_postman_url(request.get("url"));
    let headers = request
        .get("header")
        .and_then(Value::as_array)
        .map(|items| parse_postman_headers(items))
        .unwrap_or_default();
    let auth = parse_postman_auth(request.get("auth"));
    let body = parse_postman_body(request.get("body"), warnings);

    if request.get("event").is_some() || item.get("event").is_some() {
        warnings.push(format!(
            "{}: los scripts/tests de Postman no se convierten automáticamente a responseTests.",
            full_name
        ));
    }

    Ok(RequestDraft {
        id: None,
        name: full_name,
        method,
        url,
        query,
        headers,
        auth,
        body,
        timeout_ms: 30000,
        environment_id: None,
        response_tests: Vec::new(),
    })
}

fn parse_postman_variables(items: &[Value]) -> Vec<KeyValueRow> {
    items
        .iter()
        .filter_map(|item| {
            let key = item.get("key").and_then(Value::as_str)?.trim().to_string();
            if key.is_empty() {
                return None;
            }

            let value = value_as_string(item.get("value")).unwrap_or_default();
            let enabled = !item
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);

            Some(KeyValueRow {
                id: Uuid::new_v4().to_string(),
                key,
                value,
                enabled,
            })
        })
        .collect()
}

fn parse_postman_headers(items: &[Value]) -> Vec<KeyValueRow> {
    items
        .iter()
        .filter_map(|item| {
            if item.is_string() {
                return None;
            }

            let key = item.get("key").and_then(Value::as_str)?.trim().to_string();
            if key.is_empty() {
                return None;
            }

            Some(KeyValueRow {
                id: Uuid::new_v4().to_string(),
                key,
                value: value_as_string(item.get("value")).unwrap_or_default(),
                enabled: !item
                    .get("disabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            })
        })
        .collect()
}

fn parse_postman_auth(value: Option<&Value>) -> AuthConfig {
    let Some(value) = value else {
        return AuthConfig::None;
    };

    let Some(auth_type) = value.get("type").and_then(Value::as_str) else {
        return AuthConfig::None;
    };

    match auth_type {
        "bearer" => AuthConfig::Bearer {
            token: auth_attr(value.get("bearer"), "token").unwrap_or_default(),
        },
        "basic" => AuthConfig::Basic {
            username: auth_attr(value.get("basic"), "username").unwrap_or_default(),
            password: auth_attr(value.get("basic"), "password").unwrap_or_default(),
        },
        "apikey" => AuthConfig::ApiKey {
            key: auth_attr(value.get("apikey"), "key").unwrap_or_default(),
            value: auth_attr(value.get("apikey"), "value").unwrap_or_default(),
            placement: match auth_attr(value.get("apikey"), "in")
                .unwrap_or_else(|| "header".to_string())
                .as_str()
            {
                "query" => ApiKeyPlacement::Query,
                _ => ApiKeyPlacement::Header,
            },
        },
        _ => AuthConfig::None,
    }
}

fn parse_postman_body(value: Option<&Value>, warnings: &mut Vec<String>) -> RequestBodyDraft {
    let Some(body) = value else {
        return RequestBodyDraft {
            mode: BodyMode::None,
            value: String::new(),
            form_data: Vec::new(),
        };
    };

    let Some(mode) = body.get("mode").and_then(Value::as_str) else {
        return RequestBodyDraft {
            mode: BodyMode::None,
            value: String::new(),
            form_data: Vec::new(),
        };
    };

    match mode {
        "raw" => {
            let raw = body
                .get("raw")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let language = body
                .get("options")
                .and_then(|options| options.get("raw"))
                .and_then(|raw| raw.get("language"))
                .and_then(Value::as_str)
                .unwrap_or_default();

            RequestBodyDraft {
                mode: if language.eq_ignore_ascii_case("json") {
                    BodyMode::Json
                } else {
                    BodyMode::Text
                },
                value: raw,
                form_data: Vec::new(),
            }
        }
        "formdata" => {
            let rows = body
                .get("formdata")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            let key = value_as_string(item.get("key"))?;
                            if key.trim().is_empty() {
                                return None;
                            }
                            let kind = match value_as_string(item.get("type"))
                                .unwrap_or_else(|| "text".to_string())
                                .as_str()
                            {
                                "file" => FormDataFieldKind::File,
                                _ => FormDataFieldKind::Text,
                            };
                            let value = if matches!(kind, FormDataFieldKind::File) {
                                value_as_string(item.get("src")).unwrap_or_default()
                            } else {
                                value_as_string(item.get("value")).unwrap_or_default()
                            };
                            Some(FormDataRow {
                                id: Uuid::new_v4().to_string(),
                                key,
                                value,
                                enabled: !item
                                    .get("disabled")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                                kind,
                                file_name: None,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            RequestBodyDraft {
                mode: BodyMode::FormData,
                value: String::new(),
                form_data: rows,
            }
        }
        unsupported => {
            warnings.push(format!(
                "Encontré un body Postman con modo '{}' y lo importé como vacío.",
                unsupported
            ));
            RequestBodyDraft {
                mode: BodyMode::None,
                value: String::new(),
                form_data: Vec::new(),
            }
        }
    }
}

fn export_postman_auth(auth: &AuthConfig) -> Value {
    match auth {
        AuthConfig::None => json!({ "type": "noauth", "noauth": [] }),
        AuthConfig::Bearer { token } => json!({
            "type": "bearer",
            "bearer": [
                { "key": "token", "value": token, "type": "string" }
            ]
        }),
        AuthConfig::Basic { username, password } => json!({
            "type": "basic",
            "basic": [
                { "key": "username", "value": username, "type": "string" },
                { "key": "password", "value": password, "type": "string" }
            ]
        }),
        AuthConfig::ApiKey {
            key,
            value,
            placement,
        } => json!({
            "type": "apikey",
            "apikey": [
                { "key": "key", "value": key, "type": "string" },
                { "key": "value", "value": value, "type": "string" },
                { "key": "in", "value": match placement {
                    ApiKeyPlacement::Header => "header",
                    ApiKeyPlacement::Query => "query"
                }, "type": "string" }
            ]
        }),
    }
}

fn export_postman_body(body: &RequestBodyDraft) -> Option<Value> {
    match body.mode {
        BodyMode::None => None,
        BodyMode::Json => Some(json!({
            "mode": "raw",
            "raw": body.value,
            "options": {
                "raw": { "language": "json" }
            }
        })),
        BodyMode::Text => Some(json!({
            "mode": "raw",
            "raw": body.value
        })),
        BodyMode::FormData => Some(json!({
            "mode": "formdata",
            "formdata": body.form_data.iter().map(|row| json!({
                "key": row.key,
                "value": row.value,
                "disabled": !row.enabled,
                "type": match row.kind {
                    FormDataFieldKind::Text => "text",
                    FormDataFieldKind::File => "file"
                },
                "src": row.value
            })).collect::<Vec<_>>()
        })),
    }
}

fn export_postman_query(query: &[KeyValueRow]) -> Vec<Value> {
    query
        .iter()
        .filter(|row| !row.key.trim().is_empty())
        .map(|row| {
            json!({
                "key": row.key,
                "value": row.value,
                "disabled": !row.enabled
            })
        })
        .collect()
}

fn build_raw_url(base_url: &str, query: &[KeyValueRow]) -> String {
    let enabled = query
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| format!("{}={}", row.key, row.value))
        .collect::<Vec<_>>();

    if enabled.is_empty() {
        base_url.to_string()
    } else if base_url.contains('?') {
        format!("{}&{}", base_url, enabled.join("&"))
    } else {
        format!("{}?{}", base_url, enabled.join("&"))
    }
}

fn extract_postman_url(value: Option<&Value>) -> (String, Vec<KeyValueRow>) {
    let Some(value) = value else {
        return (String::new(), Vec::new());
    };

    if let Some(raw) = value.as_str() {
        return split_raw_url(raw);
    }

    let Some(object) = value.as_object() else {
        return (String::new(), Vec::new());
    };

    let query = object
        .get("query")
        .and_then(Value::as_array)
        .map(|items| parse_postman_variables(items))
        .unwrap_or_default();

    if let Some(raw) = object.get("raw").and_then(Value::as_str) {
        if query.is_empty() {
            return split_raw_url(raw);
        }

        let (base_url, _) = split_raw_url(raw);
        return (base_url, query);
    }

    let protocol = object
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("https");
    let host = match object.get("host") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("."),
        Some(Value::String(text)) => text.to_string(),
        _ => String::new(),
    };
    let path = match object.get("path") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("/"),
        Some(Value::String(text)) => text.to_string(),
        _ => String::new(),
    };

    let base_url = if host.is_empty() {
        String::new()
    } else if path.is_empty() {
        format!("{}://{}", protocol, host)
    } else {
        format!("{}://{}/{}", protocol, host, path)
    };

    (base_url, query)
}

fn split_raw_url(raw: &str) -> (String, Vec<KeyValueRow>) {
    let trimmed = raw.trim();
    if let Some((base, rest)) = trimmed.split_once('?') {
        let query_part = rest.split('#').next().unwrap_or(rest);
        let query = query_part
            .split('&')
            .filter(|pair| !pair.trim().is_empty())
            .map(|pair| {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                KeyValueRow {
                    id: Uuid::new_v4().to_string(),
                    key: key.to_string(),
                    value: value.to_string(),
                    enabled: true,
                }
            })
            .collect::<Vec<_>>();
        (base.to_string(), query)
    } else {
        (trimmed.to_string(), Vec::new())
    }
}

fn auth_attr(value: Option<&Value>, key: &str) -> Option<String> {
    value
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                let matches = item
                    .get("key")
                    .and_then(Value::as_str)
                    .map(|candidate| candidate == key)
                    .unwrap_or(false);

                if matches {
                    value_as_string(item.get("value"))
                } else {
                    None
                }
            })
        })
}

fn value_as_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.to_string()),
        Some(Value::Null) | None => None,
        Some(other) => Some(other.to_string()),
    }
}

fn parse_http_method(value: &str) -> crate::domain::http::HttpMethod {
    match value.to_ascii_uppercase().as_str() {
        "POST" => crate::domain::http::HttpMethod::POST,
        "PUT" => crate::domain::http::HttpMethod::PUT,
        "PATCH" => crate::domain::http::HttpMethod::PATCH,
        "DELETE" => crate::domain::http::HttpMethod::DELETE,
        "HEAD" => crate::domain::http::HttpMethod::HEAD,
        "OPTIONS" => crate::domain::http::HttpMethod::OPTIONS,
        _ => crate::domain::http::HttpMethod::GET,
    }
}

fn http_method_text(method: &crate::domain::http::HttpMethod) -> &'static str {
    match method {
        crate::domain::http::HttpMethod::GET => "GET",
        crate::domain::http::HttpMethod::POST => "POST",
        crate::domain::http::HttpMethod::PUT => "PUT",
        crate::domain::http::HttpMethod::PATCH => "PATCH",
        crate::domain::http::HttpMethod::DELETE => "DELETE",
        crate::domain::http::HttpMethod::HEAD => "HEAD",
        crate::domain::http::HttpMethod::OPTIONS => "OPTIONS",
    }
}

pub fn import_openapi_document(value: &Value) -> AppResult<ParsedOpenApiCollection> {
    if !is_openapi_document(value) {
        return Err(AppError::Validation(
            "El payload no parece un documento OpenAPI v3.".to_string(),
        ));
    }

    let collection_name = value
        .get("info")
        .and_then(|info| info.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or("Imported OpenAPI Collection")
        .to_string();

    let mut warnings = Vec::new();
    let mut environment_variables = Vec::<KeyValueRow>::new();

    let base_url = value
        .get("servers")
        .and_then(Value::as_array)
        .and_then(|servers| servers.first())
        .and_then(|server| server.get("url"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or("https://api.example.com")
        .to_string();

    if value
        .get("servers")
        .and_then(Value::as_array)
        .map(|servers| servers.is_empty())
        .unwrap_or(true)
    {
        warnings.push(
            "El spec no declara servers. Creé una variable base_url con https://api.example.com para que la ajustes antes de enviar.".to_string(),
        );
    }

    environment_variables.push(create_env_row("base_url", &base_url));

    let global_security = value.get("security").and_then(Value::as_array);
    let security_schemes = value
        .get("components")
        .and_then(|components| components.get("securitySchemes"));

    let mut requests = Vec::new();
    if let Some(paths) = value.get("paths").and_then(Value::as_object) {
        for (path_name, path_item) in paths {
            let Some(path_object) = path_item.as_object() else {
                continue;
            };

            let path_parameters = collect_openapi_parameters(
                path_object.get("parameters").and_then(Value::as_array),
                value,
                &mut warnings,
            );

            for method_name in ["get", "post", "put", "patch", "delete", "head", "options"] {
                let Some(operation) = path_object.get(method_name) else {
                    continue;
                };
                let Some(operation_object) = operation.as_object() else {
                    continue;
                };

                let method = parse_http_method(method_name);
                let operation_parameters = collect_openapi_parameters(
                    operation_object.get("parameters").and_then(Value::as_array),
                    value,
                    &mut warnings,
                );
                let all_parameters = merge_openapi_parameters(path_parameters.clone(), operation_parameters);

                let mut query = Vec::new();
                let mut headers = Vec::new();
                let mut path_template = path_name.clone();

                for parameter in &all_parameters {
                    match parameter.location.as_str() {
                        "query" => query.push(create_row_with_value(
                            &parameter.name,
                            parameter.default_value.as_deref().unwrap_or_default(),
                        )),
                        "header" => headers.push(create_row_with_value(
                            &parameter.name,
                            parameter.default_value.as_deref().unwrap_or_default(),
                        )),
                        "path" => {
                            let placeholder = format!("{{{{{}}}}}", parameter.name);
                            path_template = path_template.replace(
                                &format!("{{{}}}", parameter.name),
                                &placeholder,
                            );
                            if !environment_variables.iter().any(|row| row.key == parameter.name) {
                                environment_variables.push(create_env_row(
                                    &parameter.name,
                                    parameter.default_value.as_deref().unwrap_or(""),
                                ));
                            }
                        }
                        _ => {}
                    }
                }

                let name = operation_object
                    .get("operationId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        operation_object
                            .get("summary")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                    })
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{} {}", method_name.to_ascii_uppercase(), path_name));

                let mut auth = AuthConfig::None;
                let security = operation_object
                    .get("security")
                    .and_then(Value::as_array)
                    .or(global_security);
                if let Some(config) = parse_openapi_security(security, security_schemes, &mut warnings) {
                    auth = config;
                }

                let body = parse_openapi_request_body(
                    operation_object.get("requestBody"),
                    value,
                    &mut warnings,
                    &mut headers,
                );

                requests.push(RequestDraft {
                    id: None,
                    name,
                    method,
                    url: format!("{{{{base_url}}}}{}", path_template),
                    query,
                    headers,
                    auth,
                    body,
                    timeout_ms: 30000,
                    environment_id: None,
                    response_tests: Vec::new(),
                });
            }
        }
    }

    if requests.is_empty() {
        warnings.push(
            "El spec se importó pero no encontré operaciones HTTP compatibles dentro de paths."
                .to_string(),
        );
    }

    Ok(ParsedOpenApiCollection {
        collection_name,
        variables: dedupe_rows(environment_variables),
        requests,
        warnings,
    })
}

#[derive(Debug, Clone)]
struct OpenApiParameter {
    name: String,
    location: String,
    default_value: Option<String>,
}

fn collect_openapi_parameters(
    items: Option<&Vec<Value>>,
    root: &Value,
    warnings: &mut Vec<String>,
) -> Vec<OpenApiParameter> {
    items
        .map(|items| {
            items
                .iter()
                .filter_map(|item| resolve_openapi_ref(item, root))
                .filter_map(|item| parse_openapi_parameter(&item, root, warnings))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn merge_openapi_parameters(
    base: Vec<OpenApiParameter>,
    override_params: Vec<OpenApiParameter>,
) -> Vec<OpenApiParameter> {
    let mut merged = base;
    for parameter in override_params {
        if let Some(index) = merged.iter().position(|existing| {
            existing.name == parameter.name && existing.location == parameter.location
        }) {
            merged[index] = parameter;
        } else {
            merged.push(parameter);
        }
    }
    merged
}

fn parse_openapi_parameter(
    value: &Value,
    root: &Value,
    _warnings: &mut Vec<String>,
) -> Option<OpenApiParameter> {
    let name = value.get("name").and_then(Value::as_str)?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let location = value.get("in").and_then(Value::as_str)?.trim().to_string();
    let schema = value.get("schema").and_then(|schema| resolve_openapi_ref(schema, root));
    let default_value = value
        .get("example")
        .and_then(|value| value_to_string_scalar(Some(value)))
        .or_else(|| {
            schema
                .as_ref()
                .and_then(|schema| schema.get("example"))
                .and_then(|value| value_to_string_scalar(Some(value)))
        })
        .or_else(|| {
            schema
                .as_ref()
                .and_then(|schema| schema.get("default"))
                .and_then(|value| value_to_string_scalar(Some(value)))
        });

    Some(OpenApiParameter {
        name,
        location,
        default_value,
    })
}

fn parse_openapi_security(
    security: Option<&Vec<Value>>,
    security_schemes: Option<&Value>,
    warnings: &mut Vec<String>,
) -> Option<AuthConfig> {
    let Some(requirements) = security else {
        return None;
    };

    let Some(first_requirement) = requirements.iter().find_map(Value::as_object) else {
        return None;
    };
    let Some((scheme_name, _)) = first_requirement.iter().next() else {
        return None;
    };
    let scheme = security_schemes
        .and_then(|value| value.get(scheme_name))
        .and_then(Value::as_object)?;

    match scheme.get("type").and_then(Value::as_str).unwrap_or_default() {
        "http" => match scheme.get("scheme").and_then(Value::as_str).unwrap_or_default() {
            "bearer" => Some(AuthConfig::Bearer {
                token: "{{secret:bearer_token}}".to_string(),
            }),
            "basic" => Some(AuthConfig::Basic {
                username: "{{basic_user}}".to_string(),
                password: "{{secret:basic_password}}".to_string(),
            }),
            other => {
                warnings.push(format!(
                    "Security scheme HTTP '{}' no soportado; la operación se importó sin auth.",
                    other
                ));
                None
            }
        },
        "apiKey" => Some(AuthConfig::ApiKey {
            key: scheme
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("x-api-key")
                .to_string(),
            value: "{{secret:api_key}}".to_string(),
            placement: match scheme.get("in").and_then(Value::as_str).unwrap_or("header") {
                "query" => ApiKeyPlacement::Query,
                _ => ApiKeyPlacement::Header,
            },
        }),
        other => {
            warnings.push(format!(
                "Security scheme '{}' no soportado todavía; la operación se importó sin auth.",
                other
            ));
            None
        }
    }
}

fn parse_openapi_request_body(
    value: Option<&Value>,
    root: &Value,
    warnings: &mut Vec<String>,
    headers: &mut Vec<KeyValueRow>,
) -> RequestBodyDraft {
    let Some(value) = value.and_then(|body| resolve_openapi_ref(body, root)) else {
        return RequestBodyDraft {
            mode: BodyMode::None,
            value: String::new(),
            form_data: Vec::new(),
        };
    };

    let Some(content) = value.get("content").and_then(Value::as_object) else {
        return RequestBodyDraft {
            mode: BodyMode::None,
            value: String::new(),
            form_data: Vec::new(),
        };
    };

    let preferred_type = if content.contains_key("application/json") {
        Some("application/json")
    } else if content.contains_key("multipart/form-data") {
        Some("multipart/form-data")
    } else if content.contains_key("text/plain") {
        Some("text/plain")
    } else if content.contains_key("application/x-www-form-urlencoded") {
        Some("application/x-www-form-urlencoded")
    } else {
        content.keys().next().map(String::as_str)
    };

    let Some(content_type) = preferred_type else {
        return RequestBodyDraft {
            mode: BodyMode::None,
            value: String::new(),
            form_data: Vec::new(),
        };
    };

    let media = content.get(content_type).and_then(|item| resolve_openapi_ref(item, root));
    let schema = media
        .as_ref()
        .and_then(|media| media.get("schema"))
        .and_then(|schema| resolve_openapi_ref(schema, root));
    let explicit_example = media
        .as_ref()
        .and_then(|media| media.get("example"))
        .cloned();

    match content_type {
        "application/json" => {
            if !headers.iter().any(|row| row.key.eq_ignore_ascii_case("content-type")) {
                headers.push(create_row_with_value("content-type", "application/json"));
            }
            let example = explicit_example
                .or_else(|| schema.as_ref().map(|schema| generate_openapi_example(schema, root)))
                .unwrap_or_else(|| json!({}));
            let value = serde_json::to_string_pretty(&example).unwrap_or_else(|_| "{}".to_string());
            RequestBodyDraft {
                mode: BodyMode::Json,
                value,
                form_data: Vec::new(),
            }
        }
        "multipart/form-data" => {
            let rows = schema
                .as_ref()
                .and_then(|schema| schema.get("properties"))
                .and_then(Value::as_object)
                .map(|properties| {
                    properties
                        .iter()
                        .map(|(key, property)| {
                            let resolved_property = resolve_openapi_ref(property, root)
                                .unwrap_or_else(|| property.clone());
                            let field_kind = if resolved_property
                                .get("format")
                                .and_then(Value::as_str)
                                .map(|format| format == "binary")
                                .unwrap_or(false)
                            {
                                FormDataFieldKind::File
                            } else {
                                FormDataFieldKind::Text
                            };
                            let value = if matches!(field_kind, FormDataFieldKind::File) {
                                "".to_string()
                            } else {
                                value_to_string_scalar(resolved_property.get("example"))
                                    .or_else(|| value_to_string_scalar(resolved_property.get("default")))
                                    .unwrap_or_default()
                            };
                            FormDataRow {
                                id: Uuid::new_v4().to_string(),
                                key: key.clone(),
                                value,
                                enabled: true,
                                kind: field_kind,
                                file_name: None,
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            RequestBodyDraft {
                mode: BodyMode::FormData,
                value: String::new(),
                form_data: rows,
            }
        }
        "application/x-www-form-urlencoded" => {
            if !headers.iter().any(|row| row.key.eq_ignore_ascii_case("content-type")) {
                headers.push(create_row_with_value(
                    "content-type",
                    "application/x-www-form-urlencoded",
                ));
            }
            let example = schema
                .as_ref()
                .map(|schema| generate_openapi_example(schema, root))
                .unwrap_or_else(|| json!({}));
            let encoded = example
                .as_object()
                .map(|map| {
                    map.iter()
                        .map(|(key, value)| format!("{}={}", key, value_to_string_scalar(Some(value)).unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join("&")
                })
                .unwrap_or_default();
            RequestBodyDraft {
                mode: BodyMode::Text,
                value: encoded,
                form_data: Vec::new(),
            }
        }
        "text/plain" => RequestBodyDraft {
            mode: BodyMode::Text,
            value: explicit_example
                .and_then(|value| value_to_string_scalar(Some(&value)))
                .or_else(|| schema.as_ref().map(|schema| generate_openapi_example(schema, root).to_string()))
                .unwrap_or_default(),
            form_data: Vec::new(),
        },
        other => {
            warnings.push(format!(
                "Content-Type '{}' importado como texto plano para que puedas ajustarlo manualmente.",
                other
            ));
            RequestBodyDraft {
                mode: BodyMode::Text,
                value: explicit_example
                    .map(|value| value.to_string())
                    .or_else(|| schema.as_ref().map(|schema| generate_openapi_example(schema, root).to_string()))
                    .unwrap_or_default(),
                form_data: Vec::new(),
            }
        }
    }
}

fn resolve_openapi_ref(value: &Value, root: &Value) -> Option<Value> {
    let reference = value.get("$ref").and_then(Value::as_str)?;
    if !reference.starts_with("#/") {
        return Some(value.clone());
    }

    let mut current = root;
    for segment in reference.trim_start_matches("#/").split('/') {
        let segment = segment.replace("~1", "/").replace("~0", "~");
        current = current.get(&segment)?;
    }

    Some(current.clone())
}

fn generate_openapi_example(schema: &Value, root: &Value) -> Value {
    let schema = resolve_openapi_ref(schema, root).unwrap_or_else(|| schema.clone());

    if let Some(example) = schema.get("example") {
        return example.clone();
    }
    if let Some(default) = schema.get("default") {
        return default.clone();
    }
    if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
        if let Some(first) = enum_values.first() {
            return first.clone();
        }
    }
    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        let mut combined = serde_json::Map::new();
        for item in all_of {
            let value = generate_openapi_example(item, root);
            if let Some(object) = value.as_object() {
                for (key, item) in object {
                    combined.insert(key.clone(), item.clone());
                }
            }
        }
        if !combined.is_empty() {
            return Value::Object(combined);
        }
    }

    let schema_type = schema
        .get("type")
        .and_then(|value| match value {
            Value::String(text) => Some(text.to_string()),
            Value::Array(items) => items.iter().find_map(Value::as_str).map(str::to_string),
            _ => None,
        })
        .unwrap_or_else(|| {
            if schema.get("properties").is_some() {
                "object".to_string()
            } else {
                "string".to_string()
            }
        });

    match schema_type.as_str() {
        "object" => {
            let mut object = serde_json::Map::new();
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                for (key, property) in properties {
                    object.insert(key.clone(), generate_openapi_example(property, root));
                }
            }
            Value::Object(object)
        }
        "array" => {
            let item = schema
                .get("items")
                .map(|items| generate_openapi_example(items, root))
                .unwrap_or(Value::String("item".to_string()));
            Value::Array(vec![item])
        }
        "integer" => json!(1),
        "number" => json!(1.0),
        "boolean" => json!(true),
        _ => Value::String(
            schema
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("value")
                .to_string(),
        ),
    }
}

fn dedupe_rows(rows: Vec<KeyValueRow>) -> Vec<KeyValueRow> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduped = Vec::new();
    for row in rows {
        if seen.insert(row.key.clone()) {
            deduped.push(row);
        }
    }
    deduped
}

fn create_row_with_value(key: &str, value: &str) -> KeyValueRow {
    KeyValueRow {
        id: Uuid::new_v4().to_string(),
        key: key.to_string(),
        value: value.to_string(),
        enabled: true,
    }
}

fn create_env_row(key: &str, value: &str) -> KeyValueRow {
    create_row_with_value(key, value)
}

fn value_to_string_scalar(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(boolean)) => Some(boolean.to_string()),
        Some(Value::Null) | None => None,
        Some(other) => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_openapi_documents() {
        let value = json!({
            "openapi": "3.0.3",
            "info": { "title": "Demo" },
            "paths": {}
        });
        assert!(is_openapi_document(&value));
        assert_eq!(
            detect_import_format(&value, WorkspaceImportFormat::Auto).unwrap(),
            WorkspaceImportFormat::OpenApiV3
        );
    }

    #[test]
    fn imports_openapi_paths_into_requests() {
        let value = json!({
            "openapi": "3.0.3",
            "info": { "title": "Users API" },
            "servers": [{ "url": "https://api.example.com" }],
            "paths": {
                "/users/{id}": {
                    "get": {
                        "operationId": "getUser",
                        "parameters": [
                            { "name": "id", "in": "path", "required": true, "schema": { "type": "string", "example": "123" } },
                            { "name": "includePosts", "in": "query", "schema": { "type": "boolean", "default": true } }
                        ]
                    },
                    "post": {
                        "summary": "Create user",
                        "requestBody": {
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string", "example": "Jane" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let parsed = import_openapi_document(&value).unwrap();
        assert_eq!(parsed.collection_name, "Users API");
        assert_eq!(parsed.requests.len(), 2);
        assert!(parsed.variables.iter().any(|row| row.key == "base_url"));
        assert!(parsed.requests.iter().any(|request| request.url.contains("{{id}}")));
    }
}
