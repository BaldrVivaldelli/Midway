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

impl HttpMethod {
    /// Todas las variantes soportadas, en el orden en que se presentan al
    /// usuario (por ejemplo en el `pick_list` de método del Request_Composer).
    pub const ALL: [HttpMethod; 7] = [
        HttpMethod::GET,
        HttpMethod::POST,
        HttpMethod::PUT,
        HttpMethod::PATCH,
        HttpMethod::DELETE,
        HttpMethod::HEAD,
        HttpMethod::OPTIONS,
    ];
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Límite por defecto de body de respuesta que se retiene en memoria.
///
/// Las respuestas se leen en streaming y se cortan al alcanzar este tamaño
/// (ver `infra::http_reqwest::execute_request`). Sin este límite, un endpoint
/// que devuelve cientos de MB obliga a materializar el payload completo en
/// RAM (y a shapear todo ese texto en el `Response_Inspector`), lo que era la
/// causa principal del consumo de memoria de la app.
///
/// 8 MiB cubre con holgura payloads de API reales y mantiene acotado el peor
/// caso por tab.
pub const DEFAULT_MAX_BODY_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseEnvelope {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<ResolvedPair>,
    pub body_text: String,
    pub duration_ms: u64,
    /// Tamaño del body retenido en `body_text`. Cuando `truncated` es `true`,
    /// este valor es el del fragmento retenido, no el del payload original
    /// (que puede ser desconocido si el servidor no envía `Content-Length`).
    pub size_bytes: u64,
    pub final_url: String,
    pub received_at: String,
    /// `true` cuando el body superó el límite y `body_text` es un prefijo del
    /// payload real. La UI lo usa para avisar que la vista está recortada.
    #[serde(default)]
    pub truncated: bool,
    /// `true` cuando la UI liberó el body de esta respuesta para no retener
    /// payloads de tabs inactivas. La metadata sigue siendo válida; solo
    /// `body_text` quedó vacío.
    #[serde(default)]
    pub body_evicted: bool,
    /// Tamaño total informado por el servidor vía `Content-Length`, cuando
    /// está disponible. Permite mostrar "8 MB de 240 MB" en una respuesta
    /// truncada.
    #[serde(default)]
    pub total_size_bytes: Option<u64>,
}

#[cfg(test)]
mod domain_surface_tests {
    //! Tests de caracterización de la superficie de dominio del baseline
    //! (Tarea 4.7; Requisitos 4.7, 6.8).
    //!
    //! Estos tests **no juzgan** si la superficie actual es la deseada: fijan la
    //! que existe hoy para que cualquier ampliación o recorte sea deliberada y
    //! visible en el diff. Las variantes ausentes ya están decididas una por una en
    //! `docs/adr/0007-variantes-ausentes-de-dominio.md` (`TRACE`, `CONNECT` y método
    //! personalizado en `HttpMethod`; `UrlEncoded`, `Binary`, `GraphQL` y `Xml` en
    //! `BodyMode`; `OAuth2`, `Digest`, `AwsSigV4`, `OAuth1`, `Ntlm`/`Hawk` y mTLS en
    //! `AuthConfig`) y registradas en `docs/feature-matrix.md`. Si uno de estos tests
    //! falla, la discusión arranca en ese ADR, no en el diff.
    //!
    //! Cada test combina dos mecanismos de tripwire:
    //!
    //! 1. Un `match` **exhaustivo sin brazo comodín**: agregar una variante rompe la
    //!    compilación de este módulo, no solo una aserción en runtime.
    //! 2. Una aserción de recuento sobre el listado local de variantes, que rompe si
    //!    alguien agrega la variante al `enum` y al `match` pero no actualiza la
    //!    caracterización.

    use super::*;

    /// Discriminante estable por variante, escrito a mano. El `match` es exhaustivo
    /// y sin comodín: si `HttpMethod` gana o pierde una variante, esto no compila.
    fn http_method_tag(method: HttpMethod) -> u8 {
        match method {
            HttpMethod::GET => 0,
            HttpMethod::POST => 1,
            HttpMethod::PUT => 2,
            HttpMethod::PATCH => 3,
            HttpMethod::DELETE => 4,
            HttpMethod::HEAD => 5,
            HttpMethod::OPTIONS => 6,
        }
    }

    #[test]
    fn http_method_all_has_exactly_seven_variants() {
        assert_eq!(
            HttpMethod::ALL.len(),
            7,
            "la superficie de HttpMethod del baseline es de 7 variantes; \
             actualizar docs/feature-matrix.md y el ADR 0007 antes de cambiarla"
        );

        // `ALL` es la única fuente que consume la UI: debe cubrir cada variante
        // exactamente una vez, sin duplicados ni faltantes.
        let mut tags: Vec<u8> = HttpMethod::ALL.iter().copied().map(http_method_tag).collect();
        tags.sort_unstable();
        assert_eq!(tags, vec![0, 1, 2, 3, 4, 5, 6]);

        // El orden de presentación también es comportamiento observable.
        assert_eq!(
            HttpMethod::ALL,
            [
                HttpMethod::GET,
                HttpMethod::POST,
                HttpMethod::PUT,
                HttpMethod::PATCH,
                HttpMethod::DELETE,
                HttpMethod::HEAD,
                HttpMethod::OPTIONS,
            ]
        );
    }

    /// `match` exhaustivo sin comodín sobre `BodyMode`.
    fn body_mode_tag(mode: BodyMode) -> u8 {
        match mode {
            BodyMode::None => 0,
            BodyMode::Json => 1,
            BodyMode::Text => 2,
            BodyMode::FormData => 3,
        }
    }

    #[test]
    fn body_mode_has_exactly_four_variants() {
        let all = [
            BodyMode::None,
            BodyMode::Json,
            BodyMode::Text,
            BodyMode::FormData,
        ];

        assert_eq!(
            all.len(),
            4,
            "la superficie de BodyMode del baseline es de 4 variantes; \
             actualizar docs/feature-matrix.md y el ADR 0007 antes de cambiarla"
        );

        let mut tags: Vec<u8> = all.iter().copied().map(body_mode_tag).collect();
        tags.sort_unstable();
        assert_eq!(tags, vec![0, 1, 2, 3]);
    }

    /// `match` exhaustivo sin comodín sobre `AuthConfig`. Las variantes con datos
    /// se ligan con `..` porque acá solo interesa la forma del `enum`.
    fn auth_config_tag(auth: &AuthConfig) -> u8 {
        match auth {
            AuthConfig::None => 0,
            AuthConfig::Bearer { .. } => 1,
            AuthConfig::Basic { .. } => 2,
            AuthConfig::ApiKey { .. } => 3,
        }
    }

    #[test]
    fn auth_config_has_exactly_four_variants() {
        let all = [
            AuthConfig::None,
            AuthConfig::Bearer {
                token: String::new(),
            },
            AuthConfig::Basic {
                username: String::new(),
                password: String::new(),
            },
            AuthConfig::ApiKey {
                key: String::new(),
                value: String::new(),
                placement: ApiKeyPlacement::Header,
            },
        ];

        assert_eq!(
            all.len(),
            4,
            "la superficie de AuthConfig del baseline es de 4 variantes; \
             actualizar docs/feature-matrix.md y el ADR 0007 antes de cambiarla"
        );

        let mut tags: Vec<u8> = all.iter().map(auth_config_tag).collect();
        tags.sort_unstable();
        assert_eq!(tags, vec![0, 1, 2, 3]);
    }
}
