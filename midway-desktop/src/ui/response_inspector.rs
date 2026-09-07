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
    Message, Midway, RequestComposerMessage, ResponseInspectorMessage, ResponseInspectorTab,
    ResponseOutcome,
};
use crate::ui::design_system::{status_color, DesignSystem, TextStyle};
use crate::ui::empty_state;
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
            let ds = state.theme.design_system();
            // Req 9.5: durante el periodo breve entre `SendPressed` y la
            // llegada de la respuesta el área no queda en blanco ni repite
            // "ejecutá el request" (que el usuario ya hizo): se muestra el
            // mismo componente con `hint` de progreso.
            let spec = if tab_state.sending {
                empty_state::response_in_flight()
            } else {
                empty_state::no_response()
            };
            empty_state::view(spec, &ds)
        }
        Some(outcome) => {
            let ds = state.theme.design_system();
            let (status, status_text, duration_ms, size_bytes) = summary_line(outcome);
            let secondary = ds.typography.secondary;
            let secondary_font = font_for(&secondary);
            let body = ds.typography.body;
            let body_font = font_for(&body);

            let status_ok = status < 400;
            let check_icon = if status_ok { "✓" } else { "✗" };
            let status_clr = status_color(&ds, status);

            // Status badge: ✓ 201 Created  323 ms  65 bytes
            let status_line = row![
                text(check_icon)
                    .size(body.size)
                    .font(body_font)
                    .color(status_clr),
                text(format!("{} {}", status, status_text))
                    .size(body.size)
                    .font(font_for(&TextStyle {
                        weight: iced::font::Weight::Bold,
                        ..body
                    }))
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

            column![response_header(status_line, &ds), tabs]
                .spacing(ds.spacing.sm)
                .width(Length::Fill)
                .into()
        }
    }
}

/// Cabecera diferenciada del inspector de respuesta (Req 9.1, diseño §8.2).
///
/// El panel entero se apoya en `surface_elevated` (lo aplica
/// `app::debug_split_content`, que es quien lo compone); esta cabecera se
/// separa de ese fondo con `background_secondary` y una línea inferior de 1 px
/// en `border`, de modo que el resumen de la respuesta se lee como banda propia
/// y no como la primera fila del contenido.
///
/// La línea es una franja propia y no `container::Style::border` porque
/// `iced::Border` es uniforme en los cuatro lados y encajonaría la banda. Los
/// dos colores salen de la escala existente de `DesignSystem`: no se agrega
/// ningún color de marca (Req 9.8).
fn response_header<'a>(
    status_line: impl Into<Element<'a, Message>>,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let background = ds.palette.background_secondary;
    let border_color = ds.palette.border;

    let band = container(status_line.into())
        .width(Length::Fill)
        .padding([ds.spacing.xs, ds.spacing.sm])
        .style(move |_theme| container::Style {
            background: Some(background.into()),
            ..container::Style::default()
        });

    let bottom_border = container(column![])
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_theme| container::Style {
            background: Some(border_color.into()),
            ..container::Style::default()
        });

    column![band, bottom_border]
        .spacing(0)
        .width(Length::Fill)
        .into()
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
    let ds_owned = *ds;
    let entries: Vec<tab_bar::TabEntry<'a, Message, ResponseInspectorTab>> = vec![
        (
            ResponseInspectorTab::Body,
            "Body",
            Box::new(move || body_tab(outcome, &ds_owned)),
        ),
        (
            ResponseInspectorTab::Headers,
            "Headers",
            Box::new(move || headers_tab(outcome)),
        ),
        (
            ResponseInspectorTab::Cookies,
            "Cookies",
            Box::new(move || cookies_tab(state, outcome, &ds_owned)),
        ),
        (
            ResponseInspectorTab::Tests,
            "Tests",
            Box::new(move || tests_tab(outcome, &ds_owned)),
        ),
    ];

    tab_bar::tabs(
        entries,
        &active,
        |tab| Message::ResponseInspector(ResponseInspectorMessage::TabSelected(tab)),
        ds,
    )
}

/// Cantidad máxima de bytes del body que se pasan al widget de texto.
///
/// `iced` shapea todo el contenido de un `text()` (no virtualiza por líneas
/// visibles), así que el costo de glyphs es proporcional al largo total, no a
/// lo que se ve en pantalla. Un body de varios MB en un único `text()` genera
/// un pico de memoria y de layout enorme.
///
/// 128 KiB es bastante más de lo que un humano puede leer en el panel y
/// mantiene el shaping acotado. El body completo (hasta el límite de
/// retención de `midway-core`) sigue disponible en `ResponseEnvelope` para
/// assertions, export y copiado.
const MAX_RENDERED_BODY_BYTES: usize = 128 * 1024;

/// Recorta `body` a `MAX_RENDERED_BODY_BYTES` respetando los límites de
/// carácter UTF-8, para no romper un carácter multibyte al cortar.
///
/// Devuelve el fragmento a renderizar y si hubo recorte.
fn clamp_rendered_body(body: &str) -> (&str, bool) {
    if body.len() <= MAX_RENDERED_BODY_BYTES {
        return (body, false);
    }

    let mut end = MAX_RENDERED_BODY_BYTES;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }

    (&body[..end], true)
}

/// Formatea una cantidad de bytes en una unidad legible.
fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let value = bytes as f64;

    if value >= MIB {
        format!("{:.1} MB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KB", value / KIB)
    } else {
        format!("{bytes} bytes")
    }
}

/// Tab Body: muestra `response.body_text` como texto plano.
///
/// El texto se recorta a `MAX_RENDERED_BODY_BYTES` antes de pasarlo al
/// widget, y cuando el body está recortado (acá o por el límite de retención
/// de `midway-core`) se muestra un aviso arriba para que quede claro que la
/// vista no es el payload completo.
fn body_tab<'a>(outcome: &'a ResponseOutcome, ds: &DesignSystem) -> Element<'a, Message> {
    let monospace = ds.typography.monospace;
    let response = &outcome.response;
    let (rendered, clamped_for_render) = clamp_rendered_body(response.body_text.as_str());

    // El body fue liberado para no retener payloads de tabs inactivas: no hay
    // texto que mostrar, solo el aviso.
    if response.body_evicted {
        let secondary = ds.typography.secondary;
        let size = response.total_size_bytes.unwrap_or(response.size_bytes);
        return text(format!(
            "El body de {} se liberó para ahorrar memoria. Reenviá el request para verlo de nuevo.",
            format_bytes(size)
        ))
        .size(secondary.size)
        .font(font_for(&secondary))
        .color(ds.palette.text_secondary)
        .width(Length::Fill)
        .into();
    }

    let body_text = text(rendered)
        .size(monospace.size)
        .font(font_for(&monospace))
        .width(Length::Fill);

    if !clamped_for_render && !response.truncated {
        return body_text.into();
    }

    let secondary = ds.typography.secondary;
    let notice = if response.truncated {
        match response.total_size_bytes {
            Some(total) => format!(
                "Mostrando {} de {}. La respuesta superó el límite de lectura y se truncó.",
                format_bytes(rendered.len() as u64),
                format_bytes(total)
            ),
            None => format!(
                "Mostrando {}. La respuesta superó el límite de lectura y se truncó.",
                format_bytes(rendered.len() as u64)
            ),
        }
    } else {
        format!(
            "Mostrando {} de {}. Body recortado para esta vista.",
            format_bytes(rendered.len() as u64),
            format_bytes(response.size_bytes)
        )
    };

    column![
        text(notice)
            .size(secondary.size)
            .font(font_for(&secondary))
            .color(ds.palette.text_secondary),
        body_text,
    ]
    .spacing(ds.spacing.xs)
    .width(Length::Fill)
    .into()
}

/// Tab Cookies: muestra las cookies del `CookieJarHandle` para la
/// `final_url` de la respuesta activa (Req 9.2, 9.3).
///
/// - ≥1 cookie: filas nombre/valor en el orden devuelto por el jar.
/// - 0 cookies: estado vacío "Sin cookies almacenadas" (con precedencia, Req
///   9.3), renderizado por [`crate::ui::empty_state`].
fn cookies_tab<'a>(
    state: &'a Midway,
    outcome: &'a ResponseOutcome,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let cookies = state
        .app_state
        .cookie_jar
        .read_for_url(&outcome.response.final_url);

    if cookies.is_empty() {
        return empty_state::view(empty_state::no_cookies(), ds);
    }

    let body = ds.typography.body;
    let body_font = font_for(&body);

    let rows: Vec<Element<'a, Message>> = cookies
        .into_iter()
        .map(|cookie: CookiePair| {
            row![
                text(cookie.name).size(body.size).font(body_font),
                text(cookie.value).size(body.size).font(body_font),
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
///
/// Sin assertions, el estado vacío ofrece "Agregar assertion", que despacha
/// `RequestComposerMessage::AssertionAdded`: el mismo mensaje que el botón
/// "+ Agregar assertion" del editor de la tab Tests del `Request_Composer`, así
/// que la acción agrega una assertion real al draft (Req 14.2).
fn tests_tab<'a>(outcome: &'a ResponseOutcome, ds: &DesignSystem) -> Element<'a, Message> {
    if outcome.assertions.results.is_empty() {
        return empty_state::view(
            empty_state::no_assertions(Message::RequestComposer(
                RequestComposerMessage::AssertionAdded,
            )),
            ds,
        );
    }

    let rows: Vec<Element<'_, Message>> = assertion_rows_data(outcome)
        .into_iter()
        .map(|(passed, name, expected, actual, message)| {
            let status_label = assertion_verdict_label(passed);
            column![
                row![text(status_label), text(name)].spacing(8),
                text(format!("valor esperado: {}", expected)),
                text(format!("valor obtenido: {}", actual)),
                text(message),
            ]
            .spacing(2)
            .into()
        })
        .collect();

    column(rows).spacing(8).into()
}

/// Veredicto de una assertion en el idioma de la interfaz (Req 9.6).
///
/// Reemplaza el `"PASSED"`/`"FAILED"` del baseline. Función aparte de la vista
/// para poder fijar ambas etiquetas en un test sin construir un `Element`.
fn assertion_verdict_label(passed: bool) -> &'static str {
    if passed {
        "PASÓ"
    } else {
        "FALLÓ"
    }
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
            truncated: false,
            body_evicted: false,
            total_size_bytes: None,
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
            truncated: false,
            body_evicted: false,
            total_size_bytes: None,
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

    // -----------------------------------------------------------------
    // Recorte del body para renderizado.
    //
    // `iced` shapea todo el texto de un `text()`, sin virtualizar por
    // líneas visibles, así que pasarle un payload de varios MB genera un
    // pico de memoria y de layout. `clamp_rendered_body` acota lo que llega
    // al widget sin tocar el `body_text` retenido en el envelope.
    // -----------------------------------------------------------------

    /// Un body por debajo del límite se renderiza completo y sin marcar.
    #[test]
    fn clamp_leaves_small_bodies_untouched() {
        let body = "{\"ok\":true}";

        let (rendered, clamped) = clamp_rendered_body(body);

        assert_eq!(rendered, body);
        assert!(!clamped);
    }

    /// Un body por encima del límite se recorta al máximo renderizable.
    #[test]
    fn clamp_truncates_large_bodies_to_the_cap() {
        let body = "x".repeat(MAX_RENDERED_BODY_BYTES * 3);

        let (rendered, clamped) = clamp_rendered_body(&body);

        assert!(clamped);
        assert_eq!(rendered.len(), MAX_RENDERED_BODY_BYTES);
    }

    /// El recorte respeta los límites de carácter UTF-8: nunca corta un
    /// carácter multibyte por la mitad (lo que causaría un panic al indexar).
    #[test]
    fn clamp_respects_utf8_char_boundaries() {
        // "€" ocupa 3 bytes, así que el límite no cae en un borde exacto.
        let body = "€".repeat(MAX_RENDERED_BODY_BYTES);

        let (rendered, clamped) = clamp_rendered_body(&body);

        assert!(clamped);
        assert!(rendered.len() <= MAX_RENDERED_BODY_BYTES);
        assert!(
            rendered.chars().all(|c| c == '€'),
            "el recorte no debe partir caracteres multibyte"
        );
    }

    /// `format_bytes` usa la unidad adecuada según la magnitud.
    #[test]
    fn format_bytes_picks_a_readable_unit() {
        assert_eq!(format_bytes(512), "512 bytes");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    // -----------------------------------------------------------------
    // Idioma del veredicto de assertions (Tarea 11.3, Requisito 9.6).
    //
    // El baseline rotulaba cada resultado con "PASSED"/"FAILED" en texto
    // visible. Los dos tests siguientes fijan las etiquetas en español y
    // dejan un tripwire para que las de inglés no vuelvan.
    // -----------------------------------------------------------------

    #[test]
    fn assertion_verdict_labels_are_in_spanish() {
        assert_eq!(assertion_verdict_label(true), "PASÓ");
        assert_eq!(assertion_verdict_label(false), "FALLÓ");
    }

    #[test]
    fn retired_english_verdict_labels_do_not_come_back() {
        for passed in [true, false] {
            let label = assertion_verdict_label(passed);
            assert_ne!(label, "PASSED");
            assert_ne!(label, "FAILED");
        }
    }
}
