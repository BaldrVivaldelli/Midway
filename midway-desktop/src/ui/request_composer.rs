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
use iced::{font, Border, Color, Element, Font, Length};

use midway_core::domain::http::{
    ApiKeyPlacement, AuthConfig, BodyMode, FormDataFieldKind, FormDataRow, HttpMethod, KeyValueRow,
    RequestPreview,
};
use midway_core::domain::testing::{AssertionOperator, AssertionSource, ResponseAssertion};

use crate::app::{Message, Midway, RequestComposerMessage, RequestTab, RequestTabState};
use crate::ui::design_system::{contrast_text_color, method_color, DesignSystem, TextStyle};
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

/// Devuelve el nombre que se muestra junto al selector de método.
///
/// Los requests creados desde cURL/OpenAPI pueden traer nombres como
/// `POST posts 1`. Como el toolbar ya muestra `POST` en su selector, repetir
/// ese prefijo añade ruido (`POST POST posts 1`). Solo se normaliza la
/// presentación: el nombre persistido permanece intacto.
fn display_request_name(name: &str, method: HttpMethod) -> &str {
    let trimmed = name.trim();
    let Some(separator) = trimmed.find(char::is_whitespace) else {
        return trimmed;
    };

    let (prefix, remainder) = trimmed.split_at(separator);
    let remainder = remainder.trim_start();

    if !remainder.is_empty() && prefix.eq_ignore_ascii_case(&method.to_string()) {
        remainder
    } else {
        trimmed
    }
}

/// Altura cómoda para editar JSON/texto dentro del panel scrolleable.
///
/// El contenedor exterior ya administra overflow en ventanas bajas, por lo
/// que este tamaño aprovecha las pantallas habituales sin perder acceso a
/// controles cuando la ventana es pequeña.
const BODY_TEXT_EDITOR_HEIGHT: f32 = 420.0;

/// Etiqueta de la opción "ningún environment seleccionado" del `pick_list` de
/// environment (Req 9.6).
///
/// Reemplaza el "No Environment" del baseline. `environment` se conserva porque
/// es el término de dominio que ya usan el resto de los textos en español de la
/// app ("El environment necesita un nombre.", "No hay environments todavía."):
/// traducirlo solo acá rompería la consistencia en vez de arreglarla.
///
/// Vive en una constante porque la misma cadena se usa como opción de la lista
/// y como placeholder, y la coincidencia entre ambas es lo que hace que la
/// opción seleccionada se vea igual que el estado por defecto.
const SIN_ENVIRONMENT_LABEL: &str = "Sin environment";

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

/// Encabezado del request: nombre, URL, ejecución, guardado y environment.
pub fn toolbar<'a>(
    state: &'a Midway,
    active_tab_index: usize,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let active_tab = &state.tabs[active_tab_index];
    let draft = &active_tab.draft;
    let body = ds.typography.body;
    let body_font = font_for(&body);
    let control_radius = ds.radius.control;
    let accent = ds.palette.accent;

    // --- URL bar: method pill + url input + send pill ---
    let m_color = method_color(draft.method);
    let m_text_color = contrast_text_color(m_color);
    let method_picker = pick_list(HttpMethod::ALL, Some(draft.method), |method| {
        Message::RequestComposer(RequestComposerMessage::MethodChanged(method))
    })
    .placeholder("GET")
    .text_size(body.size)
    .font(font_for(&TextStyle {
        weight: iced::font::Weight::Bold,
        ..body
    }))
    .style(move |theme, status| {
        let mut style = pick_list::default(theme, status);
        style.border = Border {
            radius: (control_radius + 4.0).into(),
            ..style.border
        };
        style.background = m_color.into();
        style.text_color = m_text_color;
        style
    });

    let url_input = text_input("https://api.example.com/endpoint", &draft.url)
        .on_input(|url| Message::RequestComposer(RequestComposerMessage::UrlChanged(url)))
        .on_paste(|pasted| Message::RequestComposer(RequestComposerMessage::UrlPasted(pasted)))
        .width(Length::Fill)
        .size(body.size)
        .font(body_font)
        .style(move |theme, status| {
            let mut style = text_input::default(theme, status);
            style.border = Border {
                radius: control_radius.into(),
                ..style.border
            };
            style
        });

    let send_text_color = contrast_text_color(accent);
    let send_button_content = if active_tab.sending {
        text("Enviando…").size(body.size).font(body_font)
    } else {
        text("Enviar").size(body.size).font(font_for(&TextStyle {
            weight: iced::font::Weight::Bold,
            ..body
        }))
    };

    let sending = active_tab.sending;
    let send_button = button(send_button_content)
        .padding([ds.spacing.sm, ds.spacing.lg])
        .style(move |_theme, _status| {
            let bg = if sending {
                // "Sending" state: dimmed accent (50% opacity blend with black)
                Color::from_rgba(accent.r * 0.5, accent.g * 0.5, accent.b * 0.5, 0.7)
            } else {
                accent
            };
            let txt = if sending {
                Color {
                    a: 0.6,
                    ..send_text_color
                }
            } else {
                send_text_color
            };
            button::Style {
                background: Some(bg.into()),
                text_color: txt,
                border: Border {
                    radius: (control_radius + 4.0).into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        });
    let send_button = if active_tab.sending {
        send_button
    } else {
        send_button.on_press(Message::RequestComposer(
            RequestComposerMessage::SendPressed,
        ))
    };

    let saved = active_tab
        .saved_draft
        .as_ref()
        .is_some_and(|saved_draft| saved_draft == draft);
    let save_label = if saved { "Guardado" } else { "Guardar" };
    let save_button = button(text(save_label).size(body.size).font(body_font))
        .padding([ds.spacing.sm, ds.spacing.md])
        .on_press(Message::RequestComposer(
            RequestComposerMessage::SaveRequested,
        ));

    // Environment + settings on the right
    let environment_options: Vec<EnvironmentOption> = std::iter::once(EnvironmentOption {
        id: None,
        name: SIN_ENVIRONMENT_LABEL.to_string(),
    })
    .chain(
        state
            .workspace
            .environments
            .iter()
            .map(|environment| EnvironmentOption {
                id: Some(environment.id.clone()),
                name: environment.name.clone(),
            }),
    )
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
    .placeholder(SIN_ENVIRONMENT_LABEL)
    .text_size(body.size)
    .font(body_font)
    .style(move |theme, status| {
        let mut style = pick_list::default(theme, status);
        style.border = Border {
            radius: control_radius.into(),
            ..style.border
        };
        style
    });

    let settings_button = button(text("⚙").size(body.size).font(body_font))
        .padding(ds.spacing.sm)
        .on_press(Message::RequestComposer(
            RequestComposerMessage::SettingsPressed,
        ));

    let url_bar = row![method_picker, url_input, send_button]
        .spacing(ds.spacing.sm)
        .align_y(iced::alignment::Vertical::Center);
    let request_actions = row![save_button, environment_picker, settings_button]
        .spacing(ds.spacing.sm)
        .align_y(iced::alignment::Vertical::Center);

    let request_name = if draft.name.trim().is_empty() {
        "Nueva petición"
    } else {
        display_request_name(&draft.name, draft.method)
    };
    let save_state = match active_tab.saved_draft.as_ref() {
        Some(_) if saved => "Guardado",
        Some(_) => "Cambios sin guardar",
        None => "Sin guardar",
    };
    let title = row![
        text(request_name)
            .size(ds.typography.subtitle.size)
            .color(ds.palette.text_primary),
        text(save_state)
            .size(ds.typography.secondary.size)
            .color(ds.palette.text_secondary),
    ]
    .spacing(ds.spacing.sm)
    .align_y(iced::alignment::Vertical::Center);

    column![title, url_bar, request_actions]
        .spacing(ds.spacing.sm)
        .width(Length::Fill)
        .into()
}

/// Editor de Params/Headers/Auth/Body/Tests, separado del toolbar para que
/// pueda ubicarse junto al inspector de respuesta en pantallas amplias.
pub fn editor<'a>(
    state: &'a Midway,
    active_tab_index: usize,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let active_tab = &state.tabs[active_tab_index];
    let mut content = column![config_tabs(active_tab, ds)]
        .spacing(ds.spacing.md)
        .width(Length::Fill);

    // Error messages
    if let Some(error_message) = &active_tab.curl_paste_error {
        content = content.push(text(error_message.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
    }
    if let Some(error_message) = &active_tab.send_error {
        content = content.push(text(error_message.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
    }
    if let Some(error_message) = &active_tab.preview_error {
        content = content.push(text(error_message.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
    }

    // Preview drawer
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
fn config_tabs<'a>(active_tab: &'a RequestTabState, ds: &DesignSystem) -> Element<'a, Message> {
    let entries: Vec<tab_bar::TabEntry<'a, Message, RequestTab>> = vec![
        (
            RequestTab::Params,
            "Params",
            Box::new(move || key_value_editor(&active_tab.draft.query, KeyValueTarget::Query)),
        ),
        (
            RequestTab::Headers,
            "Headers",
            Box::new(move || key_value_editor(&active_tab.draft.headers, KeyValueTarget::Headers)),
        ),
        (
            RequestTab::Auth,
            "Auth",
            Box::new(move || auth_tab(active_tab)),
        ),
        (
            RequestTab::Body,
            "Body",
            Box::new(move || body_tab(active_tab)),
        ),
        (
            RequestTab::Tests,
            "Tests",
            Box::new(move || tests_tab_editor(&active_tab.draft.response_tests)),
        ),
    ];

    // El tab bar compartido usa separación generosa para vistas de ancho
    // completo. El composer comparte espacio con la respuesta, así que usa
    // tokens compactos para conservar las cinco etiquetas completas incluso
    // en un panel de unos 280 px.
    let mut compact_ds = *ds;
    compact_ds.spacing.lg = ds.spacing.xs;
    compact_ds.spacing.sm = ds.spacing.xs;

    tab_bar::tabs(
        entries,
        &active_tab.active_request_tab,
        |tab| Message::RequestComposer(RequestComposerMessage::ConfigTabSelected(tab)),
        &compact_ds,
    )
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
    const ALL: [AuthKind; 4] = [
        AuthKind::None,
        AuthKind::Bearer,
        AuthKind::Basic,
        AuthKind::ApiKey,
    ];

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
    /// `Bearer`, `Basic` y `API Key` son los nombres canónicos de los esquemas
    /// de autenticación HTTP y se conservan tal cual (excepción registrada en
    /// `docs/known-limitations.md`); lo que sí pasa a español es la opción que
    /// no nombra un esquema sino su ausencia (Req 9.6).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            AuthKind::None => "Sin autenticación",
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
            ApiKeyPlacement::Query => "Parámetro de query",
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
                .on_input(|token| {
                    Message::RequestComposer(RequestComposerMessage::BearerTokenChanged(token))
                })
                .width(Length::Fill),
        ),
        AuthConfig::Basic { username, password } => content
            .push(
                text_input("Usuario", username)
                    .on_input(|username| {
                        Message::RequestComposer(RequestComposerMessage::BasicUsernameChanged(
                            username,
                        ))
                    })
                    .width(Length::Fill),
            )
            .push(
                text_input("Contraseña", password)
                    .on_input(|password| {
                        Message::RequestComposer(RequestComposerMessage::BasicPasswordChanged(
                            password,
                        ))
                    })
                    .secure(true)
                    .width(Length::Fill),
            ),
        AuthConfig::ApiKey {
            key,
            value,
            placement,
        } => {
            let placement_picker = pick_list(
                API_KEY_PLACEMENT_OPTIONS,
                Some(ApiKeyPlacementOption(*placement)),
                |option: ApiKeyPlacementOption| {
                    Message::RequestComposer(RequestComposerMessage::ApiKeyPlacementChanged(
                        option.0,
                    ))
                },
            )
            .placeholder("Ubicación");

            content
                .push(
                    text_input("Nombre de la clave", key)
                        .on_input(|key| {
                            Message::RequestComposer(RequestComposerMessage::ApiKeyKeyChanged(key))
                        })
                        .width(Length::Fill),
                )
                .push(
                    text_input("Valor", value)
                        .on_input(|value| {
                            Message::RequestComposer(RequestComposerMessage::ApiKeyValueChanged(
                                value,
                            ))
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

        let key_input = text_input("Clave", &row_data.key)
            .on_input(move |key| {
                Message::RequestComposer(RequestComposerMessage::KeyValueRowKeyChanged {
                    target,
                    row_id: row_id_for_key.clone(),
                    key,
                })
            })
            .width(Length::Fill);

        let value_input = text_input("Valor", &row_data.value)
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

        content =
            content.push(row![enabled_checkbox, key_input, value_input, remove_button].spacing(8));
    }

    let add_button = button(text("+ Agregar fila")).on_press(Message::RequestComposer(
        RequestComposerMessage::KeyValueRowAdded(target),
    ));

    content = content.push(add_button);

    content.into()
}

/// Envoltura de `domain::http::BodyMode` para el `pick_list` de modo de la
/// tab Body (Requisito 3.1): `BodyMode` no implementa `Display` en
/// `midway-core` (no es su responsabilidad presentacional), así que se
/// envuelve localmente igual que `AuthKind`/`ApiKeyPlacementOption`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BodyModeOption(BodyMode);

impl std::fmt::Display for BodyModeOption {
    /// `JSON` y `Form data` nombran formatos de payload (el segundo, el
    /// `multipart/form-data` del protocolo) y se conservan; `None` y `Text`
    /// pasan a español porque describen el modo, no un formato con nombre
    /// propio (Req 9.6).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.0 {
            BodyMode::None => "Sin body",
            BodyMode::Json => "JSON",
            BodyMode::Text => "Texto",
            BodyMode::FormData => "Form data",
        };
        f.write_str(label)
    }
}

const BODY_MODE_OPTIONS: [BodyModeOption; 4] = [
    BodyModeOption(BodyMode::None),
    BodyModeOption(BodyMode::Json),
    BodyModeOption(BodyMode::Text),
    BodyModeOption(BodyMode::FormData),
];

/// Envoltura de `domain::http::FormDataFieldKind` para el `pick_list` de
/// tipo de campo del editor FormData (Text/File), mismo motivo que
/// `BodyModeOption`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FormDataFieldKindOption(FormDataFieldKind);

impl std::fmt::Display for FormDataFieldKindOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.0 {
            FormDataFieldKind::Text => "Texto",
            FormDataFieldKind::File => "Archivo",
        };
        f.write_str(label)
    }
}

const FORM_DATA_FIELD_KIND_OPTIONS: [FormDataFieldKindOption; 2] = [
    FormDataFieldKindOption(FormDataFieldKind::Text),
    FormDataFieldKindOption(FormDataFieldKind::File),
];

/// Construye el editor de la tab Body: `pick_list` de modo (None/Json/
/// Text/FormData) sobre `draft.body.mode`, seguido del editor
/// correspondiente al modo seleccionado.
///
/// - `None`: sin contenido adicional (la petición no lleva body).
/// - `Json`/`Text`: `Text_Editor_Component` (`ui::text_editor`) con
///   resaltado de sintaxis JSON, sincronizado con `draft.body.value` vía
///   `RequestComposerMessage::BodyTextAction`.
/// - `FormData`: editor de filas `domain::http::FormDataRow` (checkbox
///   `enabled` + `text_input` de key + `pick_list` Text/File + `text_input`
///   de value, que para campos File representa el path del archivo en
///   disco) y botón "+ Agregar campo".
fn body_tab(active_tab: &RequestTabState) -> Element<'_, Message> {
    let mode_picker = pick_list(
        BODY_MODE_OPTIONS,
        Some(BodyModeOption(active_tab.draft.body.mode)),
        |option: BodyModeOption| {
            Message::RequestComposer(RequestComposerMessage::BodyModeChanged(option.0))
        },
    )
    .placeholder("Modo de body");

    let mut content = column![mode_picker].spacing(8).width(Length::Fill);

    content = match active_tab.draft.body.mode {
        BodyMode::None => content,
        BodyMode::Json | BodyMode::Text => {
            let editor = active_tab
                .body_editor
                .view()
                .height(Length::Fixed(BODY_TEXT_EDITOR_HEIGHT))
                .on_action(|action| {
                    Message::RequestComposer(RequestComposerMessage::BodyTextAction(action))
                });
            content.push(editor)
        }
        BodyMode::FormData => content.push(form_data_editor(&active_tab.draft.body.form_data)),
    };

    content.into()
}

/// Editor de filas del modo FormData de la tab Body (mismo patrón que
/// `key_value_editor`, pero con un `pick_list` adicional de tipo Text/File
/// por fila).
fn form_data_editor(rows: &[FormDataRow]) -> Element<'_, Message> {
    let mut content = column![].spacing(4);

    for row_data in rows {
        let row_id = row_data.id.clone();
        let id_for_key = row_id.clone();
        let id_for_value = row_id.clone();
        let id_for_enabled = row_id.clone();
        let id_for_kind = row_id.clone();
        let id_for_remove = row_id.clone();

        let enabled_checkbox = checkbox(row_data.enabled).on_toggle(move |_| {
            Message::RequestComposer(RequestComposerMessage::FormDataRowEnabledToggled {
                row_id: id_for_enabled.clone(),
            })
        });

        let key_input = text_input("Clave", &row_data.key)
            .on_input(move |key| {
                Message::RequestComposer(RequestComposerMessage::FormDataRowKeyChanged {
                    row_id: id_for_key.clone(),
                    key,
                })
            })
            .width(Length::Fill);

        let kind_picker = pick_list(
            FORM_DATA_FIELD_KIND_OPTIONS,
            Some(FormDataFieldKindOption(row_data.kind)),
            move |option: FormDataFieldKindOption| {
                Message::RequestComposer(RequestComposerMessage::FormDataRowKindChanged {
                    row_id: id_for_kind.clone(),
                    kind: option.0,
                })
            },
        )
        .placeholder("Tipo");

        let value_placeholder = match row_data.kind {
            FormDataFieldKind::Text => "Valor",
            FormDataFieldKind::File => "Ruta del archivo",
        };
        let value_input = text_input(value_placeholder, &row_data.value)
            .on_input(move |value| {
                Message::RequestComposer(RequestComposerMessage::FormDataRowValueChanged {
                    row_id: id_for_value.clone(),
                    value,
                })
            })
            .width(Length::Fill);

        let remove_button = button(text("×")).on_press(Message::RequestComposer(
            RequestComposerMessage::FormDataRowRemoved {
                row_id: id_for_remove.clone(),
            },
        ));

        content = content.push(
            row![
                enabled_checkbox,
                key_input,
                kind_picker,
                value_input,
                remove_button
            ]
            .spacing(8),
        );
    }

    let add_button = button(text("+ Agregar campo")).on_press(Message::RequestComposer(
        RequestComposerMessage::FormDataRowAdded,
    ));

    content = content.push(add_button);

    content.into()
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
    /// `Status`, `Header` y `JSON Pointer` nombran partes del protocolo HTTP y
    /// una especificación (RFC 6901) y se conservan; lo que se traduce es la
    /// prosa que las acompaña (Req 9.6).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.0 {
            AssertionSource::Status => "Status",
            AssertionSource::Header => "Header",
            AssertionSource::BodyText => "Body (texto)",
            AssertionSource::JsonPointer => "JSON Pointer",
            AssertionSource::FinalUrl => "URL final",
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
            AssertionOperator::Equals => "Es igual a",
            AssertionOperator::Contains => "Contiene",
            AssertionOperator::NotContains => "No contiene",
            AssertionOperator::Exists => "Existe",
            AssertionOperator::NotExists => "No existe",
            AssertionOperator::GreaterOrEqual => "Mayor o igual que",
            AssertionOperator::LessOrEqual => "Menor o igual que",
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
        .placeholder("Origen");

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
        .placeholder("Operador");

        let selector_input = text_input(
            "Selector (opcional)",
            &assertion.selector.clone().unwrap_or_default(),
        )
        .on_input(move |selector| {
            Message::RequestComposer(RequestComposerMessage::AssertionSelectorChanged {
                assertion_id: id_for_selector.clone(),
                selector,
            })
        })
        .width(Length::Fill);

        let expected_input = text_input("Valor esperado", &assertion.expected)
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

    let add_button = button(text("+ Agregar assertion")).on_press(Message::RequestComposer(
        RequestComposerMessage::AssertionAdded,
    ));

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
        text("Vista previa").size(16),
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

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn matching_method_prefix_is_not_repeated_in_the_title() {
        assert_eq!(
            display_request_name("POST posts 1", HttpMethod::POST),
            "posts 1"
        );
        assert_eq!(
            display_request_name("post   posts 1", HttpMethod::POST),
            "posts 1"
        );
    }

    #[test]
    fn meaningful_or_different_prefix_is_preserved() {
        assert_eq!(display_request_name("POST", HttpMethod::POST), "POST");
        assert_eq!(
            display_request_name("GET posts 1", HttpMethod::POST),
            "GET posts 1"
        );
        assert_eq!(
            display_request_name("Public posts", HttpMethod::POST),
            "Public posts"
        );
    }

    #[test]
    fn body_editor_uses_a_workspace_sized_height() {
        const { assert!(BODY_TEXT_EDITOR_HEIGHT >= 360.0) };
    }
}

#[cfg(test)]
mod language_tests {
    //! Feature: midway-baseline-audit-and-first-vertical, Tarea 11.3
    //! Requisito 9.6.
    //!
    //! Tests de ejemplo del idioma de las etiquetas de los `pick_list` del
    //! composer, que el baseline tenía en inglés. Lo que fijan es el texto
    //! concreto de cada etiqueta, más un tripwire que impide que vuelvan las
    //! formas en inglés que se retiraron.
    //!
    //! No es un detector de idioma: los términos que se conservan a propósito
    //! (`Bearer`, `Basic`, `API Key`, `JSON`, `Header`, `Status`,
    //! `JSON Pointer`, `Form data`) son nombres de esquemas de autenticación,
    //! de formatos y de partes del protocolo HTTP, y están enumerados como
    //! excepción deliberada en `docs/known-limitations.md`.
    //!
    //! Headless: cada test formatea una opción y compara la cadena. Sin
    //! ventana, sin disco y sin red.

    use super::*;

    #[test]
    fn auth_kind_labels_translate_only_the_absence_of_a_scheme() {
        assert_eq!(AuthKind::None.to_string(), "Sin autenticación");
        // Nombres canónicos de esquemas HTTP: se conservan (excepción registrada).
        assert_eq!(AuthKind::Bearer.to_string(), "Bearer");
        assert_eq!(AuthKind::Basic.to_string(), "Basic");
        assert_eq!(AuthKind::ApiKey.to_string(), "API Key");
    }

    #[test]
    fn api_key_placement_labels_are_in_spanish_except_the_protocol_term() {
        assert_eq!(
            ApiKeyPlacementOption(ApiKeyPlacement::Header).to_string(),
            "Header"
        );
        assert_eq!(
            ApiKeyPlacementOption(ApiKeyPlacement::Query).to_string(),
            "Parámetro de query"
        );
    }

    #[test]
    fn body_mode_labels_translate_the_mode_and_keep_the_format_names() {
        assert_eq!(BodyModeOption(BodyMode::None).to_string(), "Sin body");
        assert_eq!(BodyModeOption(BodyMode::Json).to_string(), "JSON");
        assert_eq!(BodyModeOption(BodyMode::Text).to_string(), "Texto");
        assert_eq!(BodyModeOption(BodyMode::FormData).to_string(), "Form data");
    }

    #[test]
    fn form_data_field_kind_labels_are_in_spanish() {
        assert_eq!(
            FormDataFieldKindOption(FormDataFieldKind::Text).to_string(),
            "Texto"
        );
        assert_eq!(
            FormDataFieldKindOption(FormDataFieldKind::File).to_string(),
            "Archivo"
        );
    }

    #[test]
    fn assertion_operator_labels_are_fully_in_spanish() {
        let labels: Vec<String> = ASSERTION_OPERATOR_OPTIONS
            .iter()
            .map(ToString::to_string)
            .collect();

        assert_eq!(
            labels,
            vec![
                "Es igual a",
                "Contiene",
                "No contiene",
                "Existe",
                "No existe",
                "Mayor o igual que",
                "Menor o igual que",
            ]
        );
    }

    #[test]
    fn assertion_source_labels_translate_the_prose_and_keep_the_spec_names() {
        let labels: Vec<String> = ASSERTION_SOURCE_OPTIONS
            .iter()
            .map(ToString::to_string)
            .collect();

        assert_eq!(
            labels,
            vec![
                "Status",
                "Header",
                "Body (texto)",
                "JSON Pointer",
                "URL final",
            ]
        );
    }

    #[test]
    fn no_environment_option_is_in_spanish() {
        assert_eq!(SIN_ENVIRONMENT_LABEL, "Sin environment");
    }

    /// Tripwire: las etiquetas en inglés que esta tarea retiró no pueden
    /// volver por un cambio posterior sin que un test falle. Se listan las
    /// cadenas concretas del baseline, no un criterio de idioma.
    #[test]
    fn retired_english_labels_do_not_come_back() {
        const RETIRED: &[&str] = &[
            "None",
            "Text",
            "Form Data",
            "File",
            "Query Param",
            "Equals",
            "Contains",
            "Not Contains",
            "Exists",
            "Not Exists",
            "Greater or Equal",
            "Less or Equal",
            "Final URL",
            "No Environment",
        ];

        let current: Vec<String> = AuthKind::ALL
            .iter()
            .map(ToString::to_string)
            .chain(API_KEY_PLACEMENT_OPTIONS.iter().map(ToString::to_string))
            .chain(BODY_MODE_OPTIONS.iter().map(ToString::to_string))
            .chain(FORM_DATA_FIELD_KIND_OPTIONS.iter().map(ToString::to_string))
            .chain(ASSERTION_SOURCE_OPTIONS.iter().map(ToString::to_string))
            .chain(ASSERTION_OPERATOR_OPTIONS.iter().map(ToString::to_string))
            .chain(std::iter::once(SIN_ENVIRONMENT_LABEL.to_string()))
            .collect();

        for retired in RETIRED {
            assert!(
                !current.iter().any(|label| label == retired),
                "la etiqueta en inglés {retired:?} volvió a las opciones del composer"
            );
        }
    }
}
