//! `Response_Inspector`: status/tiempo/tamaño y tabs Body/Headers/Cookies/Tests
//! de la respuesta de la tab de request activa.
//!
//! Muestra, al finalizar una petición, el código de status (coloreado con
//! `status_color`), el tiempo de respuesta en milisegundos y el tamaño del
//! payload en bytes (Requisito 2.10), además de tabs separadas para Body,
//! Headers, Cookies y Tests. La tab Cookies lee el `CookieJarHandle` para
//! la `final_url` de la respuesta (Requisito 9.1–9.3). No se incluyen tabs
//! Preview ni Timeline (Requisito 12.5, 12.6).
//!
//! Ver diseño: "Components and Interfaces > Response_Inspector (tab Cookies)".
//! Ver requisitos: 2.10, 2.11, 9.1, 9.2, 9.3, 9.4, 12.5, 12.6.

use iced::widget::{column, container, row, text};
use iced::{font, Element, Font, Length};

use midway_core::domain::cookies::CookiePair;
use midway_core::domain::http::ResolvedPair;
use midway_core::domain::testing::AssertionResult;

use crate::app::{
    Message, Midway, ResponseInspectorMessage, ResponseInspectorTab, ResponseOutcome,
};
use crate::ui::design_system::{status_color, DesignSystem, TextStyle};
use crate::ui::tab_bar;

fn font_for(style: &TextStyle) -> Font {
    Font {
        family: if style.monospace {
            font::Family::Monospace
        } else {
            font::Family::SansSerif
        },
        weight: style.weight,
        ..Font::DEFAULT
    }
}

/// Construye el `Response_Inspector` para la tab de request activa.
///
/// El llamador (`app::view`) es responsable de solo invocar esta función
/// cuando existe una tab activa (`active_tab_index` es válido).
pub fn view<'a>(state: &'a Midway, active_tab_index: usize) -> Element<'a, Message> {
    let tab_state = &state.tabs[active_tab_index];

    match &tab_state.response {
        None => {
            let ds = DesignSystem::for_mode(state.theme_mode);
            container(
                text("Sin respuesta aún")
                    .size(ds.typography.body.size)
                    .color(ds.palette.text_secondary),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
        }
        Some(outcome) => {
            let ds = DesignSystem::for_mode(state.theme_mode);
            let (status, status_text, duration_ms, size_bytes) = summary_line(outcome);
            let secondary = ds.typography.secondary;
            let secondary_font = font_for(&secondary);
            let body = ds.typography.body;
            let body_font = font_for(&body);

            let status_ok = status < 400;
            let check_icon = if status_ok { "✓" } else { "✗" };
            let status_clr = status_color(&ds, status);

            // Status badge: ✓ 201 Created  323 ms  65 bytes
            let status_row = row![
                text(check_icon).size(body.size).font(body_font).color(status_clr),
                text(format!("{} {}", status, status_text))
                    .size(body.size)
                    .font(font_for(&TextStyle { weight: iced::font::Weight::Bold, ..body }))
                    .color(status_clr),
                text(format!("{} ms", duration_ms))
                    .size(secondary.size)
                    .font(secondary_font)
                    .color(ds.palette.text_secondary),
                text(format!("{} bytes", size_bytes))
                    .size(secondary.size)
                    .font(secondary_font)
                    .color(ds.palette.text_secondary),
            ]
            .spacing(ds.spacing.sm)
            .align_y(iced::alignment::Vertical::Center);

            // Response tabs
            let tabs = response_tabs(state, outcome, tab_state.response_tab, &ds);

            column![status_row, tabs]
                .spacing(ds.spacing.sm)
                .width(Length::Fill)
                .into()
        }
    }
}

/// Fila con status, tiempo de respuesta y tamaño (Requisito 2.10). Los
/// valores se muestran tal cual vienen en `ResponseEnvelope`, sin
/// transformación. Usa `subtitle` y separación `md` para distinguir el
/// resumen del cuerpo de la respuesta (Requisito 3.2).
#[allow(dead_code)]
fn summary_row<'a>(outcome: &'a ResponseOutcome, ds: &DesignSystem) -> Element<'a, Message> {
    let (status, status_text, duration_ms, size_bytes) = summary_line(outcome);
    let subtitle = ds.typography.subtitle;
    let subtitle_font = font_for(&subtitle);

    row![
        text(format!("{} {}", status, status_text))
            .size(subtitle.size)
            .font(subtitle_font)
            .color(status_color(ds, status)),
        text(format!("{} ms", duration_ms))
            .size(subtitle.size)
            .font(subtitle_font),
        text(format!("{} bytes", size_bytes))
            .size(subtitle.size)
            .font(subtitle_font),
    ]
    .spacing(ds.spacing.md)
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
fn response_tabs<'a>(
    state: &'a Midway,
    outcome: &'a ResponseOutcome,
    active: ResponseInspectorTab,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let entries = vec![
        (ResponseInspectorTab::Body, "Body", body_tab(outcome, ds)),
        (
            ResponseInspectorTab::Headers,
            "Headers",
            headers_tab(outcome),
        ),
        (
            ResponseInspectorTab::Cookies,
            "Cookies",
            cookies_tab(state, outcome, ds),
        ),
        (ResponseInspectorTab::Tests, "Tests", tests_tab(outcome)),
    ];

    tab_bar::tabs(
        entries,
        &active,
        |tab| Message::ResponseInspector(ResponseInspectorMessage::TabSelected(tab)),
        ds,
    )
}

/// Tab Body: muestra `response.body_text` como texto plano. La
/// resaltación de sintaxis JSON vía `Text_Editor_Component`
/// (`ui::text_editor`) se integra en una tarea posterior.
fn body_tab<'a>(outcome: &'a ResponseOutcome, ds: &DesignSystem) -> Element<'a, Message> {
    let monospace = ds.typography.monospace;
    text(outcome.response.body_text.as_str())
        .size(monospace.size)
        .font(font_for(&monospace))
        .width(Length::Fill)
        .into()
}

/// Tab Cookies: muestra las cookies del `CookieJarHandle` para la
/// `final_url` de la respuesta activa (Req 9.2, 9.3).
///
/// - ≥1 cookie: filas nombre/valor en el orden devuelto por el jar.
/// - 0 cookies: estado vacío "No hay cookies almacenadas" (con precedencia, Req 9.3).
fn cookies_tab<'a>(state: &'a Midway, outcome: &'a ResponseOutcome, ds: &DesignSystem) -> Element<'a, Message> {
    let cookies = state
        .app_state
        .cookie_jar
        .read_for_url(&outcome.response.final_url);

    if cookies.is_empty() {
        return text("No hay cookies almacenadas").into();
    }

    let body = ds.typography.body;
    let body_font = font_for(&body);

    let rows: Vec<Element<'a, Message>> = cookies
        .into_iter()
        .map(|cookie: CookiePair| {
            row![
                text(cookie.name)
                    .size(body.size)
                    .font(body_font),
                text(cookie.value)
                    .size(body.size)
                    .font(body_font),
            ]
            .spacing(ds.spacing.sm)
            .into()
        })
        .collect();

    column(rows).spacing(4).width(Length::Fill).into()
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
