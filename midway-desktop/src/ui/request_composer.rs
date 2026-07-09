//! `Request_Composer` (Tarea 3.2, Fase 1): fila superior del composer.
//!
//! Implementa únicamente la fila superior descrita en el diseño ("Método +
//! URL + Send + environment selector + settings"): `pick_list` de método
//! HTTP, `text_input` de URL, botón Send, `pick_list` de environment y
//! botón de settings (ícono de engranaje). El resto del `Request_Composer`
//! (Curl_Importer, tabs de configuración, preview drawer, ejecución real de
//! Send) se implementa en tareas posteriores (3.3 en adelante).
//!
//! Ver diseño: "Components and Interfaces > Request_Composer y
//! Response_Inspector (Fase 1)".
//! Ver requisitos: 2.1, 2.2, 2.3, 2.4, 2.5.

use iced::widget::{button, checkbox, column, pick_list, row, text, text_input};
use iced::{Color, Element, Length};

use midway_core::domain::http::{ApiKeyPlacement, AuthConfig, HttpMethod, KeyValueRow, RequestPreview};
use midway_core::domain::testing::{AssertionOperator, AssertionSource, ResponseAssertion};

use crate::app::{Message, Midway, RequestComposerMessage, RequestTab, RequestTabState};
use crate::ui::tab_bar;

/// Ítem del `pick_list` de environment: envuelve el id del environment
/// (`None` = "sin environment") junto con el nombre a mostrar, ya que
/// `pick_list` requiere `ToString` para renderizar cada opción y
/// `domain::workspace::EnvironmentRecord` no implementa dicho trait.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EnvironmentOption {
    id: Option<String>,
    name: String,
}

impl std::fmt::Display for EnvironmentOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// Construye la fila superior del `Request_Composer` para la tab activa.
///
/// El llamador (`app::view`) es responsable de solo invocar esta función
/// cuando existe una tab activa (`Midway::active_tab` es `Some`).
pub fn view<'a>(state: &'a Midway, active_tab_index: usize) -> Element<'a, Message> {
    let active_tab = &state.tabs[active_tab_index];
    let draft = &active_tab.draft;

    let method_picker = pick_list(
        HttpMethod::ALL,
        Some(draft.method),
        |method| Message::RequestComposer(RequestComposerMessage::MethodChanged(method)),
    )
    .placeholder("Método");

    // `on_paste` captura específicamente el evento de "pegar" del
    // `text_input` (distinto de `on_input`, que dispara en cada tecla); el
    // enrutamiento del Curl_Importer (Tarea 3.5) se despacha solo desde ahí.
    let url_input = text_input("https://api.ejemplo.com/recurso", &draft.url)
        .on_input(|url| Message::RequestComposer(RequestComposerMessage::UrlChanged(url)))
        .on_paste(|pasted| Message::RequestComposer(RequestComposerMessage::UrlPasted(pasted)))
        .width(Length::Fill);

    let send_button = button(text(if active_tab.sending { "Sending…" } else { "Send" }));
    let send_button = if active_tab.sending {
        send_button
    } else {
        send_button.on_press(Message::RequestComposer(RequestComposerMessage::SendPressed))
    };

    let environment_options: Vec<EnvironmentOption> = std::iter::once(EnvironmentOption {
        id: None,
        name: "Sin environment".to_string(),
    })
    .chain(state.workspace.environments.iter().map(|environment| EnvironmentOption {
        id: Some(environment.id.clone()),
        name: environment.name.clone(),
    }))
    .collect();

    let selected_environment = environment_options
        .iter()
        .find(|option| option.id == draft.environment_id)
        .cloned();

    let environment_picker = pick_list(
        environment_options,
        selected_environment,
        |option: EnvironmentOption| {
            Message::RequestComposer(RequestComposerMessage::EnvironmentChanged(option.id))
        },
    )
    .placeholder("Sin environment");

    let settings_button = button(text("⚙"))
        .on_press(Message::RequestComposer(RequestComposerMessage::SettingsPressed));

    let top_row = row![
        method_picker,
        url_input,
        send_button,
        environment_picker,
        settings_button,
    ]
    .spacing(8);

    // Criterio 2.19: si el último pegado de cURL falló al parsear, se
    // muestra el mensaje de error debajo de la fila superior sin haber
    // modificado el contenido existente de la URL. Análogamente (Tarea
    // 3.18), si la última ejecución de Send falló, se muestra su mensaje.
    let mut content = column![top_row].spacing(4);

    if let Some(error_message) = &active_tab.curl_paste_error {
        content = content.push(text(error_message.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
    }

    if let Some(error_message) = &active_tab.send_error {
        content = content.push(text(error_message.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
    }

    if let Some(error_message) = &active_tab.preview_error {
        content = content.push(text(error_message.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
    }

    // Tarea 5.1 (Requisito 3.1): contenedor de tabs de configuración del
    // request (Params/Headers/Auth/Body/Tests). Solo el widget de tabs y el
    // cambio de tab activa; el contenido real de cada tab (editor de
    // params/headers, auth, tests) se implementa en tareas posteriores
    // (5.4, 5.6, 5.8) y por ahora muestra un placeholder.
    content = content.push(config_tabs(active_tab));

    // Tarea 3.20 (Requisito 2.18): preview drawer. Se muestra debajo de la
    // fila superior siempre que `tab.preview` esté poblado (drawer
    // "abierto"); `tab.preview` en `None` significa "drawer cerrado" (no se
    // usa un booleano separado: ver diseño de la Tarea 3.20).
    if let Some(preview) = &active_tab.preview {
        content = content.push(preview_view(preview));
    }

    content.into()
}

/// Construye el contenedor de tabs Params/Headers/Auth/Body/Tests (Tarea
/// 5.1, Requisito 3.1) usando el widget compartido `ui::tab_bar` (envoltura
/// de `iced_aw::{TabBar, Tabs}`), reutilizado también por el
/// `Response_Inspector` y (en la Fase 3) por el `Workspace_Panel`.
///
/// La tab Auth (Tarea 5.4), las tabs Params/Headers (Tarea 5.6) y la tab
/// Tests (Tarea 5.8) ya tienen su editor real; solo Body sigue mostrando un
/// placeholder.
fn config_tabs(active_tab: &RequestTabState) -> Element<'_, Message> {
    let entries = vec![
        (
            RequestTab::Params,
            "Params",
            key_value_editor(&active_tab.draft.query, KeyValueTarget::Query),
        ),
        (
            RequestTab::Headers,
            "Headers",
            key_value_editor(&active_tab.draft.headers, KeyValueTarget::Headers),
        ),
        (RequestTab::Auth, "Auth", auth_tab(active_tab)),
        (RequestTab::Body, "Body", config_tab_placeholder("Body")),
        (
            RequestTab::Tests,
            "Tests",
            tests_tab_editor(&active_tab.draft.response_tests),
        ),
    ];

    tab_bar::tabs(entries, &active_tab.active_request_tab, |tab| {
        Message::RequestComposer(RequestComposerMessage::ConfigTabSelected(tab))
    })
}

/// Ítem del `pick_list` de tipo de autenticación (Tarea 5.4, Requisitos
/// 3.4-3.7): "kind" sin datos, distinto de `domain::http::AuthConfig`,
/// porque `pick_list` requiere que su tipo de ítem sea `PartialEq + Clone`
/// y renderizable vía `Display`, algo incómodo de expresar directamente
/// sobre `AuthConfig` (sus variantes cargan los campos de cada tipo de
/// auth). Mismo patrón que `RequestTab`/`ResponseInspectorTab`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    None,
    Bearer,
    Basic,
    ApiKey,
}

impl AuthKind {
    const ALL: [AuthKind; 4] = [AuthKind::None, AuthKind::Bearer, AuthKind::Basic, AuthKind::ApiKey];

    /// Deriva el `AuthKind` correspondiente a la variante actual de
    /// `AuthConfig`, para preseleccionar el `pick_list`.
    fn from_auth_config(auth: &AuthConfig) -> Self {
        match auth {
            AuthConfig::None => AuthKind::None,
            AuthConfig::Bearer { .. } => AuthKind::Bearer,
            AuthConfig::Basic { .. } => AuthKind::Basic,
            AuthConfig::ApiKey { .. } => AuthKind::ApiKey,
        }
    }
}

impl std::fmt::Display for AuthKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AuthKind::None => "None",
            AuthKind::Bearer => "Bearer",
            AuthKind::Basic => "Basic",
            AuthKind::ApiKey => "API Key",
        };
        f.write_str(label)
    }
}

/// Envoltura de `domain::http::ApiKeyPlacement` para el `pick_list` de
/// ubicación de la tab Auth (Tarea 5.4, Requisito 3.7): `ApiKeyPlacement`
/// no implementa `Display` en `midway-core` (es un tipo de dominio sin
/// preocupaciones de presentación), así que se envuelve localmente en vez
/// de agregarle ese impl al crate de dominio por un detalle puramente de UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ApiKeyPlacementOption(ApiKeyPlacement);

impl std::fmt::Display for ApiKeyPlacementOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.0 {
            ApiKeyPlacement::Header => "Header",
            ApiKeyPlacement::Query => "Query Param",
        };
        f.write_str(label)
    }
}

const API_KEY_PLACEMENT_OPTIONS: [ApiKeyPlacementOption; 2] = [
    ApiKeyPlacementOption(ApiKeyPlacement::Header),
    ApiKeyPlacementOption(ApiKeyPlacement::Query),
];

/// Construye el editor de la tab Auth (Tarea 5.4, Requisitos 3.4-3.7,
/// 3.10): `pick_list` de tipo sobre `domain::http::AuthConfig` seguido de
/// los campos propios de la variante actualmente seleccionada.
fn auth_tab(active_tab: &RequestTabState) -> Element<'_, Message> {
    let auth = &active_tab.draft.auth;
    let selected_kind = AuthKind::from_auth_config(auth);

    let type_picker = pick_list(AuthKind::ALL, Some(selected_kind), |kind| {
        Message::RequestComposer(RequestComposerMessage::AuthTypeChanged(kind))
    })
    .placeholder("Tipo de autenticación");

    let mut content = column![type_picker].spacing(8);

    content = match auth {
        AuthConfig::None => content,
        AuthConfig::Bearer { token } => content.push(
            text_input("Token", token)
                .on_input(|token| Message::RequestComposer(RequestComposerMessage::BearerTokenChanged(token)))
                .width(Length::Fill),
        ),
        AuthConfig::Basic { username, password } => content
            .push(
                text_input("Usuario", username)
                    .on_input(|username| {
                        Message::RequestComposer(RequestComposerMessage::BasicUsernameChanged(username))
                    })
                    .width(Length::Fill),
            )
            .push(
                text_input("Contraseña", password)
                    .on_input(|password| {
                        Message::RequestComposer(RequestComposerMessage::BasicPasswordChanged(password))
                    })
                    .secure(true)
                    .width(Length::Fill),
            ),
        AuthConfig::ApiKey { key, value, placement } => {
            let placement_picker = pick_list(
                API_KEY_PLACEMENT_OPTIONS,
                Some(ApiKeyPlacementOption(*placement)),
                |option: ApiKeyPlacementOption| {
                    Message::RequestComposer(RequestComposerMessage::ApiKeyPlacementChanged(option.0))
                },
            )
            .placeholder("Ubicación");

            content
                .push(
                    text_input("Nombre de la clave", key)
                        .on_input(|key| Message::RequestComposer(RequestComposerMessage::ApiKeyKeyChanged(key)))
                        .width(Length::Fill),
                )
                .push(
                    text_input("Valor", value)
                        .on_input(|value| {
                            Message::RequestComposer(RequestComposerMessage::ApiKeyValueChanged(value))
                        })
                        .width(Length::Fill),
                )
                .push(placement_picker)
        }
    };

    content.into()
}

/// Discrimina a qué lista de `domain::http::KeyValueRow` del draft apunta
/// una acción del editor key/value (Tarea 5.6, Requisito 3.8): `Query`
/// para la tab Params (`draft.query`) y `Headers` para la tab Headers
/// (`draft.headers`). Mismo patrón que `AuthKind`/`RequestTab`: un
/// selector pequeño propio de la UI (enrutamiento de mensajes), no un tipo
/// de dominio, ya que `midway-core` no necesita distinguir "para qué tab"
/// se usa un `Vec<KeyValueRow>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyValueTarget {
    Query,
    Headers,
}

/// Construye el editor de filas key/value compartido por las tabs Params y
/// Headers (Tarea 5.6, Requisito 3.8): una fila por `KeyValueRow` (checkbox
/// `enabled` + `text_input` de key + `text_input` de value + botón de
/// eliminar) y un botón "+ Agregar fila" al final, siempre visible (incluso
/// con `rows` vacío, para poder agregar la primera fila).
fn key_value_editor(rows: &[KeyValueRow], target: KeyValueTarget) -> Element<'_, Message> {
    let mut content = column![].spacing(4);

    for row_data in rows {
        let row_id = row_data.id.clone();
        let row_id_for_key = row_id.clone();
        let row_id_for_value = row_id.clone();
        let row_id_for_remove = row_id.clone();

        let enabled_checkbox = checkbox(row_data.enabled).on_toggle(move |_| {
            Message::RequestComposer(RequestComposerMessage::KeyValueRowEnabledToggled {
                target,
                row_id: row_id.clone(),
            })
        });

        let key_input = text_input("Key", &row_data.key)
            .on_input(move |key| {
                Message::RequestComposer(RequestComposerMessage::KeyValueRowKeyChanged {
                    target,
                    row_id: row_id_for_key.clone(),
                    key,
                })
            })
            .width(Length::Fill);

        let value_input = text_input("Value", &row_data.value)
            .on_input(move |value| {
                Message::RequestComposer(RequestComposerMessage::KeyValueRowValueChanged {
                    target,
                    row_id: row_id_for_value.clone(),
                    value,
                })
            })
            .width(Length::Fill);

        let remove_button = button(text("×")).on_press(Message::RequestComposer(
            RequestComposerMessage::KeyValueRowRemoved {
                target,
                row_id: row_id_for_remove.clone(),
            },
        ));

        content = content.push(row![enabled_checkbox, key_input, value_input, remove_button].spacing(8));
    }

    let add_button = button(text("+ Agregar fila"))
        .on_press(Message::RequestComposer(RequestComposerMessage::KeyValueRowAdded(target)));

    content = content.push(add_button);

    content.into()
}

/// Contenido placeholder de una tab de configuración del request, hasta que
/// la tarea correspondiente (Body) implemente su editor real.
fn config_tab_placeholder(tab_name: &'static str) -> Element<'static, Message> {
    text(format!("Tab {} (contenido en tareas posteriores)", tab_name)).into()
}

/// Envoltura de `domain::testing::AssertionSource` para el `pick_list` de
/// la tab Tests (Tarea 5.8, Requisito 3.9): al igual que
/// `ApiKeyPlacementOption`, `AssertionSource` es un tipo de dominio sin
/// preocupaciones de presentación (no implementa `Display`), así que se
/// envuelve localmente en vez de agregarle ese impl al crate de dominio por
/// un detalle puramente de UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AssertionSourceOption(AssertionSource);

impl std::fmt::Display for AssertionSourceOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.0 {
            AssertionSource::Status => "Status",
            AssertionSource::Header => "Header",
            AssertionSource::BodyText => "Body (texto)",
            AssertionSource::JsonPointer => "JSON Pointer",
            AssertionSource::FinalUrl => "Final URL",
        };
        f.write_str(label)
    }
}

const ASSERTION_SOURCE_OPTIONS: [AssertionSourceOption; 5] = [
    AssertionSourceOption(AssertionSource::Status),
    AssertionSourceOption(AssertionSource::Header),
    AssertionSourceOption(AssertionSource::BodyText),
    AssertionSourceOption(AssertionSource::JsonPointer),
    AssertionSourceOption(AssertionSource::FinalUrl),
];

/// Envoltura de `domain::testing::AssertionOperator` para el `pick_list` de
/// la tab Tests (Tarea 5.8, Requisito 3.9), mismo motivo que
/// `AssertionSourceOption`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AssertionOperatorOption(AssertionOperator);

impl std::fmt::Display for AssertionOperatorOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.0 {
            AssertionOperator::Equals => "Equals",
            AssertionOperator::Contains => "Contains",
            AssertionOperator::NotContains => "Not Contains",
            AssertionOperator::Exists => "Exists",
            AssertionOperator::NotExists => "Not Exists",
            AssertionOperator::GreaterOrEqual => "Greater or Equal",
            AssertionOperator::LessOrEqual => "Less or Equal",
        };
        f.write_str(label)
    }
}

const ASSERTION_OPERATOR_OPTIONS: [AssertionOperatorOption; 7] = [
    AssertionOperatorOption(AssertionOperator::Equals),
    AssertionOperatorOption(AssertionOperator::Contains),
    AssertionOperatorOption(AssertionOperator::NotContains),
    AssertionOperatorOption(AssertionOperator::Exists),
    AssertionOperatorOption(AssertionOperator::NotExists),
    AssertionOperatorOption(AssertionOperator::GreaterOrEqual),
    AssertionOperatorOption(AssertionOperator::LessOrEqual),
];

/// Construye el editor de la tab Tests (Tarea 5.8, Requisito 3.9): una fila
/// por `domain::testing::ResponseAssertion` (checkbox `enabled` +
/// `text_input` de `name` + `pick_list` de `source` + `pick_list` de
/// `operator` + `text_input` de `selector` opcional + `text_input` de
/// `expected` + botón de eliminar) y un botón "+ Agregar assertion" al
/// final, siempre visible (incluso con `assertions` vacío, para poder
/// agregar la primera assertion). La evaluación de estas assertions contra
/// una respuesta ya se realiza en otro punto del flujo de Send (Tarea 3.18,
/// `domain::testing::evaluate_response_assertions`); este editor solo
/// gestiona `draft.response_tests`.
fn tests_tab_editor(assertions: &[ResponseAssertion]) -> Element<'_, Message> {
    let mut content = column![].spacing(4);

    for assertion in assertions {
        let assertion_id = assertion.id.clone();
        let id_for_name = assertion_id.clone();
        let id_for_enabled = assertion_id.clone();
        let id_for_source = assertion_id.clone();
        let id_for_operator = assertion_id.clone();
        let id_for_selector = assertion_id.clone();
        let id_for_expected = assertion_id.clone();
        let id_for_remove = assertion_id.clone();

        let enabled_checkbox = checkbox(assertion.enabled).on_toggle(move |_| {
            Message::RequestComposer(RequestComposerMessage::AssertionEnabledToggled {
                assertion_id: id_for_enabled.clone(),
            })
        });

        let name_input = text_input("Nombre", &assertion.name)
            .on_input(move |name| {
                Message::RequestComposer(RequestComposerMessage::AssertionNameChanged {
                    assertion_id: id_for_name.clone(),
                    name,
                })
            })
            .width(Length::Fill);

        let source_picker = pick_list(
            ASSERTION_SOURCE_OPTIONS,
            Some(AssertionSourceOption(assertion.source)),
            move |option: AssertionSourceOption| {
                Message::RequestComposer(RequestComposerMessage::AssertionSourceChanged {
                    assertion_id: id_for_source.clone(),
                    source: option.0,
                })
            },
        )
        .placeholder("Source");

        let operator_picker = pick_list(
            ASSERTION_OPERATOR_OPTIONS,
            Some(AssertionOperatorOption(assertion.operator)),
            move |option: AssertionOperatorOption| {
                Message::RequestComposer(RequestComposerMessage::AssertionOperatorChanged {
                    assertion_id: id_for_operator.clone(),
                    operator: option.0,
                })
            },
        )
        .placeholder("Operator");

        let selector_input = text_input("Selector (opcional)", &assertion.selector.clone().unwrap_or_default())
            .on_input(move |selector| {
                Message::RequestComposer(RequestComposerMessage::AssertionSelectorChanged {
                    assertion_id: id_for_selector.clone(),
                    selector,
                })
            })
            .width(Length::Fill);

        let expected_input = text_input("Expected", &assertion.expected)
            .on_input(move |expected| {
                Message::RequestComposer(RequestComposerMessage::AssertionExpectedChanged {
                    assertion_id: id_for_expected.clone(),
                    expected,
                })
            })
            .width(Length::Fill);

        let remove_button = button(text("×")).on_press(Message::RequestComposer(
            RequestComposerMessage::AssertionRemoved {
                assertion_id: id_for_remove.clone(),
            },
        ));

        content = content.push(
            row![
                enabled_checkbox,
                name_input,
                source_picker,
                operator_picker,
                selector_input,
                expected_input,
                remove_button,
            ]
            .spacing(8),
        );
    }

    let add_button = button(text("+ Agregar assertion"))
        .on_press(Message::RequestComposer(RequestComposerMessage::AssertionAdded));

    content = content.push(add_button);

    content.into()
}

/// Renderiza el contenido del preview drawer (Tarea 3.20, Requisito 2.18):
/// método, URL final, headers, body y comando cURL equivalente de la
/// request resuelta. Una lista vertical simple de `text(...)` es suficiente
/// para este alcance (la integración completa con el
/// `Text_Editor_Component` es una mejora posterior, no requerida por el
/// Requisito 2.18).
fn preview_view(preview: &RequestPreview) -> Element<'_, Message> {
    let mut content = column![
        text("Preview").size(16),
        text(format!("{} {}", preview.method, preview.resolved_url)),
        text("Headers:"),
    ]
    .spacing(4);

    if preview.headers.is_empty() {
        content = content.push(text("  (sin headers)"));
    } else {
        for header in &preview.headers {
            content = content.push(text(format!("  {}: {}", header.key, header.value)));
        }
    }

    content = content.push(text("Body:"));
    content = match &preview.body_text {
        Some(body_text) if !body_text.is_empty() => content.push(text(body_text.clone())),
        _ => content.push(text("  (sin body)")),
    };

    content = content.push(text("cURL:"));
    content = content.push(text(preview.curl_command.clone()));

    content.into()
}
