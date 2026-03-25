use std::{error::Error as StdError, ffi::OsStr, path::Path, time::{Duration, Instant}};

use chrono::Utc;
use reqwest::{multipart::{Form, Part}, Method};

use crate::{
    app::errors::{AppError, AppResult},
    domain::http::{
        HttpMethod, ResolvedBody, ResolvedFormDataField, ResolvedPair, ResolvedRequest,
        ResponseEnvelope,
    },
};

pub async fn execute_request(
    client: &reqwest::Client,
    request: ResolvedRequest,
) -> AppResult<ResponseEnvelope> {
    let start = Instant::now();
    let method = to_reqwest_method(&request.method);
    let is_form_data = matches!(request.body, ResolvedBody::FormData { .. });

    let mut builder = client
        .request(method, &request.url)
        .timeout(Duration::from_millis(request.timeout_ms));

    for header in &request.headers {
        if is_form_data && header.key.eq_ignore_ascii_case("content-type") {
            continue;
        }

        builder = builder.header(&header.key, &header.value);
    }

    builder = match request.body.clone() {
        ResolvedBody::None => builder,
        ResolvedBody::Json { text, .. } => builder.body(text),
        ResolvedBody::Text { text } => builder.body(text),
        ResolvedBody::FormData { fields } => {
            let form = build_multipart_form(fields).await?;
            builder.multipart(form)
        }
    };

    let response = builder
        .send()
        .await
        .map_err(|error| normalize_reqwest_error(&request, error))?;

    let status = response.status();
    let status_text = status.canonical_reason().unwrap_or("").to_string();
    let final_url = response.url().to_string();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(key, value)| {
            value.to_str().ok().map(|text| ResolvedPair {
                key: key.to_string(),
                value: text.to_string(),
            })
        })
        .collect::<Vec<_>>();

    let bytes = response
        .bytes()
        .await
        .map_err(|error| normalize_reqwest_error(&request, error))?;
    let body_text = String::from_utf8_lossy(&bytes).to_string();

    Ok(ResponseEnvelope {
        status: status.as_u16(),
        status_text,
        headers,
        body_text,
        duration_ms: start.elapsed().as_millis() as u64,
        size_bytes: bytes.len() as u64,
        final_url,
        received_at: Utc::now().to_rfc3339(),
    })
}

async fn build_multipart_form(fields: Vec<ResolvedFormDataField>) -> AppResult<Form> {
    let mut form = Form::new();

    for field in fields {
        match field {
            ResolvedFormDataField::Text { key, value } => {
                form = form.text(key, value);
            }
            ResolvedFormDataField::File {
                key,
                path,
                file_name,
            } => {
                let bytes = tokio::fs::read(&path).await.map_err(|error| {
                    AppError::Io(format!(
                        "No pude leer el archivo para multipart ({path}): {error}"
                    ))
                })?;
                let fallback_name = Path::new(&path)
                    .file_name()
                    .and_then(OsStr::to_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| "upload.bin".to_string());
                let name = file_name.unwrap_or(fallback_name);
                let content_type = infer_content_type(&name);

                let part = Part::bytes(bytes)
                    .file_name(name)
                    .mime_str(content_type)
                    .map_err(|error| AppError::Http(error.to_string()))?;

                form = form.part(key, part);
            }
        }
    }

    Ok(form)
}

fn infer_content_type(file_name: &str) -> &'static str {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".json") {
        "application/json"
    } else if lower.ends_with(".txt") {
        "text/plain"
    } else if lower.ends_with(".html") {
        "text/html"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".pdf") {
        "application/pdf"
    } else {
        "application/octet-stream"
    }
}

fn normalize_reqwest_error(request: &ResolvedRequest, error: reqwest::Error) -> AppError {
    let url = request.url.clone();
    let source_text = error_chain_text(&error).to_ascii_lowercase();

    if error.is_timeout() {
        AppError::Http(format!(
            "Se agotó el timeout del request hacia {url}. Probá con más timeout o revisá la red."
        ))
    } else if error.is_connect() {
        if source_text.contains("certificate") || source_text.contains("tls") || source_text.contains("ssl") {
            AppError::Http(format!(
                "Falló la negociación TLS/SSL hacia {url}: {error}"
            ))
        } else if source_text.contains("dns")
            || source_text.contains("lookup")
            || source_text.contains("name or service not known")
            || source_text.contains("nodename nor servname")
            || source_text.contains("failed to lookup")
        {
            AppError::Http(format!(
                "No pude resolver el host de {url}. Verificá el dominio o tu DNS."
            ))
        } else {
            AppError::Http(format!(
                "No pude conectar con el servidor ({url}): {error}"
            ))
        }
    } else if error.is_redirect() {
        AppError::Http(format!(
            "La política de redirects rechazó la navegación desde {url}: {error}"
        ))
    } else if error.is_decode() {
        AppError::Http(format!(
            "No pude decodificar la respuesta de {url}: {error}"
        ))
    } else if error.is_request() {
        AppError::Http(format!(
            "El request hacia {url} es inválido o no pudo enviarse: {error}"
        ))
    } else if error.is_body() {
        AppError::Http(format!(
            "No pude transmitir el body del request hacia {url}: {error}"
        ))
    } else {
        AppError::Http(format!("Error HTTP en {url}: {error}"))
    }
}

fn error_chain_text(error: &reqwest::Error) -> String {
    let mut chain = Vec::new();
    let mut current = error.source();

    while let Some(source) = current {
        chain.push(source.to_string());
        current = source.source();
    }

    chain.join(" :: ")
}

fn to_reqwest_method(method: &HttpMethod) -> Method {
    match method {
        HttpMethod::GET => Method::GET,
        HttpMethod::POST => Method::POST,
        HttpMethod::PUT => Method::PUT,
        HttpMethod::PATCH => Method::PATCH,
        HttpMethod::DELETE => Method::DELETE,
        HttpMethod::HEAD => Method::HEAD,
        HttpMethod::OPTIONS => Method::OPTIONS,
    }
}
