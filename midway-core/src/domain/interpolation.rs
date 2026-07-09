use std::collections::{BTreeMap, BTreeSet};

use url::Url;

use crate::{
    app::errors::{AppError, AppResult},
    domain::{
        auth::{apply_auth, AppliedAuth},
        http::{
            AuthConfig, BodyMode, FormDataFieldKind, KeyValueRow, RequestDraft, Resolution,
            ResolvedBody, ResolvedFormDataField, ResolvedPair, ResolvedRequest,
        },
    },
};

const MAX_INTERPOLATION_PASSES: usize = 8;
const REDACTION_TOKEN: &str = "******";

#[derive(Debug, Clone, Copy)]
pub enum SecretRenderMode {
    Resolve,
    Redact,
}

pub fn resolve_request(
    draft: &RequestDraft,
    environment_name: Option<String>,
    environment_rows: &[KeyValueRow],
    secrets: &BTreeMap<String, String>,
    secret_mode: SecretRenderMode,
) -> AppResult<Resolution> {
    let raw_environment = collect_enabled_rows(environment_rows);
    let mut used_secret_aliases = BTreeSet::new();
    let mut missing_secret_aliases = BTreeSet::new();

    let mut applied_environment = BTreeMap::new();
    for (key, raw_value) in &raw_environment {
        let value = render_template(
            raw_value,
            &raw_environment,
            secrets,
            secret_mode,
            &mut used_secret_aliases,
            &mut missing_secret_aliases,
        )?;
        applied_environment.insert(key.clone(), value);
    }

    let resolved_base_url = render_template(
        &draft.url,
        &applied_environment,
        secrets,
        secret_mode,
        &mut used_secret_aliases,
        &mut missing_secret_aliases,
    )?;

    let mut query = resolve_rows(
        &draft.query,
        &applied_environment,
        secrets,
        secret_mode,
        &mut used_secret_aliases,
        &mut missing_secret_aliases,
    )?;

    let mut headers = resolve_rows(
        &draft.headers,
        &applied_environment,
        secrets,
        secret_mode,
        &mut used_secret_aliases,
        &mut missing_secret_aliases,
    )?;

    let body = match draft.body.mode {
        BodyMode::None => ResolvedBody::None,
        BodyMode::Json => {
            let rendered = render_template(
                &draft.body.value,
                &applied_environment,
                secrets,
                secret_mode,
                &mut used_secret_aliases,
                &mut missing_secret_aliases,
            )?;

            let trimmed = rendered.trim();
            if trimmed.is_empty() {
                ResolvedBody::None
            } else {
                let value = serde_json::from_str::<serde_json::Value>(trimmed)
                    .map_err(|error| AppError::InvalidJson(error.to_string()))?;
                let text = serde_json::to_string_pretty(&value)
                    .map_err(|error| AppError::Serialization(error.to_string()))?;
                ResolvedBody::Json { text, value }
            }
        }
        BodyMode::Text => {
            let rendered = render_template(
                &draft.body.value,
                &applied_environment,
                secrets,
                secret_mode,
                &mut used_secret_aliases,
                &mut missing_secret_aliases,
            )?;

            if rendered.is_empty() {
                ResolvedBody::None
            } else {
                ResolvedBody::Text { text: rendered }
            }
        }
        BodyMode::FormData => {
            let mut fields = Vec::new();

            for row in &draft.body.form_data {
                if !row.enabled || row.key.trim().is_empty() {
                    continue;
                }

                let key = render_template(
                    row.key.trim(),
                    &applied_environment,
                    secrets,
                    secret_mode,
                    &mut used_secret_aliases,
                    &mut missing_secret_aliases,
                )?;
                let value = render_template(
                    &row.value,
                    &applied_environment,
                    secrets,
                    secret_mode,
                    &mut used_secret_aliases,
                    &mut missing_secret_aliases,
                )?;
                let file_name = row
                    .file_name
                    .as_ref()
                    .map(|name| {
                        render_template(
                            name,
                            &applied_environment,
                            secrets,
                            secret_mode,
                            &mut used_secret_aliases,
                            &mut missing_secret_aliases,
                        )
                    })
                    .transpose()?;

                match row.kind {
                    FormDataFieldKind::Text => {
                        fields.push(ResolvedFormDataField::Text { key, value });
                    }
                    FormDataFieldKind::File => {
                        if value.trim().is_empty() {
                            continue;
                        }
                        fields.push(ResolvedFormDataField::File {
                            key,
                            path: value,
                            file_name,
                        });
                    }
                }
            }

            if fields.is_empty() {
                ResolvedBody::None
            } else {
                ResolvedBody::FormData { fields }
            }
        }
    };

    let rendered_auth = render_auth(
        &draft.auth,
        &applied_environment,
        secrets,
        secret_mode,
        &mut used_secret_aliases,
        &mut missing_secret_aliases,
    )?;

    apply_auth(&mut headers, &mut query, rendered_auth);

    if matches!(&body, ResolvedBody::Json { .. })
        && !headers
            .iter()
            .any(|header| header.key.eq_ignore_ascii_case("content-type"))
    {
        headers.push(ResolvedPair {
            key: "content-type".to_string(),
            value: "application/json".to_string(),
        });
    }

    if matches!(secret_mode, SecretRenderMode::Resolve) && !missing_secret_aliases.is_empty() {
        return Err(AppError::Validation(format!(
            "Faltan secretos requeridos: {}",
            missing_secret_aliases
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    let final_url = build_final_url(&resolved_base_url, &query)?;

    Ok(Resolution {
        request: ResolvedRequest {
            method: draft.method,
            url: final_url,
            headers,
            body,
            timeout_ms: draft.timeout_ms.max(1),
        },
        applied_environment,
        environment_name,
        used_secret_aliases: used_secret_aliases.into_iter().collect(),
        missing_secret_aliases: missing_secret_aliases.into_iter().collect(),
    })
}

fn render_auth(
    auth: &AuthConfig,
    environment: &BTreeMap<String, String>,
    secrets: &BTreeMap<String, String>,
    secret_mode: SecretRenderMode,
    used_secret_aliases: &mut BTreeSet<String>,
    missing_secret_aliases: &mut BTreeSet<String>,
) -> AppResult<AppliedAuth> {
    let rendered = match auth {
        AuthConfig::None => AppliedAuth::None,
        AuthConfig::Bearer { token } => AppliedAuth::Bearer {
            token: render_template(
                token,
                environment,
                secrets,
                secret_mode,
                used_secret_aliases,
                missing_secret_aliases,
            )?,
        },
        AuthConfig::Basic { username, password } => AppliedAuth::Basic {
            username: render_template(
                username,
                environment,
                secrets,
                secret_mode,
                used_secret_aliases,
                missing_secret_aliases,
            )?,
            password: render_template(
                password,
                environment,
                secrets,
                secret_mode,
                used_secret_aliases,
                missing_secret_aliases,
            )?,
        },
        AuthConfig::ApiKey {
            key,
            value,
            placement,
        } => AppliedAuth::ApiKey {
            key: render_template(
                key,
                environment,
                secrets,
                secret_mode,
                used_secret_aliases,
                missing_secret_aliases,
            )?,
            value: render_template(
                value,
                environment,
                secrets,
                secret_mode,
                used_secret_aliases,
                missing_secret_aliases,
            )?,
            placement: *placement,
        },
    };

    Ok(rendered)
}

fn collect_enabled_rows(rows: &[KeyValueRow]) -> BTreeMap<String, String> {
    rows.iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| (row.key.trim().to_string(), row.value.clone()))
        .collect()
}

fn resolve_rows(
    rows: &[KeyValueRow],
    environment: &BTreeMap<String, String>,
    secrets: &BTreeMap<String, String>,
    secret_mode: SecretRenderMode,
    used_secret_aliases: &mut BTreeSet<String>,
    missing_secret_aliases: &mut BTreeSet<String>,
) -> AppResult<Vec<ResolvedPair>> {
    let mut out = Vec::new();

    for row in rows {
        if !row.enabled || row.key.trim().is_empty() {
            continue;
        }

        let key = render_template(
            row.key.trim(),
            environment,
            secrets,
            secret_mode,
            used_secret_aliases,
            missing_secret_aliases,
        )?;
        let value = render_template(
            &row.value,
            environment,
            secrets,
            secret_mode,
            used_secret_aliases,
            missing_secret_aliases,
        )?;

        out.push(ResolvedPair { key, value });
    }

    Ok(out)
}

fn render_template(
    template: &str,
    environment: &BTreeMap<String, String>,
    secrets: &BTreeMap<String, String>,
    secret_mode: SecretRenderMode,
    used_secret_aliases: &mut BTreeSet<String>,
    missing_secret_aliases: &mut BTreeSet<String>,
) -> AppResult<String> {
    let mut current = template.to_string();

    for _ in 0..MAX_INTERPOLATION_PASSES {
        let (next, replaced_any) = interpolate_once(
            &current,
            environment,
            secrets,
            secret_mode,
            used_secret_aliases,
            missing_secret_aliases,
        )?;

        if !replaced_any || next == current {
            return Ok(next);
        }

        current = next;
    }

    Ok(current)
}

fn interpolate_once(
    template: &str,
    environment: &BTreeMap<String, String>,
    secrets: &BTreeMap<String, String>,
    secret_mode: SecretRenderMode,
    used_secret_aliases: &mut BTreeSet<String>,
    missing_secret_aliases: &mut BTreeSet<String>,
) -> AppResult<(String, bool)> {
    let mut output = String::with_capacity(template.len());
    let mut remainder = template;
    let mut replaced_any = false;

    while let Some(start) = remainder.find("{{") {
        let (prefix, after_start) = remainder.split_at(start);
        output.push_str(prefix);

        let Some(end_relative) = after_start[2..].find("}}") else {
            output.push_str(after_start);
            return Ok((output, replaced_any));
        };

        let placeholder_end = 2 + end_relative + 2;
        let placeholder = &after_start[..placeholder_end];
        let token = after_start[2..2 + end_relative].trim();

        if let Some(alias) = token.strip_prefix("secret:") {
            let alias = alias.trim();

            if alias.is_empty() {
                output.push_str(placeholder);
            } else if let Some(secret) = secrets.get(alias) {
                used_secret_aliases.insert(alias.to_string());
                replaced_any = true;

                match secret_mode {
                    SecretRenderMode::Resolve => output.push_str(secret),
                    SecretRenderMode::Redact => output.push_str(REDACTION_TOKEN),
                }
            } else {
                missing_secret_aliases.insert(alias.to_string());
                replaced_any = true;

                match secret_mode {
                    SecretRenderMode::Resolve => {}
                    SecretRenderMode::Redact => {
                        output.push_str(&format!("<missing-secret:{alias}>"));
                    }
                }
            }
        } else if let Some(value) = environment.get(token) {
            output.push_str(value);
            replaced_any = true;
        } else {
            output.push_str(placeholder);
        }

        remainder = &after_start[placeholder_end..];
    }

    output.push_str(remainder);
    Ok((output, replaced_any))
}

fn build_final_url(base_url: &str, query: &[ResolvedPair]) -> AppResult<String> {
    let mut parsed = Url::parse(base_url)
        .map_err(|error| AppError::Validation(format!("URL inválida: {error}")))?;

    {
        let mut pairs = parsed.query_pairs_mut();
        for pair in query {
            pairs.append_pair(&pair.key, &pair.value);
        }
    }

    Ok(parsed.to_string())
}
