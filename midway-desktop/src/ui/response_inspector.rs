//! `Response_Inspector` (Tarea 3.9, Fase 1): status/tiempo/tamaño y tabs
//! Body/Headers/Tests de la respuesta de la tab de request activa.
//!
//! Muestra, al finalizar una petición, el código de status, el tiempo de
//! respuesta en milisegundos y el tamaño del payload en bytes (Requisito
//! 2.10), además de tabs separadas para Body, Headers y Tests, donde la tab
//! Tests muestra, para cada assertion configurada, si su evaluación resultó
//! aprobada o fallida (Requisito 2.11).
//!
//! Esta tarea es de solo presentación ("passthrough"): la ejecución real de
//! la petición que popula `RequestTabState::response` se implementa en la
//! Tarea 3.18. Hasta entonces, esta vista muestra el estado vacío.
//!
//! Ver diseño: "Components and Interfaces > Request_Composer y
//! Response_Inspector (Fase 1)"; "Flujo de una request (Fase 1)".
//! Ver requisitos: 2.10, 2.11.

use iced::widget::{column, row, text};
use iced::{Element, Length};

use midway_core::domain::http::ResolvedPair;
use midway_core::domain::testing::AssertionResult;

use crate::app::{Message, Midway, ResponseInspectorMessage, ResponseInspectorTab, ResponseOutcome};
use crate::ui::tab_bar;

/// Construye el `Response_Inspector` para la tab de request activa.
///
/// El llamador (`app::view`) es responsable de solo invocar esta función
/// cuando existe una tab activa (`active_tab_index` es válido).
pub fn view<'a>(state: &'a Midway, active_tab_index: usize) -> Element<'a, Message> {
    let tab_state = &state.tabs[active_tab_index];

    match &tab_state.response {
        None => text("Sin respuesta aún").into(),
        Some(outcome) => column![summary_row(outcome), response_tabs(outcome, tab_state.response_tab)]
            .spacing(12)
            .into(),
    }
}

/// Fila con status, tiempo de respuesta y tamaño (Requisito 2.10). Los
/// valores se muestran tal cual vienen en `ResponseEnvelope`, sin
/// transformación.
fn summary_row(outcome: &ResponseOutcome) -> Element<'_, Message> {
    let (status, status_text, duration_ms, size_bytes) = summary_line(outcome);
    row![
        text(format!("{} {}", status, status_text)),
        text(format!("{} ms", duration_ms)),
        text(format!("{} bytes", size_bytes)),
    ]
    .spacing(16)
    .into()
}

/// Extrae, sin transformación, los valores de status/tiempo/tamaño de
/// `outcome.response` (Requisito 2.10, passthrough). Aislada de la
/// construcción del `Element` para poder verificarla en tests unitarios.
fn summary_line(outcome: &ResponseOutcome) -> (u16, &str, u64, u64) {
    let response = &outcome.response;
    (
        response.status,
        response.status_text.as_str(),
        response.duration_ms,
        response.size_bytes,
    )
}

/// Tabs Body/Headers/Tests del `Response_Inspector` (Tarea 5.1, Requisito
/// 3.1), usando el widget compartido `ui::tab_bar` (envoltura de
/// `iced_aw::{TabBar, Tabs}`) en lugar de botones simples, reutilizando el
/// mismo widget que las tabs de configuración del `Request_Composer`.
fn response_tabs(outcome: &ResponseOutcome, active: ResponseInspectorTab) -> Element<'_, Message> {
    let entries = vec![
        (ResponseInspectorTab::Body, "Body", body_tab(outcome)),
        (ResponseInspectorTab::Headers, "Headers", headers_tab(outcome)),
        (ResponseInspectorTab::Tests, "Tests", tests_tab(outcome)),
    ];

    tab_bar::tabs(entries, &active, |tab| {
        Message::ResponseInspector(ResponseInspectorMessage::TabSelected(tab))
    })
}

/// Tab Body: muestra `response.body_text` como texto plano. La
/// resaltación de sintaxis JSON vía `Text_Editor_Component`
/// (`ui::text_editor`) se integra en una tarea posterior.
fn body_tab(outcome: &ResponseOutcome) -> Element<'_, Message> {
    text(outcome.response.body_text.as_str())
        .width(Length::Fill)
        .into()
}

/// Tab Headers: lista de filas `key: value`.
fn headers_tab(outcome: &ResponseOutcome) -> Element<'_, Message> {
    let rows: Vec<Element<'_, Message>> = header_rows_data(outcome)
        .into_iter()
        .map(|(key, value)| text(format!("{}: {}", key, value)).into())
        .collect();

    column(rows).spacing(4).into()
}

/// Extrae los pares `(key, value)` de `outcome.response.headers` en el
/// mismo orden y sin transformación (Requisito 2.10, passthrough).
fn header_rows_data(outcome: &ResponseOutcome) -> Vec<(String, String)> {
    outcome
        .response
        .headers
        .iter()
        .map(|pair: &ResolvedPair| (pair.key.clone(), pair.value.clone()))
        .collect()
}

/// Tab Tests: para cada `AssertionResult`, muestra si aprobó o falló junto
/// con su nombre, valor esperado, valor actual y mensaje (Requisito 2.11).
fn tests_tab(outcome: &ResponseOutcome) -> Element<'_, Message> {
    if outcome.assertions.results.is_empty() {
        return text("Sin assertions configuradas").into();
    }

    let rows: Vec<Element<'_, Message>> = assertion_rows_data(outcome)
        .into_iter()
        .map(|(passed, name, expected, actual, message)| {
            let status_label = if passed { "PASSED" } else { "FAILED" };
            column![
                row![text(status_label), text(name)].spacing(8),
                text(format!("esperado: {}", expected)),
                text(format!("actual: {}", actual)),
                text(message),
            ]
            .spacing(2)
            .into()
        })
        .collect();

    column(rows).spacing(8).into()
}

/// Extrae, para cada `AssertionResult`, la tupla
/// `(passed, name, expected, actual, message)` sin transformación
/// (Requisito 2.11, passthrough). El valor `actual` se reemplaza por `"-"`
/// cuando es `None`, único procesamiento aplicado.
fn assertion_rows_data(outcome: &ResponseOutcome) -> Vec<(bool, String, String, String, String)> {
    outcome
        .assertions
        .results
        .iter()
        .map(|result: &AssertionResult| {
            let actual = result.actual.clone().unwrap_or_else(|| "-".to_string());
            (
                result.passed,
                result.name.clone(),
                result.expected.clone(),
                actual,
                result.message.clone(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use midway_core::domain::http::ResponseEnvelope;
    use midway_core::domain::testing::{AssertionOperator, AssertionReport, AssertionSource};

    fn make_outcome(response: ResponseEnvelope, results: Vec<AssertionResult>) -> ResponseOutcome {
        let total = results.len() as u64;
        let passed = results.iter().filter(|r| r.passed).count() as u64;
        let failed = total - passed;
        ResponseOutcome {
            response,
            assertions: AssertionReport {
                total,
                passed,
                failed,
                results,
            },
        }
    }

    fn make_response(headers: Vec<ResolvedPair>) -> ResponseEnvelope {
        ResponseEnvelope {
            status: 200,
            status_text: "OK".to_string(),
            headers,
            body_text: "{}".to_string(),
            duration_ms: 42,
            size_bytes: 128,
            final_url: "https://example.com".to_string(),
            received_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    // Requisito 2.10: summary_line debe devolver status/status_text/
    // duration_ms/size_bytes exactamente como llegan en ResponseEnvelope,
    // sin ninguna transformación.
    #[test]
    fn summary_line_passthrough() {
        let response = ResponseEnvelope {
            status: 404,
            status_text: "Not Found".to_string(),
            headers: Vec::new(),
            body_text: "not found".to_string(),
            duration_ms: 123,
            size_bytes: 9001,
            final_url: "https://example.com/missing".to_string(),
            received_at: "2024-02-02T10:00:00Z".to_string(),
        };
        let outcome = make_outcome(response, Vec::new());

        let (status, status_text, duration_ms, size_bytes) = summary_line(&outcome);

        assert_eq!(status, 404);
        assert_eq!(status_text, "Not Found");
        assert_eq!(duration_ms, 123);
        assert_eq!(size_bytes, 9001);
    }

    // Requisito 2.10: header_rows_data debe devolver las mismas parejas
    // key/value, en el mismo orden, sin transformación adicional.
    #[test]
    fn header_rows_data_passthrough() {
        let headers = vec![
            ResolvedPair {
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
            },
            ResolvedPair {
                key: "X-Request-Id".to_string(),
                value: "abc-123".to_string(),
            },
            ResolvedPair {
                key: "Cache-Control".to_string(),
                value: "no-cache".to_string(),
            },
        ];
        let outcome = make_outcome(make_response(headers.clone()), Vec::new());

        let rows = header_rows_data(&outcome);

        let expected: Vec<(String, String)> = headers
            .into_iter()
            .map(|pair| (pair.key, pair.value))
            .collect();
        assert_eq!(rows, expected);
    }

    // Requisito 2.11: assertion_rows_data debe reflejar passed/name/
    // expected/actual/message de cada AssertionResult sin transformación,
    // salvo el placeholder "-" cuando actual es None.
    #[test]
    fn assertion_rows_data_passthrough() {
        let results = vec![
            AssertionResult {
                id: "a1".to_string(),
                name: "Status es 200".to_string(),
                passed: true,
                source: AssertionSource::Status,
                operator: AssertionOperator::Equals,
                selector: None,
                expected: "200".to_string(),
                actual: Some("200".to_string()),
                message: "La aserción pasó.".to_string(),
            },
            AssertionResult {
                id: "a2".to_string(),
                name: "Header existe".to_string(),
                passed: false,
                source: AssertionSource::Header,
                operator: AssertionOperator::Exists,
                selector: Some("X-Missing".to_string()),
                expected: "".to_string(),
                actual: None,
                message: "Se esperaba un valor, pero no existe.".to_string(),
            },
            AssertionResult {
                id: "a3".to_string(),
                name: "Body contiene texto".to_string(),
                passed: true,
                source: AssertionSource::BodyText,
                operator: AssertionOperator::Contains,
                selector: None,
                expected: "ok".to_string(),
                actual: Some("todo ok aqui".to_string()),
                message: "La aserción pasó: el valor cumple con contener \"ok\".".to_string(),
            },
        ];
        let outcome = make_outcome(make_response(Vec::new()), results);

        let rows = assertion_rows_data(&outcome);

        assert_eq!(
            rows,
            vec![
                (
                    true,
                    "Status es 200".to_string(),
                    "200".to_string(),
                    "200".to_string(),
                    "La aserción pasó.".to_string(),
                ),
                (
                    false,
                    "Header existe".to_string(),
                    "".to_string(),
                    "-".to_string(),
                    "Se esperaba un valor, pero no existe.".to_string(),
                ),
                (
                    true,
                    "Body contiene texto".to_string(),
                    "ok".to_string(),
                    "todo ok aqui".to_string(),
                    "La aserción pasó: el valor cumple con contener \"ok\".".to_string(),
                ),
            ]
        );
    }
}
