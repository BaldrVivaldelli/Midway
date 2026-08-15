use crate::domain::http::{RequestPreview, Resolution, ResolvedBody, ResolvedFormDataField};

pub fn make_preview(resolution: Resolution) -> RequestPreview {
    let body_text = match &resolution.request.body {
        ResolvedBody::None => None,
        ResolvedBody::Json { text, .. } => Some(text.clone()),
        ResolvedBody::Text { text } => Some(text.clone()),
        ResolvedBody::FormData { fields } => Some(
            fields
                .iter()
                .map(|field| match field {
                    ResolvedFormDataField::Text { key, value } => format!("{key}={value}"),
                    ResolvedFormDataField::File { key, path, .. } => {
                        format!("{key}=@{path}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    };

    let curl_command = make_curl_command(
        &resolution.request.method.to_string(),
        &resolution.request.url,
        &resolution.request.headers,
        &resolution.request.body,
    );

    RequestPreview {
        method: resolution.request.method,
        resolved_url: resolution.request.url,
        headers: resolution.request.headers,
        body_text,
        curl_command,
        environment_name: resolution.environment_name,
        used_secret_aliases: resolution.used_secret_aliases,
        missing_secret_aliases: resolution.missing_secret_aliases,
    }
}

fn make_curl_command(
    method: &str,
    url: &str,
    headers: &[crate::domain::http::ResolvedPair],
    body: &ResolvedBody,
) -> String {
    let mut parts = vec![
        "curl".to_string(),
        "-X".to_string(),
        method.to_string(),
        shell_escape(url),
    ];

    for header in headers {
        if matches!(body, ResolvedBody::FormData { .. })
            && header.key.eq_ignore_ascii_case("content-type")
        {
            continue;
        }

        parts.push("-H".to_string());
        parts.push(shell_escape(&format!("{}: {}", header.key, header.value)));
    }

    match body {
        ResolvedBody::None => {}
        ResolvedBody::Json { text, .. } | ResolvedBody::Text { text } => {
            parts.push("--data-binary".to_string());
            parts.push(shell_escape(text));
        }
        ResolvedBody::FormData { fields } => {
            for field in fields {
                parts.push("-F".to_string());
                match field {
                    ResolvedFormDataField::Text { key, value } => {
                        parts.push(shell_escape(&format!("{key}={value}")));
                    }
                    ResolvedFormDataField::File {
                        key,
                        path,
                        file_name,
                    } => {
                        let file_segment = if let Some(file_name) = file_name {
                            format!("{key}=@{path};filename={file_name}")
                        } else {
                            format!("{key}=@{path}")
                        };
                        parts.push(shell_escape(&file_segment));
                    }
                }
            }
        }
    }

    parts.join(" ")
}

fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    let safe = value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-._~/:?&=%@+;{}".contains(ch));

    if safe {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

impl std::fmt::Display for crate::domain::http::HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            crate::domain::http::HttpMethod::GET => write!(f, "GET"),
            crate::domain::http::HttpMethod::POST => write!(f, "POST"),
            crate::domain::http::HttpMethod::PUT => write!(f, "PUT"),
            crate::domain::http::HttpMethod::PATCH => write!(f, "PATCH"),
            crate::domain::http::HttpMethod::DELETE => write!(f, "DELETE"),
            crate::domain::http::HttpMethod::HEAD => write!(f, "HEAD"),
            crate::domain::http::HttpMethod::OPTIONS => write!(f, "OPTIONS"),
        }
    }
}
