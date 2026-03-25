use base64::Engine;

use super::http::{ApiKeyPlacement, AuthConfig, ResolvedPair};

#[derive(Debug, Clone)]
pub enum AppliedAuth {
    None,
    Bearer {
        token: String,
    },
    Basic {
        username: String,
        password: String,
    },
    ApiKey {
        key: String,
        value: String,
        placement: ApiKeyPlacement,
    },
}

pub fn apply_auth(
    headers: &mut Vec<ResolvedPair>,
    query: &mut Vec<ResolvedPair>,
    auth: AppliedAuth,
) {
    match auth {
        AppliedAuth::None => {}
        AppliedAuth::Bearer { token } => {
            headers.push(ResolvedPair {
                key: "authorization".to_string(),
                value: format!("Bearer {token}"),
            });
        }
        AppliedAuth::Basic { username, password } => {
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{username}:{password}"));

            headers.push(ResolvedPair {
                key: "authorization".to_string(),
                value: format!("Basic {encoded}"),
            });
        }
        AppliedAuth::ApiKey {
            key,
            value,
            placement,
        } => match placement {
            ApiKeyPlacement::Header => headers.push(ResolvedPair { key, value }),
            ApiKeyPlacement::Query => query.push(ResolvedPair { key, value }),
        },
    }
}

pub fn auth_type_name(auth: &AuthConfig) -> &'static str {
    match auth {
        AuthConfig::None => "none",
        AuthConfig::Bearer { .. } => "bearer",
        AuthConfig::Basic { .. } => "basic",
        AuthConfig::ApiKey { .. } => "apiKey",
    }
}
