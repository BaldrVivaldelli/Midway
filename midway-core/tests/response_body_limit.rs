//! Verifica el límite de body de respuesta retenido en memoria
//! (`infra::http_reqwest::execute_request_with_body_limit`).
//!
//! Antes de este límite, `execute_request` hacía `response.bytes()` sin cota:
//! un endpoint que devolvía cientos de MB obligaba a materializar el payload
//! completo en RAM (más una segunda copia al convertirlo a `String`). Estos
//! tests fijan el comportamiento de corte para que no vuelva a regresar.

use midway_core::domain::http::{
    HttpMethod, ResolvedBody, ResolvedRequest, DEFAULT_MAX_BODY_BYTES,
};
use midway_core::infra::http_reqwest::{execute_request, execute_request_with_body_limit};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// El límite por defecto debe cubrir payloads de API reales (>= 8 MiB).
/// Se verifica en tiempo de compilación: `DEFAULT_MAX_BODY_BYTES` es una
/// constante, así que un `assert!` en runtime no aportaba nada.
const _: () = assert!(DEFAULT_MAX_BODY_BYTES >= 8 * 1024 * 1024);

fn get_request(url: String) -> ResolvedRequest {
    ResolvedRequest {
        method: HttpMethod::GET,
        url,
        headers: Vec::new(),
        body: ResolvedBody::None,
        timeout_ms: 30_000,
    }
}

/// Un body más chico que el límite se devuelve completo y sin marcar.
#[tokio::test]
async fn body_under_limit_is_returned_intact() {
    let server = MockServer::start().await;
    let payload = "a".repeat(1024);

    Mock::given(method("GET"))
        .and(path("/small"))
        .respond_with(ResponseTemplate::new(200).set_body_string(payload.clone()))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let response = execute_request_with_body_limit(
        &client,
        get_request(format!("{}/small", server.uri())),
        8 * 1024,
    )
    .await
    .expect("el request debería completarse");

    assert_eq!(response.body_text, payload);
    assert_eq!(response.size_bytes, 1024);
    assert!(!response.truncated, "no debería marcarse como truncado");
}

/// Un body que excede el límite se corta exactamente en el límite y se marca
/// como truncado, en lugar de acumularse completo en memoria.
#[tokio::test]
async fn body_over_limit_is_truncated_at_the_cap() {
    let server = MockServer::start().await;
    let limit = 4 * 1024;
    let payload = "b".repeat(64 * 1024);

    Mock::given(method("GET"))
        .and(path("/large"))
        .respond_with(ResponseTemplate::new(200).set_body_string(payload))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let response = execute_request_with_body_limit(
        &client,
        get_request(format!("{}/large", server.uri())),
        limit as u64,
    )
    .await
    .expect("el request debería completarse");

    assert!(response.truncated, "debería marcarse como truncado");
    assert_eq!(
        response.body_text.len(),
        limit,
        "el body retenido debe cortarse en el límite"
    );
    assert_eq!(response.size_bytes, limit as u64);
    assert!(
        response.body_text.bytes().all(|byte| byte == b'b'),
        "el fragmento retenido debe ser un prefijo del payload"
    );
}

/// El corte no debe romper caracteres multibyte: el body resultante tiene que
/// seguir siendo UTF-8 válido incluso si el límite cae en medio de un carácter.
#[tokio::test]
async fn truncation_does_not_corrupt_multibyte_characters() {
    let server = MockServer::start().await;
    // "é" ocupa 2 bytes, así que un límite impar cae en medio de un carácter.
    let payload = "é".repeat(1024);

    Mock::given(method("GET"))
        .and(path("/utf8"))
        .respond_with(ResponseTemplate::new(200).set_body_string(payload))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let response = execute_request_with_body_limit(
        &client,
        get_request(format!("{}/utf8", server.uri())),
        101,
    )
    .await
    .expect("el request debería completarse");

    assert!(response.truncated);
    // `from_utf8_lossy` sustituye el carácter partido en lugar de fallar, así
    // que el resultado siempre es texto válido y renderizable.
    assert!(
        response
            .body_text
            .chars()
            .all(|c| c == 'é' || c == char::REPLACEMENT_CHARACTER),
        "el body truncado debe seguir siendo texto válido"
    );
}

/// `execute_request` aplica `DEFAULT_MAX_BODY_BYTES` sin necesidad de pasar
/// el límite explícitamente.
#[tokio::test]
async fn default_entry_point_applies_the_default_limit() {
    let server = MockServer::start().await;
    let payload = "c".repeat(2048);

    Mock::given(method("GET"))
        .and(path("/default"))
        .respond_with(ResponseTemplate::new(200).set_body_string(payload.clone()))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let response = execute_request(&client, get_request(format!("{}/default", server.uri())))
        .await
        .expect("el request debería completarse");

    assert_eq!(response.body_text, payload);
    assert!(!response.truncated);
}
