//! Esqueleto de la arquitectura Elm de `midway-desktop` (Tarea 3.1).
//!
//! Define el `Message` de nivel superior (agrupado por área), el `State`
//! (`Midway`) y las funciones `update`/`view`/`subscription` mínimas que
//! arrancan la aplicación. Las fases posteriores (3.2 en adelante) sustituyen
//! los tipos "placeholder" definidos aquí por módulos propios
//! (`request_composer`, `response_inspector`, `workspace_panel`, etc.) con
//! los mensajes y estados reales descritos en el diseño.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use iced::futures::channel::mpsc;
use iced::futures::stream::{self, BoxStream, StreamExt};
use iced::widget::{column, container, mouse_area, responsive, row, scrollable, text};
use iced::{Element, Length, Subscription, Task};
use tokio::sync::oneshot;

use midway_core::domain::http::{
    ApiKeyPlacement, AuthConfig, BodyMode, FormDataFieldKind, FormDataRow, HttpMethod, KeyValueRow,
    RequestDraft, RequestPreview, ResponseEnvelope,
};
use midway_core::domain::interop::{
    detect_import_format, export_postman_collection, import_openapi_document,
    import_postman_collection, make_native_bundle, parse_json_or_yaml_payload, parse_native_bundle,
    ImportedRequest, NativeWorkspaceBundle, WorkspaceExportFormat, WorkspaceImportFormat,
};
use midway_core::domain::interpolation::{resolve_request, SecretRenderMode};
use midway_core::domain::preview::make_preview;
use midway_core::domain::runner::{
    CollectionRunPhase, CollectionRunProgressEvent, CollectionRunReport, RunCollectionInput,
};
use midway_core::domain::secrets::collect_secret_aliases;
use midway_core::domain::testing::{
    evaluate_response_assertions, AssertionOperator, AssertionReport, AssertionSource,
    ResponseAssertion,
};
use midway_core::domain::workspace::{
    CollectionSummary, CollectionWithRequests, EnvironmentRecord, Folder, SaveEnvironmentInput,
    SaveFolderInput, SaveRequestInput, WorkspaceSnapshot,
};

use crate::command_palette;
use crate::curl::{self, create_blank_draft};
use crate::diagnostics::{self, CrashRecord};
use crate::state::AppState;
use crate::ui::design_system::{DesignSystem, ThemeMode};
use crate::ui::empty_state;
use crate::ui::request_composer::{self, AuthKind, KeyValueTarget};
use crate::ui::response_inspector;
use crate::ui::text_editor::TextEditorState;
use crate::ui::theme_settings::{self, ThemeEvent, ThemeSettingsState};
use crate::ui::workspace_panel;

/// Mensaje de nivel superior de `midway-desktop`, agrupado por área (patrón
/// estándar de composición de mensajes en apps `iced` de tamaño medio/grande).
///
/// Ver diseño: "Data Models > Mensajes de nivel superior".
#[derive(Debug, Clone)]
pub enum Message {
    RequestComposer(RequestComposerMessage),
    ResponseInspector(ResponseInspectorMessage),
    Workspace(WorkspaceMessage),
    Runner(RunnerMessage),
    Palette(PaletteMessage),
    Theme(ThemeMessage),
    Session(SessionMessage),
    /// Scaffolding: el flujo del Updater in-app (Fase 6) está implementado en
    /// `updater.rs` pero todavía no se emite desde una subscription/UI, de ahí
    /// `#[allow(dead_code)]` hasta que se cablee al runtime.
    #[allow(dead_code)]
    Updater(UpdaterMessage),
    Keyboard(KeyboardMessage),
    /// Mensajes del Activity_Bar (Tarea 10.1).
    ActivityBar(ActivityBarMessage),
    /// Mensajes del Request_Tree_Pane (Tarea 11.1).
    #[allow(dead_code)]
    Tree(TreeMessage),
    /// Alta, modificación y baja de colecciones, carpetas y requests.
    WorkspaceCrud(WorkspaceCrudMessage),
    /// Mensajes del Top_Bar (Tarea 12.1).
    TopBar(TopBarMessage),
    /// Mensajes de redimensionamiento de paneles (dividers arrastrables).
    PanelResize(PanelResizeMessage),
    /// Autosave / progreso periódico. Scaffolding: aún no lo emite ninguna
    /// subscription (el autosave se agenda en la Fase 5 pero no está cableado
    /// al runtime todavía).
    #[allow(dead_code)]
    Tick,
}

// ---------------------------------------------------------------------------
// Panel resize types
// ---------------------------------------------------------------------------

/// Divisor que está siendo arrastrado actualmente.
///
/// `ResponseHeight` transporta `area_bottom_y`, el borde inferior del área
/// Debug en coordenadas de ventana. El listener global de eventos solo conoce
/// la posición del cursor en la ventana, así que la geometría del área tiene
/// que viajar desde la vista —que sí la recibe del `responsive`— hasta el
/// reducer. Viaja en el mensaje de inicio de arrastre y queda guardada acá
/// mientras el arrastre está activo, en vez de en un campo aparte de `Midway`
/// que habría que mantener sincronizado con `panel_dragging`.
///
/// `PanelDragState` identifica **cuál** divisor está en juego; la geometría que
/// `ResponseHeight` acarrea no forma parte de esa identidad. Por eso `PartialEq`
/// está escrito a mano y compara solo el divisor: `panel_hovered` y
/// `panel_dragging` se comparan por divisor en la vista, y un cambio de tamaño
/// de ventana no debe leerse como "otro divisor". Ese `PartialEq` es reflexivo
/// (dos `ResponseHeight` cualesquiera son iguales, incluso con `NaN` dentro),
/// de modo que `Eq` sigue siendo válido.
#[derive(Debug, Clone, Copy)]
pub enum PanelDragState {
    TreeMain,
    RequestResponse,
    /// Divisor horizontal entre el editor de request y el inspector de
    /// respuesta en la disposición apilada del área Debug.
    ResponseHeight {
        area_bottom_y: f32,
    },
}

impl PartialEq for PanelDragState {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::TreeMain, Self::TreeMain)
                | (Self::RequestResponse, Self::RequestResponse)
                | (Self::ResponseHeight { .. }, Self::ResponseHeight { .. })
        )
    }
}

impl Eq for PanelDragState {}

/// Mensajes del sistema de redimensionamiento de paneles.
#[derive(Debug, Clone)]
pub enum PanelResizeMessage {
    /// El usuario presionó el divider del tree pane (inicia arrastre).
    TreeDividerDragStarted,
    /// El usuario presionó el divider entre request y response.
    RequestResponseDividerDragStarted,
    /// El usuario presionó el divider horizontal entre editor de request e
    /// inspector de respuesta (disposición apilada).
    ///
    /// `area_bottom_y` es el borde inferior del área Debug en coordenadas de
    /// ventana, calculado por la vista a partir del `Size` que ya recibe del
    /// `responsive`: es lo que le permite al reducer convertir la Y del cursor
    /// en alto del panel de respuesta sin estado nuevo.
    ResponseHeightDividerDragStarted { area_bottom_y: f32 },
    /// El puntero entró en la zona de agarre de un divisor.
    DividerHovered(PanelDragState),
    /// El puntero salió de la zona de agarre de un divisor.
    DividerUnhovered(PanelDragState),
    /// El ratón se movió durante un arrastre activo (coordenadas de ventana).
    ///
    /// Transporta los dos ejes: los divisores verticales usan solo `x` (su
    /// comportamiento no cambia) y el horizontal usa solo `y`.
    DividerDragged { x: f32, y: f32 },
    /// El usuario soltó el botón del ratón (finaliza cualquier arrastre).
    DividerDragEnded,
}

// ---------------------------------------------------------------------------
// Mensajes por área (placeholders mínimos; se completan en fases 1-6).
// ---------------------------------------------------------------------------

/// Mensajes de la fila superior del `Request_Composer` (Tarea 3.2): método,
/// URL, Send, environment y settings. El resto del `Request_Composer`
/// (Curl_Importer, tabs de configuración, preview drawer, ejecución real de
/// Send) agrega sus propias variantes en tareas posteriores (3.5, 3.18,
/// 3.20, etc.).
#[derive(Debug, Clone)]
pub enum RequestComposerMessage {
    /// El usuario seleccionó un método HTTP distinto en el `pick_list`.
    MethodChanged(HttpMethod),
    /// El usuario editó el contenido de la barra de URL (tecleo normal, no
    /// pegado). Siempre reemplaza `draft.url` con el texto recibido.
    UrlChanged(String),
    /// El usuario pegó texto en la barra de URL (evento de "paste" distinto
    /// de `UrlChanged`, vía `text_input::on_paste`).
    ///
    /// Implementa el enrutamiento del `Curl_Importer` (Tarea 3.5, Criterios
    /// 2.7/2.8/2.9/2.19): si el texto pegado no parece un comando cURL, se
    /// inserta como texto plano en la URL (igual que `UrlChanged`); si
    /// parece cURL pero el parseo falla, se muestra un error sin modificar
    /// el contenido existente; si el parseo tiene éxito, se sobrescribe la
    /// tab activa (si está vacía) o se crea una nueva tab (si no lo está).
    UrlPasted(String),
    /// El usuario presionó el botón Send.
    ///
    /// Dispara la resolución (interpolación + auth) y ejecución HTTP del
    /// draft de la tab activa mediante llamadas directas a `midway-core`
    /// (Tarea 3.18, Requisito 2.17). El resultado llega de forma asíncrona
    /// como `SendCompleted`.
    SendPressed,
    /// Resultado asíncrono de una ejecución de Send disparada por
    /// `SendPressed` (Tarea 3.18). Se identifica la tab por `tab_id` (no por
    /// índice de tab activa) porque el usuario puede haber cambiado de tab
    /// mientras la request estaba en curso.
    SendCompleted {
        tab_id: String,
        result: Result<ResponseOutcome, String>,
    },
    /// El usuario seleccionó un environment distinto (o "Sin environment",
    /// representado como `None`) en el `pick_list` de environment.
    EnvironmentChanged(Option<String>),
    /// El usuario presionó el botón de settings (ícono de engranaje).
    ///
    /// Implementa el preview drawer (Tarea 3.20, Requisito 2.18): alterna
    /// (toggle) la visibilidad del preview de la tab activa. Si el preview
    /// ya está abierto (`tab.preview` es `Some`), se cierra (se pone en
    /// `None`); si está cerrado, se dispara el cómputo asíncrono del
    /// preview (interpolación con `SecretRenderMode::Redact` +
    /// `domain::preview::make_preview`), cuyo resultado llega como
    /// `PreviewLoaded`.
    SettingsPressed,
    /// Abre el diálogo para guardar el request activo en una colección.
    SaveRequested,
    /// Cambia el nombre del request dentro del diálogo de guardado.
    SaveNameChanged(String),
    /// Cambia la colección de destino y reinicia la carpeta seleccionada.
    SaveCollectionChanged(String),
    /// Cambia la carpeta de destino (`None` representa la raíz).
    SaveFolderChanged(Option<String>),
    /// Confirma y persiste el request en la colección seleccionada.
    SaveConfirmed,
    /// Cierra el diálogo sin guardar.
    SaveCancelled,
    /// Resultado asíncrono del guardado explícito del request.
    SaveCompleted(Result<RequestSaveOutcome, String>),
    /// Resultado asíncrono de un cómputo de preview disparado por
    /// `SettingsPressed` (Tarea 3.20). Se identifica la tab por `tab_id`
    /// (no por índice de tab activa), por la misma razón que
    /// `SendCompleted`: el usuario puede haber cambiado de tab mientras el
    /// preview se estaba calculando.
    PreviewLoaded {
        tab_id: String,
        result: Result<RequestPreview, String>,
    },
    /// El usuario seleccionó una tab de configuración distinta (Params/
    /// Headers/Auth/Body/Tests) para la tab de request activa (Tarea 5.1).
    ///
    /// Cambia `active_tab.active_request_tab` y fija
    /// `active_tab.manual_tab_override` en `true` (Tarea 5.2, Requisito
    /// 3.3): a partir de este punto, los cambios de método HTTP ya no
    /// reemplazan la tab seleccionada manualmente por el usuario.
    ConfigTabSelected(RequestTab),
    /// El usuario seleccionó un tipo de autenticación distinto en el
    /// `pick_list` de la tab Auth (Tarea 5.4, Requisitos 3.4, 3.10).
    ///
    /// Reemplaza `active_tab.draft.auth` completo por el default vacío de
    /// la variante `AuthConfig` correspondiente al `AuthKind` seleccionado,
    /// descartando los campos de la variante anterior (Requisito 3.10).
    AuthTypeChanged(AuthKind),
    /// El usuario editó el campo de token de la tab Auth (variante Bearer,
    /// Tarea 5.4, Requisito 3.5).
    BearerTokenChanged(String),
    /// El usuario editó el campo de usuario de la tab Auth (variante Basic,
    /// Tarea 5.4, Requisito 3.6).
    BasicUsernameChanged(String),
    /// El usuario editó el campo de contraseña de la tab Auth (variante
    /// Basic, Tarea 5.4, Requisito 3.6).
    BasicPasswordChanged(String),
    /// El usuario editó el nombre de clave de la tab Auth (variante ApiKey,
    /// Tarea 5.4, Requisito 3.7).
    ApiKeyKeyChanged(String),
    /// El usuario editó el valor de la tab Auth (variante ApiKey, Tarea
    /// 5.4, Requisito 3.7).
    ApiKeyValueChanged(String),
    /// El usuario cambió la ubicación (header o query param) de la tab Auth
    /// (variante ApiKey, Tarea 5.4, Requisito 3.7).
    ApiKeyPlacementChanged(ApiKeyPlacement),
    /// El usuario presionó "Agregar fila" en el editor key/value de la tab
    /// Params o Headers (Tarea 5.6, Requisito 3.8). Agrega una
    /// `KeyValueRow` vacía (habilitada por defecto) al final de
    /// `draft.query` o `draft.headers`, según `target`.
    KeyValueRowAdded(KeyValueTarget),
    /// El usuario editó la columna "key" de una fila existente del editor
    /// key/value (Tarea 5.6, Requisito 3.8), identificada por `row_id`.
    KeyValueRowKeyChanged {
        target: KeyValueTarget,
        row_id: String,
        key: String,
    },
    /// El usuario editó la columna "value" de una fila existente del
    /// editor key/value (Tarea 5.6, Requisito 3.8), identificada por
    /// `row_id`.
    KeyValueRowValueChanged {
        target: KeyValueTarget,
        row_id: String,
        value: String,
    },
    /// El usuario alternó el checkbox "enabled" de una fila del editor
    /// key/value (Tarea 5.6, Requisito 3.8), identificada por `row_id`.
    KeyValueRowEnabledToggled {
        target: KeyValueTarget,
        row_id: String,
    },
    /// El usuario presionó el botón de eliminar de una fila del editor
    /// key/value (Tarea 5.6, Requisito 3.8), identificada por `row_id`.
    KeyValueRowRemoved {
        target: KeyValueTarget,
        row_id: String,
    },
    /// El usuario presionó "Agregar assertion" en la tab Tests (Tarea 5.8,
    /// Requisito 3.9). Agrega una `domain::testing::ResponseAssertion`
    /// nueva (source `Status`, operator `Equals`, sin selector, `expected`
    /// vacío) al final de `draft.response_tests`.
    AssertionAdded,
    /// El usuario editó el campo `name` de una assertion existente de la
    /// tab Tests (Tarea 5.8, Requisito 3.9), identificada por
    /// `assertion_id`.
    AssertionNameChanged { assertion_id: String, name: String },
    /// El usuario alternó el checkbox `enabled` de una assertion de la tab
    /// Tests (Tarea 5.8, Requisito 3.9), identificada por `assertion_id`.
    AssertionEnabledToggled { assertion_id: String },
    /// El usuario cambió el `source` de una assertion en el `pick_list` de
    /// la tab Tests (Tarea 5.8, Requisito 3.9), identificada por
    /// `assertion_id`.
    AssertionSourceChanged {
        assertion_id: String,
        source: AssertionSource,
    },
    /// El usuario cambió el `operator` de una assertion en el `pick_list`
    /// de la tab Tests (Tarea 5.8, Requisito 3.9), identificada por
    /// `assertion_id`.
    AssertionOperatorChanged {
        assertion_id: String,
        operator: AssertionOperator,
    },
    /// El usuario editó el `selector` (opcional) de una assertion de la tab
    /// Tests (Tarea 5.8, Requisito 3.9), identificada por `assertion_id`.
    /// Un texto vacío se traduce a `None` (sin selector), acorde a la
    /// semántica de `ResponseAssertion::selector: Option<String>`.
    AssertionSelectorChanged {
        assertion_id: String,
        selector: String,
    },
    /// El usuario editó el `expected` de una assertion de la tab Tests
    /// (Tarea 5.8, Requisito 3.9), identificada por `assertion_id`.
    AssertionExpectedChanged {
        assertion_id: String,
        expected: String,
    },
    /// El usuario presionó el botón de eliminar de una assertion de la tab
    /// Tests (Tarea 5.8, Requisito 3.9), identificada por `assertion_id`.
    AssertionRemoved { assertion_id: String },
    /// El usuario seleccionó un modo de body distinto (None/Json/Text/
    /// FormData) en el `pick_list` de la tab Body.
    ///
    /// Al cambiar a Json o Text, sincroniza el `Text_Editor_Component` de
    /// la tab con el `draft.body.value` actual, para que el editor muestre
    /// el contenido existente (por ejemplo, tras importar un cURL con
    /// body). El contenido de `draft.body.value` y `draft.body.form_data`
    /// no se descarta al cambiar de modo (a diferencia de Auth, cambiar el
    /// modo de body es reversible sin perder lo ya escrito).
    BodyModeChanged(BodyMode),
    /// El usuario editó el contenido del `Text_Editor_Component` de la tab
    /// Body (modos Json y Text): la acción se aplica al `Content` del
    /// editor y el texto resultante se refleja en `draft.body.value`.
    BodyTextAction(iced::widget::text_editor::Action),
    /// El usuario presionó "Agregar campo" en el editor de filas de la tab
    /// Body (modo FormData). Agrega una `FormDataRow` vacía de tipo Text
    /// (habilitada por defecto) al final de `draft.body.form_data`.
    FormDataRowAdded,
    /// El usuario editó la columna "key" de una fila del editor FormData,
    /// identificada por `row_id`.
    FormDataRowKeyChanged { row_id: String, key: String },
    /// El usuario editó la columna "value" de una fila del editor FormData
    /// (texto libre para campos Text, o path del archivo para campos
    /// File), identificada por `row_id`.
    FormDataRowValueChanged { row_id: String, value: String },
    /// El usuario alternó el checkbox "enabled" de una fila del editor
    /// FormData, identificada por `row_id`.
    FormDataRowEnabledToggled { row_id: String },
    /// El usuario cambió el tipo (Text/File) de una fila del editor
    /// FormData, identificada por `row_id`.
    FormDataRowKindChanged {
        row_id: String,
        kind: FormDataFieldKind,
    },
    /// El usuario presionó el botón de eliminar de una fila del editor
    /// FormData, identificada por `row_id`.
    FormDataRowRemoved { row_id: String },
    /// El usuario cerró una tab de request, identificada por `tab_id`
    /// (Tarea 11.8, Requisito 6.5; Tarea 11.10, Requisito 6.6).
    ///
    /// Si la tab NO tiene cambios sin guardar (`!tab_is_dirty`), se cierra
    /// de inmediato, exactamente igual que antes de la Tarea 11.10: se
    /// quita de `state.tabs`, se convierte en `TabSnapshot` y se empuja al
    /// FRENTE de `state.closed_tabs` (orden más-reciente-primero, igual
    /// que `SessionSnapshot::closed_tabs`); si el stack supera
    /// [`crate::session::CLOSED_TABS_LIMIT`] entradas, descarta la más
    /// antigua (el fondo del `VecDeque`). Si la tab cerrada era la tab
    /// activa, selecciona como nueva tab activa la que quedó en el mismo
    /// índice (la siguiente tab a la derecha, o la última restante si se
    /// cerró la última tab), o `None` si no quedan tabs. Marca la sesión
    /// como `dirty` para que el autosave (Tarea 11.4) persista el cambio.
    ///
    /// Si la tab SÍ tiene cambios sin guardar (`tab_is_dirty`, Tarea 11.10,
    /// Requisito 6.6), NO se cierra todavía: en su lugar se muestra el
    /// aviso de unsaved changes (`state.unsaved_changes_prompt = Some(...)`),
    /// dejando la tab intacta hasta que el usuario elija Guardar,
    /// Descartar o Cancelar (`UnsavedChangesSaveRequested`/
    /// `DiscardRequested`/`CancelRequested`).
    #[allow(dead_code)] // Scaffolding: aún sin control de UI que dispare el cierre de tab.
    TabClosed { tab_id: String },
    /// El usuario seleccionó una tab desde el panel de requests (Tarea
    /// 8.1, Requisitos 2.5 y 2.7): activa el índice indicado y devuelve el
    /// foco del contenido principal a la request, sin ensuciar la sesión.
    #[allow(dead_code)] // Se construirá al integrar el panel en el layout (Tarea 10.3).
    TabSelected { index: usize },
    /// El usuario solicitó reabrir la tab cerrada más recientemente
    /// (Tarea 11.8, Requisito 6.5), por ejemplo vía el shortcut
    /// Ctrl+Shift+T (Tarea 11.12).
    ///
    /// Extrae (`pop`) la entrada del FRENTE de `state.closed_tabs` (la más
    /// recientemente cerrada, orden LIFO), la reconstruye como
    /// `RequestTabState` y la agrega al final de `state.tabs`, activándola.
    /// Si `state.closed_tabs` está vacío, no hace nada (`Task::none()`).
    ClosedTabReopened,
    /// El usuario presionó "Guardar" en el aviso de unsaved changes (Tarea
    /// 11.10, Requisito 6.6), mostrado por `TabClosed` sobre una tab
    /// "dirty" (`state.unsaved_changes_prompt.is_some()`).
    ///
    /// Dispara la persistencia del draft de la tab pendiente de cierre
    /// (`persist_tab_draft`, ver su doc para el alcance exacto de qué
    /// significa "guardar" cuando la tab nunca estuvo asociada a un
    /// `SavedRequestRecord`) y, si tiene éxito, cierra la tab (misma
    /// lógica que `TabClosed` sobre una tab no-dirty). Mientras la
    /// persistencia está en curso (`saving: true`), el resultado llega de
    /// forma asíncrona como `UnsavedChangesSaveCompleted`.
    UnsavedChangesSaveRequested,
    /// Resultado asíncrono de la persistencia disparada por
    /// `UnsavedChangesSaveRequested`. En éxito, cierra la tab (idéntico a
    /// `TabClosed` sobre una tab limpia) y descarta el aviso. En error,
    /// el aviso permanece abierto con el mensaje de error visible
    /// (`UnsavedChangesPromptState::error`), sin cerrar la tab ni alterar
    /// su `draft`/`saved_draft`.
    UnsavedChangesSaveCompleted(Result<SavedRequestSaveOutcome, String>),
    /// El usuario presionó "Descartar" en el aviso de unsaved changes
    /// (Tarea 11.10, Requisito 6.6): cierra la tab pendiente de cierre
    /// INMEDIATAMENTE (misma lógica que `TabClosed` sobre una tab
    /// no-dirty), sin invocar ningún flujo de persistencia. `saved_draft`
    /// permanece exactamente como estaba (no se toca en absoluto), lo cual
    /// ya garantiza por construcción que el último draft persistido no se
    /// altera.
    UnsavedChangesDiscardRequested,
    /// El usuario presionó "Cancelar" en el aviso de unsaved changes
    /// (Tarea 11.10, Requisito 6.6): descarta el aviso
    /// (`state.unsaved_changes_prompt = None`) SIN cerrar la tab ni mutar
    /// su `draft`/`saved_draft` de ninguna forma.
    UnsavedChangesCancelRequested,
}

#[derive(Debug, Clone)]
pub enum ResponseInspectorMessage {
    /// El usuario seleccionó una tab distinta (Body/Headers/Tests) del
    /// `Response_Inspector` para la tab de request activa.
    TabSelected(ResponseInspectorTab),
}

#[derive(Debug, Clone)]
pub enum WorkspaceMessage {
    /// El usuario seleccionó una sección distinta (Environments/Data/
    /// History/Diagnostics/App updates) del `Workspace_Panel` (Tarea 7.1,
    /// Requisito 4.1).
    SectionSelected(WorkspacePanelSection),
    /// El usuario presionó el botón de colapsar/expandir el `Workspace_Panel`
    /// (Tarea 7.1, Requisito 4.1: "panel lateral colapsable").
    ToggleCollapsed,
    /// El usuario editó el campo de nombre del formulario de creación/edición
    /// de environment de la sección Environments (Tarea 7.2, Requisito 4.2).
    EnvironmentNameInputChanged(String),
    /// El usuario presionó "Crear"/"Guardar" en el formulario de la sección
    /// Environments (Tarea 7.2, Requisito 4.2): valida duplicados y límites
    /// (100 environments, 100 caracteres de nombre) antes de invocar
    /// `save_environment` (Requisito 4.9).
    EnvironmentCreateOrUpdateSubmitted,
    /// El usuario presionó "Editar" sobre un environment existente de la
    /// lista de la sección Environments (Tarea 7.2): carga dicho
    /// environment en el formulario de creación/edición, identificado por
    /// `environment_id`.
    EnvironmentEditRequested(String),
    /// El usuario presionó "Cancelar" mientras editaba un environment
    /// existente (Tarea 7.2): descarta el formulario en curso y vuelve al
    /// estado de "nuevo environment" vacío.
    EnvironmentEditCancelled,
    /// El usuario presionó "Eliminar" sobre un environment existente de la
    /// lista de la sección Environments (Tarea 7.2), identificado por
    /// `environment_id`.
    EnvironmentDeleteRequested(String),
    /// Resultado asíncrono de una llamada a `save_environment` disparada por
    /// `EnvironmentCreateOrUpdateSubmitted` tras pasar la validación local
    /// (Tarea 7.2).
    EnvironmentSavedResult(Result<EnvironmentRecord, String>),
    /// Resultado asíncrono de una llamada a `delete_environment` disparada
    /// por `EnvironmentDeleteRequested` (Tarea 7.2, Requisito 4.10).
    EnvironmentDeletedResult {
        environment_id: String,
        result: Result<(), String>,
    },
    /// Resultado asíncrono de una llamada a `workspace_snapshot` disparada
    /// al seleccionar la sección History por primera vez (Tarea 7.11,
    /// Requisito 4.6). En caso de éxito, reemplaza `state.workspace`
    /// completo (collections/environments/history/secrets) con el
    /// snapshot recién cargado desde la base de datos; en caso de error,
    /// se muestra en la sección History sin mutar `state.workspace`.
    WorkspaceSnapshotLoaded(Result<WorkspaceSnapshot, String>),
    /// El usuario seleccionó un formato de export distinto (nativo v1 o
    /// Postman Collection v2.1) en el `pick_list` de la sección Data
    /// (Tarea 7.6, Requisito 4.3).
    ExportFormatChanged(WorkspaceExportFormat),
    /// El usuario editó el campo de ruta de destino del export de la
    /// sección Data (Tarea 7.6, Requisito 4.3). No hay diálogo nativo de
    /// selección de archivo todavía, así que la ruta se ingresa como texto
    /// plano.
    ExportPathInputChanged(String),
    /// El usuario editó el campo de id de collection del export de la
    /// sección Data (Tarea 7.6, Requisito 4.3), solo relevante para el
    /// formato Postman Collection v2.1 (que exporta una única collection).
    ExportCollectionIdInputChanged(String),
    /// El usuario presionó "Exportar" en la sección Data (Tarea 7.6,
    /// Requisito 4.3): valida el formulario localmente (ruta no vacía; id
    /// de collection no vacío si el formato es Postman) y, si la
    /// validación pasa, dispara la orquestación asíncrona de export.
    ExportSubmitted,
    /// Resultado asíncrono de un export disparado por `ExportSubmitted`
    /// (Tarea 7.6): mensaje de éxito (ruta + bytes escritos) o mensaje de
    /// error.
    ExportCompleted(Result<String, String>),
    /// El usuario editó el campo de payload a importar (texto pegado, en
    /// lugar de un diálogo nativo de selección de archivo) en la sección
    /// Data (Tarea 7.7, Requisitos 4.4, 4.5).
    ImportPayloadInputChanged(String),
    /// El usuario seleccionó un formato de import distinto (Auto/nativo v1/
    /// Postman v2.1/OpenAPI v3) en el `pick_list` de la sección Data (Tarea
    /// 7.7, Requisito 4.4).
    ImportFormatChanged(WorkspaceImportFormat),
    /// El usuario presionó "Importar" en la sección Data (Tarea 7.7,
    /// Requisitos 4.4, 4.5): valida el payload localmente (no vacío, tamaño
    /// máximo de 10 MB) y, si la validación pasa, dispara la orquestación
    /// asíncrona de import. Si la validación falla, se fija
    /// `import_form.result_message` como error sin mutar el estado de datos
    /// existente.
    ImportSubmitted,
    /// Resultado asíncrono de un import disparado por `ImportSubmitted`
    /// (Tarea 7.7): en caso de éxito, contiene el snapshot completo
    /// recargado del workspace (tras persistir lo importado) junto con un
    /// mensaje descriptivo; en caso de error, contiene el mensaje de error
    /// a mostrar, sin haber mutado el estado de datos existente.
    ImportCompleted(Result<(WorkspaceSnapshot, String), String>),
}

/// Mensajes del `Collection_Runner` (Fase 4). El reporte de progreso
/// incremental (Tarea 9.2, Requisito 5.2) sustituye al
/// `app.emit(COLLECTION_RUN_PROGRESS_EVENT, ...)` de la variante Tauri por
/// `ProgressReceived`, reenviado a `update` desde la `iced::subscription`
/// que consume el extremo receptor del canal `mpsc` de
/// `CollectionRunnerState` mientras `Midway.runner` sea `Some`.
#[derive(Debug, Clone)]
pub enum RunnerMessage {
    /// El usuario disparó la ejecución de una collection (botón "Run" del
    /// Collection_Runner, todavía sin UI dedicada: se expone este mensaje
    /// para permitir disparar la ejecución de forma programática/testeable
    /// hasta que la Tarea 9.3 en adelante agregue el disparador real de UI).
    /// Si ya hay una ejecución en curso (`Midway.runner` es `Some`), se
    /// ignora.
    #[allow(dead_code)] // Scaffolding: sin botón "Run" en la UI que lo dispare todavía.
    StartRequested(RunCollectionInput),
    /// Progreso incremental emitido por `collection_runner::run_collection`
    /// a través del canal `mpsc` (inicio de la ejecución, inicio/fin de
    /// cada request, fin de la ejecución), reenviado por la
    /// `iced::subscription` de progreso.
    ProgressReceived(CollectionRunProgressEvent),
    /// La ejecución de la collection finalizó (con o sin error) y ya no
    /// quedan más eventos de progreso por recibir: se limpia
    /// `Midway.runner` (lo que además detiene la `iced::subscription` de
    /// progreso, ver Requisito 5.2 "mientras ... sea `Some`").
    Finished(Result<CollectionRunReport, String>),
    /// El usuario disparó la cancelación de la ejecución en curso (botón
    /// "Cancelar" del Collection_Runner, Tarea 9.6, Requisito 5.8). Toma
    /// (`Option::take`) el `oneshot::Sender<()>` almacenado en
    /// `CollectionRunnerState::cancel_tx` y le envía `()`, análogo al
    /// patrón "take y send" de `RequestExecutorHandle::cancel`. Si no hay
    /// ejecución en curso, o si ya se envió una cancelación previamente, o
    /// si la ejecución ya terminó (el receptor fue soltado dentro de
    /// `run_collection`), no hace nada: `Sender::send` devuelve un `Err`
    /// ignorado en ese caso, sin entrar en pánico.
    #[allow(dead_code)] // Scaffolding: sin botón "Cancelar" en la UI que lo dispare todavía.
    CancelRequested,
}

/// Mensajes del Command Palette (Tarea 11.2, Requisitos 6.1, 6.2).
///
/// El algoritmo de scoring/búsqueda (`search_palette_items`) ya está
/// implementado en `command_palette.rs` (Tarea 11.1); esta tarea agrega la
/// apertura/cierre del overlay y la ejecución del ítem seleccionado.
#[derive(Debug, Clone)]
pub enum PaletteMessage {
    /// El usuario presionó Ctrl/Cmd+K (Tarea 11.2, Requisito 6.1),
    /// capturado por la `iced::keyboard::listen()` subscription de
    /// `subscription()`. Alterna (toggle) la visibilidad del palette: si ya
    /// está abierto, lo cierra (limpiando la query); si está cerrado, lo
    /// abre con la query en blanco. El shortcut completo Ctrl+K de la Tarea
    /// 11.12 (tabla global de shortcuts) reutiliza este mismo mensaje, en
    /// lugar de introducir un segundo camino de apertura.
    Toggled,
    /// El usuario editó el texto de búsqueda del `text_input` del palette.
    QueryChanged(String),
    /// El usuario seleccionó (click) un ítem de los resultados mostrados
    /// (Tarea 11.2, Requisito 6.2): resuelve `item_id` a la acción
    /// correspondiente según el esquema de prefijos documentado en
    /// `command_palette::build_palette_items` (`action:*`, `collection:*`,
    /// `request:*`), la ejecuta, y cierra el palette (limpiando la query).
    ItemSelected { item_id: String },
    /// El usuario cerró el palette sin seleccionar ningún ítem (click fuera
    /// del cuadro de búsqueda/resultados, vía el `on_press` del overlay
    /// semitransparente). El cierre por tecla Esc se agrega en la Tarea
    /// 11.12 (tabla global de shortcuts) despachando este mismo mensaje.
    Dismissed,
}

/// Mensajes de la vertical Tema/Ajustes. La definición vive en
/// `crate::ui::theme_settings` (Tarea 7.1, Req 8.1): la App_Raíz los
/// **envuelve** en `Message::Theme`, no los define. Se reexporta acá para no
/// romper los sitios de construcción existentes (`ui/top_bar.rs`).
pub use crate::ui::theme_settings::ThemeMessage;

#[derive(Debug, Clone)]
pub enum SessionMessage {
    /// Tick periódico del timer de autosave (Tarea 11.4, Requisito 6.3),
    /// emitido por la `iced::time::every` subscription combinada en
    /// `subscription()`. Si `Midway.session.dirty` es `true`, construye un
    /// `SessionSnapshot` a partir del estado actual y dispara su escritura
    /// atómica de forma asíncrona (`Task::perform`); si no hay cambios
    /// desde el último guardado, no hace nada (evita re-escrituras
    /// innecesarias, ver diseño Property 19).
    AutosaveTick,
    /// Resultado asíncrono de una escritura de sesión disparada por
    /// `AutosaveTick`. Un error no se propaga a la UI de forma bloqueante
    /// (el autosave es best-effort): por ahora solo se descarta, dejando
    /// `dirty` como quedó fijado optimistamente en `AutosaveTick` (ver doc
    /// de esa variante para la justificación de "marcar limpio antes de
    /// confirmar la escritura").
    AutosaveWritten(Result<(), String>),
}

#[derive(Debug, Clone)]
pub enum UpdaterMessage {
    /// Scaffolding: variante de relleno hasta que la Fase 6 cablee los
    /// mensajes reales del Updater (check/download/install) al runtime.
    #[allow(dead_code)]
    Placeholder,
}

/// Mensajes de la tabla global de shortcuts de teclado (Tarea 11.12,
/// Requisito 6.8).
///
/// La mayoría de los shortcuts de la tabla (Ctrl+Enter, Ctrl+Shift+P,
/// Ctrl+., Ctrl+Shift+T y Ctrl/Cmd+K) se despachan DIRECTAMENTE como el
/// mensaje de área ya existente que implementa esa acción
/// (`RequestComposerMessage::SendPressed`, `SettingsPressed`,
/// `ClosedTabReopened`; `WorkspaceMessage::ToggleCollapsed`;
/// `PaletteMessage::Toggled`, este último manejado por separado en
/// `palette_shortcut_subscription`, ver `subscription()`), sin pasar por
/// este enum: ese mensaje ya contiene toda la lógica necesaria y no
/// depende de resolver ningún estado adicional en el punto de despacho.
///
/// Las variantes de este enum cubren únicamente los shortcuts que SÍ
/// necesitan lógica propia: o bien porque no existe todavía un mensaje de
/// área equivalente (`SaveRequested`, `NewBlankTabRequested`), o porque
/// necesitan resolver estado de la aplicación (tab activa, cuál overlay
/// está abierto) que no está disponible dentro de la `iced::keyboard::
/// listen()` subscription de `subscription()` (que solo recibe el
/// `Event` de teclado, no una referencia a `Midway`): esa resolución se
/// hace en `update_keyboard`, no en la subscription.
#[derive(Debug, Clone)]
pub enum KeyboardMessage {
    /// Ctrl+S ("Guardar", Requisito 6.8).
    /// Abre el mismo diálogo de nombre/colección/carpeta que el botón
    /// Guardar del composer.
    SaveRequested,
    /// Ctrl+Shift+N ("Nuevo request", Requisito 6.8).
    ///
    /// Abre una tab en blanco y la activa, replicando exactamente el
    /// comportamiento de la acción fija "Nuevo request" del Command
    /// Palette (`PALETTE_ACTION_NEW_REQUEST`, Tarea 11.2): no existía
    /// hasta ahora un mensaje de nivel superior invocable fuera del
    /// palette para esta acción, así que se agrega aquí en lugar de
    /// duplicar la lógica de `execute_palette_item`.
    NewBlankTabRequested,
    /// Ctrl+W ("Cerrar tab activa", Requisito 6.8).
    ///
    /// La subscription de teclado (`subscription()`) no tiene acceso a
    /// `state.active_tab`/`state.tabs` (solo recibe el `Event`, no una
    /// referencia a `Midway`): por eso este mensaje no lleva ningún
    /// `tab_id`, y es `update_keyboard` quien resuelve cuál es la tab
    /// activa en el momento en que el mensaje llega a `update`, delegando
    /// después en el mismo helper que ya usa
    /// `RequestComposerMessage::TabClosed` (`close_tab_or_prompt_unsaved_changes`,
    /// Tarea 11.10, Requisito 6.6: si la tab activa tiene cambios sin
    /// guardar, muestra el aviso en lugar de cerrarla de inmediato). No-op
    /// si no hay ninguna tab activa.
    CloseActiveTabShortcut,
    /// Alt+1..9 ("ir a tab N", Requisito 6.8), 0-based: Alt+1 -> `index`
    /// 0, ..., Alt+9 -> `index` 8.
    ///
    /// Por la misma razón que `CloseActiveTabShortcut`, la resolución de
    /// "cuántas tabs hay abiertas ahora" ocurre en `update_keyboard`, no
    /// en la subscription. Si `index` está fuera de rango de
    /// `state.tabs` (por ejemplo Alt+9 con solo 3 tabs abiertas), no hace
    /// nada.
    GoToTabShortcut { index: usize },
    /// Esc ("cerrar panel/settings", Requisito 6.8).
    ///
    /// Precedencia (documentada aquí porque no hay una convención previa
    /// en el código para "qué overlay tiene prioridad"): si el Command
    /// Palette está abierto (`state.palette.is_open`), Esc lo cierra
    /// PRIMERO (despachando `PaletteMessage::Dismissed`), sin tocar el
    /// preview drawer aunque también esté abierto. Si el palette NO está
    /// abierto pero el preview drawer de la tab activa sí lo está
    /// (`tab.preview` es `Some`), Esc lo cierra despachando
    /// `RequestComposerMessage::SettingsPressed` (que, al ser un toggle,
    /// cierra el preview en lugar de abrirlo porque ya está abierto). Si
    /// ninguno de los dos está abierto, no hace nada.
    EscapePressed,
}

// ---------------------------------------------------------------------------
// State (`Midway`) y tipos auxiliares (placeholders mínimos).
// ---------------------------------------------------------------------------

/// Tab de configuración de un request (Params | Headers | Auth | Body |
/// Tests). Se completa con más variantes/lógica en la Tarea 3.2 (Fase 2).
///
/// Deriva `Serialize`/`Deserialize` (Tarea 11.4) porque se persiste como
/// parte de `TabSnapshot`/`SessionSnapshot` en `session.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestTab {
    #[default]
    Params,
    Headers,
    Auth,
    Body,
    Tests,
}

/// Tab del `Response_Inspector` (Body | Headers | Cookies | Tests) para la
/// respuesta de la tab de request activa. Ver diseño: "Components and
/// Interfaces > Request_Composer y Response_Inspector (Fase 1)"; Requisito
/// 2.11, 9.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseInspectorTab {
    #[default]
    Body,
    Headers,
    Cookies,
    Tests,
}

// ---------------------------------------------------------------------------
// Nuevos tipos de estado y mensajes para el rediseño Insomnia (Tarea 9.1).
// ---------------------------------------------------------------------------

/// Modo del Top_Bar: Debug (Composer+Inspector) o Test (Collection_Runner).
/// Default: Debug (Req 6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TopBarMode {
    #[default]
    Debug,
    Test,
}

/// Estado del Request_Tree_Pane: filtro de texto y estado de expandido/
/// colapsado por folder (Req 5.3, 5.11).
#[derive(Debug, Clone, Default)]
pub struct TreeViewState {
    /// Texto actual del campo de filtro.
    pub filter: String,
    /// Conjunto de folder ids actualmente colapsados (default: expandido).
    pub collapsed: HashSet<String>,
    /// Respaldo del estado de colapsado/expandido justo antes de que el
    /// filtro forzara la expansión (Req 5.11). Se restaura al vaciar el
    /// filtro.
    pub collapsed_snapshot: Option<HashSet<String>>,
    /// Error de apertura de request (Req 5.7).
    pub error: Option<String>,
    /// Request que se está arrastrando y destino actualmente señalado.
    pub request_drag: Option<RequestDragState>,
    /// Request cuya nueva ubicación se está persistiendo.
    pub moving_request_id: Option<String>,
    /// Fila bajo el puntero, usada para revelar acciones contextuales.
    pub hovered_item: Option<TreeHoveredItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeHoveredItem {
    Collection,
    Folder(String),
    Request(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestDragState {
    pub request_id: String,
    pub target: Option<RequestDropTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestDropTarget {
    Root,
    Folder(String),
}

/// Estado del prompt de creación de colección del Activity_Bar (Req 2.5,
/// 2.7, 2.8).
#[derive(Debug, Clone)]
pub struct CreateCollectionPromptState {
    /// Texto actual del campo de nombre.
    pub name_input: String,
    /// Mensaje de error si el nombre es inválido (vacío tras trim, Req 2.8).
    pub error: Option<String>,
}

/// Estado del diálogo para guardar el request activo.
#[derive(Debug, Clone)]
pub struct SaveRequestPromptState {
    pub tab_id: String,
    pub name_input: String,
    pub collection_id: Option<String>,
    pub folder_id: Option<String>,
    pub saving: bool,
    pub error: Option<String>,
}

/// Operación actualmente abierta en el diálogo ABM del árbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceCrudKind {
    CreateFolder {
        collection_id: String,
        parent_folder_id: Option<String>,
    },
    RenameCollection {
        collection_id: String,
    },
    DeleteCollection {
        collection_id: String,
    },
    RenameFolder {
        folder_id: String,
    },
    DeleteFolder {
        folder_id: String,
    },
    DeleteRequest {
        request_id: String,
    },
}

/// Estado del diálogo de alta, modificación o baja del árbol.
#[derive(Debug, Clone)]
pub struct WorkspaceCrudDialogState {
    pub kind: WorkspaceCrudKind,
    pub entity_name: String,
    pub name_input: String,
    pub busy: bool,
    pub error: Option<String>,
}

/// Mensajes del ABM de colecciones, carpetas y requests.
#[derive(Debug, Clone)]
pub enum WorkspaceCrudMessage {
    CreateFolderRequested { parent_folder_id: Option<String> },
    RenameCollectionRequested(String),
    DeleteCollectionRequested(String),
    RenameFolderRequested(String),
    DeleteFolderRequested(String),
    DeleteRequestRequested(String),
    NameChanged(String),
    Confirmed,
    Cancelled,
    Completed(Result<WorkspaceSnapshot, String>),
}

/// Mensajes del Activity_Bar (Tarea 10.1, Requisito 2).
#[derive(Debug, Clone)]
pub enum ActivityBarMessage {
    /// El usuario seleccionó una colección existente por su id (Req 2.3).
    CollectionSelected(String),
    /// El usuario presionó el icono Home para navegar al Workspace_Panel
    /// (Req 11.1, 11.11): fija `main_content_focus = WorkspaceSection`.
    HomePressed,
    /// El usuario presionó el icono Activity para navegar a
    /// WorkspaceSection/History o toggle back (Req 8.1).
    ActivityPressed,
    /// El usuario presionó el botón "+" para crear una colección (Req 2.5).
    CreateCollectionPressed,
    /// El usuario editó el nombre en el prompt de creación.
    CreateCollectionNameChanged(String),
    /// El usuario confirmó la creación (Req 2.5, 2.8).
    CreateCollectionConfirmed,
    /// El usuario canceló el prompt de creación (Req 2.7).
    CreateCollectionCancelled,
    /// Resultado asíncrono de la creación de una colección.
    CollectionCreated(Result<CollectionSummary, String>),
}

/// Mensajes del Request_Tree_Pane (Tarea 11.1, Requisito 5).
#[derive(Debug, Clone)]
pub enum TreeMessage {
    /// El usuario editó el texto del filtro (Req 5.2, 5.10, 5.11).
    FilterChanged(String),
    /// El usuario colapsó/expandió una folder (Req 5.4).
    FolderToggled(String),
    /// El usuario abrió un request guardado (Req 5.6).
    RequestOpened(String),
    /// Abre un request y muestra su diálogo de nombre/ubicación para editarlo.
    RequestEditRequested(String),
    /// Inicia el arrastre desde el asa de un request.
    RequestDragStarted(String),
    /// El cursor entró en un destino válido de drop.
    RequestDropTargetEntered(RequestDropTarget),
    /// El cursor salió de un destino de drop.
    RequestDropTargetLeft(RequestDropTarget),
    /// Finaliza el gesto y persiste el cambio si hay un destino.
    RequestDragReleased,
    /// Cancela el gesto sin modificar el workspace.
    RequestDragCancelled,
    /// Resultado de persistir la nueva carpeta del request.
    RequestMoveCompleted {
        request_id: String,
        destination_folder_id: Option<String>,
        result: Result<(), String>,
    },
    /// Revela las acciones contextuales de una fila.
    ItemHovered(TreeHoveredItem),
    /// Oculta las acciones si el puntero sigue saliendo de la misma fila.
    ItemUnhovered(TreeHoveredItem),
    /// El usuario presionó "crear primer request" en el estado vacío (Req 5.8).
    CreateFirstRequestPressed,
}

/// Mensajes del Top_Bar (Tarea 12.1, Requisito 6).
#[derive(Debug, Clone)]
pub enum TopBarMessage {
    /// El usuario seleccionó un modo (Debug/Test) (Req 6.4, 6.5).
    ModeSelected(TopBarMode),
    /// El usuario presionó el segmento "Midway" del breadcrumb para navegar
    /// al estado inicial (Req 3.5).
    BreadcrumbRootClicked,
    /// El usuario presionó "← Back" para regresar de WorkspaceSection al
    /// Composer (Req 4.5).
    BackToComposer,
}

/// Sección del `Workspace_Panel` lateral (Environments | Data | History |
/// Diagnostics | App updates, Tarea 7.1, Requisito 4.1). El contenido real
/// de cada sección (CRUD de environments, export/import de datos, lista de
/// history, diagnostics, estado del updater) se agrega en tareas
/// posteriores (7.2 en adelante); esta tarea solo cubre el contenedor y el
/// mecanismo de selección de sección.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkspacePanelSection {
    #[default]
    Environments,
    Data,
    History,
    Diagnostics,
    AppUpdates,
}

/// Qué muestra el `Main_Content_Pane`: el `Request_Composer`/`Response_Inspector`
/// de la tab de request activa, o el contenido de una sección del `Sidebar`
/// (Environments/Data/History/Diagnostics/App updates). El
/// `Request_List_Pane` permanece visible y sin cambios en ambos casos
/// (siempre lista los tabs de request abiertos).
///
/// Campo puramente de presentación (no persistido, no de dominio): no
/// modifica ningún `Message`/`State` de `midway-core`.
/// Ver diseño: "Layout de tres paneles (Requisito 2)".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainContentFocus {
    #[default]
    RequestTab,
    WorkspaceSection,
}

/// Resultado de una ejecución de request: la respuesta HTTP más el reporte
/// de assertions evaluado sobre ella (`domain::testing::evaluate_response_assertions`).
#[derive(Debug, Clone)]
pub struct ResponseOutcome {
    pub response: ResponseEnvelope,
    pub assertions: AssertionReport,
}

/// Resultado asíncrono de `persist_tab_draft` (Tarea 11.10, Requisito 6.6):
/// identifica la tab guardada por `tab_id` (la tab pendiente de cierre
/// puede no ser la tab activa) y lleva el `RequestDraft` efectivamente
/// persistido (con su `id` ya asignado por `save_request` si la tab nunca
/// tuvo uno), para que el handler pueda fijar
/// `tab.saved_draft = Some(saved_draft)` antes de cerrarla.
#[derive(Debug, Clone)]
pub struct SavedRequestSaveOutcome {
    pub tab_id: String,
    pub saved_draft: RequestDraft,
}

/// Resultado del guardado explícito desde el composer.
#[derive(Debug, Clone)]
pub struct RequestSaveOutcome {
    pub tab_id: String,
    pub saved_draft: RequestDraft,
    pub workspace: WorkspaceSnapshot,
    pub collection_id: String,
}

/// Estado de una tab de request abierta en la UI.
pub struct RequestTabState {
    pub id: String,
    /// `domain::http::RequestDraft` (midway-core).
    pub draft: RequestDraft,
    /// Último estado persistido, para detectar "dirty".
    pub saved_draft: Option<RequestDraft>,
    pub active_request_tab: RequestTab,
    pub manual_tab_override: bool,
    pub response: Option<ResponseOutcome>,
    /// Tab actualmente seleccionada del `Response_Inspector` (Body por
    /// defecto) para la respuesta de esta tab de request.
    pub response_tab: ResponseInspectorTab,
    pub preview: Option<RequestPreview>,
    pub sending: bool,
    pub execution_id: Option<String>,
    /// Mensaje de error del último intento de pegado de cURL que falló al
    /// parsear (Tarea 3.5, Criterio 2.19). Se limpia en cualquier acción
    /// posterior que mute el `draft` (edición de URL, pegado exitoso, etc.)
    /// para no obstruir la UI indefinidamente.
    pub curl_paste_error: Option<String>,
    /// Mensaje de error de la última ejecución de Send que falló (Tarea
    /// 3.18). Se limpia al iniciar una nueva ejecución.
    pub send_error: Option<String>,
    /// Mensaje de error del último cómputo de preview que falló (Tarea
    /// 3.20). `preview` permanece en `None` en ese caso (el drawer no tiene
    /// contenido que mostrar), pero el mensaje se conserva para dar
    /// visibilidad del fallo en la UI. Se limpia al iniciar un nuevo
    /// cómputo de preview.
    pub preview_error: Option<String>,
    /// `Text_Editor_Component` del body del request (modos Json/Text de la
    /// tab Body), con resaltado de sintaxis JSON. Se mantiene sincronizado
    /// con `draft.body.value`: `BodyModeChanged` lo repuebla con el valor
    /// actual del draft al entrar a un modo de texto, y `BodyTextAction`
    /// aplica cada acción de edición tanto al `Content` de este editor
    /// como a `draft.body.value`.
    pub body_editor: TextEditorState,
}

/// Snapshot de una tab, usado tanto para el stack de tabs cerradas en memoria
/// como para la persistencia de sesión (Fase 5, `session.rs`).
///
/// Deriva `Serialize`/`Deserialize` (Tarea 11.4) para poder persistirse
/// dentro de `session::SessionSnapshot` (`session.rs`), que reutiliza este
/// tipo tal cual en lugar de definir uno duplicado (coincide exactamente
/// con la forma descrita en el diseño: "Data Models > Formato de sesión
/// persistida").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSnapshot {
    pub id: String,
    pub draft: RequestDraft,
    pub active_request_tab: RequestTab,
}

/// Estado del panel lateral (Environments/Data/History/Diagnostics/App
/// updates, Tarea 7.1, Requisito 4.1). El contenido de cada sección se
/// agrega en tareas posteriores (7.2 en adelante); por ahora solo se
/// almacena la sección activa, si el panel está colapsado, y el estado del
/// formulario de creación/edición de environments (Tarea 7.2).
#[derive(Debug, Default)]
pub struct WorkspacePanelState {
    pub active_section: WorkspacePanelSection,
    /// Panel lateral colapsable (Requisito 4.1): cuando es `true`, `view`
    /// solo renderiza un botón para expandirlo, ocultando las secciones.
    pub collapsed: bool,
    /// Error de navegación a una sección (Req 11.10): si un control de
    /// sección no responde o falla, se muestra este error y se mantiene la
    /// vista actual sin cambiar `active_section` ni `main_content_focus`.
    pub navigation_error: Option<String>,
    /// Formulario de creación/edición de environments de la sección
    /// Environments (Tarea 7.2, Requisito 4.2).
    pub environment_form: EnvironmentFormState,
    /// Cuando es `true`, hay una llamada a `save_environment` o
    /// `delete_environment` en curso (Tarea 7.2); usado para deshabilitar
    /// los controles del formulario/lista mientras se resuelve.
    pub environment_busy: bool,
    /// `true` mientras hay una llamada a `workspace_snapshot` en curso
    /// (Tarea 7.11, Requisito 4.6), disparada al seleccionar la sección
    /// History por primera vez.
    pub history_loading: bool,
    /// `true` una vez que `state.workspace` se cargó al menos una vez desde
    /// `workspace_snapshot` (Tarea 7.11): evita relanzar la carga cada vez
    /// que el usuario vuelve a seleccionar la sección History.
    pub history_loaded: bool,
    /// Mensaje de error de la última llamada a `workspace_snapshot` que
    /// falló (Tarea 7.11). Se limpia al reintentar la carga.
    pub history_error: Option<String>,
    /// Formulario de export de la sección Data (Tarea 7.6, Requisito 4.3).
    pub export_form: ExportFormState,
    /// Formulario de import de la sección Data (Tarea 7.7, Requisitos 4.4,
    /// 4.5).
    pub import_form: ImportFormState,
}

/// Estado del formulario de creación/edición de un environment de la
/// sección Environments (Tarea 7.2, Requisito 4.2).
///
/// `editing_id` distingue "crear nuevo" (`None`) de "editar existente"
/// (`Some(id)`): el mismo formulario se reutiliza para ambos casos, igual
/// que `SaveEnvironmentInput::environment_id` en `midway-core`.
#[derive(Debug, Default)]
pub struct EnvironmentFormState {
    pub editing_id: Option<String>,
    pub name_input: String,
    /// Mensaje de error de la última validación local (duplicado, límite de
    /// 100 environments, límite de 100 caracteres) o del último fallo de
    /// `save_environment`/`delete_environment` (Requisito 4.9). Se limpia
    /// al editar el campo de nombre o al completar una operación con éxito.
    pub error: Option<String>,
}

/// Estado del formulario de export de la sección Data del `Workspace_Panel`
/// (Tarea 7.6, Requisito 4.3).
///
/// No existe todavía un diálogo nativo de selección de archivo (Fase 3 no
/// lo requiere) ni una lista de collections navegable en la UI, así que
/// tanto la ruta de destino como el id de la collection a exportar (solo
/// relevante para el formato Postman, que exporta una única collection) se
/// ingresan como `text_input` en texto plano, igual que el `path: String`
/// del comando Tauri equivalente (`export_workspace_data`).
#[derive(Debug)]
pub struct ExportFormState {
    pub export_format: WorkspaceExportFormat,
    pub path_input: String,
    pub collection_id_input: String,
    /// `true` mientras hay una llamada de export en curso (Tarea 7.6);
    /// deshabilita el botón "Exportar" mientras se resuelve.
    pub busy: bool,
    /// Mensaje de resultado del último intento de export (éxito o error),
    /// mostrado debajo del formulario hasta el próximo intento.
    pub result_message: Option<String>,
    /// `true` cuando `result_message` corresponde a un error (para
    /// distinguir el color del mensaje en la vista).
    pub result_is_error: bool,
}

impl Default for ExportFormState {
    fn default() -> Self {
        Self {
            export_format: WorkspaceExportFormat::NativeWorkspaceV1,
            path_input: String::new(),
            collection_id_input: String::new(),
            busy: false,
            result_message: None,
            result_is_error: false,
        }
    }
}

/// Límite de tamaño del payload de import de la sección Data (Requisito
/// 4.4, 4.5): 10 MB, medido en bytes de la representación UTF-8 del texto
/// pegado, antes de invocar cualquier parser (`parse_json_or_yaml_payload`,
/// `import_postman_collection`, `import_openapi_document`,
/// `parse_native_bundle`).
const MAX_IMPORT_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;

/// Estado del formulario de import de la sección Data del `Workspace_Panel`
/// (Tarea 7.7, Requisitos 4.4, 4.5).
///
/// No existe todavía un diálogo nativo de selección de archivo (Fase 3 no
/// lo requiere), así que el contenido a importar se ingresa como texto
/// pegado en un `text_input`/`text_editor`, igual que
/// `ImportWorkspacePayloadInput::payload` del comando Tauri equivalente
/// (`import_workspace_payload`).
#[derive(Debug)]
pub struct ImportFormState {
    pub import_format: WorkspaceImportFormat,
    pub payload_input: String,
    /// `true` mientras hay una llamada de import en curso (Tarea 7.7);
    /// deshabilita el botón "Importar" mientras se resuelve.
    pub busy: bool,
    /// Mensaje de resultado del último intento de import (éxito o error),
    /// mostrado debajo del formulario hasta el próximo intento.
    pub result_message: Option<String>,
    /// `true` cuando `result_message` corresponde a un error (para
    /// distinguir el color del mensaje en la vista).
    pub result_is_error: bool,
}

impl Default for ImportFormState {
    fn default() -> Self {
        Self {
            import_format: WorkspaceImportFormat::Auto,
            payload_input: String::new(),
            busy: false,
            result_message: None,
            result_is_error: false,
        }
    }
}

/// Estado del Command Palette (Tarea 11.2, Requisitos 6.1, 6.2).
///
/// Los resultados (`search_palette_items` sobre `build_palette_items`) se
/// calculan en `view()` a partir de `is_open`/`query` en lugar de cachearse
/// aquí: la lista de ítems depende de `state.workspace.collections` (y de
/// `state.tabs`, para la acción fija "Nuevo request"), que puede cambiar
/// por causas ajenas al palette (import/export, CRUD de environments,
/// etc.); cachear y mantener sincronizado ese resultado agregaría una
/// fuente de invalidación adicional sin necesidad, dado que el costo de
/// recalcularlo (un `Vec` de un puñado de ítems, filtrado en memoria) es
/// insignificante frente a la complejidad de invalidarlo correctamente.
#[derive(Debug, Default)]
pub struct PaletteState {
    /// `true` mientras el overlay del Command Palette está abierto.
    pub is_open: bool,
    /// Texto de búsqueda actual, ingresado en el `text_input` del overlay.
    pub query: String,
}

/// Estado del aviso de unsaved changes (Tarea 11.10, Requisito 6.6),
/// mostrado como overlay (mismo patrón `iced::widget::stack` que el
/// Command Palette, Tarea 11.2) cuando el usuario intenta cerrar una tab
/// "dirty" (`tab_is_dirty`).
///
/// `tab_id` identifica la tab pendiente de cierre en lugar de un índice:
/// igual que `RequestComposerMessage::SendCompleted`/`PreviewLoaded`, un
/// índice podría dejar de ser válido si `state.tabs` cambia mientras el
/// aviso está visible (por ejemplo, si en el futuro se agregan más formas
/// de mutar `tabs` que no pasen por este aviso).
#[derive(Debug, Clone)]
pub struct UnsavedChangesPromptState {
    pub tab_id: String,
    /// `true` mientras el flujo de Guardar (`handle_unsaved_changes_save_requested`)
    /// tiene una llamada de persistencia en curso; deshabilita los tres
    /// botones del overlay mientras se resuelve, para evitar que el
    /// usuario dispare Guardar/Descartar/Cancelar dos veces sobre la misma
    /// tab en proceso de guardarse.
    pub saving: bool,
    /// Mensaje de error del último intento de Guardar que falló (por
    /// ejemplo, si `save_request` devuelve un error). El overlay permanece
    /// abierto en ese caso (no se cierra la tab ni se descarta el aviso),
    /// para que el usuario pueda reintentar o elegir Descartar/Cancelar.
    pub error: Option<String>,
}

/// Handle clonable e identificable por igualdad de puntero (`Hash`/`Eq`
/// manuales, ver más abajo) sobre el extremo receptor del canal `mpsc` de
/// progreso de una ejecución de `Collection_Runner` en curso.
///
/// `Subscription::run_with` (Tarea 9.2) exige que su parámetro `data` sea
/// `Hash`, cosa que `mpsc::UnboundedReceiver` no es. Se envuelve entonces en
/// un `Arc<Mutex<Option<...>>>` (clonable, e identificable de forma única
/// por la dirección de su `Arc`) del que el *builder* de la subscription
/// (una función sin closures, ver `subscription()`) extrae el receptor una
/// única vez a través de `Option::take` la primera vez que se construye el
/// stream subyacente; llamadas posteriores a `subscription()` con el mismo
/// handle (mismo puntero `Arc`, mismo hash) no reconstruyen el stream, por
/// lo que ese `take` solo ocurre una vez por ejecución.
#[derive(Clone)]
pub struct ProgressReceiverHandle(
    Arc<std::sync::Mutex<Option<mpsc::UnboundedReceiver<CollectionRunProgressEvent>>>>,
);

impl ProgressReceiverHandle {
    pub fn new(receiver: mpsc::UnboundedReceiver<CollectionRunProgressEvent>) -> Self {
        Self(Arc::new(std::sync::Mutex::new(Some(receiver))))
    }

    /// Extrae el receptor la primera vez que se invoca; devuelve `None` en
    /// invocaciones posteriores (el receptor ya fue tomado).
    fn take(&self) -> Option<mpsc::UnboundedReceiver<CollectionRunProgressEvent>> {
        self.0
            .lock()
            .expect("el mutex del ProgressReceiverHandle no debería estar envenenado")
            .take()
    }
}

impl std::hash::Hash for ProgressReceiverHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

/// Estado del `Collection_Runner` (Fase 4). `running` es `Some` (envolviendo
/// el handle del extremo receptor del canal de progreso) únicamente mientras
/// hay una ejecución en curso, desde `RunnerMessage::StartRequested` hasta
/// `RunnerMessage::Finished`: la `iced::subscription` de progreso
/// (`subscription()`) solo se incluye mientras `running` sea `Some` (Tarea
/// 9.2, Requisito 5.2: "mientras `CollectionRunnerState::running` sea
/// `Some`"). `latest_progress`/`report`/`error` se conservan también
/// después de finalizar la ejecución, para que una UI de Collection_Runner
/// (agregada en tareas posteriores) pueda seguir mostrando el último
/// progreso/reporte conocido.
#[derive(Default)]
pub struct CollectionRunnerState {
    pub running: Option<ProgressReceiverHandle>,
    pub latest_progress: Option<CollectionRunProgressEvent>,
    pub report: Option<CollectionRunReport>,
    pub error: Option<String>,
    /// Extremo emisor del `oneshot::channel<()>` de cancelación de la
    /// ejecución en curso (Tarea 9.6, Requisito 5.8), tomado (`Option::take`)
    /// por `RunnerMessage::CancelRequested`. Se fija en `Some` en
    /// `StartRequested` (junto al `Receiver` correspondiente, pasado a
    /// `collection_runner::run_collection`) y queda en `None` tanto tras
    /// cancelarse como tras finalizar normalmente la ejecución.
    pub cancel_tx: Option<oneshot::Sender<()>>,
}

/// Estado del Session_Store (autosave, closed-tab stack, restore) (Fase 5).
///
/// `dirty` (Tarea 11.4, Requisito 6.3) rastrea si hubo cambios relevantes
/// para la sesión (tabs, tab activa, tamaños de panel, tabs cerradas) desde
/// el último autosave exitoso: el timer de `subscription()` solo dispara
/// una escritura de `session.json` cuando `dirty` es `true`, evitando
/// re-escrituras innecesarias cuando nada cambió (diseño, Property 19).
///
/// Ninguna fuente de mutación real (tabs/panel resize/etc.) fija `dirty`
/// todavía en esta tarea: las Tareas 11.6, 11.8, 11.10 y siguientes son las
/// que van a marcar `dirty = true` a medida que agreguen la lógica de
/// sesión correspondiente (restauración, stack de tabs cerradas, unsaved
/// changes). Esta tarea solo entrega el MECANISMO completo (timer ->
/// guardado condicional -> marcar limpio), no las fuentes de mutación.
#[derive(Debug, Default)]
pub struct SessionStoreState {
    /// `true` si hubo cambios desde el último autosave exitoso.
    pub dirty: bool,
    /// Tamaños de panel actuales, persistidos como parte de la sesión.
    /// Sin lógica de redimensionamiento de paneles todavía (Fase 5,
    /// tareas posteriores a esta), se usa el valor por defecto de
    /// `crate::session::PanelSizes`.
    pub panel_sizes: crate::session::PanelSizes,
    /// Tema restaurado junto con los tamaños de panel y copiado al estado
    /// superior de `Midway` durante el arranque. Mantenerlo aquí permite
    /// que `restore_pending_session` consuma un único snapshot sin agregar
    /// otro mecanismo de persistencia paralelo al `Session_Store`.
    pub theme_mode: ThemeMode,
    /// Sesión cargada con éxito al arrancar (Tarea 11.16,
    /// `crate::session::load_session_or_default`), pendiente de que la
    /// Tarea 11.6 (restauración real de tabs abiertas, tab activa y
    /// tamaños de panel) la consuma. Esta tarea solo la deja disponible
    /// aquí: no restaura nada por sí misma todavía.
    pub pending_restore: Option<crate::session::SessionSnapshot>,
    /// Mensaje de arranque pendiente de mostrar al usuario (Tarea 11.16,
    /// Criterio 6.10), fijado cuando `session.json` existía pero se
    /// descartó por estar corrupto o por tener una `version` incompatible.
    /// `None` en el caso normal (sesión cargada con éxito o primer
    /// arranque sin sesión previa).
    ///
    /// No existe todavía un sistema genérico de notificaciones/toasts en
    /// `midway-desktop`: se usa este campo simple, en el mismo estilo que
    /// los campos de mensaje puntuales ya existentes en el codebase (por
    /// ejemplo `ExportFormState::result_message`,
    /// `WorkspacePanelState::history_error`), en lugar de construir un
    /// mecanismo de notificaciones más general para un único caso de uso.
    pub startup_notice: Option<String>,
}

/// Estado actual del Updater in-app (Tarea 7.15, Requisito 4.8), reemplazo
/// directo del estado manejado por `UpdateCenterCard.tsx`: sin
/// actualizaciones disponibles, actualización disponible (con versión),
/// descargando (con progreso) o lista para instalar (con versión).
///
/// La lógica de red que produce las transiciones entre estas variantes
/// (comprobar manifest, descargar, verificar checksum) se implementa en la
/// Fase 6 (Tareas 13.1-13.10); esta tarea solo define el modelo de estados
/// y lo renderiza como una card estática en el `Workspace_Panel`.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum UpdaterStatus {
    /// Sin actualizaciones disponibles: estado inicial por defecto, antes
    /// de que exista lógica de comprobación (Fase 6).
    #[default]
    NoUpdatesAvailable,
    // Las variantes de estado "activo" del Updater se construyen desde la
    // lógica de red de la Fase 6 (updater.rs), todavía sin cablear a las
    // transiciones de estado de la UI; de ahí `#[allow(dead_code)]`.
    /// Hay una actualización disponible para descargar.
    #[allow(dead_code)]
    UpdateAvailable { version: String },
    /// Se está descargando la actualización (`progress_percent` en 0-100).
    #[allow(dead_code)]
    Downloading { progress_percent: u8 },
    /// La actualización ya se descargó e instaló; falta reiniciar la app
    /// para aplicarla.
    #[allow(dead_code)]
    ReadyToInstall { version: String },
}

/// Estado del Updater in-app (Fase 6). Por ahora solo envuelve el
/// `UpdaterStatus` actual (Tarea 7.15); campos adicionales (canal, última
/// comprobación, error, etc., ver `UpdateCenterCard.tsx`) se agregan junto
/// con la lógica de red correspondiente en la Fase 6.
#[derive(Debug, Clone, Default)]
pub struct UpdaterState {
    pub status: UpdaterStatus,
}

/// Estado (`State` en la arquitectura Elm de `iced`) de `midway-desktop`.
///
/// Ver diseño: "Data Models > Mensajes de nivel superior".
pub struct Midway {
    /// Repositorio SQLite, executor de requests y executor de secrets.
    pub app_state: Arc<AppState>,
    /// Cache en memoria del workspace, refrescado tras cada mutación.
    pub workspace: midway_core::domain::workspace::WorkspaceSnapshot,
    pub tabs: Vec<RequestTabState>,
    pub active_tab: Option<usize>,
    pub closed_tabs: VecDeque<TabSnapshot>,
    pub workspace_panel: WorkspacePanelState,
    pub palette: PaletteState,
    pub runner: Option<CollectionRunnerState>,
    pub session: SessionStoreState,
    /// Estado de la vertical Tema/Ajustes
    /// (`crate::ui::theme_settings`). El modo activo sigue persistiéndose
    /// por el autosave existente dentro de `SessionSnapshot`: el cambio es
    /// de estado en memoria, no de esquema (Tarea 7.1, Req 8.1).
    pub theme: ThemeSettingsState,
    /// Qué muestra el `Main_Content_Pane` (tab de request activa o sección
    /// del Sidebar). Puramente de presentación, no persistido.
    pub main_content_focus: MainContentFocus,
    pub updater: UpdaterState,
    pub crash_log: Vec<CrashRecord>,
    /// Aviso de unsaved changes (Tarea 11.10, Requisito 6.6), `Some`
    /// mientras el overlay de Guardar/Descartar/Cancelar está visible
    /// sobre una tab con cambios sin guardar pendiente de cierre.
    pub unsaved_changes_prompt: Option<UnsavedChangesPromptState>,
    // ----- Nuevos campos del rediseño Insomnia (Tarea 9.1) -----
    /// Colección actualmente seleccionada en el Activity_Bar (Req 2.3).
    pub active_collection_id: Option<String>,
    /// Modo del Top_Bar: Debug o Test (default Debug, Req 6.4).
    pub top_bar_mode: TopBarMode,
    /// Estado del Request_Tree_Pane (filtro, expandido/colapsado, Req 5).
    pub tree: TreeViewState,
    /// Prompt de creación de colección del Activity_Bar (Req 2.5, 2.7, 2.8).
    /// `Some` mientras el prompt está visible.
    pub create_collection_prompt: Option<CreateCollectionPromptState>,
    /// Diálogo para nombrar y guardar el request activo en una colección.
    pub save_request_prompt: Option<SaveRequestPromptState>,
    /// Diálogo ABM contextual para colecciones, carpetas y requests.
    pub workspace_crud_dialog: Option<WorkspaceCrudDialogState>,
    /// Estado de arrastre activo del divider del tree pane. `Some` mientras
    /// el usuario está arrastrando el divider para redimensionar el panel.
    pub panel_dragging: Option<PanelDragState>,
    /// Divisor bajo el cursor; permite resaltar sólo el control relevante.
    pub panel_hovered: Option<PanelDragState>,
}

impl Midway {
    /// Construye el estado inicial a partir de un `AppState` ya inicializado.
    ///
    /// La carga segura de la sesión persistida (Tarea 11.16, Criterio 6.10)
    /// corre aquí: nunca aborta el arranque sin importar lo que haya (o no)
    /// en `session.json`. Si la carga tiene éxito, el snapshot queda
    /// temporalmente en `session.pending_restore`; si `session.json`
    /// existía pero se descartó por estar corrupto o ser incompatible,
    /// `session.startup_notice` queda con un mensaje describiendo el
    /// motivo, para que `view()` lo muestre.
    ///
    /// [`restore_pending_session`] (Tarea 11.6, Requisito 6.4) consume ese
    /// `pending_restore` de inmediato (dejándolo en `None`, un campo de un
    /// solo uso en el arranque) para calcular las `tabs`/`active_tab`
    /// iniciales reales; ver su documentación para la lógica de
    /// restauración y sus fallbacks.
    pub fn new(app_state: AppState) -> Self {
        let mut session = SessionStoreState::default();
        match crate::session::load_session_or_default() {
            crate::session::SessionLoadOutcome::Loaded(snapshot) => {
                session.pending_restore = Some(snapshot);
            }
            crate::session::SessionLoadOutcome::NotFound => {}
            crate::session::SessionLoadOutcome::DiscardedCorruptOrIncompatible { reason } => {
                session.startup_notice = Some(format!(
                    "No se pudo restaurar la sesión anterior, se descartó y se inició una sesión vacía. Motivo: {reason}"
                ));
            }
        }

        let (tabs, active_tab, session_active_collection_id) =
            restore_pending_session(&mut session);
        let theme = ThemeSettingsState::new(session.theme_mode);

        // El snapshot real se carga inmediatamente después de `Midway::new`.
        // Conservamos temporalmente el id de sesión para poder validarlo
        // contra las colecciones cuando llegue `WorkspaceSnapshotLoaded`.
        let workspace_collections: Vec<midway_core::domain::workspace::CollectionWithRequests> =
            Vec::new();
        let active_collection_id = session_active_collection_id;

        Self {
            app_state: Arc::new(app_state),
            workspace: midway_core::domain::workspace::WorkspaceSnapshot {
                collections: workspace_collections,
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
            tabs,
            active_tab,
            closed_tabs: VecDeque::new(),
            workspace_panel: WorkspacePanelState::default(),
            palette: PaletteState::default(),
            runner: None,
            session,
            theme,
            main_content_focus: MainContentFocus::default(),
            updater: UpdaterState::default(),
            crash_log: Vec::new(),
            unsaved_changes_prompt: None,
            active_collection_id,
            top_bar_mode: TopBarMode::default(),
            tree: TreeViewState::default(),
            create_collection_prompt: None,
            save_request_prompt: None,
            workspace_crud_dialog: None,
            panel_dragging: None,
            panel_hovered: None,
        }
    }
}

/// Consume `session.pending_restore` (dejándolo en `None`) y calcula las
/// `tabs`/`active_tab` iniciales de `Midway` a partir de él (Tarea 11.6,
/// Requisito 6.4).
///
/// Extraída como función libre operando solo sobre `&mut SessionStoreState`
/// (en vez de vivir inline dentro de `Midway::new` u operar sobre un
/// `&mut Midway` completo) para poder testear la lógica de restauración
/// construyendo un `SessionStoreState` con un `pending_restore` fabricado a
/// mano, sin pasar por un `AppState` real ni por I/O real de
/// `session.json` (que `Midway::new` sí requiere vía
/// `AppState::initialize`/`crate::session::load_session_or_default`).
///
/// - Si `session.pending_restore` es `Some(snapshot)` (sesión previa
///   cargada con éxito por `Midway::new`, Tarea 11.16):
///   - Restaura `open_tabs` vía [`RequestTabState::from_snapshot`] para
///     cada `TabSnapshot`, preservando `id` y `active_request_tab`.
///   - Copia `snapshot.panel_sizes` a `session.panel_sizes` (reemplazando
///     el `PanelSizes::default()` de `SessionStoreState::default()`).
///   - Copia `snapshot.theme_mode` a `session.theme_mode` para que
///     `Midway::new` restaure el tema activo junto con el resto de la
///     sesión.
///   - Si `open_tabs` está vacío (una sesión válida guardada sin ninguna
///     tab abierta, plausible si el usuario cerró todo antes de salir), se
///     usa el mismo fallback de una única tab en blanco que el arranque
///     sin sesión previa: el resto del código de `midway-desktop` asume
///     que `active_tab` es significativo cuando `tabs` no está vacío (por
///     ejemplo, el `Request_Composer` siempre opera sobre "la tab
///     activa"), así que arrancar con cero tabs introduciría una
///     invariante nueva y no solicitada por esta tarea, además de una UI
///     vacía y potencialmente confusa.
///   - En caso contrario, resuelve `active_tab_id` al índice de la tab
///     restaurada con ese `id`; si no coincide con ninguna (referencia
///     obsoleta o corrupta) cae a `Some(0)` (hay al menos una tab
///     restaurada en esta rama).
///   - `closed_tabs` NO se restaura aquí: fuera del alcance explícito de
///     esta tarea, que solo menciona "tabs abiertas, tab activa y paneles
///     redimensionados" (no "tabs cerradas"), aunque `snapshot.closed_tabs`
///     exista. `Midway::new` sigue inicializando `closed_tabs` como un
///     `VecDeque` vacío.
/// - Si `session.pending_restore` es `None` (sin sesión previa cargada,
///   `SessionLoadOutcome::NotFound`/`DiscardedCorruptOrIncompatible`):
///   comportamiento por defecto sin cambios, una única tab en blanco con
///   `active_tab: Some(0)`.
fn restore_pending_session(
    session: &mut SessionStoreState,
) -> (Vec<RequestTabState>, Option<usize>, Option<String>) {
    let Some(snapshot) = session.pending_restore.take() else {
        return (vec![RequestTabState::blank()], Some(0), None);
    };

    let tabs: Vec<RequestTabState> = snapshot
        .open_tabs
        .into_iter()
        .map(RequestTabState::from_snapshot)
        .collect();

    session.panel_sizes = snapshot.panel_sizes;
    session.theme_mode = snapshot.theme_mode;

    // Preserve the session's active_collection_id for startup resolution
    // (Requirements 6.2, 9.2).
    let session_active_collection_id = snapshot.active_collection_id;

    if tabs.is_empty() {
        return (
            vec![RequestTabState::blank()],
            Some(0),
            session_active_collection_id,
        );
    }

    let active_tab = snapshot
        .active_tab_id
        .as_ref()
        .and_then(|target_id| tabs.iter().position(|tab| &tab.id == target_id))
        .or(Some(0));

    (tabs, active_tab, session_active_collection_id)
}

impl RequestTabState {
    /// Crea una tab de request en blanco (método `GET`, URL vacía), usando
    /// el mismo draft por defecto que el `Curl_Importer` (`create_blank_draft`).
    pub fn blank() -> Self {
        Self::from_draft(create_blank_draft())
    }

    /// Crea una tab de request a partir de un `RequestDraft` ya construido
    /// (por ejemplo, el resultado de `curl::parse_curl_command_to_draft`).
    ///
    /// Usado por el enrutamiento de pegado de cURL (Tarea 3.5, Criterio 2.8)
    /// para crear una nueva tab sin modificar la tab activa original.
    pub fn from_draft(draft: RequestDraft) -> Self {
        let body_editor = TextEditorState::new(&draft.body.value);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            draft,
            saved_draft: None,
            active_request_tab: RequestTab::default(),
            manual_tab_override: false,
            response: None,
            response_tab: ResponseInspectorTab::default(),
            preview: None,
            sending: false,
            execution_id: None,
            curl_paste_error: None,
            send_error: None,
            preview_error: None,
            body_editor,
        }
    }

    /// Reconstruye una tab de request a partir de un `TabSnapshot` (Tarea
    /// 11.8, Requisito 6.5), preservando el `id` original (para que
    /// reabrir una tab cerrada conserve su identidad, en vez de generar un
    /// `id` nuevo como hace `from_draft`) y la tab de configuración
    /// (`active_request_tab`) que tenía seleccionada al cerrarse. El resto
    /// de los campos (respuesta, preview, errores, etc.) no se persisten
    /// en `TabSnapshot`, así que se reinicializan a su valor por defecto,
    /// igual que una tab recién abierta.
    pub fn from_snapshot(snapshot: TabSnapshot) -> Self {
        let body_editor = TextEditorState::new(&snapshot.draft.body.value);
        Self {
            id: snapshot.id,
            draft: snapshot.draft,
            saved_draft: None,
            active_request_tab: snapshot.active_request_tab,
            manual_tab_override: false,
            response: None,
            response_tab: ResponseInspectorTab::default(),
            preview: None,
            sending: false,
            execution_id: None,
            curl_paste_error: None,
            send_error: None,
            preview_error: None,
            body_editor,
        }
    }

    /// Convierte esta tab en su `TabSnapshot` (Tarea 11.8, Requisito 6.5),
    /// usado tanto para el stack de tabs cerradas como para la
    /// persistencia de sesión (`build_session_snapshot`).
    pub fn to_snapshot(&self) -> TabSnapshot {
        TabSnapshot {
            id: self.id.clone(),
            draft: self.draft.clone(),
            active_request_tab: self.active_request_tab,
        }
    }
}

/// Calcula la tab de configuración por defecto según el método HTTP (Tarea
/// 5.2, Requisito 3.2): Params para GET/HEAD/OPTIONS, Body para
/// POST/PUT/PATCH/DELETE.
fn default_tab_for_method(method: HttpMethod) -> RequestTab {
    match method {
        HttpMethod::GET | HttpMethod::HEAD | HttpMethod::OPTIONS => RequestTab::Params,
        HttpMethod::POST | HttpMethod::PUT | HttpMethod::PATCH | HttpMethod::DELETE => {
            RequestTab::Body
        }
    }
}

/// Port de la definición del Criterio 2.7 de "tab vacía": método `GET`, URL
/// vacía (sin contar espacios) y sin params/headers/body/auth configurados.
///
/// Usado por el enrutamiento de pegado de cURL (Tarea 3.5) para decidir si
/// se sobrescribe la tab activa o se crea una nueva tab.
fn is_draft_empty(draft: &RequestDraft) -> bool {
    draft.method == HttpMethod::GET
        && draft.url.trim().is_empty()
        && draft.query.is_empty()
        && draft.headers.is_empty()
        && matches!(draft.auth, AuthConfig::None)
        && matches!(draft.body.mode, BodyMode::None)
}

/// Resolves which collection should be active at startup.
///
/// Returns `session_id` if it matches an existing collection, otherwise
/// `collections[0].id`, or `None` for an empty collections list.
///
/// This is a pure function extracted for testability (Property 7,
/// Requirements 6.1, 6.2, 6.3, 6.4).
pub fn resolve_startup_collection(
    collections: &[CollectionWithRequests],
    session_id: Option<&str>,
) -> Option<String> {
    // If session_id matches an existing collection, return it
    if let Some(sid) = session_id {
        if collections.iter().any(|c| c.collection.id == sid) {
            return Some(sid.to_string());
        }
    }
    // Fallback: first collection's id, or None if empty
    collections.first().map(|c| c.collection.id.clone())
}

/// Determina si una tab tiene cambios sin guardar (Tarea 11.10, Requisito
/// 6.6): usado para decidir si cerrarla debe disparar el aviso de unsaved
/// changes (`Midway.unsaved_changes_prompt`) en lugar de cerrarla de
/// inmediato.
///
/// Decisión de diseño (no hay una convención previa para "dirty" en el
/// codebase): se consideran dos casos por separado, en lugar de comparar
/// únicamente `Some(draft) != saved_draft`, porque una tab que NUNCA se
/// asoció a un `SavedRequestRecord` (`saved_draft: None`, el caso normal de
/// cualquier tab nueva) técnicamente "difiere" de "ningún draft guardado"
/// en cuanto el usuario escribe una sola tecla, lo cual mostraría el aviso
/// constantemente para el flujo más común de la aplicación (abrir una tab
/// en blanco, escribir una URL, cerrarla sin querer guardarla como request
/// de una collection):
///
/// - Si la tab SÍ tiene un `saved_draft` (alguna vez se cargó desde, o se
///   guardó como, un `SavedRequestRecord`, Tarea 11.2): es "dirty" si su
///   draft actual difiere de `saved_draft`, siguiendo literalmente el
///   enunciado de la Property 22 ("draft distinto de su último
///   `saved_draft`").
/// - Si la tab NUNCA se asoció a un request guardado (`saved_draft: None`):
///   es "dirty" únicamente si además tiene contenido real que el usuario
///   perdería al cerrarla (`!is_draft_empty`, el mismo concepto de "tab
///   vacía" ya usado por el enrutamiento de pegado de cURL, Tarea 3.5). Una
///   tab en blanco nunca dispara el aviso, sin importar cuánto tiempo lleve
///   abierta.
fn tab_is_dirty(tab: &RequestTabState) -> bool {
    match &tab.saved_draft {
        Some(saved_draft) => saved_draft != &tab.draft,
        None => !is_draft_empty(&tab.draft),
    }
}

// ---------------------------------------------------------------------------
// update / view / subscription
// ---------------------------------------------------------------------------

/// Extrae un mensaje textual del payload de un panic capturado por
/// `std::panic::catch_unwind` (Tarea 11.14, Requisito 6.9), replicando el
/// patrón habitual de `std::panic::set_hook`/`PanicHookInfo::payload()`:
/// la inmensa mayoría de panics en el ecosistema Rust (`panic!("...")`,
/// `.unwrap()`, `.expect("...")`, aserciones) tienen un payload `&str` o
/// `String`; cualquier otro tipo de payload (poco común, típicamente de
/// código que hace `panic_any` con un valor custom) cae en un mensaje
/// genérico en lugar de perder el panic silenciosamente.
fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        message.to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "panic con payload no textual".to_string()
    }
}

/// `Error_Boundary` (Tarea 11.14, Requisito 6.9) para el lado `update`:
/// envuelve el handler de un área (`component_name`) en
/// `std::panic::catch_unwind`, usando `AssertUnwindSafe` porque `&mut
/// Midway` no es `UnwindSafe` por construcción (una mutación a medio
/// terminar en el momento del panic podría dejar ESE estado
/// inconsistente; el diseño acepta ese riesgo localizado al componente que
/// panickeó a cambio de que el resto de la aplicación siga respondiendo,
/// ver "Error Handling" en el diseño).
///
/// Camino nominal (`handler` no panickea): el `Task<Message>` producido se
/// devuelve sin cambios. Si `handler` panickea: se registra exactamente un
/// `CrashRecord` nuevo con `source = ComponentBoundary` y el mensaje del
/// panic vía `diagnostics::append_crash_record`, y se devuelve
/// `Task::none()` (un handler que panickeó no puede producir de forma
/// fiable ningún trabajo asíncrono adicional).
fn guarded_update<F>(component_name: &'static str, state: &mut Midway, handler: F) -> Task<Message>
where
    F: FnOnce(&mut Midway) -> Task<Message>,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handler(state))) {
        Ok(task) => task,
        Err(payload) => {
            let message = panic_payload_message(payload);
            diagnostics::append_crash_record(
                diagnostics::CrashSource::ComponentBoundary,
                format!("{component_name}: {message}"),
                None,
            );
            Task::none()
        }
    }
}

/// `Error_Boundary` (Tarea 11.14, Requisito 6.9) para el lado `view`:
/// envuelve el cierre de `view` de un componente aislado (`component_name`)
/// en `std::panic::catch_unwind`. A diferencia de `guarded_update`, la
/// mayoría de los cierres envueltos aquí solo capturan `&Midway`
/// (referencia compartida, sin la posibilidad de mutación a medio terminar
/// que motiva la nota de riesgo de `guarded_update`), pero `Element` en sí
/// no es `UnwindSafe` por construcción (puede contener closures/datos de
/// mensaje internos), de ahí el mismo uso de `AssertUnwindSafe`.
///
/// Camino nominal: el `Element` producido se devuelve sin cambios. Si
/// `handler` panickea: se registra un `CrashRecord` (mismo criterio que
/// `guarded_update`) y se sustituye el `Element` de ese componente por un
/// mensaje de error visible, sin interrumpir el resto de la vista.
fn guarded_view<'a, F>(component_name: &'static str, handler: F) -> Element<'a, Message>
where
    F: FnOnce() -> Element<'a, Message>,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(handler)) {
        Ok(element) => element,
        Err(payload) => {
            let message = panic_payload_message(payload);
            diagnostics::append_crash_record(
                diagnostics::CrashSource::ComponentBoundary,
                format!("{component_name}: {message}"),
                None,
            );
            text(format!(
                "Error en el componente {component_name}: ver Diagnostics"
            ))
            .color(iced::Color::from_rgb(0.8, 0.1, 0.1))
            .into()
        }
    }
}

/// Ciclo `update` de la arquitectura Elm. Cada área se despacha a su propia
/// función de actualización, envuelta en el `Error_Boundary` de
/// `guarded_update` (Tarea 11.14, Requisito 6.9) para que un panic dentro
/// de un área no derribe el resto de la aplicación.
pub fn update(state: &mut Midway, message: Message) -> Task<Message> {
    match message {
        Message::Tick => Task::none(),
        Message::RequestComposer(message) => guarded_update("RequestComposer", state, |state| {
            update_request_composer(state, message)
        }),
        Message::ResponseInspector(message) => {
            guarded_update("ResponseInspector", state, |state| {
                update_response_inspector(state, message)
            })
        }
        Message::Workspace(message) => {
            guarded_update("Workspace", state, |state| update_workspace(state, message))
        }
        Message::Runner(message) => {
            guarded_update("Runner", state, |state| update_runner(state, message))
        }
        Message::Session(message) => {
            guarded_update("Session", state, |state| update_session(state, message))
        }
        Message::Palette(message) => {
            guarded_update("Palette", state, |state| update_palette(state, message))
        }
        // Vertical Tema/Ajustes (Tarea 7.2, Req 8.1, 8.4, 8.6): la App_Raíz
        // no aplica la transición de estado, solo delega en
        // `theme_settings::update` y traduce cada Evento_Ascendente en su
        // efecto local. `ThemeEvent::ThemeChanged` marca la sesión como sucia,
        // lo que dispara el autosave existente que persiste `theme_mode`
        // dentro de `SessionSnapshot`. No hay trabajo asíncrono, de ahí
        // `Task::none()`.
        Message::Theme(message) => guarded_update("Theme", state, |state| {
            for event in theme_settings::update(&mut state.theme, message) {
                match event {
                    ThemeEvent::ThemeChanged { .. } => state.session.dirty = true,
                }
            }

            Task::none()
        }),
        Message::Keyboard(message) => {
            guarded_update("Keyboard", state, |state| update_keyboard(state, message))
        }
        Message::Updater(_) => Task::none(),
        Message::ActivityBar(message) => guarded_update("ActivityBar", state, |state| {
            update_activity_bar(state, message)
        }),
        Message::Tree(message) => guarded_update("Tree", state, |state| {
            crate::ui::request_tree_pane::update_tree(state, message)
        }),
        Message::WorkspaceCrud(message) => guarded_update("WorkspaceCrud", state, |state| {
            update_workspace_crud(state, message)
        }),
        Message::TopBar(message) => {
            guarded_update("TopBar", state, |state| update_top_bar(state, message))
        }
        Message::PanelResize(message) => guarded_update("PanelResize", state, |state| {
            update_panel_resize(state, message)
        }),
    }
}

/// Actualiza el estado en respuesta a un `RunnerMessage` (Tarea 9.2, Requisito
/// 5.2). `StartRequested` arranca la ejecución (ignorado si ya hay una en
/// curso); `ProgressReceived` acumula el progreso incremental reenviado por
/// la `iced::subscription` de `subscription()`; `Finished` limpia
/// `running` (deteniendo dicha subscription) y fija el reporte final o el
/// error.
fn update_runner(state: &mut Midway, message: RunnerMessage) -> Task<Message> {
    match message {
        RunnerMessage::StartRequested(input) => {
            if state
                .runner
                .as_ref()
                .is_some_and(|runner| runner.running.is_some())
            {
                return Task::none();
            }

            let (progress_tx, progress_rx) = mpsc::unbounded();
            let (cancel_tx, cancel_rx) = oneshot::channel();
            state.runner = Some(CollectionRunnerState {
                running: Some(ProgressReceiverHandle::new(progress_rx)),
                latest_progress: None,
                report: None,
                error: None,
                cancel_tx: Some(cancel_tx),
            });

            let app_state = Arc::clone(&state.app_state);

            Task::perform(
                async move {
                    crate::collection_runner::run_collection(
                        app_state,
                        input,
                        progress_tx,
                        cancel_rx,
                    )
                    .await
                    .map_err(|error| error.to_string())
                },
                |result| Message::Runner(RunnerMessage::Finished(result)),
            )
        }
        RunnerMessage::ProgressReceived(event) => {
            if let Some(runner) = state.runner.as_mut() {
                runner.latest_progress = Some(event);
            }
            Task::none()
        }
        RunnerMessage::Finished(result) => {
            if let Some(runner) = state.runner.as_mut() {
                runner.running = None;
                runner.cancel_tx = None;
                match result {
                    Ok(report) => {
                        runner.report = Some(report);
                        runner.error = None;
                    }
                    Err(message) => {
                        runner.error = Some(message);
                    }
                }
            }
            Task::none()
        }
        RunnerMessage::CancelRequested => {
            if let Some(runner) = state.runner.as_mut() {
                if let Some(cancel_tx) = runner.cancel_tx.take() {
                    // Se ignora el resultado de `send`: un `Err` significa
                    // que el `Receiver` correspondiente ya fue soltado
                    // (la ejecución ya terminó dentro de `run_collection`
                    // antes de que esta cancelación llegara a tiempo), lo
                    // cual es una condición de carrera normal y no un error
                    // a propagar (análogo a `RequestExecutorHandle::cancel`,
                    // que también ignora ese caso).
                    let _ = cancel_tx.send(());
                }
            }
            Task::none()
        }
    }
}

/// Actualiza el estado en respuesta a un `SessionMessage` (Tarea 11.4,
/// Requisito 6.3).
///
/// `AutosaveTick` construye un `SessionSnapshot` del estado actual (tabs
/// abiertas, tab activa, tabs cerradas, tamaños de panel) solo si
/// `state.session.dirty` es `true`, y dispara su escritura atómica de forma
/// asíncrona vía `Task::perform`; si no hay cambios pendientes, no hace
/// nada (`Task::none()`), evitando re-escrituras innecesarias.
///
/// `dirty` se fija en `false` de forma OPTIMISTA (antes de que la
/// escritura asíncrona termine), no al recibir `AutosaveWritten`: el
/// siguiente tick del timer (1 segundo, ver `subscription()`) ocurre
/// mucho antes de que cualquier mutación adicional pueda perderse de forma
/// permanente, así que el peor caso ante un fallo de escritura es
/// simplemente reintentar en el tick siguiente una vez que una mutación
/// posterior vuelva a marcar `dirty = true` (o, si no hay mutaciones
/// posteriores, perder como máximo un guardado dentro del margen de 2
/// segundos del Requisito 6.3, que de por sí solo garantiza "persistir
/// dentro de los 2 segundos desde la última modificación", no "persistir
/// cada modificación individual sin excepción"). Esto evita la complejidad
/// de un estado "guardando" intermedio para un mecanismo que en esta tarea
/// todavía no tiene ninguna fuente real de mutación conectada.
fn update_session(state: &mut Midway, message: SessionMessage) -> Task<Message> {
    match message {
        SessionMessage::AutosaveTick => {
            if !state.session.dirty {
                return Task::none();
            }

            state.session.dirty = false;

            let snapshot = build_session_snapshot(state);

            Task::perform(
                async move {
                    crate::session::write_session_snapshot(&snapshot)
                        .map_err(|error| error.to_string())
                },
                |result| Message::Session(SessionMessage::AutosaveWritten(result)),
            )
        }
        SessionMessage::AutosaveWritten(_result) => {
            // Best-effort: un error de escritura no se propaga a la UI de
            // forma bloqueante en esta tarea. `dirty` ya fue fijado en
            // `false` de forma optimista en `AutosaveTick` (ver doc de
            // `update_session` arriba); un fallo aquí simplemente significa
            // que ese guardado en particular no llegó a disco, y se
            // reintentará en el siguiente tick una vez que una mutación
            // real vuelva a marcar `dirty = true`.
            Task::none()
        }
    }
}

/// Esquema de `id` de `CommandPaletteItem` usado por
/// `build_palette_items`/`execute_palette_item` (Tarea 11.2, Requisito
/// 6.1): un prefijo fijo seguido de `:` y, para `collection`/`request`, el
/// id del registro correspondiente en `state.workspace`. Las acciones
/// fijas no llevan datos adicionales: su prefijo completo (por ejemplo
/// `"action:new-request"`) ya identifica la acción de forma única.
const PALETTE_ACTION_NEW_REQUEST: &str = "action:new-request";
const PALETTE_PREFIX_COLLECTION: &str = "collection:";
const PALETTE_PREFIX_REQUEST: &str = "request:";

/// Construye la lista completa de `CommandPaletteItem`s indexables por el
/// Command Palette (Tarea 11.2, Requisito 6.1): acciones fijas, seguidas de
/// un ítem por colección y un ítem por request guardado dentro de cada
/// colección, en el mismo orden en que aparecen en `state.workspace.collections`.
///
/// Mantenida deliberadamente mínima en acciones fijas (una sola, "Nuevo
/// request"): el resto de acciones fijas de la referencia TypeScript
/// (guardar, enviar, copiar como cURL/fetch/axios, abrir Workspace_Panel)
/// dependen de funcionalidad de `midway-desktop` que todavía no tiene un
/// mensaje de nivel superior dedicado invocable de forma genérica fuera de
/// su propio flujo de UI (por ejemplo no existe todavía un
/// `WorkspaceMessage::SaveActiveRequest`); agregarlas aquí implicaría
/// introducir esa funcionalidad por primera vez, fuera del alcance de esta
/// tarea (11.2 solo cubre "abrir/cerrar el palette y ejecutar ítems", no
/// "agregar nuevas acciones ejecutables a la aplicación").
pub fn build_palette_items(state: &Midway) -> Vec<command_palette::CommandPaletteItem> {
    let mut items = vec![command_palette::CommandPaletteItem {
        id: PALETTE_ACTION_NEW_REQUEST.to_string(),
        title: "Nuevo request".to_string(),
        subtitle: Some("Abrir una pestaña en blanco".to_string()),
        keywords: vec![
            "new".to_string(),
            "request".to_string(),
            "nuevo".to_string(),
        ],
        section: "Acción".to_string(),
    }];

    for collection in &state.workspace.collections {
        items.push(command_palette::CommandPaletteItem {
            id: format!("{PALETTE_PREFIX_COLLECTION}{}", collection.collection.id),
            title: collection.collection.name.clone(),
            subtitle: Some(format!("{} request(s)", collection.requests.len())),
            keywords: vec![
                "collection".to_string(),
                "colección".to_string(),
                collection.collection.name.clone(),
            ],
            section: "Colección".to_string(),
        });

        for request in &collection.requests {
            items.push(command_palette::CommandPaletteItem {
                id: format!("{PALETTE_PREFIX_REQUEST}{}", request.id),
                title: request.name.clone(),
                subtitle: Some(format!("{} {}", request.draft.method, request.draft.url)),
                keywords: vec![
                    request.draft.method.to_string(),
                    request.name.clone(),
                    request.draft.url.clone(),
                    collection.collection.name.clone(),
                ],
                section: "Request".to_string(),
            });
        }
    }

    items
}

/// Resuelve y ejecuta la acción correspondiente a `item_id` (Tarea 11.2,
/// Requisito 6.2), según el esquema de prefijos de `build_palette_items`.
/// Un `item_id` que no calza con ningún prefijo/id conocido (por ejemplo un
/// request eliminado entre que se construyeron los resultados y el click)
/// no hace nada, en lugar de entrar en pánico.
fn execute_palette_item(state: &mut Midway, item_id: &str) {
    if item_id == PALETTE_ACTION_NEW_REQUEST {
        state.tabs.push(RequestTabState::blank());
        state.active_tab = Some(state.tabs.len() - 1);
        return;
    }

    if let Some(collection_id) = item_id.strip_prefix(PALETTE_PREFIX_COLLECTION) {
        // No existe todavía un concepto de "colección seleccionada/activa"
        // en `Midway` (el `Workspace_Panel` no tiene una noción de
        // colección actual fuera del formulario de export/import, que usa
        // un `text_input` de id en lugar de una selección persistente):
        // por ahora, seleccionar una colección desde el palette abre la
        // sección Data del `Workspace_Panel` (expandiéndolo si estaba
        // colapsado) con su id precargado en el campo de export, que es la
        // operación más cercana a "ver el contenido de esta colección" que
        // ya existe en la UI actual.
        state.workspace_panel.collapsed = false;
        state.workspace_panel.active_section = WorkspacePanelSection::Data;
        state.workspace_panel.export_form.collection_id_input = collection_id.to_string();
        return;
    }

    if let Some(request_id) = item_id.strip_prefix(PALETTE_PREFIX_REQUEST) {
        open_saved_request_in_tab(state, request_id);
    }
}

/// Abre el `SavedRequestRecord` identificado por `request_id` en una tab
/// (Tarea 11.2, Requisito 6.2): si ya hay una tab abierta cuyo último
/// draft guardado (`saved_draft`) corresponde a este mismo request (mismo
/// id de request, comparado por `id` del draft), la reutiliza y la activa;
/// si no, crea una nueva tab con el draft guardado y la activa. No existe
/// todavía en el codebase un mecanismo genérico de "cargar un
/// SavedRequestRecord en una tab" (el patrón más cercano,
/// `handle_url_pasted`, crea tabs a partir de un draft parseado de cURL,
/// no de un request ya guardado): esta función implementa la versión
/// mínima necesaria para el palette, siguiendo la misma convención de
/// `RequestTabState::from_draft` ya usada en el resto de `app.rs`.
fn open_saved_request_in_tab(state: &mut Midway, request_id: &str) {
    let existing_tab_index = state.tabs.iter().position(|tab| {
        tab.saved_draft
            .as_ref()
            .is_some_and(|saved_draft| saved_draft.id.as_deref() == Some(request_id))
    });

    if let Some(existing_tab_index) = existing_tab_index {
        state.active_tab = Some(existing_tab_index);
        return;
    }

    let saved_request = state
        .workspace
        .collections
        .iter()
        .flat_map(|collection| collection.requests.iter())
        .find(|request| request.id == request_id);

    let Some(saved_request) = saved_request else {
        return;
    };

    let draft = saved_request.draft.clone();
    let mut new_tab = RequestTabState::from_draft(draft.clone());
    new_tab.saved_draft = Some(draft);
    state.tabs.push(new_tab);
    state.active_tab = Some(state.tabs.len() - 1);
}

/// Actualiza el estado en respuesta a un `PaletteMessage` (Tarea 11.2,
/// Requisitos 6.1, 6.2).
fn update_palette(state: &mut Midway, message: PaletteMessage) -> Task<Message> {
    match message {
        PaletteMessage::Toggled => {
            state.palette.is_open = !state.palette.is_open;
            state.palette.query.clear();
            Task::none()
        }
        PaletteMessage::QueryChanged(query) => {
            state.palette.query = query;
            Task::none()
        }
        PaletteMessage::ItemSelected { item_id } => {
            execute_palette_item(state, &item_id);
            state.palette.is_open = false;
            state.palette.query.clear();
            Task::none()
        }
        PaletteMessage::Dismissed => {
            state.palette.is_open = false;
            state.palette.query.clear();
            Task::none()
        }
    }
}

/// Actualiza el estado en respuesta a un `ActivityBarMessage` (Tarea 10.1,
/// Requisitos 2.1–2.8).
fn update_activity_bar(state: &mut Midway, message: ActivityBarMessage) -> Task<Message> {
    match message {
        ActivityBarMessage::HomePressed => {
            // Req 4.1, 4.2: Toggle main_content_focus between RequestTab and
            // WorkspaceSection without modifying active_collection_id, tabs,
            // or active_tab.
            state.workspace_panel.navigation_error = None;
            match state.main_content_focus {
                MainContentFocus::RequestTab => {
                    state.main_content_focus = MainContentFocus::WorkspaceSection;
                }
                MainContentFocus::WorkspaceSection => {
                    state.main_content_focus = MainContentFocus::RequestTab;
                }
            }
            Task::none()
        }
        ActivityBarMessage::CollectionSelected(id) => {
            // Req 2.3, 4.3: fijar la colección activa y refrescar tree, sin
            // tocar las tabs del Composer (ni cuál está activa, ni su
            // contenido no guardado).
            // If currently in WorkspaceSection, switch back to RequestTab.
            if state.main_content_focus == MainContentFocus::WorkspaceSection {
                state.main_content_focus = MainContentFocus::RequestTab;
            }
            state.active_collection_id = Some(id);
            // Reset tree state for the new collection
            state.tree = TreeViewState::default();
            Task::none()
        }
        ActivityBarMessage::ActivityPressed => {
            // Req 8.1, 8.2, 8.3: Navigate to WorkspaceSection/History, or
            // toggle back to RequestTab if already viewing History.
            match state.main_content_focus {
                MainContentFocus::RequestTab => {
                    state.main_content_focus = MainContentFocus::WorkspaceSection;
                    state.workspace_panel.active_section = WorkspacePanelSection::History;
                }
                MainContentFocus::WorkspaceSection
                    if state.workspace_panel.active_section == WorkspacePanelSection::History =>
                {
                    state.main_content_focus = MainContentFocus::RequestTab;
                }
                MainContentFocus::WorkspaceSection => {
                    // Already in workspace but in another section: navigate to History
                    state.workspace_panel.active_section = WorkspacePanelSection::History;
                }
            }
            Task::none()
        }
        ActivityBarMessage::CreateCollectionPressed => {
            // Req 2.5: abrir el prompt de creación
            state.create_collection_prompt = Some(CreateCollectionPromptState {
                name_input: String::new(),
                error: None,
            });
            Task::none()
        }
        ActivityBarMessage::CreateCollectionNameChanged(s) => {
            if let Some(prompt) = &mut state.create_collection_prompt {
                prompt.name_input = s;
                // Clear previous error when user edits
                prompt.error = None;
            }
            Task::none()
        }
        ActivityBarMessage::CreateCollectionConfirmed => {
            let Some(prompt) = &state.create_collection_prompt else {
                return Task::none();
            };
            let trimmed = prompt.name_input.trim().to_string();
            if trimmed.is_empty() {
                // Req 2.8: reject empty name, show error, keep prompt open
                if let Some(prompt) = &mut state.create_collection_prompt {
                    prompt.error = Some("La colección necesita un nombre.".to_string());
                }
                return Task::none();
            }
            // Valid name: dispatch async creation
            let app_state = Arc::clone(&state.app_state);
            let name = trimmed;
            Task::perform(
                async move {
                    app_state
                        .repository
                        .create_collection(name)
                        .await
                        .map_err(|e| e.to_string())
                },
                |result| Message::ActivityBar(ActivityBarMessage::CollectionCreated(result)),
            )
        }
        ActivityBarMessage::CreateCollectionCancelled => {
            // Req 2.7: close prompt without creating anything
            state.create_collection_prompt = None;
            Task::none()
        }
        ActivityBarMessage::CollectionCreated(result) => {
            match result {
                Ok(summary) => {
                    // Close the prompt
                    state.create_collection_prompt = None;
                    // Set as active collection
                    let id = summary.id.clone();
                    // Add to workspace collections as a new entry
                    state.workspace.collections.push(
                        midway_core::domain::workspace::CollectionWithRequests {
                            collection: summary,
                            folders: Vec::new(),
                            requests: Vec::new(),
                        },
                    );
                    state.active_collection_id = Some(id);
                    // Reset tree for new (empty) collection
                    state.tree = TreeViewState::default();
                    state.session.dirty = true;
                }
                Err(msg) => {
                    // Show error in prompt
                    if let Some(prompt) = &mut state.create_collection_prompt {
                        prompt.error = Some(msg);
                    }
                }
            }
            Task::none()
        }
    }
}

/// Actualiza el estado en respuesta a las acciones ABM del árbol.
fn update_workspace_crud(state: &mut Midway, message: WorkspaceCrudMessage) -> Task<Message> {
    match message {
        WorkspaceCrudMessage::CreateFolderRequested { parent_folder_id } => {
            let Some(collection_id) = state.active_collection_id.clone() else {
                state.tree.error =
                    Some("Seleccioná una colección antes de crear una carpeta.".to_string());
                return Task::none();
            };

            let parent_name = parent_folder_id.as_deref().and_then(|folder_id| {
                state
                    .workspace
                    .collections
                    .iter()
                    .find(|collection| collection.collection.id == collection_id)
                    .and_then(|collection| {
                        collection
                            .folders
                            .iter()
                            .find(|folder| folder.id == folder_id)
                    })
                    .map(|folder| folder.name.clone())
            });
            if parent_folder_id.is_some() && parent_name.is_none() {
                state.tree.error = Some("No se pudo encontrar la carpeta padre.".to_string());
                return Task::none();
            }

            state.workspace_crud_dialog = Some(WorkspaceCrudDialogState {
                kind: WorkspaceCrudKind::CreateFolder {
                    collection_id,
                    parent_folder_id,
                },
                entity_name: parent_name.unwrap_or_else(|| "raíz de la colección".to_string()),
                name_input: String::new(),
                busy: false,
                error: None,
            });
            state.tree.error = None;
            Task::none()
        }
        ref message @ (WorkspaceCrudMessage::RenameCollectionRequested(ref collection_id)
        | WorkspaceCrudMessage::DeleteCollectionRequested(ref collection_id)) => {
            let collection_id = collection_id.clone();
            let Some(collection) = state
                .workspace
                .collections
                .iter()
                .find(|collection| collection.collection.id == collection_id)
            else {
                state.tree.error = Some("No se pudo encontrar la colección.".to_string());
                return Task::none();
            };
            let is_delete = matches!(message, WorkspaceCrudMessage::DeleteCollectionRequested(_));
            if is_delete
                && state
                    .runner
                    .as_ref()
                    .is_some_and(|runner| runner.running.is_some())
            {
                state.tree.error = Some(
                    "Cancelá o esperá a que termine la ejecución antes de borrar.".to_string(),
                );
                return Task::none();
            }
            let kind = if is_delete {
                WorkspaceCrudKind::DeleteCollection { collection_id }
            } else {
                WorkspaceCrudKind::RenameCollection { collection_id }
            };
            let name = collection.collection.name.clone();
            state.workspace_crud_dialog = Some(WorkspaceCrudDialogState {
                kind,
                entity_name: name.clone(),
                name_input: name,
                busy: false,
                error: None,
            });
            state.tree.error = None;
            Task::none()
        }
        ref message @ (WorkspaceCrudMessage::RenameFolderRequested(ref folder_id)
        | WorkspaceCrudMessage::DeleteFolderRequested(ref folder_id)) => {
            let folder_id = folder_id.clone();
            let Some(folder) = state
                .workspace
                .collections
                .iter()
                .flat_map(|collection| collection.folders.iter())
                .find(|folder| folder.id == folder_id)
            else {
                state.tree.error = Some("No se pudo encontrar la carpeta.".to_string());
                return Task::none();
            };
            let is_delete = matches!(message, WorkspaceCrudMessage::DeleteFolderRequested(_));
            if is_delete
                && state
                    .runner
                    .as_ref()
                    .is_some_and(|runner| runner.running.is_some())
            {
                state.tree.error = Some(
                    "Cancelá o esperá a que termine la ejecución antes de borrar.".to_string(),
                );
                return Task::none();
            }
            let kind = if is_delete {
                WorkspaceCrudKind::DeleteFolder { folder_id }
            } else {
                WorkspaceCrudKind::RenameFolder { folder_id }
            };
            let name = folder.name.clone();
            state.workspace_crud_dialog = Some(WorkspaceCrudDialogState {
                kind,
                entity_name: name.clone(),
                name_input: name,
                busy: false,
                error: None,
            });
            state.tree.error = None;
            Task::none()
        }
        WorkspaceCrudMessage::DeleteRequestRequested(request_id) => {
            if state
                .runner
                .as_ref()
                .is_some_and(|runner| runner.running.is_some())
            {
                state.tree.error = Some(
                    "Cancelá o esperá a que termine la ejecución antes de borrar.".to_string(),
                );
                return Task::none();
            }
            let Some(request) = state
                .workspace
                .collections
                .iter()
                .flat_map(|collection| collection.requests.iter())
                .find(|request| request.id == request_id)
            else {
                state.tree.error = Some("No se pudo encontrar el request.".to_string());
                return Task::none();
            };
            state.workspace_crud_dialog = Some(WorkspaceCrudDialogState {
                kind: WorkspaceCrudKind::DeleteRequest { request_id },
                entity_name: request.name.clone(),
                name_input: request.name.clone(),
                busy: false,
                error: None,
            });
            state.tree.error = None;
            Task::none()
        }
        WorkspaceCrudMessage::NameChanged(name) => {
            if let Some(dialog) = state.workspace_crud_dialog.as_mut() {
                if !dialog.busy {
                    dialog.name_input = name;
                    dialog.error = None;
                }
            }
            Task::none()
        }
        WorkspaceCrudMessage::Confirmed => {
            let Some(dialog) = state.workspace_crud_dialog.as_mut() else {
                return Task::none();
            };
            if dialog.busy {
                return Task::none();
            }
            let needs_name = matches!(
                dialog.kind,
                WorkspaceCrudKind::CreateFolder { .. }
                    | WorkspaceCrudKind::RenameCollection { .. }
                    | WorkspaceCrudKind::RenameFolder { .. }
            );
            let name = dialog.name_input.trim().to_string();
            if needs_name && name.is_empty() {
                dialog.error = Some("El nombre no puede quedar vacío.".to_string());
                return Task::none();
            }

            dialog.busy = true;
            dialog.error = None;
            let kind = dialog.kind.clone();
            let app_state = Arc::clone(&state.app_state);

            Task::perform(
                async move {
                    match kind {
                        WorkspaceCrudKind::CreateFolder {
                            collection_id,
                            parent_folder_id,
                        } => app_state
                            .repository
                            .create_folder(SaveFolderInput {
                                collection_id,
                                parent_folder_id,
                                name,
                            })
                            .await
                            .map(|_| ()),
                        WorkspaceCrudKind::RenameCollection { collection_id } => app_state
                            .repository
                            .rename_collection(collection_id, name)
                            .await
                            .map(|_| ()),
                        WorkspaceCrudKind::DeleteCollection { collection_id } => {
                            app_state.repository.delete_collection(collection_id).await
                        }
                        WorkspaceCrudKind::RenameFolder { folder_id } => app_state
                            .repository
                            .rename_folder(folder_id, name)
                            .await
                            .map(|_| ()),
                        WorkspaceCrudKind::DeleteFolder { folder_id } => {
                            app_state.repository.delete_folder(folder_id).await
                        }
                        WorkspaceCrudKind::DeleteRequest { request_id } => {
                            app_state.repository.delete_request(request_id).await
                        }
                    }
                    .map_err(|error| error.to_string())?;

                    app_state
                        .repository
                        .workspace_snapshot(HISTORY_LIMIT)
                        .await
                        .map_err(|error| error.to_string())
                },
                |result| Message::WorkspaceCrud(WorkspaceCrudMessage::Completed(result)),
            )
        }
        WorkspaceCrudMessage::Cancelled => {
            if state
                .workspace_crud_dialog
                .as_ref()
                .is_some_and(|dialog| !dialog.busy)
            {
                state.workspace_crud_dialog = None;
            }
            Task::none()
        }
        WorkspaceCrudMessage::Completed(result) => {
            match result {
                Ok(workspace) => reconcile_workspace_after_crud(state, workspace),
                Err(error) => {
                    if let Some(dialog) = state.workspace_crud_dialog.as_mut() {
                        dialog.busy = false;
                        dialog.error = Some(error);
                    }
                }
            }
            Task::none()
        }
    }
}

/// Reemplaza el snapshot y elimina toda referencia de UI a entidades borradas.
fn reconcile_workspace_after_crud(state: &mut Midway, workspace: WorkspaceSnapshot) {
    let old_request_ids: HashSet<String> = state
        .workspace
        .collections
        .iter()
        .flat_map(|collection| collection.requests.iter().map(|request| request.id.clone()))
        .collect();
    let surviving_request_ids: HashSet<String> = workspace
        .collections
        .iter()
        .flat_map(|collection| collection.requests.iter().map(|request| request.id.clone()))
        .collect();
    let deleted_request_ids: HashSet<String> = old_request_ids
        .difference(&surviving_request_ids)
        .cloned()
        .collect();

    let previous_active_collection = state.active_collection_id.clone();
    let previous_active_index = state.active_tab;
    let previous_active_tab_id = previous_active_index
        .and_then(|index| state.tabs.get(index))
        .map(|tab| tab.id.clone());

    state.tabs.retain(|tab| {
        !tab.draft
            .id
            .as_ref()
            .is_some_and(|request_id| deleted_request_ids.contains(request_id))
    });
    state.closed_tabs.retain(|tab| {
        !tab.draft
            .id
            .as_ref()
            .is_some_and(|request_id| deleted_request_ids.contains(request_id))
    });

    if state.tabs.is_empty() {
        state.tabs.push(RequestTabState::blank());
        state.active_tab = Some(0);
    } else {
        state.active_tab = previous_active_tab_id
            .as_deref()
            .and_then(|tab_id| state.tabs.iter().position(|tab| tab.id == tab_id))
            .or_else(|| Some(previous_active_index.unwrap_or(0).min(state.tabs.len() - 1)));
    }

    let open_tab_ids: HashSet<String> = state.tabs.iter().map(|tab| tab.id.clone()).collect();
    if state
        .save_request_prompt
        .as_ref()
        .is_some_and(|prompt| !open_tab_ids.contains(&prompt.tab_id))
    {
        state.save_request_prompt = None;
    }
    if state
        .unsaved_changes_prompt
        .as_ref()
        .is_some_and(|prompt| !open_tab_ids.contains(&prompt.tab_id))
    {
        state.unsaved_changes_prompt = None;
    }

    let surviving_folder_ids: HashSet<String> = workspace
        .collections
        .iter()
        .flat_map(|collection| collection.folders.iter().map(|folder| folder.id.clone()))
        .collect();
    state
        .tree
        .collapsed
        .retain(|folder_id| surviving_folder_ids.contains(folder_id));
    if let Some(snapshot) = state.tree.collapsed_snapshot.as_mut() {
        snapshot.retain(|folder_id| surviving_folder_ids.contains(folder_id));
    }

    state.active_collection_id = resolve_startup_collection(
        &workspace.collections,
        previous_active_collection.as_deref(),
    );
    if state.active_collection_id != previous_active_collection {
        state.tree = TreeViewState::default();
    } else {
        state.tree.error = None;
    }
    if state.active_collection_id.is_none() {
        state.top_bar_mode = TopBarMode::Debug;
    }

    state.workspace = workspace;
    state.workspace_crud_dialog = None;
    state.session.dirty = true;
}

/// Actualiza el estado en respuesta a un `TopBarMessage` (Tarea 12.1).
///
/// `ModeSelected(mode)`:
/// - Si mode == Test y no hay `active_collection_id`: se ignora (la tab está
///   deshabilitada, no debería llegar, pero por seguridad se descarta).
/// - En caso contrario: fija `top_bar_mode = mode`.
fn update_top_bar(state: &mut Midway, message: TopBarMessage) -> Task<Message> {
    match message {
        TopBarMessage::ModeSelected(mode) => {
            if mode == TopBarMode::Test && state.active_collection_id.is_none() {
                // Tab Test disabled without active collection (Req 6.7) — ignore.
                return Task::none();
            }
            state.top_bar_mode = mode;
            // Selecting a mode tab always brings focus back to the request/test
            // area (away from WorkspaceSection if that was showing).
            state.main_content_focus = MainContentFocus::RequestTab;
            Task::none()
        }
        TopBarMessage::BreadcrumbRootClicked => {
            // Req 3.5: Navigate to initial state (RequestTab + Debug mode).
            state.main_content_focus = MainContentFocus::RequestTab;
            state.top_bar_mode = TopBarMode::Debug;
            Task::none()
        }
        TopBarMessage::BackToComposer => {
            // Req 4.5: Back navigation from WorkspaceSection to RequestTab.
            state.main_content_focus = MainContentFocus::RequestTab;
            Task::none()
        }
    }
}

/// Constantes de redimensionamiento del tree pane.
const TREE_PANE_MIN_WIDTH: f32 = 150.0;
const TREE_PANE_MAX_WIDTH: f32 = 500.0;
const TREE_DIVIDER_HIT_WIDTH: f32 = 10.0;
const DEBUG_DIVIDER_HIT_WIDTH: f32 = 10.0;
const DEBUG_PANE_MIN_WIDTH: f32 = 220.0;
const DEBUG_REQUEST_PANE_MAX_WIDTH: f32 = 1_200.0;
/// Ancho fijo del Activity_Bar (no redimensionable).
const ACTIVITY_BAR_WIDTH: f32 = 48.0;

/// Constantes de redimensionamiento vertical del panel de respuesta en la
/// disposición apilada del área Debug.
///
/// `RESPONSE_PANE_MIN_HEIGHT` y `RESPONSE_PANE_MAX_HEIGHT` son exactamente el
/// rango del clamp fijo que `debug_split_content` aplicaba en línea, extraído
/// a constantes: para las sesiones existentes no cambia lo que se ve.
const RESPONSE_PANE_MIN_HEIGHT: f32 = 180.0;
const RESPONSE_PANE_MAX_HEIGHT: f32 = 360.0;
/// Alto de la zona sensible del divisor horizontal, simétrico a los
/// divisores verticales (`TREE_DIVIDER_HIT_WIDTH`, `DEBUG_DIVIDER_HIT_WIDTH`).
const RESPONSE_DIVIDER_HIT_HEIGHT: f32 = 10.0;
/// Alto mínimo que se le reserva al editor de request para que arrastrar el
/// divisor horizontal no lo colapse.
const REQUEST_PANE_MIN_HEIGHT: f32 = 160.0;

fn tree_pane_width_from_cursor(cursor_x: f32) -> f32 {
    (cursor_x - ACTIVITY_BAR_WIDTH).clamp(TREE_PANE_MIN_WIDTH, TREE_PANE_MAX_WIDTH)
}

fn request_panel_width_from_cursor(cursor_x: f32, tree_pane_width: f32) -> f32 {
    (cursor_x - ACTIVITY_BAR_WIDTH - tree_pane_width - TREE_DIVIDER_HIT_WIDTH)
        .clamp(DEBUG_PANE_MIN_WIDTH, DEBUG_REQUEST_PANE_MAX_WIDTH)
}

fn request_panel_width_for_available(stored_width: f32, available_width: f32) -> f32 {
    let max_width = (available_width - DEBUG_DIVIDER_HIT_WIDTH - DEBUG_PANE_MIN_WIDTH)
        .max(DEBUG_PANE_MIN_WIDTH);
    stored_width.clamp(DEBUG_PANE_MIN_WIDTH, max_width)
}

/// Altura del panel de respuesta a partir de la coordenada Y del cursor y del
/// borde inferior del área Debug.
///
/// Semántica ante valores no finitos, decidida acá de forma explícita (criterio
/// de cierre de L35 en `docs/known-limitations.md`):
///
/// - `+inf` como alto resultante queda en `RESPONSE_PANE_MAX_HEIGHT` y `-inf`
///   en `RESPONSE_PANE_MIN_HEIGHT`: `f32::clamp` ya ordena los infinitos.
/// - `NaN` —incluido el que produce `inf - inf` o cualquier argumento `NaN`—
///   **no se propaga**: degrada a `RESPONSE_PANE_MIN_HEIGHT`, el destino
///   seguro, porque `f32::clamp` deja pasar `NaN` sin acotarlo.
/// - No hay pánico posible: los dos límites son constantes finitas con
///   `min <= max`, la única condición bajo la que `f32::clamp` entra en pánico.
///
/// El resultado siempre queda dentro de
/// `[RESPONSE_PANE_MIN_HEIGHT, RESPONSE_PANE_MAX_HEIGHT]`.
fn response_panel_height_from_cursor(cursor_y: f32, area_bottom_y: f32) -> f32 {
    let height = area_bottom_y - cursor_y;
    if height.is_nan() {
        RESPONSE_PANE_MIN_HEIGHT
    } else {
        height.clamp(RESPONSE_PANE_MIN_HEIGHT, RESPONSE_PANE_MAX_HEIGHT)
    }
}

/// Altura efectiva del panel de respuesta dado el alto disponible: nunca deja
/// al editor de request por debajo de `REQUEST_PANE_MIN_HEIGHT`.
///
/// El rango de salida es exactamente el del clamp fijo que `debug_split_content`
/// aplicaba en línea: `[RESPONSE_PANE_MIN_HEIGHT, RESPONSE_PANE_MAX_HEIGHT]`.
/// El techo se recorta además al espacio que sobra tras reservar el divisor y
/// el mínimo del editor, sin bajar nunca del mínimo del panel de respuesta.
///
/// Semántica ante valores no finitos, decidida acá de forma explícita:
///
/// - `available_height` no finito o tan chico que no deja espacio: el techo se
///   satura en `RESPONSE_PANE_MIN_HEIGHT`, así que el resultado es el mínimo.
///   `f32::max` descarta `NaN`, de modo que un alto disponible `NaN` degrada al
///   mínimo en vez de contaminar los límites de `clamp`.
/// - `stored_height` igual a `NaN`: degrada a `RESPONSE_PANE_MIN_HEIGHT` en vez
///   de propagarse, igual que en `response_panel_height_from_cursor`.
/// - No hay pánico posible: el techo se construye con `max` y `min` sobre
///   constantes finitas, así que queda en `[RESPONSE_PANE_MIN_HEIGHT,
///   RESPONSE_PANE_MAX_HEIGHT]` y nunca es `NaN` ni menor que el piso.
// `clippy::manual_clamp` sugiere reemplazar `max(...).min(...)` por
// `clamp(...)` en el cálculo de `max_height`. NO se aplica a propósito: la
// semántica ante `NaN` es distinta. `f32::max` descarta `NaN` (devuelve el
// otro operando), mientras que `f32::clamp` lo propaga. El orden
// `max`-luego-`min` es justamente lo que sanea un `available_height` igual a
// `NaN` degradándolo al mínimo, en vez de contaminar el techo y con él el
// resultado. Ese comportamiento está fijado por
// `property_5_resize_bounds_clamp_and_are_idempotent` (Propiedad 5) en el
// módulo `panel_resize_tests` y es el criterio de cierre de la limitación
// `L35` en `docs/known-limitations.md`. Aplicar la sugerencia reintroduciría
// la propagación de `NaN` y rompería la Propiedad 5.
#[allow(clippy::manual_clamp)]
fn response_panel_height_for_available(stored_height: f32, available_height: f32) -> f32 {
    let room_for_response =
        available_height - RESPONSE_DIVIDER_HIT_HEIGHT - REQUEST_PANE_MIN_HEIGHT;
    // `max` primero: descarta `NaN` y garantiza `min <= max` para el `clamp`
    // final. `min` después: preserva el techo de 360 del baseline.
    let max_height = room_for_response
        .max(RESPONSE_PANE_MIN_HEIGHT)
        .min(RESPONSE_PANE_MAX_HEIGHT);
    if stored_height.is_nan() {
        RESPONSE_PANE_MIN_HEIGHT
    } else {
        stored_height.clamp(RESPONSE_PANE_MIN_HEIGHT, max_height)
    }
}

#[cfg(test)]
mod panel_resize_tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn tree_pane_width_tracks_cursor_after_activity_bar() {
        assert_eq!(
            tree_pane_width_from_cursor(ACTIVITY_BAR_WIDTH + 320.0),
            320.0
        );
    }

    #[test]
    fn tree_pane_width_is_clamped_to_supported_range() {
        assert_eq!(tree_pane_width_from_cursor(0.0), TREE_PANE_MIN_WIDTH);
        assert_eq!(
            tree_pane_width_from_cursor(ACTIVITY_BAR_WIDTH + TREE_PANE_MAX_WIDTH + 100.0),
            TREE_PANE_MAX_WIDTH
        );
    }

    #[test]
    fn panel_resize_start_drag_and_end_updates_state() {
        let mut state = super::tests::build_test_midway(create_blank_draft());

        let _ = update_panel_resize(&mut state, PanelResizeMessage::TreeDividerDragStarted);
        assert_eq!(state.panel_dragging, Some(PanelDragState::TreeMain));

        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerDragged {
                x: ACTIVITY_BAR_WIDTH + 360.0,
                y: 0.0,
            },
        );
        assert_eq!(state.session.panel_sizes.workspace_panel_width, 360.0);
        assert!(state.session.dirty);

        let _ = update_panel_resize(&mut state, PanelResizeMessage::DividerDragEnded);
        assert!(state.panel_dragging.is_none());
    }

    #[test]
    fn request_response_resize_tracks_cursor_after_left_panels() {
        let mut state = super::tests::build_test_midway(create_blank_draft());
        state.session.panel_sizes.workspace_panel_width = 300.0;

        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::RequestResponseDividerDragStarted,
        );
        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerDragged {
                x: ACTIVITY_BAR_WIDTH + 300.0 + TREE_DIVIDER_HIT_WIDTH + 420.0,
                y: 0.0,
            },
        );

        assert_eq!(state.panel_dragging, Some(PanelDragState::RequestResponse));
        assert_eq!(state.session.panel_sizes.request_panel_width, 420.0);
        assert!(state.session.dirty);
    }

    #[test]
    fn response_height_resize_tracks_cursor_and_persists_on_release() {
        let mut state = super::tests::build_test_midway(create_blank_draft());
        let initial_tree_width = state.session.panel_sizes.workspace_panel_width;
        let initial_request_width = state.session.panel_sizes.request_panel_width;

        // El divisor vive en `area_bottom_y = 700`; el cursor a 460 deja
        // 240 px de alto para el panel de respuesta, dentro del rango.
        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::ResponseHeightDividerDragStarted {
                area_bottom_y: 700.0,
            },
        );
        assert_eq!(
            state.panel_dragging,
            Some(PanelDragState::ResponseHeight {
                area_bottom_y: 700.0
            })
        );

        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerDragged { x: 999.0, y: 460.0 },
        );

        assert_eq!(state.session.panel_sizes.response_panel_height, 240.0);
        assert!(state.session.dirty);
        // El eje X no toca los divisores verticales durante un arrastre
        // horizontal (Req 9.7).
        assert_eq!(
            state.session.panel_sizes.workspace_panel_width,
            initial_tree_width
        );
        assert_eq!(
            state.session.panel_sizes.request_panel_width,
            initial_request_width
        );

        let _ = update_panel_resize(&mut state, PanelResizeMessage::DividerDragEnded);
        assert!(state.panel_dragging.is_none());
        // Al soltar, la altura escrita durante el arrastre es la que queda en
        // el snapshot que persiste el autosave existente (Req 9.3).
        assert_eq!(state.session.panel_sizes.response_panel_height, 240.0);
        assert!(state.session.dirty);
    }

    #[test]
    fn response_height_drag_is_ignored_without_active_drag() {
        let mut state = super::tests::build_test_midway(create_blank_draft());
        let initial_height = state.session.panel_sizes.response_panel_height;

        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerDragged { x: 0.0, y: 460.0 },
        );

        assert_eq!(
            state.session.panel_sizes.response_panel_height,
            initial_height
        );
        assert!(!state.session.dirty);
    }

    #[test]
    fn response_height_drag_state_compares_by_divider_not_by_geometry() {
        // `panel_dragging`/`panel_hovered` se comparan en la vista para decidir
        // el estado visual del divisor: un cambio de tamaño de ventana (otro
        // `area_bottom_y`) no debe leerse como "otro divisor".
        assert_eq!(
            PanelDragState::ResponseHeight {
                area_bottom_y: 700.0
            },
            PanelDragState::ResponseHeight {
                area_bottom_y: 320.0
            }
        );
        assert_ne!(
            PanelDragState::ResponseHeight {
                area_bottom_y: 700.0
            },
            PanelDragState::RequestResponse
        );
    }

    #[test]
    fn response_height_divider_propagates_the_area_geometry_to_the_reducer() {
        // Tarea 10.3: el divisor apilado construye su mensaje de inicio de
        // arrastre con `divider_drag_started_message`, que es lo que lleva el
        // `area_bottom_y` calculado por la vista hasta el reducer. Sin esa
        // propagación el reducer no podría convertir la Y del cursor en alto de
        // panel (Req 9.2).
        let started = divider_drag_started_message(PanelDragState::ResponseHeight {
            area_bottom_y: 640.0,
        });
        assert!(matches!(
            started,
            PanelResizeMessage::ResponseHeightDividerDragStarted { area_bottom_y }
                if area_bottom_y == 640.0
        ));

        // Los dos divisores verticales siguen produciendo exactamente sus
        // mensajes del baseline (Req 9.7).
        assert!(matches!(
            divider_drag_started_message(PanelDragState::TreeMain),
            PanelResizeMessage::TreeDividerDragStarted
        ));
        assert!(matches!(
            divider_drag_started_message(PanelDragState::RequestResponse),
            PanelResizeMessage::RequestResponseDividerDragStarted
        ));
    }

    #[test]
    fn both_dividers_share_the_same_three_visual_states() {
        // El divisor horizontal debe leerse como parte del mismo sistema que
        // los verticales: no redefine colores ni grosores, usa los mismos
        // `divider_line_color` / `divider_line_thickness` que ya usaba
        // `vertical_resize_divider`.
        let ds = DesignSystem::for_mode(crate::ui::design_system::ThemeMode::Dark);
        let accent = ds.palette.accent;

        // Reposo: acento con α 0.18 y línea de 2 px.
        let rest = divider_line_color(&ds, false, false);
        assert_eq!(rest.a, 0.18);
        assert_eq!((rest.r, rest.g, rest.b), (accent.r, accent.g, accent.b));
        assert_eq!(divider_line_thickness(false), 2.0);

        // Hover: mismo acento con α 0.75, sigue en 2 px.
        let hovered = divider_line_color(&ds, false, true);
        assert_eq!(hovered.a, 0.75);
        assert_eq!(
            (hovered.r, hovered.g, hovered.b),
            (accent.r, accent.g, accent.b)
        );

        // Activo: acento sólido y línea de 3 px. `active` gana sobre `hovered`,
        // que es el estado real durante un arrastre.
        assert_eq!(divider_line_color(&ds, true, false), accent);
        assert_eq!(divider_line_color(&ds, true, true), accent);
        assert_eq!(divider_line_thickness(true), 3.0);
    }

    #[test]
    fn stacked_divider_drag_resolves_to_the_height_the_layout_renders() {
        // Recorrido completo del cableado de la Tarea 10.3, sin ventana: la
        // vista calcula `area_bottom_y` a partir del alto de ventana, el
        // divisor emite el mensaje de inicio, el listener global aporta la Y
        // del cursor y el alto resultante es el que el layout apilado le da al
        // inspector para ese mismo alto disponible.
        let mut state = super::tests::build_test_midway(create_blank_draft());
        let window_height = 720.0;
        // Alto del área Debug: la ventana menos el cromo de arriba (top bar y
        // toolbar). El valor exacto no importa, sí que sea menor que la ventana.
        let available_height = 600.0;

        let _ = update_panel_resize(
            &mut state,
            divider_drag_started_message(PanelDragState::ResponseHeight {
                area_bottom_y: window_height,
            }),
        );
        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerDragged {
                x: 0.0,
                y: window_height - 250.0,
            },
        );
        let _ = update_panel_resize(&mut state, PanelResizeMessage::DividerDragEnded);

        assert_eq!(state.session.panel_sizes.response_panel_height, 250.0);
        // Con 600 px de área hay lugar de sobra (250 + 10 + 160 < 600), así que
        // el layout apilado renderiza exactamente la altura arrastrada.
        assert_eq!(
            response_panel_height_for_available(
                state.session.panel_sizes.response_panel_height,
                available_height
            ),
            250.0
        );
        // La altura queda en la sesión marcada como sucia: la persiste el
        // autosave existente como `panelSizes.responsePanelHeight` (Req 9.3) y
        // `restore_pending_session` la devuelve al reiniciar (Req 9.4). No hay
        // ningún camino de escritura paralelo.
        assert!(state.session.dirty);
        assert!(state.panel_dragging.is_none());
    }

    #[test]
    fn request_panel_width_keeps_both_panes_usable() {
        assert_eq!(
            request_panel_width_for_available(900.0, 700.0),
            700.0 - DEBUG_DIVIDER_HIT_WIDTH - DEBUG_PANE_MIN_WIDTH,
        );
        assert_eq!(
            request_panel_width_for_available(10.0, 700.0),
            DEBUG_PANE_MIN_WIDTH,
        );
    }

    #[test]
    fn global_mouse_events_map_to_drag_messages() {
        let moved =
            panel_resize_message_for_event(&iced::Event::Mouse(iced::mouse::Event::CursorMoved {
                position: iced::Point::new(444.0, 120.0),
            }));
        assert!(matches!(
            moved,
            Some(PanelResizeMessage::DividerDragged { x, y }) if x == 444.0 && y == 120.0
        ));

        let released = panel_resize_message_for_event(&iced::Event::Mouse(
            iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
        ));
        assert!(matches!(
            released,
            Some(PanelResizeMessage::DividerDragEnded)
        ));

        let left_window =
            panel_resize_message_for_event(&iced::Event::Mouse(iced::mouse::Event::CursorLeft));
        assert!(matches!(
            left_window,
            Some(PanelResizeMessage::DividerDragEnded)
        ));
    }

    #[test]
    fn panel_resize_ignores_cursor_movement_without_active_drag() {
        let mut state = super::tests::build_test_midway(create_blank_draft());
        let initial_width = state.session.panel_sizes.workspace_panel_width;

        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerDragged {
                x: ACTIVITY_BAR_WIDTH + 400.0,
                y: 0.0,
            },
        );

        assert_eq!(
            state.session.panel_sizes.workspace_panel_width,
            initial_width
        );
        assert!(!state.session.dirty);
    }

    #[test]
    fn divider_hover_tracks_only_the_current_handle() {
        let mut state = super::tests::build_test_midway(create_blank_draft());

        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerHovered(PanelDragState::TreeMain),
        );
        assert_eq!(state.panel_hovered, Some(PanelDragState::TreeMain));

        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerUnhovered(PanelDragState::RequestResponse),
        );
        assert_eq!(state.panel_hovered, Some(PanelDragState::TreeMain));

        let _ = update_panel_resize(
            &mut state,
            PanelResizeMessage::DividerUnhovered(PanelDragState::TreeMain),
        );
        assert!(state.panel_hovered.is_none());
    }

    // --- Caracterización de los límites de redimensionamiento vigentes (Req 6.1, 6.5) ---
    //
    // Los tests que siguen fijan el comportamiento OBSERVADO del baseline sin
    // modificar `tree_pane_width_from_cursor`, `request_panel_width_from_cursor`
    // ni `request_panel_width_for_available`. Afirman lo que el código hace hoy,
    // no lo que debería hacer. El test de propiedad de estas funciones llega en
    // la tarea 10.4 (Property 5).

    #[test]
    fn tree_pane_width_clamps_exactly_at_range_boundaries() {
        // Justo en los bordes el resultado es el borde mismo.
        assert_eq!(
            tree_pane_width_from_cursor(ACTIVITY_BAR_WIDTH + TREE_PANE_MIN_WIDTH),
            TREE_PANE_MIN_WIDTH
        );
        assert_eq!(
            tree_pane_width_from_cursor(ACTIVITY_BAR_WIDTH + TREE_PANE_MAX_WIDTH),
            TREE_PANE_MAX_WIDTH
        );
        // Un paso fuera de cada borde queda clampeado al borde.
        assert_eq!(
            tree_pane_width_from_cursor(ACTIVITY_BAR_WIDTH + TREE_PANE_MIN_WIDTH - 0.5),
            TREE_PANE_MIN_WIDTH
        );
        assert_eq!(
            tree_pane_width_from_cursor(ACTIVITY_BAR_WIDTH + TREE_PANE_MAX_WIDTH + 0.5),
            TREE_PANE_MAX_WIDTH
        );
    }

    #[test]
    fn tree_pane_width_handles_degenerate_cursor_coordinates() {
        // Cursor negativo o a la izquierda del Activity_Bar: mínimo.
        assert_eq!(tree_pane_width_from_cursor(-1_000.0), TREE_PANE_MIN_WIDTH);
        assert_eq!(
            tree_pane_width_from_cursor(f32::NEG_INFINITY),
            TREE_PANE_MIN_WIDTH
        );
        // Infinito positivo: máximo.
        assert_eq!(
            tree_pane_width_from_cursor(f32::INFINITY),
            TREE_PANE_MAX_WIDTH
        );
        // Comportamiento observado con NaN: `f32::clamp` no clampea NaN, lo
        // propaga. El baseline no entra en pánico, pero tampoco acota.
        assert!(tree_pane_width_from_cursor(f32::NAN).is_nan());
    }

    #[test]
    fn request_panel_width_from_cursor_passes_through_inside_range() {
        let tree_pane_width = 300.0;
        let origin = ACTIVITY_BAR_WIDTH + tree_pane_width + TREE_DIVIDER_HIT_WIDTH;

        assert_eq!(
            request_panel_width_from_cursor(origin + 640.0, tree_pane_width),
            640.0
        );
        assert_eq!(
            request_panel_width_from_cursor(origin + DEBUG_PANE_MIN_WIDTH, tree_pane_width),
            DEBUG_PANE_MIN_WIDTH
        );
        assert_eq!(
            request_panel_width_from_cursor(origin + DEBUG_REQUEST_PANE_MAX_WIDTH, tree_pane_width),
            DEBUG_REQUEST_PANE_MAX_WIDTH
        );
    }

    #[test]
    fn request_panel_width_from_cursor_clamps_outside_range() {
        let tree_pane_width = 300.0;
        let origin = ACTIVITY_BAR_WIDTH + tree_pane_width + TREE_DIVIDER_HIT_WIDTH;

        assert_eq!(
            request_panel_width_from_cursor(origin + DEBUG_PANE_MIN_WIDTH - 1.0, tree_pane_width),
            DEBUG_PANE_MIN_WIDTH
        );
        assert_eq!(
            request_panel_width_from_cursor(
                origin + DEBUG_REQUEST_PANE_MAX_WIDTH + 1.0,
                tree_pane_width
            ),
            DEBUG_REQUEST_PANE_MAX_WIDTH
        );
        // Cursor en el origen de la ventana o negativo: mínimo.
        assert_eq!(
            request_panel_width_from_cursor(0.0, tree_pane_width),
            DEBUG_PANE_MIN_WIDTH
        );
        assert_eq!(
            request_panel_width_from_cursor(-5_000.0, tree_pane_width),
            DEBUG_PANE_MIN_WIDTH
        );
    }

    #[test]
    fn request_panel_width_from_cursor_handles_degenerate_inputs() {
        let tree_pane_width = 300.0;

        assert_eq!(
            request_panel_width_from_cursor(f32::INFINITY, tree_pane_width),
            DEBUG_REQUEST_PANE_MAX_WIDTH
        );
        assert_eq!(
            request_panel_width_from_cursor(f32::NEG_INFINITY, tree_pane_width),
            DEBUG_PANE_MIN_WIDTH
        );
        // NaN en cualquiera de los dos argumentos se propaga sin clampear.
        assert!(request_panel_width_from_cursor(f32::NAN, tree_pane_width).is_nan());
        assert!(request_panel_width_from_cursor(600.0, f32::NAN).is_nan());

        // Comportamiento observado con un ancho de explorador negativo: no se
        // saneiza, se resta, de modo que infla el ancho resultante.
        assert_eq!(
            request_panel_width_from_cursor(400.0, -200.0),
            400.0 - ACTIVITY_BAR_WIDTH + 200.0 - TREE_DIVIDER_HIT_WIDTH
        );
    }

    #[test]
    fn request_panel_width_from_cursor_ignores_available_width() {
        // El arrastre solo conoce el rango fijo [220, 1200]: puede fijar un
        // ancho almacenado mayor al que el layout luego permitirá. La
        // reconciliación queda a cargo de `request_panel_width_for_available`.
        let stored = request_panel_width_from_cursor(5_000.0, 150.0);
        assert_eq!(stored, DEBUG_REQUEST_PANE_MAX_WIDTH);
        assert_eq!(
            request_panel_width_for_available(stored, 700.0),
            700.0 - DEBUG_DIVIDER_HIT_WIDTH - DEBUG_PANE_MIN_WIDTH
        );
    }

    #[test]
    fn request_panel_width_for_available_passes_through_inside_range() {
        // available = 1000 → máximo efectivo = 1000 - 10 - 220 = 770.
        assert_eq!(request_panel_width_for_available(500.0, 1_000.0), 500.0);
        assert_eq!(request_panel_width_for_available(770.0, 1_000.0), 770.0);
        assert_eq!(request_panel_width_for_available(771.0, 1_000.0), 770.0);
        assert_eq!(
            request_panel_width_for_available(219.9, 1_000.0),
            DEBUG_PANE_MIN_WIDTH
        );
    }

    #[test]
    fn request_panel_width_for_available_collapses_to_minimum_when_space_is_tight() {
        // Comportamiento observado: por debajo de 450 px de ancho disponible el
        // máximo efectivo se satura en DEBUG_PANE_MIN_WIDTH, así que el editor
        // se queda con 220 px y el panel restante recibe menos que ese mínimo.
        assert_eq!(
            request_panel_width_for_available(400.0, 449.0),
            DEBUG_PANE_MIN_WIDTH
        );
        assert_eq!(
            request_panel_width_for_available(400.0, 0.0),
            DEBUG_PANE_MIN_WIDTH
        );
        assert_eq!(
            request_panel_width_for_available(400.0, -800.0),
            DEBUG_PANE_MIN_WIDTH
        );
        // El primer ancho disponible que deja crecer al editor es 451.
        assert_eq!(
            request_panel_width_for_available(400.0, 450.0),
            DEBUG_PANE_MIN_WIDTH
        );
        assert_eq!(request_panel_width_for_available(400.0, 451.0), 221.0);
    }

    #[test]
    fn request_panel_width_for_available_handles_non_finite_inputs() {
        // `f32::max` descarta NaN, así que un ancho disponible NaN degrada al
        // mínimo en lugar de entrar en pánico dentro de `clamp`.
        assert_eq!(
            request_panel_width_for_available(400.0, f32::NAN),
            DEBUG_PANE_MIN_WIDTH
        );
        // Un ancho almacenado NaN, en cambio, se propaga.
        assert!(request_panel_width_for_available(f32::NAN, 1_000.0).is_nan());
        // Sin límite superior real, el ancho almacenado pasa tal cual.
        assert_eq!(
            request_panel_width_for_available(500.0, f32::INFINITY),
            500.0
        );
        assert_eq!(
            request_panel_width_for_available(500.0, f32::NEG_INFINITY),
            DEBUG_PANE_MIN_WIDTH
        );
    }

    // Feature: midway-baseline-audit-and-first-vertical, Tarea 10.1
    // Requisitos 9.2, 9.7.
    //
    // Tests de ejemplo de las funciones nuevas de altura. Fijan la semántica
    // elegida —clampeada y sin pánico ante valores no finitos, con `NaN`
    // degradando al mínimo— que es el criterio de cierre de L35 en
    // `docs/known-limitations.md`. El test de propiedad de las cinco funciones
    // (Property 5) llega en la tarea 10.4.

    #[test]
    fn response_panel_height_tracks_cursor_from_bottom_edge() {
        // La altura es la distancia del cursor al borde inferior del área.
        assert_eq!(response_panel_height_from_cursor(400.0, 700.0), 300.0);
        // Los bordes del rango se devuelven tal cual.
        assert_eq!(
            response_panel_height_from_cursor(700.0 - RESPONSE_PANE_MIN_HEIGHT, 700.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
        assert_eq!(
            response_panel_height_from_cursor(700.0 - RESPONSE_PANE_MAX_HEIGHT, 700.0),
            RESPONSE_PANE_MAX_HEIGHT
        );
    }

    #[test]
    fn response_panel_height_from_cursor_is_clamped_to_supported_range() {
        // Cursor por debajo del borde inferior: altura negativa → mínimo.
        assert_eq!(
            response_panel_height_from_cursor(900.0, 700.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
        // Cursor muy arriba: altura enorme → máximo.
        assert_eq!(
            response_panel_height_from_cursor(-5_000.0, 700.0),
            RESPONSE_PANE_MAX_HEIGHT
        );
    }

    #[test]
    fn response_panel_height_from_cursor_handles_non_finite_inputs() {
        // Infinitos: `clamp` los ordena en los bordes, sin pánico.
        assert_eq!(
            response_panel_height_from_cursor(f32::NEG_INFINITY, 700.0),
            RESPONSE_PANE_MAX_HEIGHT
        );
        assert_eq!(
            response_panel_height_from_cursor(f32::INFINITY, 700.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
        assert_eq!(
            response_panel_height_from_cursor(400.0, f32::INFINITY),
            RESPONSE_PANE_MAX_HEIGHT
        );
        assert_eq!(
            response_panel_height_from_cursor(400.0, f32::NEG_INFINITY),
            RESPONSE_PANE_MIN_HEIGHT
        );
        // A diferencia de las funciones de ancho (L35), `NaN` no se propaga:
        // degrada al mínimo. Incluye el `NaN` que produce `inf - inf`.
        assert_eq!(
            response_panel_height_from_cursor(f32::NAN, 700.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
        assert_eq!(
            response_panel_height_from_cursor(400.0, f32::NAN),
            RESPONSE_PANE_MIN_HEIGHT
        );
        assert_eq!(
            response_panel_height_from_cursor(f32::INFINITY, f32::INFINITY),
            RESPONSE_PANE_MIN_HEIGHT
        );
    }

    #[test]
    fn response_panel_height_for_available_preserves_the_baseline_range() {
        // Con alto disponible amplio el resultado es exactamente el del
        // `.clamp(180.0, 360.0)` que había en línea en `debug_split_content`.
        for available in [530.0f32, 700.0, 1_200.0, 10_000.0] {
            for stored in [0.0f32, 179.9, 180.0, 320.0, 360.0, 360.1, 5_000.0] {
                assert_eq!(
                    response_panel_height_for_available(stored, available),
                    stored.clamp(RESPONSE_PANE_MIN_HEIGHT, RESPONSE_PANE_MAX_HEIGHT),
                    "stored={stored}, available={available}"
                );
            }
        }
    }

    #[test]
    fn response_panel_height_for_available_reserves_the_request_editor_minimum() {
        // 530 = 360 + 10 + 160 es el primer alto disponible que permite el
        // techo completo. Por debajo, el techo cede para no colapsar el editor.
        assert_eq!(
            response_panel_height_for_available(360.0, 530.0),
            RESPONSE_PANE_MAX_HEIGHT
        );
        assert_eq!(response_panel_height_for_available(360.0, 500.0), 330.0);
        // Cuando ya no queda espacio, el techo se satura en el mínimo del
        // panel de respuesta: el resultado nunca baja de 180.
        assert_eq!(
            response_panel_height_for_available(360.0, 300.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
        assert_eq!(
            response_panel_height_for_available(360.0, 0.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
        assert_eq!(
            response_panel_height_for_available(360.0, -800.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
    }

    #[test]
    fn response_panel_height_for_available_handles_non_finite_inputs() {
        // Alto disponible no finito: sin pánico y dentro del rango.
        assert_eq!(
            response_panel_height_for_available(320.0, f32::INFINITY),
            320.0
        );
        assert_eq!(
            response_panel_height_for_available(320.0, f32::NEG_INFINITY),
            RESPONSE_PANE_MIN_HEIGHT
        );
        // `f32::max` descarta el `NaN` del techo, así que el clamp queda con
        // límites finitos y el valor almacenado se acota al mínimo.
        assert_eq!(
            response_panel_height_for_available(320.0, f32::NAN),
            RESPONSE_PANE_MIN_HEIGHT
        );
        // Altura almacenada `NaN`: degrada al mínimo en vez de propagarse.
        assert_eq!(
            response_panel_height_for_available(f32::NAN, 1_000.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
        assert_eq!(
            response_panel_height_for_available(f32::INFINITY, 1_000.0),
            RESPONSE_PANE_MAX_HEIGHT
        );
        assert_eq!(
            response_panel_height_for_available(f32::NEG_INFINITY, 1_000.0),
            RESPONSE_PANE_MIN_HEIGHT
        );
    }

    // --- Property 5 (Tarea 10.4, Requisitos 6.5 y 9.7) ---
    //
    // Espacio de entrada y su recorte declarado.
    //
    // El enunciado de la Property 5 habla de "toda coordenada de cursor y todo
    // ancho o alto disponible". El espacio de entrada real de las cinco
    // funciones NO es uniforme, y esta sección lo declara antes de generar
    // nada, porque la asimetría está documentada como comportamiento fijado del
    // baseline en `docs/known-limitations.md`:
    //
    // - `L35`: las tres funciones de ancho vigentes **no acotan `NaN`, lo
    //   propagan** (`f32::clamp` deja pasar `NaN`). El criterio de cierre de
    //   `L35` dice literalmente que si la Property 5 obligara a sanear `NaN` en
    //   esas tres funciones, ese cambio se decide con el usuario y no entra en
    //   esta tarea. Así que acá **no** se toca producción: se excluye `NaN` del
    //   generador de las tres funciones de ancho y la exclusión queda escrita.
    //   Para `request_panel_width_from_cursor` la exclusión alcanza también a
    //   `±inf` en el ancho de explorador almacenado, porque `inf - inf` produce
    //   `NaN` y eso es la misma propagación de `L35` por otra vía.
    // - `L36` y `L37`: por debajo de 451 px el panel derecho recibe menos que
    //   su mínimo, y un ancho de explorador negativo infla el ancho de request.
    //   Ninguna de las dos cosas rompe el recorte de la función que se mide
    //   acá, así que los negativos y los anchos disponibles chicos **sí** entran
    //   al generador: son parte del espacio válido, no una excepción.
    // - Las dos funciones nuevas de altura de la tarea 10.1 sí sanean `NaN` al
    //   mínimo, así que para ellas el generador incluye `NaN` sin excepción
    //   alguna. Eso es lo que cierra la mitad "funciones nuevas" del criterio
    //   de `L35`.
    //
    // La exclusión no vuelve la propiedad vacía: sobre las funciones de ancho se
    // siguen generando cero, negativos, los bordes exactos de cada constante,
    // los extremos finitos del tipo y `±inf`, y se afirma recorte, no
    // propagación, idempotencia y determinismo.

    /// Extensión de layout arbitraria **sin `NaN`**: coordenadas de cursor y
    /// anchos que se le pasan a las tres funciones de ancho vigentes.
    ///
    /// Cubre a propósito las clases que un rango uniforme casi nunca alcanzaría:
    /// cero y `-0.0`, negativos, los bordes exactos de cada constante de rango,
    /// los extremos finitos del tipo y los dos infinitos.
    fn arb_layout_extent_without_nan() -> impl Strategy<Value = f32> {
        prop_oneof![
            // Coordenadas plausibles de ventana, incluyendo negativas.
            6 => -3_000.0f32..8_000.0f32,
            // Ceros y bordes exactos de los rangos declarados, donde vive el
            // límite real de cada clamp.
            4 => prop_oneof![
                Just(0.0f32),
                Just(-0.0f32),
                Just(ACTIVITY_BAR_WIDTH),
                Just(TREE_PANE_MIN_WIDTH),
                Just(TREE_PANE_MAX_WIDTH),
                Just(ACTIVITY_BAR_WIDTH + TREE_PANE_MIN_WIDTH),
                Just(ACTIVITY_BAR_WIDTH + TREE_PANE_MAX_WIDTH),
                Just(DEBUG_PANE_MIN_WIDTH),
                Just(DEBUG_REQUEST_PANE_MAX_WIDTH),
                Just(RESPONSE_PANE_MIN_HEIGHT),
                Just(RESPONSE_PANE_MAX_HEIGHT),
                // 450 y 451: el borde de `L36`.
                Just(450.0f32),
                Just(451.0f32),
                // 530 = 360 + 10 + 160: primer alto que permite el techo completo.
                Just(530.0f32),
            ],
            // Extremos finitos del tipo.
            1 => prop_oneof![
                Just(f32::MIN),
                Just(f32::MAX),
                Just(f32::MIN_POSITIVE),
                Just(-f32::MIN_POSITIVE),
            ],
            // No finitos representables como orden: `clamp` sí los acota.
            2 => prop_oneof![
                Just(f32::INFINITY),
                Just(f32::NEG_INFINITY),
            ],
        ]
    }

    /// Igual que [`arb_layout_extent_without_nan`] pero **con `NaN`**: es el
    /// espacio de entrada de las dos funciones de altura de la tarea 10.1, que
    /// sanean `NaN` al mínimo en vez de propagarlo.
    fn arb_layout_extent_with_nan() -> impl Strategy<Value = f32> {
        prop_oneof![
            8 => arb_layout_extent_without_nan(),
            2 => prop_oneof![Just(f32::NAN), Just(-f32::NAN)],
        ]
    }

    /// Ancho de explorador **finito** almacenado en la sesión, usado como
    /// desplazamiento por `request_panel_width_from_cursor`.
    ///
    /// Finito por la razón de `L35` explicada arriba: `inf - inf` es `NaN` y
    /// esta tarea no cambia producción para sanearlo. Incluye negativos y cero
    /// a propósito: son el espacio de `L37`, que la propiedad sí cubre.
    fn arb_finite_stored_pane_width() -> impl Strategy<Value = f32> {
        prop_oneof![
            6 => -1_000.0f32..2_000.0f32,
            4 => prop_oneof![
                Just(0.0f32),
                Just(-0.0f32),
                Just(-200.0f32),
                Just(TREE_PANE_MIN_WIDTH),
                Just(TREE_PANE_MAX_WIDTH),
            ],
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // Feature: midway-baseline-audit-and-first-vertical, Property 5: Los límites de redimensionamiento acotan e idempotizan
        /// Para toda coordenada de cursor y todo ancho o alto disponible del
        /// espacio declarado arriba, las cinco funciones de redimensionamiento
        /// devuelven un valor dentro de su rango declarado, son idempotentes al
        /// reaplicarse sobre su propio resultado y terminan sin pánico.
        ///
        /// Cómo se lee cada una de las tres partes:
        ///
        /// - **Acotan.** Se afirma el rango declarado de cada función y, en las
        ///   dos que reconcilian un valor almacenado contra el espacio
        ///   disponible, que el recorte nunca *agranda* el valor almacenado por
        ///   encima del piso.
        /// - **Idempotizan.** `*_for_available` toma `(almacenado, disponible)`,
        ///   así que reaplicarla sobre su propio resultado con el mismo
        ///   disponible es un punto fijo exacto, y eso es lo que se afirma. Las
        ///   `*_from_cursor` mapean una coordenada a una extensión: son dos
        ///   espacios distintos y `f(f(x))` no es una reaplicación (de hecho es
        ///   falsa para un clamp trasladado: `tree_pane_width_from_cursor(500)`
        ///   da 452). Para ellas la reaplicación que corresponde es la del
        ///   propio recorte sobre el resultado, más el punto fijo de la
        ///   composición real del producto: arrastre seguido de reconciliación
        ///   de layout, que es la secuencia que ocurre en cada frame.
        /// - **Sin pánico.** `f32::clamp` entra en pánico si `min > max`. Que la
        ///   ejecución llegue a las afirmaciones ya es la evidencia de que
        ///   ninguna de las cinco lo hace para esta entrada.
        #[test]
        fn property_5_resize_bounds_clamp_and_are_idempotent(
            cursor_x in arb_layout_extent_without_nan(),
            tree_pane_width in arb_finite_stored_pane_width(),
            stored_request_width in arb_layout_extent_without_nan(),
            cursor_y in arb_layout_extent_with_nan(),
            area_bottom_y in arb_layout_extent_with_nan(),
            stored_response_height in arb_layout_extent_with_nan(),
            available_extent in arb_layout_extent_with_nan(),
        ) {
            // --- 1. `tree_pane_width_from_cursor` ---
            let tree_width = tree_pane_width_from_cursor(cursor_x);
            prop_assert!(
                !tree_width.is_nan(),
                "tree_pane_width_from_cursor({cursor_x}) devolvió NaN: sin NaN en la \
                 entrada el resultado tiene que quedar acotado (L35)"
            );
            prop_assert!(
                (TREE_PANE_MIN_WIDTH..=TREE_PANE_MAX_WIDTH).contains(&tree_width),
                "tree_pane_width_from_cursor({}) = {} quedó fuera de [{}, {}]",
                cursor_x,
                tree_width,
                TREE_PANE_MIN_WIDTH,
                TREE_PANE_MAX_WIDTH
            );
            prop_assert_eq!(
                tree_width.clamp(TREE_PANE_MIN_WIDTH, TREE_PANE_MAX_WIDTH),
                tree_width,
                "reaplicar el recorte del explorador sobre su propio resultado debe ser \
                 un punto fijo"
            );
            prop_assert_eq!(
                tree_pane_width_from_cursor(cursor_x),
                tree_width,
                "tree_pane_width_from_cursor debe ser determinista"
            );

            // --- 2. `request_panel_width_from_cursor` ---
            let request_width = request_panel_width_from_cursor(cursor_x, tree_pane_width);
            prop_assert!(
                !request_width.is_nan(),
                "request_panel_width_from_cursor({cursor_x}, {tree_pane_width}) devolvió \
                 NaN con entrada sin NaN ni infinitos en el ancho almacenado (L35)"
            );
            prop_assert!(
                (DEBUG_PANE_MIN_WIDTH..=DEBUG_REQUEST_PANE_MAX_WIDTH).contains(&request_width),
                "request_panel_width_from_cursor({}, {}) = {} quedó fuera de [{}, {}]",
                cursor_x,
                tree_pane_width,
                request_width,
                DEBUG_PANE_MIN_WIDTH,
                DEBUG_REQUEST_PANE_MAX_WIDTH
            );
            prop_assert_eq!(
                request_width.clamp(DEBUG_PANE_MIN_WIDTH, DEBUG_REQUEST_PANE_MAX_WIDTH),
                request_width,
                "reaplicar el recorte del editor de request sobre su propio resultado \
                 debe ser un punto fijo"
            );
            prop_assert_eq!(
                request_panel_width_from_cursor(cursor_x, tree_pane_width),
                request_width,
                "request_panel_width_from_cursor debe ser determinista"
            );

            // --- 3. `request_panel_width_for_available` ---
            let fitted_width =
                request_panel_width_for_available(stored_request_width, available_extent);
            prop_assert!(
                !fitted_width.is_nan(),
                "request_panel_width_for_available({stored_request_width}, \
                 {available_extent}) devolvió NaN con un ancho almacenado sin NaN (L35)"
            );
            prop_assert!(
                fitted_width >= DEBUG_PANE_MIN_WIDTH,
                "request_panel_width_for_available({}, {}) = {} bajó del mínimo {}",
                stored_request_width,
                available_extent,
                fitted_width,
                DEBUG_PANE_MIN_WIDTH
            );
            // El recorte solo puede achicar: nunca devuelve más que el ancho
            // almacenado, salvo cuando lo levanta hasta el piso.
            prop_assert!(
                fitted_width <= stored_request_width.max(DEBUG_PANE_MIN_WIDTH),
                "request_panel_width_for_available({}, {}) = {} agrandó el ancho \
                 almacenado por encima de max(almacenado, mínimo) = {}",
                stored_request_width,
                available_extent,
                fitted_width,
                stored_request_width.max(DEBUG_PANE_MIN_WIDTH)
            );
            // Punto fijo exacto: reconciliar dos veces contra el mismo espacio
            // disponible da el mismo resultado.
            prop_assert_eq!(
                request_panel_width_for_available(fitted_width, available_extent),
                fitted_width,
                "request_panel_width_for_available debe ser idempotente sobre su propio \
                 resultado"
            );

            // --- 4. `response_panel_height_from_cursor` ---
            let dragged_height = response_panel_height_from_cursor(cursor_y, area_bottom_y);
            // A diferencia de las funciones de ancho, acá `NaN` sí entra al
            // generador y el resultado debe quedar acotado igual: es el criterio
            // de cierre de L35 para las funciones nuevas de la tarea 10.1.
            prop_assert!(
                dragged_height.is_finite(),
                "response_panel_height_from_cursor({cursor_y}, {area_bottom_y}) = \
                 {dragged_height} tiene que ser finito incluso con entradas no finitas"
            );
            prop_assert!(
                (RESPONSE_PANE_MIN_HEIGHT..=RESPONSE_PANE_MAX_HEIGHT).contains(&dragged_height),
                "response_panel_height_from_cursor({}, {}) = {} quedó fuera de [{}, {}]",
                cursor_y,
                area_bottom_y,
                dragged_height,
                RESPONSE_PANE_MIN_HEIGHT,
                RESPONSE_PANE_MAX_HEIGHT
            );
            prop_assert_eq!(
                dragged_height.clamp(RESPONSE_PANE_MIN_HEIGHT, RESPONSE_PANE_MAX_HEIGHT),
                dragged_height,
                "reaplicar el recorte de altura sobre su propio resultado debe ser un \
                 punto fijo"
            );
            prop_assert_eq!(
                response_panel_height_from_cursor(cursor_y, area_bottom_y),
                dragged_height,
                "response_panel_height_from_cursor debe ser determinista"
            );

            // --- 5. `response_panel_height_for_available` ---
            let fitted_height =
                response_panel_height_for_available(stored_response_height, available_extent);
            prop_assert!(
                fitted_height.is_finite(),
                "response_panel_height_for_available({stored_response_height}, \
                 {available_extent}) = {fitted_height} tiene que ser finito"
            );
            prop_assert!(
                (RESPONSE_PANE_MIN_HEIGHT..=RESPONSE_PANE_MAX_HEIGHT).contains(&fitted_height),
                "response_panel_height_for_available({}, {}) = {} quedó fuera de [{}, {}]",
                stored_response_height,
                available_extent,
                fitted_height,
                RESPONSE_PANE_MIN_HEIGHT,
                RESPONSE_PANE_MAX_HEIGHT
            );
            if !stored_response_height.is_nan() {
                prop_assert!(
                    fitted_height <= stored_response_height.max(RESPONSE_PANE_MIN_HEIGHT),
                    "response_panel_height_for_available({}, {}) = {} agrandó la altura \
                     almacenada por encima de max(almacenada, mínimo) = {}",
                    stored_response_height,
                    available_extent,
                    fitted_height,
                    stored_response_height.max(RESPONSE_PANE_MIN_HEIGHT)
                );
            }
            prop_assert_eq!(
                response_panel_height_for_available(fitted_height, available_extent),
                fitted_height,
                "response_panel_height_for_available debe ser idempotente sobre su propio \
                 resultado"
            );

            // --- 6. Las composiciones que ocurren en el producto ---
            //
            // Arrastrar y después reconciliar contra el espacio disponible es la
            // secuencia real de cada frame: el arrastre escribe en la sesión y
            // la vista recorta contra el `Size` del `responsive`. Esa
            // composición también tiene que ser un punto fijo, o el panel
            // "saltaría" mientras la ventana no cambia de tamaño.
            let width_pipeline = request_panel_width_for_available(request_width, available_extent);
            prop_assert_eq!(
                request_panel_width_for_available(width_pipeline, available_extent),
                width_pipeline,
                "arrastrar y reconciliar el ancho de request debe estabilizarse en un paso"
            );
            prop_assert!(
                width_pipeline >= DEBUG_PANE_MIN_WIDTH && !width_pipeline.is_nan(),
                "el ancho de request tras arrastre y reconciliación quedó en {width_pipeline}"
            );

            let height_pipeline =
                response_panel_height_for_available(dragged_height, available_extent);
            prop_assert_eq!(
                response_panel_height_for_available(height_pipeline, available_extent),
                height_pipeline,
                "arrastrar y reconciliar la altura de respuesta debe estabilizarse en un paso"
            );
            prop_assert!(
                (RESPONSE_PANE_MIN_HEIGHT..=RESPONSE_PANE_MAX_HEIGHT).contains(&height_pipeline),
                "la altura de respuesta tras arrastre y reconciliación quedó en \
                 {height_pipeline}"
            );
        }
    }

    // --- Property 6 (Tarea 10.5, Requisitos 9.2 y 9.3) ---
    //
    // Acá el sujeto ya no son las funciones puras de recorte (esa es la
    // Property 5) sino el reducer completo: `update_panel_resize` recorriendo
    // una secuencia de arrastre real, de `ResponseHeightDividerDragStarted` a
    // `DividerDragEnded`.
    //
    // Espacio de entrada y por qué es el que es:
    //
    // - `area_bottom_y` y las coordenadas de cada movimiento se generan con
    //   `arb_layout_extent_with_nan` (Tarea 10.4), que ya incluye cero,
    //   negativos, los bordes exactos de cada constante, los extremos finitos
    //   del tipo, `±inf` y `NaN`. Para el eje Y eso es legítimo sin excepción:
    //   `response_panel_height_from_cursor` sanea `NaN` al mínimo en vez de
    //   propagarlo. Para el eje X es aún más fuerte que legítimo: es
    //   precisamente el punto donde una regresión del Req 9.7 se haría visible,
    //   porque durante un arrastre horizontal el reducer no debe mirar `x` ni
    //   siquiera cuando vale `NaN` o `f32::MAX`.
    // - La secuencia tiene **al menos** un `DividerDragged`. Un arrastre sin
    //   ningún movimiento no escribe nada —el reducer solo guarda la geometría
    //   al empezar y la limpia al soltar—, así que no marcaría la sesión sucia:
    //   ese caso no es un redimensionamiento y ya está fijado como ejemplo en
    //   `response_height_drag_is_ignored_without_active_drag`.

    /// Secuencia acotada de movimientos del cursor durante un arrastre activo,
    /// como pares `(x, y)` en coordenadas de ventana.
    ///
    /// Entre 1 y 8 movimientos: alcanza para distinguir "el último gana" de
    /// "se acumula" y mantiene el costo por caso acotado, ya que cada caso
    /// construye un `Midway` real.
    fn arb_response_drag_moves() -> impl Strategy<Value = Vec<(f32, f32)>> {
        proptest::collection::vec(
            (arb_layout_extent_with_nan(), arb_layout_extent_with_nan()),
            1..=8,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

        // Feature: midway-baseline-audit-and-first-vertical, Property 6: El arrastre del divisor de respuesta deja altura acotada y estado consistente
        /// Para toda secuencia de mensajes de arrastre iniciada con
        /// `ResponseHeightDividerDragStarted`, seguida de cualquier cantidad de
        /// `DividerDragged { x, y }` y terminada con `DividerDragEnded`, la
        /// altura resultante en `session.panel_sizes.response_panel_height`
        /// queda dentro del rango soportado, `session.dirty` queda en `true`,
        /// `panel_dragging` queda en `None` al final, y ni el ancho del
        /// explorador ni el ancho del editor de request cambian respecto de sus
        /// valores previos.
        ///
        /// Las cuatro afirmaciones se leen así:
        ///
        /// - **Altura acotada.** El rango es
        ///   `[RESPONSE_PANE_MIN_HEIGHT, RESPONSE_PANE_MAX_HEIGHT]`, el mismo
        ///   que el clamp fijo del baseline, y el valor además es finito: nunca
        ///   queda `NaN` guardado en la sesión, ni con `NaN` en la entrada.
        /// - **Sesión sucia.** Es la condición que dispara el autosave
        ///   existente, que es lo único que persiste la altura como
        ///   `panelSizes.responsePanelHeight` (Req 9.3). Sin esa marca el
        ///   arrastre se perdería al reiniciar.
        /// - **Arrastre cerrado.** `panel_dragging == None` al final: soltar
        ///   siempre libera el divisor, sin importar qué pasó en el medio.
        /// - **No interferencia (Req 9.7).** Los dos anchos verticales quedan
        ///   exactamente en sus valores previos, con igualdad de `f32` y no con
        ///   tolerancia: el reducer no debe tocarlos durante un arrastre
        ///   horizontal.
        #[test]
        fn property_6_response_divider_drag_bounds_height_and_keeps_state_consistent(
            area_bottom_y in arb_layout_extent_with_nan(),
            moves in arb_response_drag_moves(),
        ) {
            let mut state = super::tests::build_test_midway(create_blank_draft());
            let initial_tree_width = state.session.panel_sizes.workspace_panel_width;
            let initial_request_width = state.session.panel_sizes.request_panel_width;
            prop_assert!(
                !state.session.dirty,
                "el estado de prueba tiene que arrancar limpio para que la marca de sucio \
                 que se afirma abajo sea la del arrastre"
            );

            let _ = update_panel_resize(
                &mut state,
                PanelResizeMessage::ResponseHeightDividerDragStarted { area_bottom_y },
            );
            prop_assert_eq!(
                state.panel_dragging,
                Some(PanelDragState::ResponseHeight { area_bottom_y }),
                "iniciar el arrastre debe dejar activo el divisor horizontal"
            );

            for (x, y) in &moves {
                let _ = update_panel_resize(
                    &mut state,
                    PanelResizeMessage::DividerDragged { x: *x, y: *y },
                );
                // Invariante durante todo el arrastre, no solo al final: el
                // panel sigue al cursor sin salirse nunca del rango, así que
                // ningún frame intermedio muestra una altura inválida.
                let height = state.session.panel_sizes.response_panel_height;
                prop_assert!(
                    height.is_finite()
                        && (RESPONSE_PANE_MIN_HEIGHT..=RESPONSE_PANE_MAX_HEIGHT)
                            .contains(&height),
                    "durante el arrastre la altura quedó en {} con cursor ({}, {}) y \
                     area_bottom_y = {}",
                    height,
                    x,
                    y,
                    area_bottom_y
                );
            }

            let _ = update_panel_resize(&mut state, PanelResizeMessage::DividerDragEnded);

            // 1. Altura acotada y finita.
            let final_height = state.session.panel_sizes.response_panel_height;
            prop_assert!(
                final_height.is_finite(),
                "la altura persistida quedó no finita ({final_height}) tras el arrastre"
            );
            prop_assert!(
                (RESPONSE_PANE_MIN_HEIGHT..=RESPONSE_PANE_MAX_HEIGHT).contains(&final_height),
                "la altura persistida {} quedó fuera de [{}, {}]",
                final_height,
                RESPONSE_PANE_MIN_HEIGHT,
                RESPONSE_PANE_MAX_HEIGHT
            );
            // El último movimiento es el que queda: el arrastre sigue al
            // cursor, no acumula desplazamientos.
            let (_, last_y) = moves[moves.len() - 1];
            prop_assert_eq!(
                final_height,
                response_panel_height_from_cursor(last_y, area_bottom_y),
                "la altura persistida debe ser la del último movimiento del arrastre"
            );

            // 2. Sesión sucia: es lo que habilita al autosave existente a
            //    persistir `panelSizes.responsePanelHeight` (Req 9.3).
            prop_assert!(
                state.session.dirty,
                "el arrastre debe marcar la sesión como sucia para que el autosave la \
                 persista"
            );

            // 3. Arrastre cerrado.
            prop_assert_eq!(
                state.panel_dragging,
                None,
                "soltar el divisor debe dejar panel_dragging en None"
            );

            // 4. No interferencia con los divisores verticales (Req 9.7),
            //    incluso con coordenadas X extremas o `NaN`.
            prop_assert_eq!(
                state.session.panel_sizes.workspace_panel_width,
                initial_tree_width,
                "un arrastre horizontal no debe cambiar el ancho del explorador"
            );
            prop_assert_eq!(
                state.session.panel_sizes.request_panel_width,
                initial_request_width,
                "un arrastre horizontal no debe cambiar el ancho del editor de request"
            );
        }
    }
}

/// Actualiza el estado en respuesta a un `PanelResizeMessage`.
///
/// Lógica de arrastre directa: el ancho del tree pane es simplemente
/// `cursor_x - ACTIVITY_BAR_WIDTH`, clampeado al rango permitido.
fn update_panel_resize(state: &mut Midway, message: PanelResizeMessage) -> Task<Message> {
    match message {
        PanelResizeMessage::TreeDividerDragStarted => {
            state.panel_dragging = Some(PanelDragState::TreeMain);
            Task::none()
        }
        PanelResizeMessage::RequestResponseDividerDragStarted => {
            state.panel_dragging = Some(PanelDragState::RequestResponse);
            Task::none()
        }
        PanelResizeMessage::ResponseHeightDividerDragStarted { area_bottom_y } => {
            // La geometría del área Debug viaja con el estado de arrastre: el
            // listener global solo aporta la posición del cursor.
            state.panel_dragging = Some(PanelDragState::ResponseHeight { area_bottom_y });
            Task::none()
        }
        PanelResizeMessage::DividerHovered(divider) => {
            state.panel_hovered = Some(divider);
            Task::none()
        }
        PanelResizeMessage::DividerUnhovered(divider) => {
            if state.panel_hovered == Some(divider) {
                state.panel_hovered = None;
            }
            Task::none()
        }
        PanelResizeMessage::DividerDragged {
            x: current_x,
            y: current_y,
        } => {
            // El eje lo elige el divisor activo: los verticales siguen leyendo
            // solo `x`, el horizontal solo `y`.
            match state.panel_dragging {
                Some(PanelDragState::TreeMain) => {
                    state.session.panel_sizes.workspace_panel_width =
                        tree_pane_width_from_cursor(current_x);
                    state.session.dirty = true;
                }
                Some(PanelDragState::RequestResponse) => {
                    state.session.panel_sizes.request_panel_width = request_panel_width_from_cursor(
                        current_x,
                        state.session.panel_sizes.workspace_panel_width,
                    );
                    state.session.dirty = true;
                }
                Some(PanelDragState::ResponseHeight { area_bottom_y }) => {
                    // Se escribe en cada movimiento, igual que los divisores
                    // verticales, para que el panel siga al cursor. Al soltar,
                    // el último valor escrito es el que persiste el autosave
                    // existente como `panelSizes.responsePanelHeight`.
                    state.session.panel_sizes.response_panel_height =
                        response_panel_height_from_cursor(current_y, area_bottom_y);
                    state.session.dirty = true;
                }
                None => {}
            }
            Task::none()
        }
        PanelResizeMessage::DividerDragEnded => {
            state.panel_dragging = None;
            Task::none()
        }
    }
}

/// Req 6.8: Si la colección activa ya no existe en el workspace y el modo
/// actual es Test, cae automáticamente a Debug. También limpia
/// `active_collection_id` si la colección referenciada fue eliminada.
fn enforce_top_bar_mode_after_collection_change(state: &mut Midway) {
    if let Some(ref active_id) = state.active_collection_id {
        let still_exists = state
            .workspace
            .collections
            .iter()
            .any(|c| c.collection.id == *active_id);
        if !still_exists {
            state.active_collection_id = None;
            if state.top_bar_mode == TopBarMode::Test {
                state.top_bar_mode = TopBarMode::Debug;
            }
        }
    }
}

/// Construye el `SessionSnapshot` a persistir a partir del estado actual de
/// `Midway` (Tarea 11.4). El `id` de tab activa se resuelve a partir del
/// índice `active_tab` actual; `closed_tabs` refleja el stack de tabs
/// cerradas mantenido en memoria (Tarea 11.8).
fn build_session_snapshot(state: &Midway) -> crate::session::SessionSnapshot {
    let active_tab_id = state
        .active_tab
        .and_then(|index| state.tabs.get(index))
        .map(|tab| tab.id.clone());

    let open_tabs = state
        .tabs
        .iter()
        .map(RequestTabState::to_snapshot)
        .collect();

    let closed_tabs = state.closed_tabs.iter().cloned().collect();

    crate::session::SessionSnapshot {
        version: crate::session::SESSION_SCHEMA_VERSION,
        active_tab_id,
        open_tabs,
        closed_tabs,
        panel_sizes: state.session.panel_sizes.clone(),
        theme_mode: state.theme.mode(),
        saved_at: chrono::Utc::now().to_rfc3339(),
        active_collection_id: state.active_collection_id.clone(),
    }
}

/// Actualiza el estado en respuesta a un `RequestComposerMessage` (Tarea
/// 3.2). Solo cubre los controles de la fila superior: método, URL,
/// environment; `SendPressed` (Tarea 3.18) y `SettingsPressed` (Tarea 3.20)
/// se implementan en tareas posteriores.
fn update_request_composer(state: &mut Midway, message: RequestComposerMessage) -> Task<Message> {
    // `UrlPasted` puede crear una nueva tab y cambiar `active_tab`, y
    // `SendCompleted` identifica su tab por `tab_id` (no por la tab activa,
    // que puede haber cambiado mientras la request estaba en curso). Ambas
    // se manejan por separado antes de tomar un préstamo mutable de la tab
    // activa (las demás variantes sí operan directamente sobre ella).
    match message {
        RequestComposerMessage::UrlPasted(pasted_text) => {
            handle_url_pasted(state, pasted_text);
            return Task::none();
        }
        RequestComposerMessage::SendCompleted { tab_id, result } => {
            handle_send_completed(state, tab_id, result);
            return Task::none();
        }
        RequestComposerMessage::PreviewLoaded { tab_id, result } => {
            handle_preview_loaded(state, tab_id, result);
            return Task::none();
        }
        RequestComposerMessage::TabClosed { tab_id } => {
            close_tab_or_prompt_unsaved_changes(state, tab_id);
            return Task::none();
        }
        RequestComposerMessage::TabSelected { index } => {
            if index < state.tabs.len() {
                state.active_tab = Some(index);
                state.main_content_focus = MainContentFocus::RequestTab;
            }
            return Task::none();
        }
        RequestComposerMessage::ClosedTabReopened => {
            handle_closed_tab_reopened(state);
            return Task::none();
        }
        RequestComposerMessage::UnsavedChangesSaveRequested => {
            return handle_unsaved_changes_save_requested(state);
        }
        RequestComposerMessage::UnsavedChangesSaveCompleted(result) => {
            handle_unsaved_changes_save_completed(state, result);
            return Task::none();
        }
        RequestComposerMessage::UnsavedChangesDiscardRequested => {
            handle_unsaved_changes_discard_requested(state);
            return Task::none();
        }
        RequestComposerMessage::UnsavedChangesCancelRequested => {
            // Cancelar: descarta el aviso sin cerrar la tab ni mutar su
            // draft/saved_draft de ninguna forma (Requisito 6.6).
            state.unsaved_changes_prompt = None;
            return Task::none();
        }
        RequestComposerMessage::SaveRequested => {
            open_save_request_prompt(state);
            return Task::none();
        }
        RequestComposerMessage::SaveNameChanged(name) => {
            if let Some(prompt) = state.save_request_prompt.as_mut() {
                prompt.name_input = name;
                prompt.error = None;
            }
            return Task::none();
        }
        RequestComposerMessage::SaveCollectionChanged(collection_id) => {
            if state
                .workspace
                .collections
                .iter()
                .any(|collection| collection.collection.id == collection_id)
            {
                if let Some(prompt) = state.save_request_prompt.as_mut() {
                    prompt.collection_id = Some(collection_id);
                    prompt.folder_id = None;
                    prompt.error = None;
                }
            }
            return Task::none();
        }
        RequestComposerMessage::SaveFolderChanged(folder_id) => {
            let is_valid = state.save_request_prompt.as_ref().is_some_and(|prompt| {
                folder_id.is_none()
                    || state.workspace.collections.iter().any(|collection| {
                        Some(collection.collection.id.as_str()) == prompt.collection_id.as_deref()
                            && collection
                                .folders
                                .iter()
                                .any(|folder| Some(folder.id.as_str()) == folder_id.as_deref())
                    })
            });
            if is_valid {
                if let Some(prompt) = state.save_request_prompt.as_mut() {
                    prompt.folder_id = folder_id;
                    prompt.error = None;
                }
            }
            return Task::none();
        }
        RequestComposerMessage::SaveConfirmed => {
            return handle_save_request_confirmed(state);
        }
        RequestComposerMessage::SaveCancelled => {
            if state
                .save_request_prompt
                .as_ref()
                .is_some_and(|prompt| !prompt.saving)
            {
                state.save_request_prompt = None;
            }
            return Task::none();
        }
        RequestComposerMessage::SaveCompleted(result) => {
            handle_save_request_completed(state, result);
            return Task::none();
        }
        _ => {}
    }

    let Some(active_index) = state.active_tab else {
        return Task::none();
    };

    match message {
        // Tarea 5.2 (Requisitos 3.2, 3.3): al cambiar el método, selecciona
        // la tab de configuración por defecto (Params para GET/HEAD/OPTIONS,
        // Body para POST/PUT/PATCH/DELETE), salvo que el usuario ya haya
        // tomado control manual de la tab activa (`manual_tab_override`),
        // en cuyo caso la tab activa permanece sin cambios.
        RequestComposerMessage::MethodChanged(method) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.draft.method = method;
            if !active_tab.manual_tab_override {
                active_tab.active_request_tab = default_tab_for_method(method);
            }
            Task::none()
        }
        RequestComposerMessage::UrlChanged(url) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.draft.url = url;
            active_tab.curl_paste_error = None;
            Task::none()
        }
        RequestComposerMessage::EnvironmentChanged(environment_id) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.draft.environment_id = environment_id;
            Task::none()
        }
        // Tarea 3.18 (Requisito 2.17): ejecuta el request de la tab activa
        // mediante llamadas directas a `midway-core` (`resolve_request` +
        // `request_executor.execute`), sin pasar por Tauri.
        RequestComposerMessage::SendPressed => handle_send_pressed(state, active_index),
        // Tarea 3.20 (Requisito 2.18): alterna el preview drawer.
        RequestComposerMessage::SettingsPressed => handle_settings_pressed(state, active_index),
        // Tarea 5.2 (Requisito 3.3): una selección manual de tab marca
        // `manual_tab_override` en `true`, de forma que los cambios de
        // método HTTP subsiguientes ya no reemplacen la tab activa elegida
        // por el usuario. El flag permanece en `true` de forma persistente
        // (no hay regla de reset en el diseño/Requisitos 3.2-3.3): una vez
        // que el usuario toma control manual, se respeta su elección hasta
        // que la tab de request se cierre.
        RequestComposerMessage::ConfigTabSelected(tab) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.active_request_tab = tab;
            active_tab.manual_tab_override = true;
            Task::none()
        }
        // Tarea 5.4 (Requisito 3.10): al cambiar el tipo de autenticación,
        // se reemplaza la variante completa de `draft.auth` por su default
        // vacío, descartando los campos de la variante anterior.
        RequestComposerMessage::AuthTypeChanged(kind) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.draft.auth = match kind {
                AuthKind::None => AuthConfig::None,
                AuthKind::Bearer => AuthConfig::Bearer {
                    token: String::new(),
                },
                AuthKind::Basic => AuthConfig::Basic {
                    username: String::new(),
                    password: String::new(),
                },
                AuthKind::ApiKey => AuthConfig::ApiKey {
                    key: String::new(),
                    value: String::new(),
                    placement: ApiKeyPlacement::Header,
                },
            };
            Task::none()
        }
        RequestComposerMessage::BearerTokenChanged(token) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let AuthConfig::Bearer {
                token: current_token,
            } = &mut active_tab.draft.auth
            {
                *current_token = token;
            }
            Task::none()
        }
        RequestComposerMessage::BasicUsernameChanged(username) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let AuthConfig::Basic {
                username: current_username,
                ..
            } = &mut active_tab.draft.auth
            {
                *current_username = username;
            }
            Task::none()
        }
        RequestComposerMessage::BasicPasswordChanged(password) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let AuthConfig::Basic {
                password: current_password,
                ..
            } = &mut active_tab.draft.auth
            {
                *current_password = password;
            }
            Task::none()
        }
        RequestComposerMessage::ApiKeyKeyChanged(key) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let AuthConfig::ApiKey {
                key: current_key, ..
            } = &mut active_tab.draft.auth
            {
                *current_key = key;
            }
            Task::none()
        }
        RequestComposerMessage::ApiKeyValueChanged(value) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let AuthConfig::ApiKey {
                value: current_value,
                ..
            } = &mut active_tab.draft.auth
            {
                *current_value = value;
            }
            Task::none()
        }
        RequestComposerMessage::ApiKeyPlacementChanged(placement) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let AuthConfig::ApiKey {
                placement: current_placement,
                ..
            } = &mut active_tab.draft.auth
            {
                *current_placement = placement;
            }
            Task::none()
        }
        // Tarea 5.6 (Requisito 3.8): editor de filas key/value compartido
        // entre las tabs Params (`draft.query`) y Headers (`draft.headers`),
        // discriminado por `KeyValueTarget`. `key_value_rows_mut` evita
        // duplicar el `match` sobre `target` en cada una de las 5 variantes.
        RequestComposerMessage::KeyValueRowAdded(target) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            key_value_rows_mut(&mut active_tab.draft, target).push(KeyValueRow {
                id: uuid::Uuid::new_v4().to_string(),
                key: String::new(),
                value: String::new(),
                enabled: true,
            });
            Task::none()
        }
        RequestComposerMessage::KeyValueRowKeyChanged {
            target,
            row_id,
            key,
        } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(row) = key_value_rows_mut(&mut active_tab.draft, target)
                .iter_mut()
                .find(|row| row.id == row_id)
            {
                row.key = key;
            }
            Task::none()
        }
        RequestComposerMessage::KeyValueRowValueChanged {
            target,
            row_id,
            value,
        } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(row) = key_value_rows_mut(&mut active_tab.draft, target)
                .iter_mut()
                .find(|row| row.id == row_id)
            {
                row.value = value;
            }
            Task::none()
        }
        RequestComposerMessage::KeyValueRowEnabledToggled { target, row_id } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(row) = key_value_rows_mut(&mut active_tab.draft, target)
                .iter_mut()
                .find(|row| row.id == row_id)
            {
                row.enabled = !row.enabled;
            }
            Task::none()
        }
        RequestComposerMessage::KeyValueRowRemoved { target, row_id } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            key_value_rows_mut(&mut active_tab.draft, target).retain(|row| row.id != row_id);
            Task::none()
        }
        // Tarea 5.8 (Requisito 3.9): editor de `domain::testing::
        // ResponseAssertion` de la tab Tests. La evaluación (Tarea 3.18,
        // `evaluate_response_assertions`) ya está integrada en el flujo de
        // Send; estas variantes solo mutan `draft.response_tests`.
        RequestComposerMessage::AssertionAdded => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.draft.response_tests.push(ResponseAssertion {
                id: uuid::Uuid::new_v4().to_string(),
                name: String::new(),
                enabled: true,
                source: AssertionSource::Status,
                operator: AssertionOperator::Equals,
                selector: None,
                expected: String::new(),
            });
            Task::none()
        }
        RequestComposerMessage::AssertionNameChanged { assertion_id, name } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(assertion) = active_tab
                .draft
                .response_tests
                .iter_mut()
                .find(|assertion| assertion.id == assertion_id)
            {
                assertion.name = name;
            }
            Task::none()
        }
        RequestComposerMessage::AssertionEnabledToggled { assertion_id } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(assertion) = active_tab
                .draft
                .response_tests
                .iter_mut()
                .find(|assertion| assertion.id == assertion_id)
            {
                assertion.enabled = !assertion.enabled;
            }
            Task::none()
        }
        RequestComposerMessage::AssertionSourceChanged {
            assertion_id,
            source,
        } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(assertion) = active_tab
                .draft
                .response_tests
                .iter_mut()
                .find(|assertion| assertion.id == assertion_id)
            {
                assertion.source = source;
            }
            Task::none()
        }
        RequestComposerMessage::AssertionOperatorChanged {
            assertion_id,
            operator,
        } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(assertion) = active_tab
                .draft
                .response_tests
                .iter_mut()
                .find(|assertion| assertion.id == assertion_id)
            {
                assertion.operator = operator;
            }
            Task::none()
        }
        RequestComposerMessage::AssertionSelectorChanged {
            assertion_id,
            selector,
        } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(assertion) = active_tab
                .draft
                .response_tests
                .iter_mut()
                .find(|assertion| assertion.id == assertion_id)
            {
                assertion.selector = if selector.trim().is_empty() {
                    None
                } else {
                    Some(selector)
                };
            }
            Task::none()
        }
        RequestComposerMessage::AssertionExpectedChanged {
            assertion_id,
            expected,
        } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(assertion) = active_tab
                .draft
                .response_tests
                .iter_mut()
                .find(|assertion| assertion.id == assertion_id)
            {
                assertion.expected = expected;
            }
            Task::none()
        }
        RequestComposerMessage::AssertionRemoved { assertion_id } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab
                .draft
                .response_tests
                .retain(|assertion| assertion.id != assertion_id);
            Task::none()
        }
        // Tab Body: `pick_list` de modo (None/Json/Text/FormData) sobre
        // `draft.body.mode`. Al entrar a Json o Text, se repuebla el
        // `Text_Editor_Component` con el `draft.body.value` vigente, para
        // que refleje contenido ya existente (p. ej. importado por cURL o
        // restaurado de sesión) en lugar de mostrarlo vacío.
        RequestComposerMessage::BodyModeChanged(mode) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.draft.body.mode = mode;
            if matches!(mode, BodyMode::Json | BodyMode::Text) {
                active_tab.body_editor = TextEditorState::new(&active_tab.draft.body.value);
            }
            Task::none()
        }
        // Aplica la acción de edición al `Content` del editor y refleja el
        // texto resultante en `draft.body.value` (única fuente persistida;
        // `Text_Editor_Component` es puramente de presentación/edición).
        RequestComposerMessage::BodyTextAction(action) => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.body_editor.update(action);
            active_tab.draft.body.value = active_tab.body_editor.content.text();
            Task::none()
        }
        RequestComposerMessage::FormDataRowAdded => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab.draft.body.form_data.push(FormDataRow {
                id: uuid::Uuid::new_v4().to_string(),
                key: String::new(),
                value: String::new(),
                enabled: true,
                kind: FormDataFieldKind::Text,
                file_name: None,
            });
            Task::none()
        }
        RequestComposerMessage::FormDataRowKeyChanged { row_id, key } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(row) = active_tab
                .draft
                .body
                .form_data
                .iter_mut()
                .find(|row| row.id == row_id)
            {
                row.key = key;
            }
            Task::none()
        }
        RequestComposerMessage::FormDataRowValueChanged { row_id, value } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(row) = active_tab
                .draft
                .body
                .form_data
                .iter_mut()
                .find(|row| row.id == row_id)
            {
                row.value = value;
            }
            Task::none()
        }
        RequestComposerMessage::FormDataRowEnabledToggled { row_id } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(row) = active_tab
                .draft
                .body
                .form_data
                .iter_mut()
                .find(|row| row.id == row_id)
            {
                row.enabled = !row.enabled;
            }
            Task::none()
        }
        RequestComposerMessage::FormDataRowKindChanged { row_id, kind } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            if let Some(row) = active_tab
                .draft
                .body
                .form_data
                .iter_mut()
                .find(|row| row.id == row_id)
            {
                row.kind = kind;
                // Al cambiar de tipo se descarta el nombre de archivo
                // previamente asociado (solo aplica a campos File).
                row.file_name = None;
            }
            Task::none()
        }
        RequestComposerMessage::FormDataRowRemoved { row_id } => {
            let Some(active_tab) = state.tabs.get_mut(active_index) else {
                return Task::none();
            };
            active_tab
                .draft
                .body
                .form_data
                .retain(|row| row.id != row_id);
            Task::none()
        }
        RequestComposerMessage::UrlPasted(_)
        | RequestComposerMessage::SendCompleted { .. }
        | RequestComposerMessage::PreviewLoaded { .. }
        | RequestComposerMessage::TabClosed { .. }
        | RequestComposerMessage::TabSelected { .. }
        | RequestComposerMessage::ClosedTabReopened
        | RequestComposerMessage::UnsavedChangesSaveRequested
        | RequestComposerMessage::UnsavedChangesSaveCompleted(_)
        | RequestComposerMessage::UnsavedChangesDiscardRequested
        | RequestComposerMessage::UnsavedChangesCancelRequested
        | RequestComposerMessage::SaveRequested
        | RequestComposerMessage::SaveNameChanged(_)
        | RequestComposerMessage::SaveCollectionChanged(_)
        | RequestComposerMessage::SaveFolderChanged(_)
        | RequestComposerMessage::SaveConfirmed
        | RequestComposerMessage::SaveCancelled
        | RequestComposerMessage::SaveCompleted(_) => {
            unreachable!("manejado arriba")
        }
    }
}

/// Devuelve la lista de `KeyValueRow` del `draft` correspondiente al
/// `target` del editor key/value (Tarea 5.6, Requisito 3.8): `draft.query`
/// para la tab Params, `draft.headers` para la tab Headers. Centraliza el
/// `match` sobre `target` para que los 5 handlers de mensajes del editor no
/// lo dupliquen.
fn key_value_rows_mut(draft: &mut RequestDraft, target: KeyValueTarget) -> &mut Vec<KeyValueRow> {
    match target {
        KeyValueTarget::Query => &mut draft.query,
        KeyValueTarget::Headers => &mut draft.headers,
    }
}

/// Implementa la ejecución de Send (Tarea 3.18, Requisito 2.17): resuelve el
/// draft de la tab activa (interpolación de environment/secrets + auth) y lo
/// ejecuta vía `request_executor.execute`, todo mediante llamadas directas a
/// `midway-core` (sin comandos Tauri). El resultado se reporta de forma
/// asíncrona como `SendCompleted`, identificando la tab por id para seguir
/// siendo correcto aunque el usuario cambie de tab mientras la request está
/// en curso.
fn handle_send_pressed(state: &mut Midway, active_index: usize) -> Task<Message> {
    let Some(active_tab) = state.tabs.get_mut(active_index) else {
        return Task::none();
    };

    let tab_id = active_tab.id.clone();
    let draft = active_tab.draft.clone();
    let execution_id = uuid::Uuid::new_v4().to_string();

    active_tab.sending = true;
    active_tab.execution_id = Some(execution_id.clone());
    active_tab.send_error = None;

    let environment = draft
        .environment_id
        .as_ref()
        .and_then(|environment_id| {
            state
                .workspace
                .environments
                .iter()
                .find(|environment| &environment.id == environment_id)
        })
        .cloned();

    let app_state = Arc::clone(&state.app_state);

    Task::perform(
        execute_send(app_state, draft, environment, execution_id),
        move |result| {
            Message::RequestComposer(RequestComposerMessage::SendCompleted {
                tab_id: tab_id.clone(),
                result,
            })
        },
    )
}

/// Cuerpo async de la ejecución de Send: carga los secrets necesarios,
/// resuelve el draft (interpolación + auth) y ejecuta la request HTTP
/// resultante, además de evaluar las assertions configuradas. Se ejecuta en
/// el runtime de Tokio que `iced` ya deja disponible (feature `tokio`).
///
/// Reproduce la lógica de `execute_draft_with_environment` /
/// `load_secrets_for_request` de la variante Tauri (`src-tauri/src/commands/mod.rs`),
/// pero sin ningún tipo de Tauri: los errores se convierten a `String` para
/// mantener el mensaje simple, dado que todavía no existe un banner de error
/// dedicado para fallos de Send en la UI.
async fn execute_send(
    app_state: Arc<AppState>,
    draft: RequestDraft,
    environment: Option<EnvironmentRecord>,
    execution_id: String,
) -> Result<ResponseOutcome, String> {
    let environment_name = environment
        .as_ref()
        .map(|environment| environment.name.clone());
    let environment_rows = environment
        .as_ref()
        .map(|environment| environment.variables.as_slice())
        .unwrap_or(&[]);

    let aliases = collect_secret_aliases(&draft, environment_rows);
    let mut secrets = BTreeMap::new();
    for alias in aliases {
        let value = app_state
            .secret_executor
            .get(alias.clone())
            .await
            .map_err(|error| error.to_string())?;
        if let Some(value) = value {
            secrets.insert(alias, value);
        }
    }

    let resolution = resolve_request(
        &draft,
        environment_name.clone(),
        environment_rows,
        &secrets,
        SecretRenderMode::Resolve,
    )
    .map_err(|error| error.to_string())?;

    let resolved_url = resolution.request.url.clone();

    let response = app_state
        .request_executor
        .execute(execution_id, resolution.request)
        .await;

    match response {
        Ok(response) => {
            let assertions = evaluate_response_assertions(&response, &draft.response_tests);

            let _ = app_state
                .repository
                .append_history(
                    &draft,
                    resolution.environment_name.or(environment_name),
                    resolved_url,
                    Some(response.clone()),
                    None,
                )
                .await;

            Ok(ResponseOutcome {
                response,
                assertions,
            })
        }
        Err(error) => {
            let message = error.to_string();

            let _ = app_state
                .repository
                .append_history(
                    &draft,
                    environment_name,
                    resolved_url,
                    None,
                    Some(message.clone()),
                )
                .await;

            Err(message)
        }
    }
}

/// Aplica el resultado de `SendCompleted` a la tab correspondiente,
/// identificada por `tab_id` (Tarea 3.18): puede no ser la tab activa si el
/// usuario cambió de tab mientras la request estaba en curso.
fn handle_send_completed(
    state: &mut Midway,
    tab_id: String,
    result: Result<ResponseOutcome, String>,
) {
    let Some(position) = state.tabs.iter().position(|tab| tab.id == tab_id) else {
        return;
    };

    let tab = &mut state.tabs[position];
    tab.sending = false;
    tab.execution_id = None;

    match result {
        Ok(outcome) => {
            tab.response = Some(outcome);
            tab.send_error = None;
            // Cada tab retenía su body completo indefinidamente, así que el
            // consumo crecía de forma lineal con la cantidad de tabs con
            // respuesta. Al llegar una respuesta nueva se liberan los bodies
            // de las demás tabs, conservando status/tiempo/tamaño.
            evict_inactive_response_bodies(&mut state.tabs, position);
        }
        Err(message) => {
            tab.send_error = Some(message);
        }
    }
}

/// Libera el `body_text` de las respuestas de todas las tabs excepto la
/// indicada por `keep_index`.
///
/// La metadata de la respuesta (status, tiempo, tamaño, headers, assertions)
/// se conserva, así que la tab sigue mostrando su resumen; solo se descarta el
/// payload, que es lo que ocupa memoria. Se marca con `body_evicted` para que
/// el `Response_Inspector` avise que el body ya no está disponible.
///
/// Toma `&mut [RequestTabState]` en lugar de `&mut Midway` para poder
/// ejercitarse en tests sin construir el estado completo de la aplicación.
fn evict_inactive_response_bodies(tabs: &mut [RequestTabState], keep_index: usize) {
    for (index, tab) in tabs.iter_mut().enumerate() {
        if index == keep_index {
            continue;
        }

        let Some(outcome) = tab.response.as_mut() else {
            continue;
        };

        if outcome.response.body_text.is_empty() {
            continue;
        }

        if outcome.response.total_size_bytes.is_none() {
            outcome.response.total_size_bytes = Some(outcome.response.size_bytes);
        }

        outcome.response.body_text = String::new();
        outcome.response.body_evicted = true;
    }
}

/// Implementa el toggle del preview drawer (Tarea 3.20, Requisito 2.18): si
/// el preview de la tab activa ya está abierto (`tab.preview` es `Some`),
/// presionar settings lo cierra (lo pone en `None`) sin recalcular nada. Si
/// está cerrado, dispara el cómputo asíncrono del preview (interpolación
/// con `SecretRenderMode::Redact` + `domain::preview::make_preview`) sobre
/// el draft vigente de la tab activa, cuyo resultado llega como
/// `PreviewLoaded` identificado por `tab_id` (el usuario puede cambiar de
/// tab mientras el cómputo está en curso).
fn handle_settings_pressed(state: &mut Midway, active_index: usize) -> Task<Message> {
    let Some(active_tab) = state.tabs.get_mut(active_index) else {
        return Task::none();
    };

    if active_tab.preview.is_some() {
        active_tab.preview = None;
        active_tab.preview_error = None;
        return Task::none();
    }

    let tab_id = active_tab.id.clone();
    let draft = active_tab.draft.clone();
    active_tab.preview_error = None;

    let environment = draft
        .environment_id
        .as_ref()
        .and_then(|environment_id| {
            state
                .workspace
                .environments
                .iter()
                .find(|environment| &environment.id == environment_id)
        })
        .cloned();

    let app_state = Arc::clone(&state.app_state);

    Task::perform(
        compute_preview(app_state, draft, environment),
        move |result| {
            Message::RequestComposer(RequestComposerMessage::PreviewLoaded {
                tab_id: tab_id.clone(),
                result,
            })
        },
    )
}

/// Cuerpo async del cómputo de preview (Tarea 3.20, Requisito 2.18): carga
/// los secrets necesarios, resuelve el draft (interpolación + auth) con
/// `SecretRenderMode::Redact` (a diferencia de `execute_send`, que usa
/// `Resolve`; el preview nunca debe mostrar secrets en texto plano) y
/// construye el `RequestPreview` resultante (método, URL final, headers,
/// body y comando cURL equivalente) vía `domain::preview::make_preview`,
/// sin ejecutar ninguna request HTTP real.
///
/// Reproduce el comando Tauri `preview_request` (`src-tauri/src/commands/mod.rs`),
/// pero sin ningún tipo de Tauri: los errores se convierten a `String` por
/// la misma razón que en `execute_send`.
async fn compute_preview(
    app_state: Arc<AppState>,
    draft: RequestDraft,
    environment: Option<EnvironmentRecord>,
) -> Result<RequestPreview, String> {
    let environment_name = environment
        .as_ref()
        .map(|environment| environment.name.clone());
    let environment_rows = environment
        .as_ref()
        .map(|environment| environment.variables.as_slice())
        .unwrap_or(&[]);

    let aliases = collect_secret_aliases(&draft, environment_rows);
    let mut secrets = BTreeMap::new();
    for alias in aliases {
        let value = app_state
            .secret_executor
            .get(alias.clone())
            .await
            .map_err(|error| error.to_string())?;
        if let Some(value) = value {
            secrets.insert(alias, value);
        }
    }

    let resolution = resolve_request(
        &draft,
        environment_name,
        environment_rows,
        &secrets,
        SecretRenderMode::Redact,
    )
    .map_err(|error| error.to_string())?;

    Ok(make_preview(resolution))
}

/// Aplica el resultado de `PreviewLoaded` a la tab correspondiente,
/// identificada por `tab_id` (Tarea 3.20): puede no ser la tab activa si el
/// usuario cambió de tab mientras el preview se estaba calculando.
fn handle_preview_loaded(
    state: &mut Midway,
    tab_id: String,
    result: Result<RequestPreview, String>,
) {
    let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
        return;
    };

    match result {
        Ok(preview) => {
            tab.preview = Some(preview);
            tab.preview_error = None;
        }
        Err(message) => {
            tab.preview = None;
            tab.preview_error = Some(message);
        }
    }
}

/// Implementa el enrutamiento de pegado de cURL (Tarea 3.5, Criterios
/// 2.7/2.8/2.9/2.19) para el texto pegado en la barra de URL.
fn handle_url_pasted(state: &mut Midway, pasted_text: String) {
    let Some(active_index) = state.active_tab else {
        return;
    };
    let Some(active_tab) = state.tabs.get(active_index) else {
        return;
    };

    // Criterio 2.9: si no parece un comando cURL, se inserta como texto
    // plano en la URL, sin invocar ninguna extracción.
    if !curl::looks_like_curl_command(&pasted_text) {
        let active_tab = &mut state.tabs[active_index];
        active_tab.draft.url = pasted_text;
        active_tab.curl_paste_error = None;
        return;
    }

    let active_tab_is_empty = is_draft_empty(&active_tab.draft);

    match curl::parse_curl_command_to_draft(&pasted_text) {
        // Criterio 2.19: parseo fallido -> mostrar error sin modificar el
        // contenido existente de la tab activa.
        Err(message) => {
            state.tabs[active_index].curl_paste_error = Some(message);
        }
        Ok((new_draft, _warnings)) => {
            if active_tab_is_empty {
                // Criterio 2.7: tab activa vacía -> sobrescribir sus campos.
                let active_tab = &mut state.tabs[active_index];
                active_tab.body_editor = TextEditorState::new(&new_draft.body.value);
                active_tab.draft = new_draft;
                active_tab.curl_paste_error = None;
            } else {
                // Criterio 2.8: tab activa con datos -> crear una nueva tab,
                // dejando la tab activa original completamente sin modificar.
                state.tabs.push(RequestTabState::from_draft(new_draft));
                state.active_tab = Some(state.tabs.len() - 1);
            }
        }
    }
}

/// Punto de entrada compartido para "cerrar una tab" (Tarea 11.10,
/// Requisito 6.6), usado tanto por `RequestComposerMessage::TabClosed`
/// (botón de cerrar de la tab) como por `KeyboardMessage::CloseActiveTabShortcut`
/// (Ctrl+W): si la tab identificada por `tab_id` tiene cambios sin guardar
/// (`tab_is_dirty`), muestra el aviso de unsaved changes en lugar de
/// cerrarla de inmediato; si no, la cierra directamente vía
/// `handle_tab_closed`. Centralizar esta decisión en un único lugar evita
/// que exista un segundo camino (el shortcut de teclado) que se salte el
/// aviso.
fn close_tab_or_prompt_unsaved_changes(state: &mut Midway, tab_id: String) {
    let is_dirty = state
        .tabs
        .iter()
        .find(|tab| tab.id == tab_id)
        .is_some_and(tab_is_dirty);
    if is_dirty {
        state.unsaved_changes_prompt = Some(UnsavedChangesPromptState {
            tab_id,
            saving: false,
            error: None,
        });
        return;
    }
    handle_tab_closed(state, &tab_id);
}

/// Implementa el cierre de una tab de request identificada por `tab_id`
/// (Tarea 11.8, Requisito 6.5): la quita de `state.tabs`, la convierte en
/// `TabSnapshot` y la empuja al frente de `state.closed_tabs` (orden
/// más-reciente-primero), acotando el stack a
/// [`crate::session::CLOSED_TABS_LIMIT`] entradas descartando la más
/// antigua (el fondo del `VecDeque`) si se supera el límite.
///
/// Política de selección de la tab activa tras el cierre (no hay una
/// convención previa en el código para esto, ya que ninguna otra fuente de
/// mutación elimina tabs): si la tab cerrada no era la activa, el índice de
/// la tab activa se ajusta para seguir apuntando a la misma tab lógica
/// (decrementa en 1 si la tab cerrada estaba antes que ella en el
/// `Vec`); si la tab cerrada SÍ era la activa, se selecciona la tab que
/// quedó en ese mismo índice (es decir, la que antes era la siguiente a la
/// derecha), o la última tab restante si se cerró la última, o `None` si
/// no queda ninguna tab abierta.
///
/// Si `tab_id` no corresponde a ninguna tab abierta, esta función no hace
/// nada (no hay tab que cerrar).
fn handle_tab_closed(state: &mut Midway, tab_id: &str) {
    let Some(closed_index) = state.tabs.iter().position(|tab| tab.id == tab_id) else {
        return;
    };

    let closed_tab = state.tabs.remove(closed_index);

    state.closed_tabs.push_front(closed_tab.to_snapshot());
    while state.closed_tabs.len() > crate::session::CLOSED_TABS_LIMIT {
        state.closed_tabs.pop_back();
    }

    state.active_tab = match state.active_tab {
        None => None,
        Some(active_index) if active_index < closed_index => Some(active_index),
        Some(active_index) if active_index == closed_index => {
            if state.tabs.is_empty() {
                None
            } else {
                Some(closed_index.min(state.tabs.len() - 1))
            }
        }
        Some(active_index) => Some(active_index - 1),
    };

    state.session.dirty = true;
}

/// Implementa la reapertura de la tab cerrada más recientemente (Tarea
/// 11.8, Requisito 6.5): extrae (`pop_front`) la entrada del frente de
/// `state.closed_tabs` (orden LIFO, más-reciente-primero), la reconstruye
/// como `RequestTabState` (preservando su `id` y su `active_request_tab`
/// originales vía `RequestTabState::from_snapshot`) y la agrega al final de
/// `state.tabs`, activándola. Si `state.closed_tabs` está vacío, no hace
/// nada.
fn handle_closed_tab_reopened(state: &mut Midway) {
    let Some(snapshot) = state.closed_tabs.pop_front() else {
        return;
    };

    state.tabs.push(RequestTabState::from_snapshot(snapshot));
    state.active_tab = Some(state.tabs.len() - 1);
    state.session.dirty = true;
}

/// Devuelve la colección y carpeta actuales de un request persistido.
fn saved_request_location(
    workspace: &WorkspaceSnapshot,
    request_id: Option<&str>,
) -> Option<(String, Option<String>)> {
    let request_id = request_id?;
    workspace.collections.iter().find_map(|collection| {
        collection
            .requests
            .iter()
            .find(|request| request.id == request_id)
            .map(|request| (collection.collection.id.clone(), request.folder_id.clone()))
    })
}

/// Abre el diálogo de guardado con la ubicación actual o la colección activa.
fn open_save_request_prompt(state: &mut Midway) {
    let Some(active_index) = state.active_tab else {
        return;
    };
    let Some(tab) = state.tabs.get(active_index) else {
        return;
    };

    let existing_location = saved_request_location(&state.workspace, tab.draft.id.as_deref());
    let collection_id = existing_location
        .as_ref()
        .map(|(collection_id, _)| collection_id.clone())
        .or_else(|| {
            state
                .active_collection_id
                .as_ref()
                .filter(|active_id| {
                    state
                        .workspace
                        .collections
                        .iter()
                        .any(|collection| collection.collection.id == **active_id)
                })
                .cloned()
        })
        .or_else(|| {
            state
                .workspace
                .collections
                .first()
                .map(|collection| collection.collection.id.clone())
        });
    let folder_id = existing_location.and_then(|(_, folder_id)| folder_id);

    state.save_request_prompt = Some(SaveRequestPromptState {
        tab_id: tab.id.clone(),
        name_input: tab.draft.name.clone(),
        collection_id,
        folder_id,
        saving: false,
        error: None,
    });
}

fn handle_save_request_confirmed(state: &mut Midway) -> Task<Message> {
    let Some(prompt) = state.save_request_prompt.as_ref() else {
        return Task::none();
    };
    if prompt.saving {
        return Task::none();
    }

    let tab_id = prompt.tab_id.clone();
    let name = prompt.name_input.trim().to_string();
    let collection_id = prompt.collection_id.clone();
    let folder_id = prompt.folder_id.clone();

    if name.is_empty() {
        if let Some(prompt) = state.save_request_prompt.as_mut() {
            prompt.error = Some("El request necesita un nombre.".to_string());
        }
        return Task::none();
    }

    let Some(collection_id) = collection_id else {
        if let Some(prompt) = state.save_request_prompt.as_mut() {
            prompt.error = Some("Seleccioná una colección de destino.".to_string());
        }
        return Task::none();
    };

    let Some(collection) = state
        .workspace
        .collections
        .iter()
        .find(|collection| collection.collection.id == collection_id)
    else {
        if let Some(prompt) = state.save_request_prompt.as_mut() {
            prompt.error = Some("La colección seleccionada ya no existe.".to_string());
        }
        return Task::none();
    };

    if folder_id.as_ref().is_some_and(|folder_id| {
        !collection
            .folders
            .iter()
            .any(|folder| folder.id == *folder_id)
    }) {
        if let Some(prompt) = state.save_request_prompt.as_mut() {
            prompt.error = Some("La carpeta seleccionada ya no existe.".to_string());
        }
        return Task::none();
    }

    let Some(tab) = state.tabs.iter().find(|tab| tab.id == tab_id) else {
        state.save_request_prompt = None;
        return Task::none();
    };

    let mut draft = tab.draft.clone();
    draft.name = name;

    if let Some(prompt) = state.save_request_prompt.as_mut() {
        prompt.saving = true;
        prompt.error = None;
    }

    let app_state = Arc::clone(&state.app_state);
    Task::perform(
        persist_request_from_composer(app_state, tab_id, draft, collection_id, folder_id),
        |result| Message::RequestComposer(RequestComposerMessage::SaveCompleted(result)),
    )
}

async fn persist_request_from_composer(
    app_state: Arc<AppState>,
    tab_id: String,
    draft: RequestDraft,
    collection_id: String,
    folder_id: Option<String>,
) -> Result<RequestSaveOutcome, String> {
    let saved_record = app_state
        .repository
        .save_request(SaveRequestInput {
            request_id: draft.id.clone(),
            collection_id: collection_id.clone(),
            folder_id,
            draft,
        })
        .await
        .map_err(|error| error.to_string())?;

    let workspace = app_state
        .repository
        .export_full_snapshot()
        .await
        .map_err(|error| error.to_string())?;

    Ok(RequestSaveOutcome {
        tab_id,
        saved_draft: saved_record.draft,
        workspace,
        collection_id,
    })
}

fn handle_save_request_completed(state: &mut Midway, result: Result<RequestSaveOutcome, String>) {
    match result {
        Ok(outcome) => {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == outcome.tab_id) {
                tab.draft = outcome.saved_draft.clone();
                tab.saved_draft = Some(outcome.saved_draft);
            }

            let collection_changed =
                state.active_collection_id.as_deref() != Some(outcome.collection_id.as_str());
            state.workspace = outcome.workspace;
            state.active_collection_id = Some(outcome.collection_id);
            if collection_changed {
                state.tree = TreeViewState::default();
            }
            state.save_request_prompt = None;
            state.session.dirty = true;
        }
        Err(error) => {
            if let Some(prompt) = state.save_request_prompt.as_mut() {
                prompt.saving = false;
                prompt.error = Some(error);
            }
        }
    }
}

/// Nombre de la collection usada como destino por defecto al Guardar
/// (Tarea 11.10, Requisito 6.6) una tab que NUNCA se asoció a un
/// `SavedRequestRecord` (`saved_draft: None`).
///
/// Alcance deliberadamente acotado (ver doc de `persist_tab_draft`):
/// construir un selector de collection destino es una pieza de UX
/// considerablemente más grande que el aviso de unsaved changes en sí, así
/// que en su lugar se reutiliza (o se crea, si todavía no existe) una
/// única collection fija de "cajón de sastre" para este caso, en vez de
/// preguntarle al usuario dónde guardar cada vez.
const UNSAVED_CHANGES_DEFAULT_COLLECTION_NAME: &str = "Sin categoría";

/// Implementa "Guardar" del aviso de unsaved changes (Tarea 11.10,
/// Requisito 6.6): dispara la persistencia asíncrona del draft de la tab
/// pendiente de cierre (`state.unsaved_changes_prompt`) vía
/// `persist_tab_draft`. No-op si no hay ningún aviso abierto o si la tab
/// referenciada ya no existe (pudo haberse cerrado por otra vía mientras
/// el aviso estaba abierto, aunque no hay ningún camino actual en el
/// código que permita eso).
fn handle_unsaved_changes_save_requested(state: &mut Midway) -> Task<Message> {
    let Some(prompt) = state.unsaved_changes_prompt.as_mut() else {
        return Task::none();
    };
    let tab_id = prompt.tab_id.clone();
    prompt.saving = true;
    prompt.error = None;

    let Some(tab) = state.tabs.iter().find(|tab| tab.id == tab_id) else {
        return Task::none();
    };

    let draft = tab.draft.clone();
    // Si la tab ya estuvo asociada a un `SavedRequestRecord` (Tarea 11.2,
    // `tab.saved_draft.is_some()`), busca la collection que actualmente
    // contiene ese id de request en el snapshot en memoria, para
    // actualizar el MISMO registro en la MISMA collection en vez de
    // moverlo a la collection de fallback.
    let existing_collection_id = tab
        .saved_draft
        .is_some()
        .then(|| {
            state.workspace.collections.iter().find_map(|collection| {
                collection
                    .requests
                    .iter()
                    .find(|request| Some(&request.id) == draft.id.as_ref())
                    .map(|_| collection.collection.id.clone())
            })
        })
        .flatten();

    let app_state = Arc::clone(&state.app_state);

    Task::perform(
        persist_tab_draft(app_state, tab_id.clone(), draft, existing_collection_id),
        move |result| {
            Message::RequestComposer(RequestComposerMessage::UnsavedChangesSaveCompleted(result))
        },
    )
}

/// Cuerpo async de "Guardar" (Tarea 11.10, Requisito 6.6).
///
/// Alcance (documentado aquí en vez de en el diseño porque es una decisión
/// de implementación de esta tarea, no un requisito): el CORE deliverable
/// de la Tarea 11.10 es el modal y sus tres caminos de mensaje, no una UX
/// completa de "guardar como" con selección de collection. Por eso:
///
/// - Si `existing_collection_id` es `Some` (la tab ya estaba asociada a un
///   `SavedRequestRecord` dentro de esa collection, resuelto por el
///   llamador antes de esta función), se actualiza ESE mismo registro en
///   esa misma collection (`SaveRequestInput { request_id: draft.id, ... }`),
///   el camino más común y mejor definido.
/// - Si es `None` (tab que nunca se guardó, sin ninguna collection
///   asociada), se usa como fallback la collection fija
///   [`UNSAVED_CHANGES_DEFAULT_COLLECTION_NAME`], creándola primero si
///   todavía no existe en el workspace. Este fallback cubre el caso sin
///   construir un selector de collection dedicado, que queda fuera de
///   alcance de esta tarea.
async fn persist_tab_draft(
    app_state: Arc<AppState>,
    tab_id: String,
    mut draft: RequestDraft,
    existing_collection_id: Option<String>,
) -> Result<SavedRequestSaveOutcome, String> {
    let collection_id = match existing_collection_id {
        Some(collection_id) => collection_id,
        None => {
            let snapshot = app_state
                .repository
                .export_full_snapshot()
                .await
                .map_err(|error| error.to_string())?;
            let existing = snapshot
                .collections
                .iter()
                .find(|collection| {
                    collection.collection.name == UNSAVED_CHANGES_DEFAULT_COLLECTION_NAME
                })
                .map(|collection| collection.collection.id.clone());

            match existing {
                Some(collection_id) => collection_id,
                None => {
                    let created = app_state
                        .repository
                        .create_collection(UNSAVED_CHANGES_DEFAULT_COLLECTION_NAME.to_string())
                        .await
                        .map_err(|error| error.to_string())?;
                    created.id
                }
            }
        }
    };

    if draft.name.trim().is_empty() {
        draft.name = "Nueva petición".to_string();
    }

    let saved_record = app_state
        .repository
        .save_request(SaveRequestInput {
            request_id: draft.id.clone(),
            collection_id,
            folder_id: None,
            draft,
        })
        .await
        .map_err(|error| error.to_string())?;

    Ok(SavedRequestSaveOutcome {
        tab_id,
        saved_draft: saved_record.draft,
    })
}

/// Aplica el resultado de `UnsavedChangesSaveCompleted` (Tarea 11.10,
/// Requisito 6.6): en éxito, fija `saved_draft` en el draft efectivamente
/// persistido y cierra la tab (misma lógica que `handle_tab_closed`); en
/// error, deja el aviso abierto con el mensaje de error visible, sin cerrar
/// la tab ni mutar `draft`/`saved_draft`.
fn handle_unsaved_changes_save_completed(
    state: &mut Midway,
    result: Result<SavedRequestSaveOutcome, String>,
) {
    match result {
        Ok(outcome) => {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == outcome.tab_id) {
                tab.draft = outcome.saved_draft.clone();
                tab.saved_draft = Some(outcome.saved_draft);
            }
            state.unsaved_changes_prompt = None;
            handle_tab_closed(state, &outcome.tab_id);
        }
        Err(error) => {
            if let Some(prompt) = state.unsaved_changes_prompt.as_mut() {
                prompt.saving = false;
                prompt.error = Some(error);
            }
        }
    }
}

/// Implementa "Descartar" del aviso de unsaved changes (Tarea 11.10,
/// Requisito 6.6): cierra la tab pendiente de cierre de inmediato (misma
/// lógica que `handle_tab_closed`), sin invocar ningún flujo de
/// persistencia. `saved_draft` no se toca en absoluto (se descarta el
/// `draft` en memoria junto con el resto de la tab), lo cual garantiza por
/// construcción que el último draft persistido no se altera.
fn handle_unsaved_changes_discard_requested(state: &mut Midway) {
    let Some(prompt) = state.unsaved_changes_prompt.take() else {
        return;
    };
    handle_tab_closed(state, &prompt.tab_id);
}

/// Actualiza el estado en respuesta a un `KeyboardMessage` (Tarea 11.12,
/// Requisito 6.8). Ver la documentación de cada variante de
/// `KeyboardMessage` para el detalle de qué hace cada shortcut y por qué
/// se resuelve aquí (en `update`) en lugar de en la
/// `iced::keyboard::listen()` subscription de `subscription()`.
fn update_keyboard(state: &mut Midway, message: KeyboardMessage) -> Task<Message> {
    match message {
        KeyboardMessage::SaveRequested => {
            open_save_request_prompt(state);
            Task::none()
        }
        KeyboardMessage::NewBlankTabRequested => {
            state.tabs.push(RequestTabState::blank());
            state.active_tab = Some(state.tabs.len() - 1);
            state.main_content_focus = MainContentFocus::RequestTab;
            state.top_bar_mode = TopBarMode::Debug;
            state.session.dirty = true;
            Task::none()
        }
        KeyboardMessage::CloseActiveTabShortcut => {
            let Some(active_index) = state.active_tab else {
                return Task::none();
            };
            let Some(active_tab) = state.tabs.get(active_index) else {
                return Task::none();
            };
            let tab_id = active_tab.id.clone();
            close_tab_or_prompt_unsaved_changes(state, tab_id);
            Task::none()
        }
        KeyboardMessage::GoToTabShortcut { index } => {
            if index < state.tabs.len() {
                state.active_tab = Some(index);
            }
            Task::none()
        }
        KeyboardMessage::EscapePressed => {
            if state.tree.request_drag.take().is_some() {
                return Task::none();
            }

            if state
                .workspace_crud_dialog
                .as_ref()
                .is_some_and(|dialog| !dialog.busy)
            {
                state.workspace_crud_dialog = None;
                return Task::none();
            }

            if state
                .save_request_prompt
                .as_ref()
                .is_some_and(|prompt| !prompt.saving)
            {
                state.save_request_prompt = None;
                return Task::none();
            }

            if state.create_collection_prompt.is_some() {
                state.create_collection_prompt = None;
                return Task::none();
            }

            // Precedencia documentada en `KeyboardMessage::EscapePressed`:
            // el Command Palette, si está abierto, se cierra primero.
            if state.palette.is_open {
                return update_palette(state, PaletteMessage::Dismissed);
            }

            let active_preview_open = state
                .active_tab
                .and_then(|index| state.tabs.get(index))
                .is_some_and(|tab| tab.preview.is_some());

            if active_preview_open {
                let Some(active_index) = state.active_tab else {
                    return Task::none();
                };
                return handle_settings_pressed(state, active_index);
            }

            Task::none()
        }
    }
}

/// Actualiza el estado en respuesta a un `ResponseInspectorMessage` (Tarea
/// 3.9). Por ahora solo cubre el cambio de tab (Body/Headers/Tests); el
/// contenido de la respuesta se popula en la Tarea 3.18.
fn update_response_inspector(
    state: &mut Midway,
    message: ResponseInspectorMessage,
) -> Task<Message> {
    let Some(active_tab) = state.active_tab.and_then(|index| state.tabs.get_mut(index)) else {
        return Task::none();
    };

    match message {
        ResponseInspectorMessage::TabSelected(tab) => {
            active_tab.response_tab = tab;
            Task::none()
        }
    }
}

/// Máximo de environments por workspace (Requisito 4.2). Validado en
/// `midway-desktop` antes de invocar `save_environment`, dado que
/// `infra::sqlite_repository` no impone este límite.
const MAX_ENVIRONMENTS: usize = 100;

/// Máximo de caracteres del nombre de un environment (Requisito 4.2).
/// Validado en `midway-desktop` antes de invocar `save_environment`, por la
/// misma razón que `MAX_ENVIRONMENTS`.
const MAX_ENVIRONMENT_NAME_LEN: usize = 100;

/// Máximo de `HistoryEntry` a cargar desde `workspace_snapshot` para la
/// sección History del `Workspace_Panel` (Tarea 7.11, Requisito 4.6).
///
/// Constante propia de `midway-desktop`, independiente de
/// `src-tauri/src/commands/mod.rs::HISTORY_LIMIT` (que permanece en 50: la
/// variante Tauri no se modifica como parte de esta migración, ver
/// diseño).
const HISTORY_LIMIT: usize = 500;

/// Actualiza el estado en respuesta a un `WorkspaceMessage` (Tarea 7.1,
/// Requisito 4.1). Cubre el mecanismo de contenedor (cambio de sección
/// activa, colapsar/expandir el panel) y el CRUD de la sección Environments
/// (Tarea 7.2, Requisitos 4.2, 4.9, 4.10). El contenido de las demás
/// secciones se agrega en tareas posteriores.
fn update_workspace(state: &mut Midway, message: WorkspaceMessage) -> Task<Message> {
    match message {
        // Tarea 7.12 (Requisito 4.7): al seleccionar la sección Diagnostics,
        // se recarga `state.crash_log` desde `diagnostics::read_crash_records`
        // (lectura síncrona de un archivo local, sin necesidad de
        // `Task::perform`), reemplazando el contenido en memoria por el
        // estado persistido más reciente (más-reciente-primero).
        //
        // Tarea 7.11 (Requisito 4.6): al seleccionar la sección History por
        // primera vez, se dispara la carga asíncrona de `workspace_snapshot`
        // (única forma en que `state.workspace` se puebla desde la base de
        // datos: el esqueleto de la Tarea 3.1 lo inicializa vacío y solo se
        // muta en memoria vía CRUD de environments desde entonces). Cargas
        // posteriores de la sección History no repiten la llamada
        // (`history_loaded`), salvo que se dispare explícitamente de nuevo.
        WorkspaceMessage::SectionSelected(section) => {
            // Req 11.11: successful navigation sets active_section and
            // main_content_focus = WorkspaceSection.
            // Req 11.10: if the section is somehow unavailable, show error
            // and maintain the current view. In practice, section navigation
            // is synchronous and can't fail for the 5 defined sections, but
            // we guard against unexpected states.
            state.workspace_panel.navigation_error = None;
            state.main_content_focus = MainContentFocus::WorkspaceSection;
            state.workspace_panel.active_section = section;
            if section == WorkspacePanelSection::Diagnostics {
                state.crash_log = diagnostics::read_crash_records();
            }
            if section == WorkspacePanelSection::History && !state.workspace_panel.history_loaded {
                return handle_history_requested(state);
            }
            Task::none()
        }
        WorkspaceMessage::ToggleCollapsed => {
            state.workspace_panel.collapsed = !state.workspace_panel.collapsed;
            Task::none()
        }
        WorkspaceMessage::EnvironmentNameInputChanged(name) => {
            state.workspace_panel.environment_form.name_input = name;
            state.workspace_panel.environment_form.error = None;
            Task::none()
        }
        WorkspaceMessage::EnvironmentEditRequested(environment_id) => {
            if let Some(environment) = state
                .workspace
                .environments
                .iter()
                .find(|environment| environment.id == environment_id)
            {
                state.workspace_panel.environment_form = EnvironmentFormState {
                    editing_id: Some(environment.id.clone()),
                    name_input: environment.name.clone(),
                    error: None,
                };
            }
            Task::none()
        }
        WorkspaceMessage::EnvironmentEditCancelled => {
            state.workspace_panel.environment_form = EnvironmentFormState::default();
            Task::none()
        }
        WorkspaceMessage::EnvironmentCreateOrUpdateSubmitted => handle_environment_submitted(state),
        WorkspaceMessage::EnvironmentDeleteRequested(environment_id) => {
            handle_environment_delete_requested(state, environment_id)
        }
        WorkspaceMessage::EnvironmentSavedResult(result) => {
            handle_environment_saved_result(state, result);
            Task::none()
        }
        WorkspaceMessage::EnvironmentDeletedResult {
            environment_id,
            result,
        } => {
            handle_environment_deleted_result(state, environment_id, result);
            Task::none()
        }
        WorkspaceMessage::WorkspaceSnapshotLoaded(result) => {
            handle_workspace_snapshot_loaded(state, result);
            Task::none()
        }
        WorkspaceMessage::ExportFormatChanged(format) => {
            state.workspace_panel.export_form.export_format = format;
            state.workspace_panel.export_form.result_message = None;
            Task::none()
        }
        WorkspaceMessage::ExportPathInputChanged(path) => {
            state.workspace_panel.export_form.path_input = path;
            state.workspace_panel.export_form.result_message = None;
            Task::none()
        }
        WorkspaceMessage::ExportCollectionIdInputChanged(collection_id) => {
            state.workspace_panel.export_form.collection_id_input = collection_id;
            state.workspace_panel.export_form.result_message = None;
            Task::none()
        }
        WorkspaceMessage::ExportSubmitted => handle_export_submitted(state),
        WorkspaceMessage::ExportCompleted(result) => {
            handle_export_completed(state, result);
            Task::none()
        }
        WorkspaceMessage::ImportPayloadInputChanged(payload) => {
            state.workspace_panel.import_form.payload_input = payload;
            state.workspace_panel.import_form.result_message = None;
            Task::none()
        }
        WorkspaceMessage::ImportFormatChanged(format) => {
            state.workspace_panel.import_form.import_format = format;
            state.workspace_panel.import_form.result_message = None;
            Task::none()
        }
        WorkspaceMessage::ImportSubmitted => handle_import_submitted(state),
        WorkspaceMessage::ImportCompleted(result) => {
            handle_import_completed(state, result);
            Task::none()
        }
    }
}

/// Implementa la validación local del formulario de environments (Tarea
/// 7.2, Requisitos 4.2, 4.9) antes de invocar `save_environment`:
///
/// - El nombre (tras `trim`) no puede estar vacío.
/// - El nombre no puede exceder `MAX_ENVIRONMENT_NAME_LEN` caracteres.
/// - El nombre no puede duplicar el de otro environment ya existente
///   (comparación exacta, sensible a mayúsculas/minúsculas), excluyendo el
///   environment actualmente en edición (`editing_id`), de forma que
///   guardar sin cambiar el nombre no se rechace como "duplicado".
/// - Si se está creando un environment nuevo (`editing_id` es `None`), el
///   total de environments no puede exceder `MAX_ENVIRONMENTS` (editar uno
///   existente no cuenta contra este límite).
///
/// Devuelve el nombre ya "trimeado" cuando la validación es exitosa, o el
/// mensaje de error a mostrar en el formulario en caso contrario.
fn validate_environment_name(
    environments: &[EnvironmentRecord],
    editing_id: Option<&str>,
    raw_name: &str,
) -> Result<String, String> {
    let name = raw_name.trim().to_string();

    if name.is_empty() {
        return Err("El environment necesita un nombre.".to_string());
    }

    if name.chars().count() > MAX_ENVIRONMENT_NAME_LEN {
        return Err(format!(
            "El nombre del environment no puede superar los {MAX_ENVIRONMENT_NAME_LEN} caracteres."
        ));
    }

    let is_duplicate = environments
        .iter()
        .any(|environment| environment.name == name && Some(environment.id.as_str()) != editing_id);
    if is_duplicate {
        return Err(format!("Ya existe un environment llamado \"{name}\"."));
    }

    if editing_id.is_none() && environments.len() >= MAX_ENVIRONMENTS {
        return Err(format!(
            "El workspace ya tiene el máximo de {MAX_ENVIRONMENTS} environments."
        ));
    }

    Ok(name)
}

/// Maneja `EnvironmentCreateOrUpdateSubmitted` (Tarea 7.2): valida el
/// formulario localmente (Requisitos 4.2, 4.9) y, si la validación pasa,
/// dispara la llamada asíncrona a `save_environment`. Si la validación
/// falla, se fija `environment_form.error` y `state.workspace.environments`
/// permanece sin cambios (Requisito 4.9: "sin mutar el conjunto
/// existente"), sin llegar a invocar `save_environment`.
fn handle_environment_submitted(state: &mut Midway) -> Task<Message> {
    let editing_id = state.workspace_panel.environment_form.editing_id.clone();
    let raw_name = state.workspace_panel.environment_form.name_input.clone();

    let name = match validate_environment_name(
        &state.workspace.environments,
        editing_id.as_deref(),
        &raw_name,
    ) {
        Ok(name) => name,
        Err(message) => {
            state.workspace_panel.environment_form.error = Some(message);
            return Task::none();
        }
    };

    // Preserva las variables existentes al editar (el formulario de esta
    // tarea solo expone el campo de nombre; el editor de variables por fila
    // se agrega en una tarea posterior de UI si el diseño lo requiere).
    let variables = editing_id
        .as_deref()
        .and_then(|id| {
            state
                .workspace
                .environments
                .iter()
                .find(|environment| environment.id == id)
        })
        .map(|environment| environment.variables.clone())
        .unwrap_or_default();

    state.workspace_panel.environment_form.error = None;
    state.workspace_panel.environment_busy = true;

    let app_state = Arc::clone(&state.app_state);
    let input = SaveEnvironmentInput {
        environment_id: editing_id,
        name,
        variables,
    };

    Task::perform(save_environment(app_state, input), |result| {
        Message::Workspace(WorkspaceMessage::EnvironmentSavedResult(result))
    })
}

/// Cuerpo async de `save_environment` (Tarea 7.2): delega directamente en
/// `infra::sqlite_repository::SqliteRepository::save_environment`, ya que
/// la validación de duplicados y límites ocurre antes de llegar aquí
/// (`validate_environment_name`, en la capa de orquestación de
/// `midway-desktop`).
async fn save_environment(
    app_state: Arc<AppState>,
    input: SaveEnvironmentInput,
) -> Result<EnvironmentRecord, String> {
    app_state
        .repository
        .save_environment(input)
        .await
        .map_err(|error| error.to_string())
}

/// Aplica el resultado de `EnvironmentSavedResult` (Tarea 7.2): en caso de
/// éxito, reemplaza el environment editado (o agrega el nuevo) en
/// `state.workspace.environments` y limpia el formulario; en caso de
/// error, lo muestra en `environment_form.error` sin mutar la lista.
fn handle_environment_saved_result(state: &mut Midway, result: Result<EnvironmentRecord, String>) {
    state.workspace_panel.environment_busy = false;

    match result {
        Ok(saved) => {
            if let Some(existing) = state
                .workspace
                .environments
                .iter_mut()
                .find(|environment| environment.id == saved.id)
            {
                *existing = saved;
            } else {
                state.workspace.environments.push(saved);
            }
            state.workspace_panel.environment_form = EnvironmentFormState::default();
        }
        Err(message) => {
            state.workspace_panel.environment_form.error = Some(message);
        }
    }
}

/// Maneja `EnvironmentDeleteRequested` (Tarea 7.2): dispara la llamada
/// asíncrona a `delete_environment`, identificando el environment por
/// `environment_id` (no por el formulario en curso, que puede estar
/// editando un environment distinto).
fn handle_environment_delete_requested(
    state: &mut Midway,
    environment_id: String,
) -> Task<Message> {
    state.workspace_panel.environment_busy = true;

    let app_state = Arc::clone(&state.app_state);
    let environment_id_for_result = environment_id.clone();

    Task::perform(
        delete_environment(app_state, environment_id),
        move |result| {
            Message::Workspace(WorkspaceMessage::EnvironmentDeletedResult {
                environment_id: environment_id_for_result.clone(),
                result,
            })
        },
    )
}

/// Cuerpo async de `delete_environment` (Tarea 7.2): delega directamente en
/// `infra::sqlite_repository::SqliteRepository::delete_environment`.
async fn delete_environment(
    app_state: Arc<AppState>,
    environment_id: String,
) -> Result<(), String> {
    app_state
        .repository
        .delete_environment(environment_id)
        .await
        .map_err(|error| error.to_string())
}

/// Aplica el resultado de `EnvironmentDeletedResult` (Tarea 7.2, Requisito
/// 4.10): en caso de éxito, quita el environment de
/// `state.workspace.environments`, limpia el formulario si estaba editando
/// el environment eliminado, y limpia la selección de environment de
/// cualquier tab de request abierta que lo tuviera seleccionado
/// (`draft.environment_id`), dejando dicha tab sin environment activo. En
/// caso de error, lo muestra en `environment_form.error` sin mutar la
/// lista.
fn handle_environment_deleted_result(
    state: &mut Midway,
    environment_id: String,
    result: Result<(), String>,
) {
    state.workspace_panel.environment_busy = false;

    match result {
        Ok(()) => {
            state
                .workspace
                .environments
                .retain(|environment| environment.id != environment_id);

            if state.workspace_panel.environment_form.editing_id.as_deref()
                == Some(environment_id.as_str())
            {
                state.workspace_panel.environment_form = EnvironmentFormState::default();
            }

            // Requisito 4.10: al eliminar el environment activo, ninguna
            // tab que lo tuviera seleccionado debe quedar apuntando a un
            // environment inexistente; se limpia la selección a "sin
            // environment" (`None`) en cada tab afectada.
            for tab in state.tabs.iter_mut() {
                if tab.draft.environment_id.as_deref() == Some(environment_id.as_str()) {
                    tab.draft.environment_id = None;
                }
            }
        }
        Err(message) => {
            state.workspace_panel.environment_form.error = Some(message);
        }
    }
}

/// Maneja la solicitud de carga del snapshot completo del workspace (Tarea
/// 7.11, Requisito 4.6), disparada al seleccionar la sección History por
/// primera vez: fija `history_loading` y dispara la llamada asíncrona a
/// `workspace_snapshot(HISTORY_LIMIT)`.
fn handle_history_requested(state: &mut Midway) -> Task<Message> {
    state.workspace_panel.history_loading = true;
    state.workspace_panel.history_error = None;

    let app_state = Arc::clone(&state.app_state);

    Task::perform(load_workspace_snapshot(app_state), |result| {
        Message::Workspace(WorkspaceMessage::WorkspaceSnapshotLoaded(result))
    })
}

/// Carga inicial del workspace para que colecciones y requests estén
/// disponibles desde el arranque, sin visitar History primero.
pub(crate) fn initial_workspace_load(state: &Midway) -> Task<Message> {
    let app_state = Arc::clone(&state.app_state);
    Task::perform(load_workspace_snapshot(app_state), |result| {
        Message::Workspace(WorkspaceMessage::WorkspaceSnapshotLoaded(result))
    })
}

/// Cuerpo async de la carga del snapshot completo del workspace (Tarea
/// 7.11): delega directamente en
/// `infra::sqlite_repository::SqliteRepository::workspace_snapshot`, con
/// `HISTORY_LIMIT` (500) como límite de `HistoryEntry` a cargar.
async fn load_workspace_snapshot(app_state: Arc<AppState>) -> Result<WorkspaceSnapshot, String> {
    app_state
        .repository
        .workspace_snapshot(HISTORY_LIMIT)
        .await
        .map_err(|error| error.to_string())
}

/// Aplica el resultado de `WorkspaceSnapshotLoaded` (Tarea 7.11): en caso
/// de éxito, reemplaza `state.workspace` completo (collections/
/// environments/history/secrets) con el snapshot recién cargado y marca
/// `history_loaded` en `true` (evita relanzar la carga en selecciones
/// posteriores de la sección History); en caso de error, lo muestra en
/// `history_error` sin mutar `state.workspace`.
fn handle_workspace_snapshot_loaded(state: &mut Midway, result: Result<WorkspaceSnapshot, String>) {
    state.workspace_panel.history_loading = false;

    match result {
        Ok(snapshot) => {
            for tab in state.tabs.iter_mut() {
                let Some(request_id) = tab.draft.id.as_deref() else {
                    continue;
                };
                tab.saved_draft = snapshot
                    .collections
                    .iter()
                    .flat_map(|collection| collection.requests.iter())
                    .find(|request| request.id == request_id)
                    .map(|request| request.draft.clone());
            }
            state.workspace = snapshot;
            state.workspace_panel.history_loaded = true;

            // Resolve active collection after workspace loads (Requirements
            // 6.1-6.4): use the current active_collection_id (which may hold
            // the session-restored value) as the session_id hint.
            let session_id = state.active_collection_id.clone();
            state.active_collection_id =
                resolve_startup_collection(&state.workspace.collections, session_id.as_deref());

            // Req 6.8: fall back to Debug if active collection was removed.
            enforce_top_bar_mode_after_collection_change(state);
        }
        Err(message) => {
            state.workspace_panel.history_error = Some(message);
        }
    }
}

/// Maneja `ExportSubmitted` (Tarea 7.6, Requisito 4.3): valida el
/// formulario localmente (ruta no vacía; id de collection no vacío si el
/// formato seleccionado es Postman, que requiere elegir qué collection
/// exportar) y, si la validación pasa, dispara la orquestación asíncrona de
/// export. Si la validación falla, se fija `result_message` como error sin
/// llegar a tocar el sistema de archivos.
fn handle_export_submitted(state: &mut Midway) -> Task<Message> {
    let form = &state.workspace_panel.export_form;
    let path = form.path_input.trim().to_string();
    let format = form.export_format;
    let collection_id = form.collection_id_input.trim().to_string();

    if path.is_empty() {
        state.workspace_panel.export_form.result_message =
            Some("Necesitás indicar una ruta de archivo de destino.".to_string());
        state.workspace_panel.export_form.result_is_error = true;
        return Task::none();
    }

    if format == WorkspaceExportFormat::PostmanCollectionV21 && collection_id.is_empty() {
        state.workspace_panel.export_form.result_message = Some(
            "Para exportar Postman v2.1 necesitás indicar el id de la collection.".to_string(),
        );
        state.workspace_panel.export_form.result_is_error = true;
        return Task::none();
    }

    state.workspace_panel.export_form.busy = true;
    state.workspace_panel.export_form.result_message = None;

    let app_state = Arc::clone(&state.app_state);
    let collection_id_option = if collection_id.is_empty() {
        None
    } else {
        Some(collection_id)
    };

    Task::perform(
        export_workspace_data(app_state, path, format, collection_id_option),
        |result| Message::Workspace(WorkspaceMessage::ExportCompleted(result)),
    )
}

/// Cuerpo async de la orquestación de export de la sección Data (Tarea 7.6,
/// Requisito 4.3): port directo de `export_workspace_data` de
/// `src-tauri/src/commands/mod.rs`, reutilizando `make_native_bundle` y
/// `export_postman_collection` (`domain::interop`) sin reescritura. A
/// diferencia del comando Tauri, esta versión no soporta filtrar el export
/// nativo a una sola collection ni exponer los checkboxes de
/// `include_history`/`include_secret_metadata` (fuera de alcance de esta
/// tarea): siempre exporta el snapshot completo con `include_history =
/// true` e `include_secret_metadata = false`.
async fn export_workspace_data(
    app_state: Arc<AppState>,
    path: String,
    format: WorkspaceExportFormat,
    collection_id: Option<String>,
) -> Result<String, String> {
    let path = PathBuf::from(path);

    let payload = match format {
        WorkspaceExportFormat::NativeWorkspaceV1 => {
            let snapshot = app_state
                .repository
                .export_full_snapshot()
                .await
                .map_err(|error| error.to_string())?;
            let bundle = make_native_bundle(snapshot, true, false);
            serde_json::to_string_pretty(&bundle).map_err(|error| error.to_string())?
        }
        WorkspaceExportFormat::PostmanCollectionV21 => {
            let collection_id = collection_id.ok_or_else(|| {
                "Para exportar Postman v2.1 necesitás elegir una collection.".to_string()
            })?;
            let collection = app_state
                .repository
                .get_collection_with_requests(&collection_id)
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("No existe la collection {collection_id}"))?;
            serde_json::to_string_pretty(
                &export_postman_collection(&collection).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?
        }
    };

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| error.to_string())?;
    }

    tokio::fs::write(&path, payload.as_bytes())
        .await
        .map_err(|error| error.to_string())?;

    Ok(format!(
        "Exportado a {} ({} bytes).",
        path.to_string_lossy(),
        payload.len()
    ))
}

/// Aplica el resultado de `ExportCompleted` (Tarea 7.6): muestra el mensaje
/// de éxito o error en `export_form.result_message`, sin mutar ningún otro
/// estado del workspace (el export es una operación de solo lectura sobre
/// `state.workspace`).
fn handle_export_completed(state: &mut Midway, result: Result<String, String>) {
    state.workspace_panel.export_form.busy = false;

    match result {
        Ok(message) => {
            state.workspace_panel.export_form.result_message = Some(message);
            state.workspace_panel.export_form.result_is_error = false;
        }
        Err(message) => {
            state.workspace_panel.export_form.result_message = Some(message);
            state.workspace_panel.export_form.result_is_error = true;
        }
    }
}

/// Maneja `ImportSubmitted` (Tarea 7.7, Requisitos 4.4, 4.5): valida el
/// payload localmente (no vacío; tamaño de a lo sumo `MAX_IMPORT_PAYLOAD_BYTES`,
/// 10 MB, medido en bytes UTF-8 antes de invocar ningún parser) y, si la
/// validación pasa, dispara la orquestación asíncrona de import. Si la
/// validación falla, se fija `import_form.result_message` como error sin
/// llegar a invocar `detect_import_format` ni ningún parser, dejando el
/// estado de datos existente (`state.workspace`) sin mutar (Requisito 4.5).
fn handle_import_submitted(state: &mut Midway) -> Task<Message> {
    let form = &state.workspace_panel.import_form;
    let payload = form.payload_input.clone();
    let format = form.import_format;

    if payload.trim().is_empty() {
        state.workspace_panel.import_form.result_message =
            Some("Necesitás pegar el contenido a importar.".to_string());
        state.workspace_panel.import_form.result_is_error = true;
        return Task::none();
    }

    if payload.len() > MAX_IMPORT_PAYLOAD_BYTES {
        state.workspace_panel.import_form.result_message = Some(format!(
            "El contenido a importar supera el límite de {} MB.",
            MAX_IMPORT_PAYLOAD_BYTES / (1024 * 1024)
        ));
        state.workspace_panel.import_form.result_is_error = true;
        return Task::none();
    }

    state.workspace_panel.import_form.busy = true;
    state.workspace_panel.import_form.result_message = None;

    let app_state = Arc::clone(&state.app_state);

    Task::perform(
        import_workspace_data(app_state, payload, format),
        |result| Message::Workspace(WorkspaceMessage::ImportCompleted(result)),
    )
}

/// Cuerpo async de la orquestación de import de la sección Data (Tarea 7.7,
/// Requisitos 4.4, 4.5): port directo de `perform_workspace_import`/
/// `apply_imported_http_collection` de `src-tauri/src/commands/mod.rs`,
/// reutilizando `parse_json_or_yaml_payload`, `detect_import_format`,
/// `parse_native_bundle`, `import_postman_collection` e
/// `import_openapi_document` (`domain::interop`) sin reescritura. A
/// diferencia del comando Tauri, esta versión siempre importa en modo
/// "merge" (no ofrece la opción de "replace" que limpia el workspace
/// previo, fuera de alcance de esta tarea) y no soporta un
/// `collection_name_override` explícito (se usa siempre el nombre por
/// defecto detectado/generado).
///
/// Antes de persistir cualquier environment, request o colección
/// importada, se le aplica la resolución de colisión de nombres (Tarea
/// 7.9, Requisito 4.11, `unique_name_against`): si el nombre ya existe en
/// el workspace actual, se le asigna al elemento importado un nombre
/// diferenciado (sufijo numérico incremental), preservando el elemento
/// existente sin modificar y el elemento importado bajo su nuevo nombre,
/// sin sobrescribir ni eliminar nada.
async fn import_workspace_data(
    app_state: Arc<AppState>,
    payload_text: String,
    requested_format: WorkspaceImportFormat,
) -> Result<(WorkspaceSnapshot, String), String> {
    let payload = parse_json_or_yaml_payload(&payload_text).map_err(|error| error.to_string())?;
    let detected_format =
        detect_import_format(&payload, requested_format).map_err(|error| error.to_string())?;

    let existing_snapshot = app_state
        .repository
        .export_full_snapshot()
        .await
        .map_err(|error| error.to_string())?;

    let mut existing_environment_names: HashSet<String> = existing_snapshot
        .environments
        .iter()
        .map(|environment| environment.name.clone())
        .collect();
    let mut existing_request_names: HashSet<String> = existing_snapshot
        .collections
        .iter()
        .flat_map(|collection| collection.requests.iter())
        .map(|request| request.name.clone())
        .collect();
    let mut existing_collection_names: HashSet<String> = existing_snapshot
        .collections
        .iter()
        .map(|collection| collection.collection.name.clone())
        .collect();

    let message = match detected_format {
        WorkspaceImportFormat::NativeWorkspaceV1 => {
            let bundle: NativeWorkspaceBundle =
                parse_native_bundle(payload).map_err(|error| error.to_string())?;
            let mut requests_imported = 0_u64;
            let mut environments_imported = 0_u64;
            let collections_imported = bundle.snapshot.collections.len() as u64;

            for mut environment in bundle.snapshot.environments {
                environment.name =
                    unique_name_against(&existing_environment_names, &environment.name);
                existing_environment_names.insert(environment.name.clone());
                app_state
                    .repository
                    .save_environment(SaveEnvironmentInput {
                        environment_id: None,
                        name: environment.name,
                        variables: environment.variables,
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                environments_imported += 1;
            }

            for collection in bundle.snapshot.collections {
                let collection_name =
                    unique_name_against(&existing_collection_names, &collection.collection.name);
                existing_collection_names.insert(collection_name.clone());
                let created_collection = app_state
                    .repository
                    .create_collection(collection_name)
                    .await
                    .map_err(|error| error.to_string())?;

                for mut request in collection.requests {
                    request.name = unique_name_against(&existing_request_names, &request.name);
                    existing_request_names.insert(request.name.clone());
                    request.draft.name = request.name.clone();
                    app_state
                        .repository
                        .save_request(SaveRequestInput {
                            request_id: None,
                            collection_id: created_collection.id.clone(),
                            folder_id: None,
                            draft: request.draft,
                        })
                        .await
                        .map_err(|error| error.to_string())?;
                    requests_imported += 1;
                }
            }

            format!(
                "Import nativo aplicado: {collections_imported} colección(es), {requests_imported} request(s), {environments_imported} environment(s)."
            )
        }
        WorkspaceImportFormat::PostmanCollectionV21 => {
            let parsed = import_postman_collection(&payload).map_err(|error| error.to_string())?;
            import_http_collection(
                &app_state,
                parsed.collection_name,
                parsed.variables,
                parsed.folders,
                parsed.requests,
                &mut existing_collection_names,
                &mut existing_request_names,
                &mut existing_environment_names,
            )
            .await?
        }
        WorkspaceImportFormat::OpenApiV3 => {
            let parsed = import_openapi_document(&payload).map_err(|error| error.to_string())?;
            let imported_requests = parsed
                .requests
                .into_iter()
                .map(|draft| ImportedRequest {
                    draft,
                    folder_id: None,
                })
                .collect();
            import_http_collection(
                &app_state,
                parsed.collection_name,
                parsed.variables,
                Vec::new(),
                imported_requests,
                &mut existing_collection_names,
                &mut existing_request_names,
                &mut existing_environment_names,
            )
            .await?
        }
        WorkspaceImportFormat::Auto => {
            return Err("El formato Auto debería resolverse antes del match.".to_string());
        }
    };

    let refreshed_snapshot = app_state
        .repository
        .workspace_snapshot(HISTORY_LIMIT)
        .await
        .map_err(|error| error.to_string())?;

    Ok((refreshed_snapshot, message))
}

/// Importa una collection Postman/OpenAPI ya parseada (Tarea 7.7): crea una
/// nueva collection (con nombre diferenciado si colisiona, Tarea 7.9),
/// guarda cada request extraído dentro de ella (con nombre diferenciado si
/// colisiona) y, si el documento trae variables, las guarda como un
/// environment nuevo (también con nombre diferenciado si colisiona),
/// replicando la lógica de `apply_imported_http_collection` de
/// `src-tauri/src/commands/mod.rs` en modo "merge".
// `clippy::too_many_arguments` (8/7) es estructural: satisfacerlo exige
// agrupar los parámetros en un struct nuevo, o sea un refactor real de la
// firma y de todos sus call sites. Esta función ya fue tocada por la spec
// vigente y su comportamiento de deduplicación de nombres está fijado por
// tests, así que reestructurar la firma sólo para callar el lint agrega
// riesgo de regresión sin ningún beneficio de comportamiento. Se deja el
// `allow` acotado a esta función hasta que exista una tarea de refactor
// dedicada.
#[allow(clippy::too_many_arguments)]
async fn import_http_collection(
    app_state: &Arc<AppState>,
    default_collection_name: String,
    variables: Vec<KeyValueRow>,
    folders: Vec<Folder>,
    requests: Vec<ImportedRequest>,
    existing_collection_names: &mut HashSet<String>,
    existing_request_names: &mut HashSet<String>,
    existing_environment_names: &mut HashSet<String>,
) -> Result<String, String> {
    let collection_name = unique_name_against(existing_collection_names, &default_collection_name);
    existing_collection_names.insert(collection_name.clone());

    let collection = app_state
        .repository
        .create_collection(collection_name)
        .await
        .map_err(|error| error.to_string())?;

    // Persist folders, assigning the real collection_id.
    let collection_id_for_folders = collection.id.clone();
    let folders_clone = folders.clone();
    app_state
        .repository
        .insert_imported_folders(collection_id_for_folders, folders_clone)
        .await
        .map_err(|error| error.to_string())?;

    let mut requests_imported = 0_u64;
    for entry in requests {
        let mut draft = entry.draft;
        draft.name = unique_name_against(existing_request_names, &draft.name);
        existing_request_names.insert(draft.name.clone());

        app_state
            .repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: entry.folder_id.clone(),
                draft,
            })
            .await
            .map_err(|error| error.to_string())?;

        requests_imported += 1;
    }

    let environments_imported = if variables.is_empty() {
        0
    } else {
        let environment_name = unique_name_against(
            existing_environment_names,
            &format!("{} vars", collection.name),
        );
        existing_environment_names.insert(environment_name.clone());
        app_state
            .repository
            .save_environment(SaveEnvironmentInput {
                environment_id: None,
                name: environment_name,
                variables,
            })
            .await
            .map_err(|error| error.to_string())?;
        1
    };

    Ok(format!(
        "Import aplicado: colección \"{}\", {requests_imported} request(s), {environments_imported} environment(s).",
        collection.name
    ))
}

/// Resuelve colisiones de nombre al importar (Tarea 7.9, Requisito 4.11):
/// si `candidate` no está en `existing_names`, se devuelve sin cambios; si
/// ya existe, se le agrega el sufijo ` (2)`, ` (3)`, etc. (probando cada
/// entero creciente desde 2) hasta encontrar un nombre que tampoco esté en
/// `existing_names`, garantizando que el elemento importado se persista
/// bajo un nombre diferenciado sin sobrescribir ni eliminar el elemento
/// existente que aporta el nombre original (Property 17).
fn unique_name_against(existing_names: &HashSet<String>, candidate: &str) -> String {
    if !existing_names.contains(candidate) {
        return candidate.to_string();
    }

    let mut suffix = 2_u64;
    loop {
        let attempt = format!("{candidate} ({suffix})");
        if !existing_names.contains(&attempt) {
            return attempt;
        }
        suffix += 1;
    }
}

/// Aplica el resultado de `ImportCompleted` (Tarea 7.7): en caso de éxito,
/// reemplaza `state.workspace` completo con el snapshot recargado tras la
/// importación y muestra el mensaje descriptivo, limpiando el payload del
/// formulario; en caso de error, muestra el mensaje de error en
/// `import_form.result_message` sin mutar `state.workspace` (Requisito
/// 4.5: "sin modificar el estado de datos existente").
fn handle_import_completed(
    state: &mut Midway,
    result: Result<(WorkspaceSnapshot, String), String>,
) {
    state.workspace_panel.import_form.busy = false;

    match result {
        Ok((snapshot, message)) => {
            state.workspace = snapshot;
            state.workspace_panel.import_form.payload_input = String::new();
            state.workspace_panel.import_form.result_message = Some(message);
            state.workspace_panel.import_form.result_is_error = false;
            // Req 6.8: fall back to Debug if active collection was removed.
            enforce_top_bar_mode_after_collection_change(state);
        }
        Err(message) => {
            state.workspace_panel.import_form.result_message = Some(message);
            state.workspace_panel.import_form.result_is_error = true;
        }
    }
}

#[cfg(test)]
mod import_name_collision_tests {
    //! Feature: tauri-to-iced-migration, Tarea 7.9 (Requisito 4.11):
    //! unit tests de camino nominal y borde para `unique_name_against`.
    //! La property test que valida esta lógica de forma exhaustiva contra
    //! el flujo completo de import (Property 17) se implementa en la Tarea
    //! 7.10 en un archivo/tarea separada.

    use super::*;

    /// Camino nominal: el nombre candidato no colisiona con ningún nombre
    /// existente, así que se devuelve exactamente igual, sin sufijo.
    #[test]
    fn unique_name_against_returns_candidate_unchanged_when_no_collision() {
        let existing: HashSet<String> = ["Alpha".to_string(), "Beta".to_string()]
            .into_iter()
            .collect();

        assert_eq!(unique_name_against(&existing, "Gamma"), "Gamma");
    }

    /// Caso borde: el nombre candidato colisiona una sola vez; el nombre
    /// diferenciado devuelto SHALL ser distinto del original y SHALL no
    /// colisionar con ningún nombre existente (Requisito 4.11).
    #[test]
    fn unique_name_against_appends_suffix_on_single_collision() {
        let existing: HashSet<String> = ["Environment".to_string()].into_iter().collect();

        let result = unique_name_against(&existing, "Environment");

        assert_ne!(result, "Environment");
        assert!(!existing.contains(&result));
        assert_eq!(result, "Environment (2)");
    }

    /// Caso borde: el nombre candidato colisiona con el original Y con
    /// varios sufijos ya usados (p. ej. tras importar el mismo nombre
    /// varias veces); el resultado SHALL seguir siendo un nombre no usado.
    #[test]
    fn unique_name_against_skips_all_already_used_suffixes() {
        let existing: HashSet<String> = [
            "Environment".to_string(),
            "Environment (2)".to_string(),
            "Environment (3)".to_string(),
        ]
        .into_iter()
        .collect();

        let result = unique_name_against(&existing, "Environment");

        assert_eq!(result, "Environment (4)");
        assert!(!existing.contains(&result));
    }
}

/// Construye el contenido principal según el foco de presentación actual.
///
/// `window_height` es el alto de la ventana en coordenadas lógicas, medido por
/// el `responsive` de `view`. Solo viaja hacia abajo para que el divisor
/// horizontal del área Debug pueda expresar su borde inferior en las mismas
/// coordenadas en las que el listener global reporta `CursorMoved`.
pub fn main_content_pane<'a>(
    state: &'a Midway,
    ds: DesignSystem,
    window_height: f32,
) -> Element<'a, Message> {
    // Top_Bar: breadcrumb + mode tabs + theme toggle (Tarea 12.1).
    let top_bar = guarded_view("TopBar", || crate::ui::top_bar::view(state, &ds));

    let body: Element<'a, Message> = if crate::ui::onboarding::should_show_onboarding(state) {
        // Onboarding view: shown instead of Composer/Inspector when there
        // are no collections and the focus is RequestTab (Requirements 1.1,
        // 1.2, 1.4).
        guarded_view("Onboarding", || crate::ui::onboarding::view(&ds))
    } else {
        match state.main_content_focus {
            MainContentFocus::RequestTab => {
                // Route by top_bar_mode (Req 6.4, 6.5).
                match state.top_bar_mode {
                    TopBarMode::Debug => debug_content(state, &ds, window_height),
                    TopBarMode::Test => test_content(state, &ds),
                }
            }
            MainContentFocus::WorkspaceSection => container(guarded_view("WorkspacePanel", || {
                workspace_panel::section_content(state, &ds)
            }))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(ds.spacing.md)
            .into(),
        }
    };

    column![top_bar, body]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Debug mode: Request_Composer + Response_Inspector (Req 6.4).
/// Shown even without an active collection.
///
/// `window_height` se recibe del `responsive` de `view` y se pasa tal cual a
/// `debug_split_content`, que lo usa como borde inferior del área Debug para el
/// divisor horizontal del panel de respuesta (Req 9.2).
fn debug_content<'a>(
    state: &'a Midway,
    ds: &DesignSystem,
    window_height: f32,
) -> Element<'a, Message> {
    match state.active_tab {
        Some(active_tab_index) if active_tab_index < state.tabs.len() => {
            let toolbar = container(guarded_view("RequestToolbar", || {
                request_composer::toolbar(state, active_tab_index, ds)
            }))
            .width(Length::Fill)
            .padding([ds.spacing.sm, ds.spacing.md]);
            let design_system = *ds;
            let split = responsive(move |size| {
                debug_split_content(
                    state,
                    active_tab_index,
                    design_system,
                    debug_pane_layout(size.width),
                    size.width,
                    size.height,
                    window_height,
                )
            });

            column![toolbar, split]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
        _ => {
            // Req 9.5 / 14.2: en vez de informar la ausencia sin salida, el
            // estado vacío ofrece la acción real de abrir una tab en blanco
            // (`KeyboardMessage::NewBlankTabRequested`, el mismo mensaje que
            // Ctrl+Shift+N y que "+ Request" del explorador).
            empty_state::view(
                empty_state::no_open_requests(Message::Keyboard(
                    KeyboardMessage::NewBlankTabRequested,
                )),
                ds,
            )
        }
    }
}

// El toolbar ocupa todo el ancho sobre ambos paneles, por lo que editor e
// inspector siguen siendo utilizables con unos 280 px cada uno.
const DEBUG_HORIZONTAL_BREAKPOINT: f32 = 560.0;

/// Mensaje de inicio de arrastre correspondiente a cada divisor.
///
/// Para `ResponseHeight` el mensaje propaga el `area_bottom_y` que el divisor
/// ya conoce, de modo que la geometría del área Debug llega al reducer sin
/// estado paralelo en `Midway`.
fn divider_drag_started_message(target: PanelDragState) -> PanelResizeMessage {
    match target {
        PanelDragState::TreeMain => PanelResizeMessage::TreeDividerDragStarted,
        PanelDragState::RequestResponse => PanelResizeMessage::RequestResponseDividerDragStarted,
        PanelDragState::ResponseHeight { area_bottom_y } => {
            PanelResizeMessage::ResponseHeightDividerDragStarted { area_bottom_y }
        }
    }
}

/// Color de la línea de un divisor según su estado de interacción.
///
/// Los tres estados —reposo `accent` con α 0.18, hover con α 0.75 y activo
/// `accent` sólido— son los del baseline, extraídos acá para que el divisor
/// horizontal los reutilice tal cual en vez de redefinirlos: así se lee como
/// parte del mismo sistema y un cambio de estilo se hace en un solo lugar.
fn divider_line_color(ds: &DesignSystem, active: bool, hovered: bool) -> iced::Color {
    if active {
        ds.palette.accent
    } else if hovered {
        iced::Color {
            a: 0.75,
            ..ds.palette.accent
        }
    } else {
        iced::Color {
            a: 0.18,
            ..ds.palette.accent
        }
    }
}

/// Grosor de la línea de un divisor: 3 px mientras se arrastra, 2 px en
/// reposo y en hover. Compartido por ambos divisores por el mismo motivo que
/// [`divider_line_color`].
fn divider_line_thickness(active: bool) -> f32 {
    if active {
        3.0
    } else {
        2.0
    }
}

/// Separador vertical con una zona de agarre cómoda y una línea visual
/// centrada. El listener global mantiene el drag aunque el cursor salga de
/// estos pocos píxeles.
fn vertical_resize_divider<'a>(
    ds: &DesignSystem,
    target: PanelDragState,
    active: bool,
    hovered: bool,
    hit_width: f32,
) -> Element<'a, Message> {
    let line_color = divider_line_color(ds, active, hovered);
    let line_width = divider_line_thickness(active);
    let line = container(column![])
        .width(Length::Fixed(line_width))
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(line_color.into()),
            ..container::Style::default()
        });

    let on_press = divider_drag_started_message(target);

    mouse_area(
        container(line)
            .width(Length::Fixed(hit_width))
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .on_press(Message::PanelResize(on_press))
    .on_release(Message::PanelResize(PanelResizeMessage::DividerDragEnded))
    .on_enter(Message::PanelResize(PanelResizeMessage::DividerHovered(
        target,
    )))
    .on_exit(Message::PanelResize(PanelResizeMessage::DividerUnhovered(
        target,
    )))
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .into()
}

/// Separador horizontal, simétrico de [`vertical_resize_divider`]: misma zona
/// de agarre cómoda, misma línea centrada y los mismos tres estados visuales
/// (`divider_line_color` / `divider_line_thickness`), con los ejes
/// intercambiados —`width(Length::Fill)` / `height(Length::Fixed(hit_height))`
/// y `align_y(Vertical::Center)`— y el cursor `ResizingVertically`.
///
/// `target` llega ya con el `area_bottom_y` que la vista calculó a partir del
/// `Size` del `responsive`, y `divider_drag_started_message` lo propaga al
/// reducer en el mensaje de inicio de arrastre. Igual que en el divisor
/// vertical, el listener global mantiene el arrastre aunque el cursor salga de
/// estos pocos píxeles.
fn horizontal_resize_divider<'a>(
    ds: &DesignSystem,
    target: PanelDragState,
    active: bool,
    hovered: bool,
    hit_height: f32,
) -> Element<'a, Message> {
    let line_color = divider_line_color(ds, active, hovered);
    let line_height = divider_line_thickness(active);
    let line = container(column![])
        .width(Length::Fill)
        .height(Length::Fixed(line_height))
        .style(move |_theme| container::Style {
            background: Some(line_color.into()),
            ..container::Style::default()
        });

    let on_press = divider_drag_started_message(target);

    mouse_area(
        container(line)
            .width(Length::Fill)
            .height(Length::Fixed(hit_height))
            .align_y(iced::alignment::Vertical::Center),
    )
    .on_press(Message::PanelResize(on_press))
    .on_release(Message::PanelResize(PanelResizeMessage::DividerDragEnded))
    .on_enter(Message::PanelResize(PanelResizeMessage::DividerHovered(
        target,
    )))
    .on_exit(Message::PanelResize(PanelResizeMessage::DividerUnhovered(
        target,
    )))
    .interaction(iced::mouse::Interaction::ResizingVertically)
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebugPaneLayout {
    SideBySide,
    Stacked,
}

fn debug_pane_layout(available_width: f32) -> DebugPaneLayout {
    if available_width >= DEBUG_HORIZONTAL_BREAKPOINT {
        DebugPaneLayout::SideBySide
    } else {
        DebugPaneLayout::Stacked
    }
}

/// Construye el contenido del área Debug: editor de request e inspector de
/// respuesta, en columnas o apilados según `layout`.
///
/// `available_width` y `available_height` son el `Size` del `responsive` propio
/// del área y acotan el reparto entre paneles. `window_height` es el alto de la
/// ventana, medido por el `responsive` de `view`, y solo se usa para ubicar el
/// divisor horizontal del panel de respuesta en las coordenadas del cursor
/// (Req 9.2).
fn debug_split_content<'a>(
    state: &'a Midway,
    active_tab_index: usize,
    ds: DesignSystem,
    layout: DebugPaneLayout,
    available_width: f32,
    available_height: f32,
    window_height: f32,
) -> Element<'a, Message> {
    // Jerarquía visual por elevación (Req 9.1, diseño §8.2): el editor de
    // request queda en el nivel base (`background_primary`) y el inspector de
    // respuesta un nivel por encima (`surface_elevated`). El explorador ocupa
    // el nivel intermedio (`background_secondary`), aplicado en
    // `request_tree_pane::pane_shell`. Los tres niveles salen de la escala ya
    // existente de `DesignSystem`: no se introduce ningún color de marca nuevo
    // (Req 9.8).
    //
    // El inspector lleva un `iced::Border` completo, uniforme en los cuatro
    // lados, y no una franja como el explorador: es una superficie elevada
    // rodeada por los otros dos niveles, así que el contorno cerrado es
    // justamente lo que la separa: al explorador, en cambio, un contorno
    // cerrado lo encajonaría contra los bordes de la ventana, y por eso ahí el
    // borde es una franja de un solo lado.
    let request_background = ds.palette.background_primary;
    let response_background = ds.palette.surface_elevated;
    let separator_color = ds.palette.border;

    let request_pane = container(scrollable(guarded_view("RequestEditor", || {
        request_composer::editor(state, active_tab_index, &ds)
    })))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([ds.spacing.sm, ds.spacing.md])
    .style(move |_theme| container::Style {
        background: Some(request_background.into()),
        ..container::Style::default()
    });

    let response_pane = container(scrollable(guarded_view("ResponseInspector", || {
        response_inspector::view(state, active_tab_index)
    })))
    .width(Length::Fill)
    .height(Length::Fill)
    .padding([ds.spacing.sm, ds.spacing.md])
    .style(move |_theme| container::Style {
        background: Some(response_background.into()),
        border: iced::Border {
            color: separator_color,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    });

    match layout {
        DebugPaneLayout::SideBySide => {
            let request_width = request_panel_width_for_available(
                state.session.panel_sizes.request_panel_width,
                available_width,
            );
            let request_pane = request_pane.width(Length::Fixed(request_width));
            let divider = vertical_resize_divider(
                &ds,
                PanelDragState::RequestResponse,
                state.panel_dragging == Some(PanelDragState::RequestResponse),
                state.panel_hovered == Some(PanelDragState::RequestResponse),
                DEBUG_DIVIDER_HIT_WIDTH,
            );

            row![request_pane, divider, response_pane]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
        DebugPaneLayout::Stacked => {
            // El área Debug es el último elemento `Fill` de la columna y no hay
            // cromo por debajo, así que su borde inferior coincide con el de la
            // ventana. `area_bottom_y` es entonces el alto de la ventana, en las
            // mismas coordenadas lógicas en las que el listener global reporta
            // `CursorMoved`: es lo que le permite al reducer convertir la Y del
            // cursor en alto de panel. `available_height` (el `Size` del
            // `responsive` del área) es el alto del área en sí, que es lo que
            // acota el reparto entre editor e inspector.
            let area_bottom_y = window_height;
            let divider = horizontal_resize_divider(
                &ds,
                PanelDragState::ResponseHeight { area_bottom_y },
                state.panel_dragging == Some(PanelDragState::ResponseHeight { area_bottom_y }),
                state.panel_hovered == Some(PanelDragState::ResponseHeight { area_bottom_y }),
                RESPONSE_DIVIDER_HIT_HEIGHT,
            );
            let response_height = response_panel_height_for_available(
                state.session.panel_sizes.response_panel_height,
                available_height,
            );

            column![
                request_pane,
                divider,
                response_pane.height(Length::Fixed(response_height)),
            ]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }
    }
}

#[cfg(test)]
mod debug_layout_tests {
    //! Feature: midway-baseline-audit-and-first-vertical, Tarea 4.5
    //! Requisito 6.4.
    //!
    //! Test_Caracterización del baseline SIN CAMBIOS sobre
    //! `debug_pane_layout`. No se modifica código de producción: ni la
    //! función ni `DEBUG_HORIZONTAL_BREAKPOINT` cambian, y todo lo que se
    //! agrega acá es `#[cfg(test)]`.
    //!
    //! Los dos tests de ejemplo preexistentes se preservan con sus valores
    //! esperados originales.
    //!
    //! Headless por construcción: `debug_pane_layout` es aritmética pura
    //! sobre un `f32`, sin ventana, sin disco y sin red.

    use super::*;
    use proptest::prelude::*;

    /// Ancho disponible arbitrario. La estrategia se construye para cubrir
    /// a propósito las clases de entrada que el Requisito 6.4 nombra y que
    /// un rango uniforme casi nunca alcanzaría:
    ///
    /// - anchos "normales" a ambos lados del breakpoint,
    /// - cero (y `-0.0`, que en `f32` es un valor distinto con la misma
    ///   comparación),
    /// - negativos,
    /// - los vecinos inmediatos del breakpoint, donde vive el límite real
    ///   de la decisión,
    /// - los extremos finitos del tipo,
    /// - y los tres valores no finitos: `NaN`, `+inf` y `-inf`.
    fn arb_available_width() -> impl Strategy<Value = f32> {
        prop_oneof![
            // Anchos plausibles de ventana, incluyendo negativos.
            6 => -2_000.0f32..6_000.0f32,
            // Ceros y frontera exacta del breakpoint con sus vecinos.
            3 => prop_oneof![
                Just(0.0f32),
                Just(-0.0f32),
                Just(DEBUG_HORIZONTAL_BREAKPOINT),
                Just(f32::from_bits(DEBUG_HORIZONTAL_BREAKPOINT.to_bits() - 1)),
                Just(f32::from_bits(DEBUG_HORIZONTAL_BREAKPOINT.to_bits() + 1)),
                Just(-DEBUG_HORIZONTAL_BREAKPOINT),
            ],
            // Extremos finitos del tipo.
            1 => prop_oneof![
                Just(f32::MIN),
                Just(f32::MAX),
                Just(f32::MIN_POSITIVE),
                Just(-f32::MIN_POSITIVE),
            ],
            // Valores no finitos.
            2 => prop_oneof![
                Just(f32::NAN),
                Just(-f32::NAN),
                Just(f32::INFINITY),
                Just(f32::NEG_INFINITY),
            ],
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// Feature: midway-baseline-audit-and-first-vertical, Property 4: La
        /// decisión de layout del área Debug es total y la determina el
        /// breakpoint.
        ///
        /// Para todo ancho disponible, incluidos cero, negativos y valores
        /// no finitos, `debug_pane_layout` termina sin pánico y devuelve
        /// `SideBySide` si y solo si el ancho es mayor o igual que
        /// `DEBUG_HORIZONTAL_BREAKPOINT`.
        ///
        /// Comportamiento del baseline que esta propiedad fija de forma
        /// explícita para `NaN`: `debug_pane_layout` decide con `>=`, y toda
        /// comparación de orden con `NaN` es falsa, así que `NaN` cae en la
        /// rama `else` y produce `Stacked`. No es un caso especial escrito
        /// en la función: es la semántica de punto flotante de IEEE 754
        /// heredada tal cual. Se documenta acá porque es exactamente el tipo
        /// de comportamiento implícito que un refactor podría cambiar sin
        /// darse cuenta.
        #[test]
        fn property_4_debug_pane_layout_is_total_and_decided_by_breakpoint(
            available_width in arb_available_width(),
        ) {
            // Totalidad: la llamada retorna. El tipo de retorno no tiene
            // variante de error ni de "sin decisión", así que obtener un
            // valor ya es la evidencia de que la función es total para esta
            // entrada; se fija además que ese valor es una de las dos
            // variantes declaradas del baseline.
            let layout = debug_pane_layout(available_width);
            prop_assert!(
                matches!(layout, DebugPaneLayout::SideBySide | DebugPaneLayout::Stacked),
                "debug_pane_layout({available_width}) debe devolver una de las dos \
                 variantes del baseline, y devolvió {layout:?}"
            );

            // "Si y solo si": la equivalencia se afirma en ambas
            // direcciones comparando el booleano de la decisión con el
            // booleano de la comparación.
            let is_side_by_side = layout == DebugPaneLayout::SideBySide;
            let at_or_above_breakpoint = available_width >= DEBUG_HORIZONTAL_BREAKPOINT;
            prop_assert_eq!(
                is_side_by_side,
                at_or_above_breakpoint,
                "debug_pane_layout({}) devolvió {:?}, pero available_width >= {} es {}",
                available_width,
                layout,
                DEBUG_HORIZONTAL_BREAKPOINT,
                at_or_above_breakpoint
            );

            // `NaN` no satisface `>=`, así que la rama `else` lo manda a
            // `Stacked`. Se afirma aparte para que un cambio en esa
            // semántica falle con un mensaje que nombre el caso.
            if available_width.is_nan() {
                prop_assert_eq!(
                    layout,
                    DebugPaneLayout::Stacked,
                    "el baseline decide con `>=`, que es falso para NaN: NaN debe dar Stacked"
                );
            }

            // La decisión no depende de nada más que del ancho: la misma
            // entrada produce la misma salida.
            prop_assert_eq!(
                debug_pane_layout(available_width),
                layout,
                "debug_pane_layout debe ser determinista para el mismo ancho"
            );
        }
    }

    #[test]
    fn wide_debug_area_uses_side_by_side_layout() {
        assert_eq!(
            debug_pane_layout(DEBUG_HORIZONTAL_BREAKPOINT),
            DebugPaneLayout::SideBySide
        );
    }

    #[test]
    fn narrow_debug_area_uses_stacked_layout() {
        assert_eq!(
            debug_pane_layout(DEBUG_HORIZONTAL_BREAKPOINT - 1.0),
            DebugPaneLayout::Stacked
        );
    }

    /// Tarea 4.5 (Requisito 6.4): ejemplos concretos de las clases de
    /// entrada que la Property 4 cubre de forma general. Fijan el
    /// comportamiento observado del baseline, incluida la decisión para
    /// `NaN`, que sale de la semántica de `>=` y no de una rama explícita.
    #[test]
    fn degenerate_and_non_finite_widths_have_a_defined_layout() {
        assert_eq!(debug_pane_layout(0.0), DebugPaneLayout::Stacked);
        assert_eq!(debug_pane_layout(-0.0), DebugPaneLayout::Stacked);
        assert_eq!(debug_pane_layout(-1.0), DebugPaneLayout::Stacked);
        assert_eq!(debug_pane_layout(f32::MIN), DebugPaneLayout::Stacked);
        assert_eq!(debug_pane_layout(f32::MAX), DebugPaneLayout::SideBySide);
        assert_eq!(
            debug_pane_layout(f32::INFINITY),
            DebugPaneLayout::SideBySide
        );
        assert_eq!(
            debug_pane_layout(f32::NEG_INFINITY),
            DebugPaneLayout::Stacked
        );
        // `NaN >= x` es falso, así que el baseline cae en la rama `else`.
        assert_eq!(debug_pane_layout(f32::NAN), DebugPaneLayout::Stacked);
    }
}

/// Línea de progreso del Collection_Runner, en español (Req 9.6).
///
/// El baseline mostraba `format!("{progress:?}")`: el `Debug` derivado de
/// `CollectionRunProgressEvent`, con nombres de campo en inglés y comillas de
/// Rust. Esto lee los campos que interesan y arma una frase; no cambia el
/// evento ni el flujo de progreso, solo su presentación.
///
/// Función aparte de la vista para poder fijarla en un test sin construir un
/// `Midway` ni abrir una ventana.
fn run_progress_label(progress: &CollectionRunProgressEvent) -> String {
    let position = format!(
        "{} de {}",
        progress.processed_requests, progress.total_requests
    );

    match progress.phase {
        CollectionRunPhase::Started => {
            format!(
                "Ejecución iniciada: {} request(s) por ejecutar.",
                progress.total_requests
            )
        }
        CollectionRunPhase::RequestStarted => match progress.request_name.as_deref() {
            Some(name) => format!("Ejecutando \"{name}\" ({position})."),
            None => format!("Ejecutando request ({position})."),
        },
        CollectionRunPhase::RequestFinished => match progress.request_name.as_deref() {
            Some(name) => format!("Terminó \"{name}\" ({position})."),
            None => format!("Terminó un request ({position})."),
        },
        CollectionRunPhase::Finished => format!(
            "Ejecución finalizada: {} completado(s), {} con error, {} assertion(s) que pasaron y {} que fallaron.",
            progress.completed_requests,
            progress.errored_requests,
            progress.passed_assertions,
            progress.failed_assertions
        ),
    }
}

/// Test mode: Collection_Runner for the active collection (Req 6.5).
fn test_content<'a>(state: &'a Midway, ds: &DesignSystem) -> Element<'a, Message> {
    let text_color = ds.palette.text_secondary;

    // Show the Collection_Runner status for the active collection.
    // The full Collection_Runner UI is wired in Tarea 17.1; here we render
    // a meaningful placeholder that shows the runner state if available.
    let collection_name = state
        .active_collection_id
        .as_deref()
        .and_then(|id| {
            state
                .workspace
                .collections
                .iter()
                .find(|c| c.collection.id == id)
                .map(|c| c.collection.name.as_str())
        })
        .unwrap_or("(sin colección)");

    // Req 9.6: el encabezado en español. `Run` sigue en inglés más abajo
    // porque nombra el control que el Collection_Runner todavía no expone en
    // la interfaz (L12, Hito 4): renombrarlo acá inventaría un nombre para un
    // botón que no existe. La excepción está registrada en
    // `docs/known-limitations.md`.
    let header = text(format!("Ejecutor de la colección — {collection_name}"))
        .size(ds.typography.subtitle.size)
        .color(ds.palette.text_primary);

    let status: Element<'a, Message> = match &state.runner {
        Some(runner) => {
            let progress_text = if let Some(ref report) = runner.report {
                format!(
                    "Ejecución finalizada: {} request(s) ejecutados.",
                    report.items.len()
                )
            } else if let Some(ref progress) = runner.latest_progress {
                run_progress_label(progress)
            } else {
                "Ejecución en curso…".to_string()
            };
            text(progress_text).color(text_color).into()
        }
        None => text("Sin ejecución en curso. Presioná Run para ejecutar la colección.")
            .color(text_color)
            .into(),
    };

    container(
        column![header, status]
            .spacing(ds.spacing.md)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(ds.spacing.md)
    .into()
}

#[cfg(test)]
mod run_progress_label_tests {
    //! Feature: midway-baseline-audit-and-first-vertical, Tarea 11.3
    //! Requisito 9.6.
    //!
    //! Tests de ejemplo de [`run_progress_label`]: el baseline mostraba el
    //! `Debug` derivado de `CollectionRunProgressEvent` en el modo Test, con
    //! nombres de campo en inglés en texto visible. Lo que se fija acá es que
    //! la línea que lo reemplaza está en español y no reexpone esos nombres.
    //!
    //! Headless: la función es una `String` a partir de un evento, sin
    //! ventana, sin disco y sin red.

    use super::*;

    fn event(phase: CollectionRunPhase) -> CollectionRunProgressEvent {
        CollectionRunProgressEvent {
            run_id: "run-1".to_string(),
            phase,
            collection_id: "col-1".to_string(),
            collection_name: "Colección".to_string(),
            total_requests: 3,
            processed_requests: 2,
            current_index: 1,
            completed_requests: 2,
            errored_requests: 0,
            passed_assertions: 4,
            failed_assertions: 1,
            request_id: Some("req-1".to_string()),
            request_name: Some("Alta de usuario".to_string()),
            environment_name: None,
            resolved_url: None,
            response_status: Some(201),
            duration_ms: Some(42),
            error_message: None,
            started_at: "2024-01-01T00:00:00Z".to_string(),
            finished_at: None,
            emitted_at: "2024-01-01T00:00:01Z".to_string(),
        }
    }

    /// Las cuatro fases producen una frase en español que nombra el request o
    /// los contadores, sin filtrar los nombres de campo del `Debug` derivado.
    #[test]
    fn every_phase_has_a_spanish_label() {
        const PHASES: [CollectionRunPhase; 4] = [
            CollectionRunPhase::Started,
            CollectionRunPhase::RequestStarted,
            CollectionRunPhase::RequestFinished,
            CollectionRunPhase::Finished,
        ];
        // Fragmentos del `Debug` derivado que el baseline dejaba a la vista.
        const DEBUG_LEAKS: [&str; 4] = [
            "CollectionRunProgressEvent",
            "run_id",
            "processed_requests",
            "Some(",
        ];

        for phase in PHASES {
            let label = run_progress_label(&event(phase));

            assert!(
                !label.trim().is_empty(),
                "la fase {phase:?} quedó sin línea de progreso"
            );
            for leak in DEBUG_LEAKS {
                assert!(
                    !label.contains(leak),
                    "la línea de la fase {phase:?} filtra el Debug derivado: {label:?}"
                );
            }
        }
    }

    /// Las fases por request nombran el request en curso, que es la
    /// información que el usuario necesita del progreso.
    #[test]
    fn per_request_phases_name_the_request_and_its_position() {
        let started = run_progress_label(&event(CollectionRunPhase::RequestStarted));
        assert_eq!(started, "Ejecutando \"Alta de usuario\" (2 de 3).");

        let finished = run_progress_label(&event(CollectionRunPhase::RequestFinished));
        assert_eq!(finished, "Terminó \"Alta de usuario\" (2 de 3).");
    }

    /// Sin nombre de request la frase sigue siendo legible: no se imprime
    /// `None` ni un nombre vacío entre comillas.
    #[test]
    fn per_request_phases_degrade_without_a_request_name() {
        let mut anonymous = event(CollectionRunPhase::RequestStarted);
        anonymous.request_name = None;

        let label = run_progress_label(&anonymous);

        assert_eq!(label, "Ejecutando request (2 de 3).");
    }

    /// La fase final reporta los contadores consolidados.
    #[test]
    fn finished_phase_reports_the_counters() {
        let label = run_progress_label(&event(CollectionRunPhase::Finished));

        assert_eq!(
            label,
            "Ejecución finalizada: 2 completado(s), 0 con error, 4 assertion(s) que pasaron y 1 que fallaron."
        );
    }
}

/// Vista de nivel superior del rediseño Insomnia (Tarea 17.1).
///
/// Layout de tres paneles (de izquierda a derecha):
/// ┌──────────┬─────────────────────┬───────────────────────────────────┐
/// │          │                     │  Top_Bar (breadcrumb + Debug|Test) │
/// │ Activity │  Request_Tree_Pane  ├───────────────────────────────────┤
/// │   Bar    │  (tree + filter)    │  Main Content:                     │
/// │ (icons)  │                     │  - Debug: Composer + Inspector     │
/// │          │                     │  - Test: Collection_Runner         │
/// │          │                     │  - Section: Workspace_Panel        │
/// └──────────┴─────────────────────┴───────────────────────────────────┘
///
/// Ruteo del contenido principal por `MainContentFocus`/`top_bar_mode`:
/// - `RequestTab` + Debug: Request_Composer + Response_Inspector
/// - `RequestTab` + Test: Collection_Runner
/// - `WorkspaceSection`: Workspace_Panel
pub fn view(state: &Midway) -> Element<'_, Message> {
    let ds = state.theme.design_system();

    // Layout principal: tres columnas, altura completa.
    //
    // El cuerpo se arma dentro de un `responsive` porque el divisor horizontal
    // del panel de respuesta necesita el alto de la ventana en coordenadas
    // lógicas: el listener global de `CursorMoved` reporta la Y del cursor
    // relativa a la ventana, así que el borde inferior del área Debug tiene que
    // expresarse en esa misma referencia. El `responsive` de `debug_content`
    // solo conoce el tamaño de su propia área. Este envoltorio mide una vez,
    // arriba, y el valor baja por parámetro (Req 9.2). El `Size` no se usa para
    // ninguna otra decisión de layout, así que el resto de la vista se comporta
    // igual que antes.
    let body = responsive(move |window_size| {
        let mut body: iced::widget::Column<'_, Message> = column![];

        // Aviso de sesión descartada
        if let Some(notice) = &state.session.startup_notice {
            body = body.push(
                container(text(notice.clone()).color(iced::Color::from_rgb(0.8, 0.5, 0.0)))
                    .padding(ds.spacing.sm),
            );
        }

        // Activity_Bar: barra angosta de iconos a la izquierda (48px)
        let activity_bar = container(guarded_view("ActivityBar", || {
            crate::ui::activity_bar::view(state, &ds)
        }))
        .width(Length::Fixed(48.0))
        .height(Length::Fill);

        // Request_Tree_Pane: árbol de folders/requests de la colección activa
        let tree_pane = container(guarded_view("RequestTreePane", || {
            crate::ui::request_tree_pane::view(state, &ds)
        }))
        .width(Length::Fixed(
            state.session.panel_sizes.workspace_panel_width,
        ))
        .height(Length::Fill);

        // Divider árbol/contenido: hitbox de 10 px y línea acentuada centrada.
        let divider = vertical_resize_divider(
            &ds,
            PanelDragState::TreeMain,
            state.panel_dragging == Some(PanelDragState::TreeMain),
            state.panel_hovered == Some(PanelDragState::TreeMain),
            TREE_DIVIDER_HIT_WIDTH,
        );

        // Main content pane: Top_Bar encima + contenido ruteado por modo
        let main = main_content_pane(state, ds, window_size.height);

        body.push(
            row![activity_bar, tree_pane, divider, main]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .into()
    });

    let background = ds.palette.background_primary;
    let content: Element<'_, Message> = container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(background.into()),
            ..container::Style::default()
        })
        .into();

    // El campo de nombre necesita más ancho que los 48 px del Activity Bar.
    let content = guarded_view("CreateCollectionOverlay", || {
        crate::ui::activity_bar::with_create_collection_overlay(state, content, &ds)
    });

    let content = guarded_view("SaveRequestOverlay", || {
        crate::ui::save_request_modal::with_overlay(state, content, &ds)
    });

    let content = guarded_view("WorkspaceCrudOverlay", || {
        crate::ui::workspace_crud_modal::with_overlay(
            state.workspace_crud_dialog.as_ref(),
            content,
            &ds,
        )
    });

    // Overlay del Command Palette
    let content = guarded_view("CommandPalette", || {
        crate::ui::command_palette::with_overlay(state, content, &ds)
    });

    // Overlay del aviso de unsaved changes
    guarded_view("UnsavedChangesModal", || {
        crate::ui::unsaved_changes_modal::with_overlay(state, content, &ds)
    })
}

/// Suscripciones combinadas: autosave, teclado global y progreso del
/// Collection Runner (Tarea 9.2, Requisito 5.2). Por ahora solo la de
/// progreso del Collection_Runner está implementada; autosave y teclado
/// global se agregan en la Fase 5.
pub fn subscription(state: &Midway) -> Subscription<Message> {
    let runner_subscription = match state
        .runner
        .as_ref()
        .and_then(|runner| runner.running.clone())
    {
        Some(handle) => Subscription::run_with(handle, collection_runner_progress_stream),
        None => Subscription::none(),
    };

    // Timer de autosave (Tarea 11.4, Requisito 6.3): un intervalo de 1
    // segundo deja un margen amplio respecto al máximo de 2 segundos
    // exigido entre una modificación y su persistencia, cubriendo el peor
    // caso (una modificación ocurre justo después de un tick) sin
    // acercarse al límite.
    let autosave_subscription = iced::time::every(std::time::Duration::from_secs(1))
        .map(|_| Message::Session(SessionMessage::AutosaveTick));

    // Shortcut Ctrl/Cmd+K para abrir/cerrar el Command Palette (Tarea 11.2,
    // Requisito 6.1). `Modifiers::COMMAND` es Ctrl en Windows/Linux y Cmd
    // en macOS (ver `iced::keyboard::Modifiers::COMMAND`), que es
    // exactamente la semántica "Ctrl/Cmd+K" del Criterio 6.1. Se usa
    // `iced::keyboard::listen()` + `filter_map` (en lugar de escuchar todos
    // los `Event`s de la app) para mantener este mensaje aislado del resto
    // de la tabla de shortcuts globales, que la Tarea 11.12 agrega por
    // separado sin tener que tocar este filtro.
    let palette_shortcut_subscription = iced::keyboard::listen().filter_map(|event| match event {
        iced::keyboard::Event::KeyPressed { key, modifiers, .. } if modifiers.command() => {
            match key.as_ref() {
                iced::keyboard::Key::Character(character)
                    if character.eq_ignore_ascii_case("k") =>
                {
                    Some(Message::Palette(PaletteMessage::Toggled))
                }
                _ => None,
            }
        }
        _ => None,
    });

    // Resto de la tabla global de shortcuts (Tarea 11.12, Requisito 6.8):
    // Ctrl+Enter, Ctrl+S, Ctrl+Shift+N, Ctrl+Shift+P, Ctrl+., Ctrl+W,
    // Ctrl+Shift+T, Alt+1..9 y Esc. Ctrl/Cmd+K queda deliberadamente fuera
    // de este filtro (`palette_shortcut_subscription` arriba ya lo cubre,
    // ver su doc): agregarlo aquí también despacharía
    // `PaletteMessage::Toggled` dos veces por la misma pulsación de tecla,
    // dado que `Subscription::batch` combina ambas subscriptions sobre el
    // mismo stream de eventos de teclado.
    //
    // Se usa `iced::keyboard::listen()` + `filter_map` (en lugar de
    // `on_key_press`, que no es una función standalone disponible en la
    // API pública de iced 0.14: `listen()` + `filter_map` sobre
    // `Event::KeyPressed` es el equivalente idiomático, y es exactamente
    // el patrón ya usado por `palette_shortcut_subscription`), por
    // consistencia con esa subscription existente.
    //
    // Los shortcuts dependientes de estado de la app (Ctrl+W, Alt+1..9,
    // Esc) no pueden resolver "cuál es la tab activa"/"qué overlay está
    // abierto" aquí: esta función solo recibe `&Midway` una vez por
    // reconstrucción de la subscription, no en cada evento de teclado, y
    // el closure de `filter_map` solo recibe el `Event`, sin ninguna
    // referencia a `state`. Por eso despachan mensajes sin payload de
    // estado (`KeyboardMessage::CloseActiveTabShortcut`,
    // `GoToTabShortcut { index }`, `EscapePressed`) que `update_keyboard`
    // resuelve contra el estado vigente en el momento en que el mensaje
    // llega a `update`, en vez de en la subscription.
    let global_shortcut_subscription = iced::keyboard::listen().filter_map(|event| match event {
        iced::keyboard::Event::KeyPressed { key, modifiers, .. }
            if modifiers.command() && modifiers.shift() =>
        {
            match key.as_ref() {
                iced::keyboard::Key::Character(character)
                    if character.eq_ignore_ascii_case("n") =>
                {
                    Some(Message::Keyboard(KeyboardMessage::NewBlankTabRequested))
                }
                iced::keyboard::Key::Character(character)
                    if character.eq_ignore_ascii_case("p") =>
                {
                    Some(Message::RequestComposer(
                        RequestComposerMessage::SettingsPressed,
                    ))
                }
                iced::keyboard::Key::Character(character)
                    if character.eq_ignore_ascii_case("t") =>
                {
                    Some(Message::RequestComposer(
                        RequestComposerMessage::ClosedTabReopened,
                    ))
                }
                _ => None,
            }
        }
        iced::keyboard::Event::KeyPressed { key, modifiers, .. } if modifiers.command() => {
            match key.as_ref() {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => Some(
                    Message::RequestComposer(RequestComposerMessage::SendPressed),
                ),
                iced::keyboard::Key::Character(character)
                    if character.eq_ignore_ascii_case("s") =>
                {
                    Some(Message::Keyboard(KeyboardMessage::SaveRequested))
                }
                iced::keyboard::Key::Character(character)
                    if character.eq_ignore_ascii_case("w") =>
                {
                    Some(Message::Keyboard(KeyboardMessage::CloseActiveTabShortcut))
                }
                iced::keyboard::Key::Character(".") => {
                    Some(Message::Workspace(WorkspaceMessage::ToggleCollapsed))
                }
                _ => None,
            }
        }
        iced::keyboard::Event::KeyPressed { key, modifiers, .. } if modifiers.alt() => {
            match key.as_ref() {
                iced::keyboard::Key::Character(character) => {
                    let digit = character.parse::<usize>().ok()?;
                    if (1..=9).contains(&digit) {
                        Some(Message::Keyboard(KeyboardMessage::GoToTabShortcut {
                            index: digit - 1,
                        }))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        iced::keyboard::Event::KeyPressed { key, modifiers, .. } if modifiers.is_empty() => {
            match key.as_ref() {
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                    Some(Message::Keyboard(KeyboardMessage::EscapePressed))
                }
                _ => None,
            }
        }
        _ => None,
    });

    // Siempre activa: si se crea recién después del press inicial, Iced
    // puede perder movimientos/liberación pertenecientes al mismo lote de
    // eventos. El reducer ignora CursorMoved cuando no hay drag activo.
    let panel_resize_subscription = iced::event::listen_with(|event, _status, _id| {
        panel_resize_message_for_event(&event).map(Message::PanelResize)
    });
    // El handle del request puede soltarse fuera de su fila o del árbol;
    // escuchamos la liberación global para completar/cancelar el drop.
    let request_drop_subscription = iced::event::listen_with(|event, _status, _id| match event {
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
            Some(Message::Tree(TreeMessage::RequestDragReleased))
        }
        iced::Event::Mouse(iced::mouse::Event::CursorLeft)
        | iced::Event::Window(iced::window::Event::Unfocused) => {
            Some(Message::Tree(TreeMessage::RequestDragCancelled))
        }
        _ => None,
    });

    Subscription::batch([
        runner_subscription,
        autosave_subscription,
        palette_shortcut_subscription,
        global_shortcut_subscription,
        panel_resize_subscription,
        request_drop_subscription,
    ])
}

fn panel_resize_message_for_event(event: &iced::Event) -> Option<PanelResizeMessage> {
    match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
            Some(PanelResizeMessage::DividerDragged {
                x: position.x,
                y: position.y,
            })
        }
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))
        | iced::Event::Mouse(iced::mouse::Event::CursorLeft)
        | iced::Event::Window(iced::window::Event::Unfocused) => {
            Some(PanelResizeMessage::DividerDragEnded)
        }
        _ => None,
    }
}

/// Construye el `Stream<Item = Message>` que consume el extremo receptor del
/// canal de progreso identificado por `handle`, reenviando cada
/// `CollectionRunProgressEvent` recibido como `Message::Runner(RunnerMessage::ProgressReceived(...))`.
///
/// `handle.take()` devuelve `None` en la segunda invocación en adelante con
/// el mismo `handle` (mismo hash para `Subscription::run_with`, ver
/// `ProgressReceiverHandle`); en ese caso se devuelve un stream vacío para
/// no reconstruir un consumidor duplicado del mismo canal.
fn collection_runner_progress_stream(
    handle: &ProgressReceiverHandle,
) -> BoxStream<'static, Message> {
    match handle.take() {
        Some(receiver) => receiver
            .map(|event| Message::Runner(RunnerMessage::ProgressReceived(event)))
            .boxed(),
        None => stream::empty().boxed(),
    }
}

#[cfg(test)]
mod tests {
    //! Feature: tauri-to-iced-migration, Property 2: Enrutamiento de pegado
    //! de cURL según estado de la tab.
    //! Validates: Requirements 2.7, 2.8
    //!
    //! Este test ejercita únicamente el ENRUTAMIENTO de `handle_url_pasted`
    //! (sobrescribir la tab activa vs. crear una nueva tab), no la
    //! corrección del parseo de cURL en sí (esa es la Property 1, cubierta
    //! en `curl.rs`, Tarea 3.4). Por eso el generador de comandos cURL usado
    //! aquí es deliberadamente simple.
    //!
    //! `handle_url_pasted` solo lee/muta `Midway.tabs`/`Midway.active_tab`
    //! (nunca `app_state`/`workspace`/etc., ver su implementación arriba),
    //! así que el `Midway` de prueba se construye directamente por literal
    //! de struct (sus campos son `pub`), evitando pasar por
    //! `Midway::new`/`AppState::initialize` (que requieren resolver un
    //! directorio de datos real del sistema). El único campo que sigue
    //! necesitando un `AppState` real es `app_state`, que se construye
    //! contra un archivo SQLite temporal que este test nunca llega a leer
    //! ni escribir.

    use super::*;
    use midway_core::domain::cookies::CookieJarHandle;
    use midway_core::domain::http::{
        ApiKeyPlacement, FormDataFieldKind, FormDataRow, KeyValueRow, RequestBodyDraft,
    };
    use midway_core::infra::sqlite_repository::SqliteRepository;
    use midway_core::runtime::request_executor::RequestExecutorHandle;
    use midway_core::runtime::secret_executor::SecretExecutorHandle;
    use proptest::prelude::*;

    /// Construye un `AppState` real pero "inerte" para el test: un
    /// `SqliteRepository` contra un archivo SQLite temporal (nunca se
    /// invoca ninguno de sus métodos durante este test) y executors
    /// spawneados de forma normal (tampoco se invoca `execute`/`get` sobre
    /// ellos), ya que `handle_url_pasted` no los toca en absoluto.
    fn build_test_app_state() -> AppState {
        let temp_file = tempfile::NamedTempFile::new()
            .expect("no se pudo crear un archivo temporal para el AppState de prueba");
        let db_path = temp_file.path().to_path_buf();

        let runtime = tokio::runtime::Runtime::new()
            .expect("no se pudo crear el runtime de tokio para el AppState de prueba");

        let app_state = runtime.block_on(async {
            let repository = SqliteRepository::open(&db_path)
                .await
                .expect("no se pudo abrir el SqliteRepository de prueba");

            let client = reqwest::Client::builder()
                .build()
                .expect("no se pudo construir el cliente reqwest de prueba");

            AppState {
                repository,
                request_executor: RequestExecutorHandle::spawn(client),
                secret_executor: SecretExecutorHandle::spawn("midway-test".to_string()),
                cookie_jar: CookieJarHandle::new(),
            }
        });

        // El archivo temporal puede eliminarse en cuanto termina `open`
        // (que ya corrió las migraciones): este test nunca vuelve a leer ni
        // escribir en el repositorio.
        drop(temp_file);

        app_state
    }

    /// Construye un `Midway` de prueba con una única tab activa cuyo draft
    /// es el recibido. Visibilidad `pub(super)` (en vez de privada): el
    /// módulo hermano `error_boundary_tests` (Tarea 11.14) también
    /// necesita este helper para sus tests de `guarded_update`/`guarded_view`.
    pub(super) fn build_test_midway(draft: RequestDraft) -> Midway {
        Midway {
            app_state: Arc::new(build_test_app_state()),
            workspace: midway_core::domain::workspace::WorkspaceSnapshot {
                collections: Vec::new(),
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
            tabs: vec![RequestTabState::from_draft(draft)],
            active_tab: Some(0),
            closed_tabs: VecDeque::new(),
            workspace_panel: WorkspacePanelState::default(),
            palette: PaletteState::default(),
            runner: None,
            session: SessionStoreState::default(),
            theme: ThemeSettingsState::default(),
            main_content_focus: MainContentFocus::default(),
            updater: UpdaterState::default(),
            crash_log: Vec::new(),
            unsaved_changes_prompt: None,
            active_collection_id: None,
            top_bar_mode: TopBarMode::default(),
            tree: TreeViewState::default(),
            create_collection_prompt: None,
            save_request_prompt: None,
            workspace_crud_dialog: None,
            panel_dragging: None,
            panel_hovered: None,
        }
    }

    fn kv_map(rows: &[KeyValueRow]) -> BTreeMap<String, String> {
        rows.iter()
            .map(|row| (row.key.clone(), row.value.clone()))
            .collect()
    }

    /// Compara los campos relevantes de un `RequestDraft` contra el
    /// resultado esperado (oráculo obtenido llamando directamente a
    /// `curl::parse_curl_command_to_draft`), ignorando `id`s generados de
    /// forma independiente en cada llamada al parser (la corrección
    /// exhaustiva del parseo en sí ya está cubierta por la Property 1 en
    /// `curl.rs`, Tarea 3.4).
    fn draft_matches_parsed_curl(actual: &RequestDraft, expected: &RequestDraft) -> bool {
        actual.method == expected.method
            && actual.url == expected.url
            && kv_map(&actual.query) == kv_map(&expected.query)
            && kv_map(&actual.headers) == kv_map(&expected.headers)
            && actual.auth == expected.auth
            && actual.body.mode == expected.body.mode
            && actual.body.value == expected.body.value
    }

    /// Draft vacío (Criterio 2.7): igual al de una tab en blanco recién
    /// creada.
    fn empty_draft_strategy() -> impl Strategy<Value = RequestDraft> {
        Just(create_blank_draft())
    }

    /// Variedad de drafts NO vacíos: cada variante difiere del draft en
    /// blanco en exactamente un campo relevante para `is_draft_empty`
    /// (método, URL, query, headers, auth, body).
    fn non_empty_draft_strategy() -> impl Strategy<Value = RequestDraft> {
        prop_oneof![
            Just({
                let mut draft = create_blank_draft();
                draft.method = HttpMethod::POST;
                draft
            }),
            Just({
                let mut draft = create_blank_draft();
                draft.url = "https://existing.example.com/tab".to_string();
                draft
            }),
            Just({
                let mut draft = create_blank_draft();
                draft.query.push(KeyValueRow {
                    id: "existing-query".to_string(),
                    key: "existing".to_string(),
                    value: "1".to_string(),
                    enabled: true,
                });
                draft
            }),
            Just({
                let mut draft = create_blank_draft();
                draft.headers.push(KeyValueRow {
                    id: "existing-header".to_string(),
                    key: "X-Existing".to_string(),
                    value: "1".to_string(),
                    enabled: true,
                });
                draft
            }),
            Just({
                let mut draft = create_blank_draft();
                draft.auth = AuthConfig::Bearer {
                    token: "existing-token".to_string(),
                };
                draft
            }),
            Just({
                let mut draft = create_blank_draft();
                draft.body = RequestBodyDraft {
                    mode: BodyMode::Text,
                    value: "existing body".to_string(),
                    form_data: vec![],
                };
                draft
            }),
        ]
    }

    /// `(es_vacío, draft_inicial)`.
    fn starting_draft_strategy() -> impl Strategy<Value = (bool, RequestDraft)> {
        prop_oneof![
            empty_draft_strategy().prop_map(|draft| (true, draft)),
            non_empty_draft_strategy().prop_map(|draft| (false, draft)),
        ]
    }

    fn curl_method_strategy() -> impl Strategy<Value = HttpMethod> {
        prop_oneof![
            Just(HttpMethod::GET),
            Just(HttpMethod::POST),
            Just(HttpMethod::PUT),
            Just(HttpMethod::DELETE),
        ]
    }

    fn curl_base_url_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("https://example.com/foo".to_string()),
            Just("https://api.example.org/v1/items".to_string()),
        ]
    }

    fn curl_token_strategy() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9]{0,7}".prop_map(|s| s.to_string())
    }

    /// Genera un comando cURL válido y variado (método + URL + header
    /// opcional). Esta propiedad ejercita el ENRUTAMIENTO de
    /// `handle_url_pasted`, no la corrección exhaustiva del parseo (Property
    /// 1, en `curl.rs`), por lo que el generador es deliberadamente simple.
    fn valid_curl_command_strategy() -> impl Strategy<Value = String> {
        (
            curl_method_strategy(),
            curl_base_url_strategy(),
            proptest::option::of((curl_token_strategy(), curl_token_strategy())),
        )
            .prop_map(|(method, url, header)| {
                let mut command = format!("curl -X {} '{}'", method, url);
                if let Some((key, value)) = header {
                    command.push_str(&format!(" -H '{}: {}'", key, value));
                }
                command
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 20, ..ProptestConfig::default() })]

        #[test]
        fn property_2_curl_paste_routing_by_tab_state(
            (starting_draft_is_empty, starting_draft) in starting_draft_strategy(),
            curl_command in valid_curl_command_strategy(),
        ) {
            let (expected_draft, _warnings) = curl::parse_curl_command_to_draft(&curl_command)
                .expect("comando generado a partir de componentes válidos debe parsear");

            let original_draft = starting_draft.clone();
            let mut midway = build_test_midway(starting_draft);

            handle_url_pasted(&mut midway, curl_command.clone());

            if starting_draft_is_empty {
                // Criterio 2.7: tab activa vacía -> se sobrescribe in-place,
                // sin crear una tab nueva.
                prop_assert_eq!(midway.tabs.len(), 1);
                prop_assert_eq!(midway.active_tab, Some(0));
                prop_assert!(draft_matches_parsed_curl(&midway.tabs[0].draft, &expected_draft));
            } else {
                // Criterio 2.8: tab activa con datos -> se crea una nueva
                // tab y se cambia a ella, dejando la original intacta.
                prop_assert_eq!(midway.tabs.len(), 2);
                prop_assert_eq!(midway.active_tab, Some(1));

                // La tab original NO debe haber sido modificada (comparación
                // por serialización: es el mismo valor clonado antes de la
                // llamada, así que también deben coincidir los `id`s).
                prop_assert_eq!(
                    serde_json::to_value(&original_draft).unwrap(),
                    serde_json::to_value(&midway.tabs[0].draft).unwrap()
                );

                prop_assert!(draft_matches_parsed_curl(&midway.tabs[1].draft, &expected_draft));
            }
        }
    }

    /// Genera texto arbitrario que NO parece un comando cURL, según el
    /// predicado real `curl::looks_like_curl_command` (en vez de rederivar
    /// a mano su lógica de límite de palabra). Cualquier string generado
    /// que sí pareciera cURL (p. ej. "curl foo", "curl.exe bar") se
    /// descarta con `.filter(...)`.
    fn non_curl_text_strategy() -> impl Strategy<Value = String> {
        ".*".prop_filter("no debe parecer un comando cURL", |s| {
            !curl::looks_like_curl_command(s)
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 20, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 3: Texto no-cURL se
        /// inserta como texto plano.
        /// Validates: Requirements 2.9
        ///
        /// Para cualquier texto pegado que no parezca un comando cURL, el
        /// resultado debe ser la inserción literal de ese texto en
        /// `draft.url`, sin crear tabs nuevas ni tocar ningún otro campo del
        /// draft ni `curl_paste_error`, sin importar si la tab activa
        /// estaba vacía o no (Criterio 2.9 es independiente del estado de
        /// la tab, a diferencia de los Criterios 2.7/2.8).
        #[test]
        fn property_3_non_curl_text_is_inserted_literally(
            (_starting_draft_is_empty, starting_draft) in starting_draft_strategy(),
            pasted_text in non_curl_text_strategy(),
        ) {
            let original_draft = starting_draft.clone();
            let mut midway = build_test_midway(starting_draft);

            handle_url_pasted(&mut midway, pasted_text.clone());

            // No se creó ninguna tab nueva ni se cambió la tab activa: el
            // texto se insertó en la tab existente.
            prop_assert_eq!(midway.tabs.len(), 1);
            prop_assert_eq!(midway.active_tab, Some(0));

            // La URL fue reemplazada literalmente por el texto pegado.
            prop_assert_eq!(&midway.tabs[0].draft.url, &pasted_text);

            // Todos los demás campos del draft permanecen exactamente
            // iguales a los del draft inicial.
            prop_assert_eq!(midway.tabs[0].draft.method, original_draft.method);
            prop_assert_eq!(kv_map(&midway.tabs[0].draft.query), kv_map(&original_draft.query));
            prop_assert_eq!(kv_map(&midway.tabs[0].draft.headers), kv_map(&original_draft.headers));
            prop_assert_eq!(&midway.tabs[0].draft.auth, &original_draft.auth);
            prop_assert_eq!(midway.tabs[0].draft.body.mode, original_draft.body.mode);
            prop_assert_eq!(&midway.tabs[0].draft.body.value, &original_draft.body.value);

            // Nunca se invoca la extracción de campos del Curl_Importer en
            // este camino, así que jamás se setea un error de pegado.
            prop_assert!(midway.tabs[0].curl_paste_error.is_none());
        }
    }

    /// Genera comandos que SÍ parecen cURL (empiezan con "curl"/"curl.exe")
    /// pero cuyo parseo falla en `curl::parse_curl_command_to_draft`: o bien
    /// un flag que requiere un valor y no lo tiene al final del comando
    /// (`-X`, `-H`, `--header`, `-u` colgantes), o bien un comando sin
    /// ningún token que pueda interpretarse como URL (`curl -X POST`).
    fn malformed_curl_command_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("curl -X".to_string()),
            Just("curl -H".to_string()),
            Just("curl -X POST".to_string()),
            Just("curl --header".to_string()),
            Just("curl.exe -u".to_string()),
            Just("curl -X POST -H".to_string()),
            Just("curl --request".to_string()),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 20, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 4: cURL malformado no
        /// muta el contenido existente.
        /// Validates: Requirements 2.19
        ///
        /// Para cualquier texto pegado que empiece con "curl"/"curl.exe"
        /// pero cuyo parseo falle, `handle_url_pasted` SHALL mostrar un
        /// mensaje de error (`curl_paste_error`) y el contenido previamente
        /// existente en la tab activa (incluyendo su URL) SHALL permanecer
        /// exactamente igual al estado anterior al pegado, sin crear una
        /// tab nueva, sin importar si la tab activa estaba vacía o no.
        #[test]
        fn property_4_malformed_curl_does_not_mutate_existing_content(
            (_starting_draft_is_empty, mut starting_draft) in starting_draft_strategy(),
            malformed_command in malformed_curl_command_strategy(),
        ) {
            // Sanity check del generador: cada patrón elegido a mano debe
            // efectivamente fallar al parsear. Si esto alguna vez no se
            // cumple, el generador tiene un bug (un patrón que creíamos
            // malformado en realidad parsea con éxito).
            prop_assert!(curl::parse_curl_command_to_draft(&malformed_command).is_err());

            // URL previa no trivial y fácil de comparar, para que la
            // aserción de "sin cambios" sea significativa incluso en el
            // caso de tab vacía (que por defecto tiene URL vacía).
            starting_draft.url = "https://original-untouched.example.com/keep-me".to_string();

            let original_draft = starting_draft.clone();
            let mut midway = build_test_midway(starting_draft);

            handle_url_pasted(&mut midway, malformed_command.clone());

            // No se creó ninguna tab nueva ni se cambió la tab activa.
            prop_assert_eq!(midway.tabs.len(), 1);
            prop_assert_eq!(midway.active_tab, Some(0));

            // El draft de la tab activa permanece EXACTAMENTE igual al
            // estado anterior al pegado (comparación campo por campo vía
            // serialización, para capturar cualquier diferencia incluyendo
            // `id`, `name`, `timeout_ms`, `environment_id`,
            // `response_tests`, etc.).
            prop_assert_eq!(
                serde_json::to_value(&original_draft).unwrap(),
                serde_json::to_value(&midway.tabs[0].draft).unwrap()
            );

            // Se mostró un mensaje de error.
            prop_assert!(midway.tabs[0].curl_paste_error.is_some());
        }
    }

    // -----------------------------------------------------------------
    // Tarea 3.19 (Requisito 2.17): test de integración end-to-end del
    // flujo Send contra un servidor HTTP mock local real (`wiremock`).
    // -----------------------------------------------------------------

    /// Versión async de `build_test_app_state`, pensada para invocarse
    /// directamente desde un `#[tokio::test]` (que ya provee su propio
    /// runtime de Tokio): evita el runtime manual + `block_on` de la
    /// versión sync, que no puede anidarse dentro de un runtime ya en
    /// marcha (`Cannot start a runtime from within a runtime`).
    async fn build_test_app_state_async() -> AppState {
        let temp_file = tempfile::NamedTempFile::new()
            .expect("no se pudo crear un archivo temporal para el AppState de prueba");
        let db_path = temp_file.path().to_path_buf();

        let repository = SqliteRepository::open(&db_path)
            .await
            .expect("no se pudo abrir el SqliteRepository de prueba");

        let client = reqwest::Client::builder()
            .build()
            .expect("no se pudo construir el cliente reqwest de prueba");

        let app_state = AppState {
            repository,
            request_executor: RequestExecutorHandle::spawn(client),
            secret_executor: SecretExecutorHandle::spawn("midway-test".to_string()),
            cookie_jar: CookieJarHandle::new(),
        };

        // El archivo temporal puede eliminarse en cuanto termina `open`
        // (que ya corrió las migraciones), igual que en la versión sync.
        drop(temp_file);

        app_state
    }

    /// Versión async de `build_test_midway`, construida sobre
    /// `build_test_app_state_async` por la misma razón (evitar anidar un
    /// runtime de Tokio dentro de otro dentro de un `#[tokio::test]`).
    async fn build_test_midway_async(draft: RequestDraft) -> Midway {
        Midway {
            app_state: Arc::new(build_test_app_state_async().await),
            workspace: midway_core::domain::workspace::WorkspaceSnapshot {
                collections: Vec::new(),
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
            tabs: vec![RequestTabState::from_draft(draft)],
            active_tab: Some(0),
            closed_tabs: VecDeque::new(),
            workspace_panel: WorkspacePanelState::default(),
            palette: PaletteState::default(),
            runner: None,
            session: SessionStoreState::default(),
            theme: ThemeSettingsState::default(),
            main_content_focus: MainContentFocus::default(),
            updater: UpdaterState::default(),
            crash_log: Vec::new(),
            unsaved_changes_prompt: None,
            active_collection_id: None,
            top_bar_mode: TopBarMode::default(),
            tree: TreeViewState::default(),
            create_collection_prompt: None,
            save_request_prompt: None,
            workspace_crud_dialog: None,
            panel_dragging: None,
            panel_hovered: None,
        }
    }

    /// Feature: tauri-to-iced-migration, Tarea 3.19 (Requisito 2.17).
    ///
    /// Test de integración (no property test): verifica el flujo Send
    /// completo contra un servidor HTTP mock local REAL (`wiremock`), sin
    /// mockear `request_executor` en sí. Cubre las dos mitades del
    /// requisito:
    ///
    /// 1. "Send invoca `resolve_request` + `execute` con el draft de la tab
    ///    activa": se llama a `execute_send` directamente (el cuerpo async
    ///    que `handle_send_pressed` dispara vía `Task::perform`) con un
    ///    draft cuya URL apunta al mock server, y se verifica con
    ///    `Mock::given(...).expect(1)` que el servidor efectivamente
    ///    recibió exactamente 1 request GET a `/ping` (si `execute_send`
    ///    no llamara a `request_executor.execute` con la request resuelta
    ///    correcta, esta expectativa fallaría al finalizar el test).
    /// 2. "la respuesta llega al Response_Inspector": se aplica el
    ///    `Result` devuelto por `execute_send` a un `Midway` de prueba vía
    ///    `handle_send_completed` (la misma función que procesa
    ///    `SendCompleted` en `update_request_composer`), y se verifica que
    ///    `tab.response` (el dato subyacente que lee el `Response_Inspector`)
    ///    queda poblado con los valores exactos devueltos por el mock
    ///    server.
    #[tokio::test]
    async fn send_flow_end_to_end_hits_mock_server_and_populates_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let expected_body = serde_json::json!({ "ok": true });

        Mock::given(method("GET"))
            .and(path("/ping"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(&expected_body)
                    .insert_header("x-midway-test", "1"),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let mut draft = create_blank_draft();
        draft.method = HttpMethod::GET;
        draft.url = format!("{}/ping", mock_server.uri());

        let app_state = Arc::new(build_test_app_state_async().await);

        let result = execute_send(
            Arc::clone(&app_state),
            draft.clone(),
            None,
            "test-execution-id".to_string(),
        )
        .await;

        // `MockServer` verifica en `drop` que el `.expect(1)` se cumplió
        // (exactamente 1 request GET a `/ping`); si `execute_send` no
        // hubiera invocado `request_executor.execute` con la request
        // resuelta correcta, el `drop` de abajo haría panic.
        let outcome = result.expect("execute_send debería devolver Ok contra el mock server");
        drop(mock_server);

        assert_eq!(outcome.response.status, 200);
        assert_eq!(
            outcome.response.body_text,
            serde_json::to_string(&expected_body).unwrap()
        );
        assert!(outcome
            .response
            .headers
            .iter()
            .any(|header| header.key.eq_ignore_ascii_case("x-midway-test") && header.value == "1"));

        // "la respuesta llega al Response_Inspector": mismo camino que
        // toma `update_request_composer` al recibir `SendCompleted`.
        let mut midway = build_test_midway_async(draft).await;
        let tab_id = midway.tabs[0].id.clone();

        handle_send_completed(&mut midway, tab_id, Ok(outcome));

        let response_outcome = midway.tabs[0]
            .response
            .as_ref()
            .expect("la tab debería tener una respuesta luego de handle_send_completed");
        assert_eq!(response_outcome.response.status, 200);
        assert_eq!(
            response_outcome.response.body_text,
            serde_json::to_string(&expected_body).unwrap()
        );
        assert!(midway.tabs[0].send_error.is_none());
        assert!(!midway.tabs[0].sending);
    }

    /// Feature: tauri-to-iced-migration, Tarea 3.19 (Requisito 2.17).
    ///
    /// Complemento al camino feliz: cuando `execute_send` falla (aquí, un
    /// puerto sin nada escuchando), `handle_send_completed` SHALL dejar
    /// `tab.response` en `None` y poblar `tab.send_error`, en vez de
    /// popular una respuesta inexistente en el `Response_Inspector`.
    #[tokio::test]
    async fn send_flow_end_to_end_reports_error_when_server_unreachable() {
        let mut draft = create_blank_draft();
        draft.method = HttpMethod::GET;
        // Puerto improbable de tener algo escuchando: fuerza un error de
        // conexión determinístico sin depender de timeouts largos.
        draft.url = "http://127.0.0.1:1/unreachable".to_string();
        draft.timeout_ms = 2_000;

        let app_state = Arc::new(build_test_app_state_async().await);

        let result = execute_send(
            Arc::clone(&app_state),
            draft.clone(),
            None,
            "test-execution-id-error".to_string(),
        )
        .await;

        let error_message = result.expect_err("una conexión rechazada debería devolver Err");
        assert!(!error_message.is_empty());

        let mut midway = build_test_midway_async(draft).await;
        let tab_id = midway.tabs[0].id.clone();

        handle_send_completed(&mut midway, tab_id, Err(error_message));

        assert!(midway.tabs[0].response.is_none());
        assert!(midway.tabs[0].send_error.is_some());
        assert!(!midway.tabs[0].sending);
    }

    // -----------------------------------------------------------------
    // Tarea 3.21 (Property 8, Requisito 2.18): fidelidad del preview
    // respecto al draft vigente.
    // -----------------------------------------------------------------

    /// Alfabeto seguro para keys/values generados en esta propiedad: a
    /// diferencia de `simple_token` en `curl.rs`, aquí basta con evitar la
    /// sintaxis de template `{{...}}` (para que `resolve_request` no tenga
    /// nada que interpolar y el oráculo sea un simple "eco" del draft), sin
    /// necesidad de evitar espacios/comillas/etc. (no se re-parsea nada).
    fn arb_preview_token() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9_-]{0,7}".prop_map(|s| s.to_string())
    }

    fn arb_key_value_row(id_prefix: &'static str) -> impl Strategy<Value = KeyValueRow> {
        (arb_preview_token(), arb_preview_token(), any::<bool>()).prop_map(
            move |(key, value, enabled)| KeyValueRow {
                id: format!("{id_prefix}-{key}"),
                key,
                value,
                enabled,
            },
        )
    }

    /// 0 a 3 filas de query params, cada una con su propio `enabled`
    /// independiente (cubre las filas deshabilitadas del Property 8).
    fn arb_key_value_rows(id_prefix: &'static str) -> impl Strategy<Value = Vec<KeyValueRow>> {
        prop::collection::vec(arb_key_value_row(id_prefix), 0..=3)
    }

    fn arb_auth_config() -> impl Strategy<Value = AuthConfig> {
        prop_oneof![
            Just(AuthConfig::None),
            arb_preview_token().prop_map(|token| AuthConfig::Bearer { token }),
            (arb_preview_token(), arb_preview_token())
                .prop_map(|(username, password)| AuthConfig::Basic { username, password }),
            (
                arb_preview_token(),
                arb_preview_token(),
                prop_oneof![Just(ApiKeyPlacement::Header), Just(ApiKeyPlacement::Query)],
            )
                .prop_map(|(key, value, placement)| AuthConfig::ApiKey {
                    key,
                    value,
                    placement,
                }),
        ]
    }

    fn arb_body_mode() -> impl Strategy<Value = RequestBodyDraft> {
        prop_oneof![
            Just(RequestBodyDraft {
                mode: BodyMode::None,
                value: String::new(),
                form_data: vec![],
            }),
            arb_preview_token().prop_map(|token| RequestBodyDraft {
                mode: BodyMode::Json,
                value: format!("{{\"field\":\"{token}\"}}"),
                form_data: vec![],
            }),
            arb_preview_token().prop_map(|value| RequestBodyDraft {
                mode: BodyMode::Text,
                value,
                form_data: vec![],
            }),
            (arb_preview_token(), arb_preview_token(), any::<bool>()).prop_map(
                |(key, value, enabled)| RequestBodyDraft {
                    mode: BodyMode::FormData,
                    value: String::new(),
                    form_data: vec![FormDataRow {
                        id: format!("form-{key}"),
                        key,
                        value,
                        enabled,
                        kind: FormDataFieldKind::Text,
                        file_name: None,
                    }],
                },
            ),
        ]
    }

    /// Genera un `RequestDraft` con MÚLTIPLES campos variados
    /// simultáneamente (método, query rows, header rows -incluyendo
    /// filas deshabilitadas-, auth y body), a diferencia de
    /// `non_empty_draft_strategy` (Tarea 3.6), que solo varía un campo por
    /// vez. Ninguno de los valores generados contiene sintaxis
    /// `{{...}}`, así que `resolve_request` nunca interpola ni resuelve
    /// secrets: el preview resultante SHALL ser un reflejo directo y
    /// determinístico del draft (Property 8).
    ///
    /// `environment_id` queda siempre en `None`: la fidelidad de la
    /// interpolación de environment ya está cubierta por los tests propios
    /// de `domain::interpolation` en `midway-core`.
    fn arb_request_draft() -> impl Strategy<Value = RequestDraft> {
        (
            curl_method_strategy(),
            curl_base_url_strategy(),
            arb_key_value_rows("query"),
            arb_key_value_rows("header"),
            arb_auth_config(),
            arb_body_mode(),
        )
            .prop_map(|(method, url, query, headers, auth, body)| RequestDraft {
                id: None,
                name: "Preview fidelity draft".to_string(),
                method,
                url,
                query,
                headers,
                auth,
                body,
                timeout_ms: 30_000,
                environment_id: None,
                response_tests: vec![],
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 30, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 8: El preview refleja
        /// exactamente el draft vigente.
        /// Validates: Requirements 2.18
        ///
        /// Para cualquier `RequestDraft` generado por `arb_request_draft`
        /// (combinando query params, headers -habilitados y
        /// deshabilitados-, auth y body), el `RequestPreview` devuelto por
        /// `compute_preview` (Tarea 3.20) SHALL ser exactamente el
        /// resultado de aplicar `domain::preview::make_preview` a la
        /// `Resolution` obtenida llamando directamente a
        /// `domain::interpolation::resolve_request` con los mismos
        /// argumentos (sin environment, sin secrets, `SecretRenderMode::
        /// Redact`) que usa `compute_preview` internamente. Se comparan
        /// ambos `RequestPreview` vía `serde_json::to_value` (no implementa
        /// `PartialEq`), lo que cubre método, `resolved_url`, `headers`
        /// (orden incluido), `body_text`, `curl_command`,
        /// `environment_name`, `used_secret_aliases` y
        /// `missing_secret_aliases` en una sola comparación.
        #[test]
        fn property_8_preview_matches_current_draft_exactly(draft in arb_request_draft()) {
            let app_state = Arc::new(build_test_app_state());

            let runtime = tokio::runtime::Runtime::new()
                .expect("no se pudo crear el runtime de tokio para el test de Property 8");

            let actual_preview = runtime
                .block_on(compute_preview(Arc::clone(&app_state), draft.clone(), None))
                .expect("compute_preview no debería fallar para un draft sin secrets/environment");

            let expected_resolution = resolve_request(
                &draft,
                None,
                &[],
                &BTreeMap::new(),
                SecretRenderMode::Redact,
            )
            .expect("resolve_request no debería fallar para un draft sin secrets/environment");
            let expected_preview = make_preview(expected_resolution);

            prop_assert_eq!(
                serde_json::to_value(&actual_preview).unwrap(),
                serde_json::to_value(&expected_preview).unwrap()
            );
        }
    }

    // -----------------------------------------------------------------
    // Tarea 5.5 (Property 11, Requisito 3.10): descarte de campos al
    // cambiar el tipo de autenticación.
    // -----------------------------------------------------------------

    /// Genera un `AuthKind` arbitrario (el tipo AL QUE se cambia), sobre
    /// `AuthKind::ALL` (Tarea 5.4): cubre las 4 variantes del `pick_list`
    /// de tipo de autenticación, incluyendo el caso "vuelve a None".
    fn arb_auth_kind() -> impl Strategy<Value = AuthKind> {
        prop_oneof![
            Just(AuthKind::None),
            Just(AuthKind::Bearer),
            Just(AuthKind::Basic),
            Just(AuthKind::ApiKey),
        ]
    }

    /// Default vacío de `AuthConfig` para un `AuthKind` dado: mismo mapeo
    /// que el implementado en `update_request_composer` para
    /// `AuthTypeChanged` (Tarea 5.4). Se reconstruye aquí como oráculo
    /// independiente en vez de reutilizar la implementación, para que el
    /// test siga siendo un chequeo real contra la especificación (Requisito
    /// 3.10) y no una tautología.
    fn empty_auth_config_for(kind: AuthKind) -> AuthConfig {
        match kind {
            AuthKind::None => AuthConfig::None,
            AuthKind::Bearer => AuthConfig::Bearer {
                token: String::new(),
            },
            AuthKind::Basic => AuthConfig::Basic {
                username: String::new(),
                password: String::new(),
            },
            AuthKind::ApiKey => AuthConfig::ApiKey {
                key: String::new(),
                value: String::new(),
                placement: ApiKeyPlacement::Header,
            },
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 11: Cambiar el tipo
        /// de autenticación descarta los campos anteriores.
        /// Validates: Requirements 3.10
        ///
        /// Para cualquier `AuthConfig` inicial (`arb_auth_config`, Tarea
        /// 3.21: incluye Bearer/Basic/ApiKey con campos no vacíos) y
        /// cualquier `AuthKind` destino (`arb_auth_kind`, incluyendo el
        /// mismo tipo que el inicial), despachar
        /// `AuthTypeChanged(target_kind)` SHALL dejar `draft.auth` como
        /// EXACTAMENTE el default vacío de `target_kind`, sin preservar
        /// ninguno de los valores de campo de la variante anterior
        /// (Requisito 3.10). En particular, esto cubre el caso "cambiar al
        /// mismo tipo actual", que también debe limpiar los campos.
        #[test]
        fn property_11_auth_type_change_discards_previous_fields(
            starting_auth in arb_auth_config(),
            target_kind in arb_auth_kind(),
        ) {
            let mut draft = create_blank_draft();
            draft.auth = starting_auth;

            let mut midway = build_test_midway(draft);

            let _ = update_request_composer(
                &mut midway,
                RequestComposerMessage::AuthTypeChanged(target_kind),
            );

            let expected_auth = empty_auth_config_for(target_kind);
            prop_assert_eq!(midway.tabs[0].draft.auth.clone(), expected_auth);
        }
    }

    // -----------------------------------------------------------------
    // Tarea 5.3 (Property 9, Requisitos 3.2, 3.3): selección de tab por
    // defecto según método HTTP, salvo override manual.
    // -----------------------------------------------------------------

    /// Genera un `HttpMethod` arbitrario cubriendo las 7 variantes
    /// soportadas (`HttpMethod::ALL`, en `midway-core`), a diferencia de
    /// `curl_method_strategy` (que solo cubre GET/POST/PUT/DELETE, pensado
    /// para el enrutamiento de pegado de cURL, Property 2/3). Esta
    /// propiedad SÍ necesita cubrir HEAD/OPTIONS/PATCH, ya que forman
    /// parte del mapeo método -> tab por defecto (Requisito 3.2).
    fn arb_http_method() -> impl Strategy<Value = HttpMethod> {
        prop_oneof![
            Just(HttpMethod::GET),
            Just(HttpMethod::HEAD),
            Just(HttpMethod::OPTIONS),
            Just(HttpMethod::POST),
            Just(HttpMethod::PUT),
            Just(HttpMethod::PATCH),
            Just(HttpMethod::DELETE),
        ]
    }

    /// Genera una `RequestTab` arbitraria, cubriendo las 5 variantes, para
    /// usarse como tab activa ANTES del cambio de método (relevante para
    /// verificar que un `manual_tab_override` en `true` deja la tab activa
    /// sin cambios, sea cual sea).
    fn arb_request_tab() -> impl Strategy<Value = RequestTab> {
        prop_oneof![
            Just(RequestTab::Params),
            Just(RequestTab::Headers),
            Just(RequestTab::Auth),
            Just(RequestTab::Body),
            Just(RequestTab::Tests),
        ]
    }

    /// Oráculo independiente del mapeo método -> tab por defecto
    /// (Requisito 3.2): se reconstruye aquí a mano en vez de llamar a
    /// `default_tab_for_method` directamente, para que el test sea un
    /// chequeo real contra la especificación y no una tautología.
    fn expected_default_tab_for(method: HttpMethod) -> RequestTab {
        match method {
            HttpMethod::GET | HttpMethod::HEAD | HttpMethod::OPTIONS => RequestTab::Params,
            HttpMethod::POST | HttpMethod::PUT | HttpMethod::PATCH | HttpMethod::DELETE => {
                RequestTab::Body
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 9: Selección de tab
        /// por defecto según método HTTP, salvo override manual.
        /// Validates: Requirements 3.2, 3.3
        ///
        /// Para cualquier método HTTP soportado y cualquier valor del flag
        /// `manual_tab_override`: si `manual_tab_override` es falso, la tab
        /// de configuración seleccionada por defecto tras despachar
        /// `MethodChanged(target_method)` SHALL ser Params cuando el método
        /// es GET, HEAD u OPTIONS, y Body cuando el método es POST, PUT,
        /// PATCH o DELETE (Requisito 3.2); si `manual_tab_override` es
        /// verdadero, la tab activa SHALL permanecer sin cambios al
        /// cambiar el método (Requisito 3.3).
        #[test]
        fn property_9_default_tab_selection_by_method_unless_manual_override(
            starting_tab in arb_request_tab(),
            manual_tab_override in proptest::bool::ANY,
            target_method in arb_http_method(),
        ) {
            let mut midway = build_test_midway(create_blank_draft());
            midway.tabs[0].active_request_tab = starting_tab;
            midway.tabs[0].manual_tab_override = manual_tab_override;

            let _ = update_request_composer(
                &mut midway,
                RequestComposerMessage::MethodChanged(target_method),
            );

            // El método del draft siempre se actualiza, sea cual sea el
            // estado de `manual_tab_override`.
            prop_assert_eq!(midway.tabs[0].draft.method, target_method);

            if manual_tab_override {
                // Requisito 3.3: override manual activo -> la tab activa
                // permanece sin cambios, sea la que sea.
                prop_assert_eq!(midway.tabs[0].active_request_tab, starting_tab);
            } else {
                // Requisito 3.2: sin override -> se reemplaza por la tab
                // por defecto del método destino, según el oráculo
                // independiente.
                prop_assert_eq!(
                    midway.tabs[0].active_request_tab,
                    expected_default_tab_for(target_method)
                );
            }
        }
    }

    // -----------------------------------------------------------------
    // Tarea 5.7 (Property 10, Requisito 3.8): CRUD de filas key/value
    // equivale a un modelo de referencia.
    // -----------------------------------------------------------------

    /// Modelo de referencia de una fila key/value: `(id, key, value,
    /// enabled)`. Se usa una tupla en vez de reutilizar `KeyValueRow` para
    /// que el modelo de referencia sea una implementación independiente de
    /// la especificación (Requisito 3.8), no un alias del tipo real.
    type RefRow = (String, String, String, bool);

    /// Agrega una fila nueva al modelo de referencia: mismo default que
    /// `RequestComposerMessage::KeyValueRowAdded` (key/value vacíos,
    /// `enabled = true`).
    fn ref_add(rows: &mut Vec<RefRow>, new_id: String) {
        rows.push((new_id, String::new(), String::new(), true));
    }

    /// Edita el `key` de la fila con el `id` dado, si existe.
    fn ref_set_key(rows: &mut [RefRow], id: &str, key: String) {
        if let Some(row) = rows.iter_mut().find(|row| row.0 == id) {
            row.1 = key;
        }
    }

    /// Edita el `value` de la fila con el `id` dado, si existe.
    fn ref_set_value(rows: &mut [RefRow], id: &str, value: String) {
        if let Some(row) = rows.iter_mut().find(|row| row.0 == id) {
            row.2 = value;
        }
    }

    /// Alterna el `enabled` de la fila con el `id` dado, si existe.
    fn ref_toggle_enabled(rows: &mut [RefRow], id: &str) {
        if let Some(row) = rows.iter_mut().find(|row| row.0 == id) {
            row.3 = !row.3;
        }
    }

    /// Elimina la fila con el `id` dado, si existe.
    fn ref_remove(rows: &mut Vec<RefRow>, id: &str) {
        rows.retain(|row| row.0 != id);
    }

    /// Operación abstracta sobre el editor de filas key/value. Las
    /// variantes de edición/toggle/remove NO llevan un `id`/índice de
    /// generación: el "id" real de cada fila se genera en tiempo de
    /// ejecución (vía `uuid::Uuid::new_v4()` dentro del handler real), así
    /// que la fila objetivo se resuelve en tiempo de APLICACIÓN, por
    /// posición dentro de las filas existentes (`operation_index %
    /// current_row_count`), de forma idéntica en ambos lados (real y
    /// modelo de referencia). Si no hay ninguna fila existente, la
    /// operación se omite en ambos lados por igual.
    #[derive(Debug, Clone)]
    enum AbstractOp {
        Add,
        EditKey(String),
        EditValue(String),
        Toggle,
        Remove,
    }

    fn arb_abstract_op() -> impl Strategy<Value = AbstractOp> {
        prop_oneof![
            Just(AbstractOp::Add),
            arb_preview_token().prop_map(AbstractOp::EditKey),
            arb_preview_token().prop_map(AbstractOp::EditValue),
            Just(AbstractOp::Toggle),
            Just(AbstractOp::Remove),
        ]
    }

    fn arb_abstract_ops() -> impl Strategy<Value = Vec<AbstractOp>> {
        prop::collection::vec(arb_abstract_op(), 0..=30)
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 10: CRUD de filas
        /// key/value equivale a un modelo de referencia.
        /// Validates: Requirements 3.8
        ///
        /// Para cualquier secuencia de operaciones de agregar, editar
        /// (key/value), alternar `enabled` y eliminar sobre las filas
        /// key/value (Tarea 5.6), aplicar esas operaciones a través de los
        /// mensajes del editor (`update_request_composer`, usando
        /// `KeyValueTarget::Query`) SHALL producir el mismo resultado
        /// (mismo número de filas, mismo contenido en cada posición) que
        /// aplicar las operaciones equivalentes a un modelo de referencia
        /// simple (`Vec<RefRow>` con las funciones `ref_*` de arriba),
        /// donde la fila objetivo de cada operación de edición/toggle/
        /// remove se resuelve por posición entre las filas existentes de
        /// forma idéntica en ambos lados.
        #[test]
        fn property_10_key_value_crud_matches_reference_model(ops in arb_abstract_ops()) {
            let mut midway = build_test_midway(create_blank_draft());
            let mut reference_rows: Vec<RefRow> = Vec::new();
            // IDs (por posición de inserción) de las filas reales creadas
            // hasta el momento, en el mismo orden en que se agregaron al
            // modelo de referencia. Como ninguna de las 4 operaciones
            // reordena las filas (`push` para agregar, `retain` para
            // eliminar, mutación in-place para editar/alternar), esta
            // lista y `reference_rows` siempre corresponden fila a fila
            // por posición tras cada operación.
            let mut known_ids: Vec<String> = Vec::new();

            for (operation_index, op) in ops.into_iter().enumerate() {
                match op {
                    AbstractOp::Add => {
                        let _ = update_request_composer(
                            &mut midway,
                            RequestComposerMessage::KeyValueRowAdded(KeyValueTarget::Query),
                        );
                        // El id real recién generado es el único presente
                        // en `draft.query` que todavía no está en
                        // `known_ids` (dado que las filas nunca se
                        // reordenan, es la última).
                        let new_id = midway.tabs[0]
                            .draft
                            .query
                            .last()
                            .expect("KeyValueRowAdded debe agregar una fila")
                            .id
                            .clone();
                        known_ids.push(new_id.clone());
                        ref_add(&mut reference_rows, new_id);
                    }
                    AbstractOp::EditKey(new_key) => {
                        if !known_ids.is_empty() {
                            let target_id = known_ids[operation_index % known_ids.len()].clone();
                            let _ = update_request_composer(
                                &mut midway,
                                RequestComposerMessage::KeyValueRowKeyChanged {
                                    target: KeyValueTarget::Query,
                                    row_id: target_id.clone(),
                                    key: new_key.clone(),
                                },
                            );
                            ref_set_key(&mut reference_rows, &target_id, new_key);
                        }
                    }
                    AbstractOp::EditValue(new_value) => {
                        if !known_ids.is_empty() {
                            let target_id = known_ids[operation_index % known_ids.len()].clone();
                            let _ = update_request_composer(
                                &mut midway,
                                RequestComposerMessage::KeyValueRowValueChanged {
                                    target: KeyValueTarget::Query,
                                    row_id: target_id.clone(),
                                    value: new_value.clone(),
                                },
                            );
                            ref_set_value(&mut reference_rows, &target_id, new_value);
                        }
                    }
                    AbstractOp::Toggle => {
                        if !known_ids.is_empty() {
                            let target_id = known_ids[operation_index % known_ids.len()].clone();
                            let _ = update_request_composer(
                                &mut midway,
                                RequestComposerMessage::KeyValueRowEnabledToggled {
                                    target: KeyValueTarget::Query,
                                    row_id: target_id.clone(),
                                },
                            );
                            ref_toggle_enabled(&mut reference_rows, &target_id);
                        }
                    }
                    AbstractOp::Remove => {
                        if !known_ids.is_empty() {
                            let target_id = known_ids[operation_index % known_ids.len()].clone();
                            let _ = update_request_composer(
                                &mut midway,
                                RequestComposerMessage::KeyValueRowRemoved {
                                    target: KeyValueTarget::Query,
                                    row_id: target_id.clone(),
                                },
                            );
                            ref_remove(&mut reference_rows, &target_id);
                            known_ids.retain(|id| id != &target_id);
                        }
                    }
                }
            }

            let actual_rows = &midway.tabs[0].draft.query;
            prop_assert_eq!(actual_rows.len(), reference_rows.len());
            for (actual_row, reference_row) in actual_rows.iter().zip(reference_rows.iter()) {
                prop_assert_eq!(&actual_row.id, &reference_row.0);
                prop_assert_eq!(&actual_row.key, &reference_row.1);
                prop_assert_eq!(&actual_row.value, &reference_row.2);
                prop_assert_eq!(actual_row.enabled, reference_row.3);
            }
        }
    }

    // -----------------------------------------------------------------
    // Tarea 5.9 (Requisito 3.9): tests unitarios del editor de la tab
    // Tests sobre el motor de assertions EXISTENTE
    // (`domain::testing::evaluate_response_assertions`).
    //
    // Estos tests NO reimplementan la lógica de evaluación: construyen un
    // `ResponseAssertion` despachando los mensajes reales del editor
    // (Tarea 5.8: `AssertionAdded` + `AssertionSourceChanged` +
    // `AssertionOperatorChanged` + `AssertionSelectorChanged` (si aplica) +
    // `AssertionExpectedChanged`) y luego pasan ese valor, sin modificarlo,
    // a la función real e inmodificada `evaluate_response_assertions`
    // (`midway-core/src/domain/testing.rs`) junto con un `ResponseEnvelope`
    // de prueba armado a mano. Esto verifica la integración
    // editor -> motor de evaluación, no el motor en sí (que ya tiene su
    // propia responsabilidad y no se toca aquí).
    // -----------------------------------------------------------------

    /// Construye un `ResponseEnvelope` de prueba mínimo. Los campos no
    /// relevantes para el caso (`status_text`, `duration_ms`,
    /// `size_bytes`, `received_at`) se rellenan con valores neutros, ya
    /// que ninguna combinación source/operator ejercitada aquí los usa.
    fn build_test_response(
        status: u16,
        headers: Vec<midway_core::domain::http::ResolvedPair>,
        body_text: &str,
        final_url: &str,
    ) -> ResponseEnvelope {
        ResponseEnvelope {
            status,
            status_text: "OK".to_string(),
            headers,
            body_text: body_text.to_string(),
            duration_ms: 10,
            size_bytes: body_text.len() as u64,
            final_url: final_url.to_string(),
            received_at: "2024-01-01T00:00:00Z".to_string(),
            truncated: false,
            body_evicted: false,
            total_size_bytes: None,
        }
    }

    // -----------------------------------------------------------------
    // Eviction de bodies de respuestas de tabs inactivas.
    //
    // Cada tab retenía su `body_text` completo mientras siguiera abierta,
    // así que el consumo de memoria crecía de forma lineal con la cantidad
    // de tabs con respuesta. `evict_inactive_response_bodies` libera los
    // payloads de las tabs que no son la que acaba de recibir respuesta,
    // preservando la metadata para seguir mostrando el resumen.
    // -----------------------------------------------------------------

    /// Construye una tab con una respuesta ya cargada y un body del largo
    /// indicado, para ejercitar la eviction sin pasar por la red.
    fn tab_with_response_body(body: &str) -> RequestTabState {
        let mut tab = RequestTabState::blank();
        tab.response = Some(ResponseOutcome {
            response: build_test_response(200, Vec::new(), body, "https://example.com"),
            assertions: AssertionReport {
                total: 0,
                passed: 0,
                failed: 0,
                results: Vec::new(),
            },
        });
        tab
    }

    /// La tab que acaba de recibir respuesta conserva su body; las demás lo
    /// liberan y quedan marcadas con `body_evicted`.
    #[test]
    fn eviction_frees_other_tabs_bodies_and_keeps_the_active_one() {
        let mut tabs = vec![
            tab_with_response_body("body-de-la-tab-0"),
            tab_with_response_body("body-de-la-tab-1"),
            tab_with_response_body("body-de-la-tab-2"),
        ];

        evict_inactive_response_bodies(&mut tabs, 1);

        let kept = tabs[1].response.as_ref().expect("la tab 1 tiene respuesta");
        assert_eq!(kept.response.body_text, "body-de-la-tab-1");
        assert!(!kept.response.body_evicted);

        for index in [0usize, 2] {
            let evicted = tabs[index]
                .response
                .as_ref()
                .expect("la tab conserva la metadata de su respuesta");
            assert!(
                evicted.response.body_text.is_empty(),
                "el body de la tab {index} debería liberarse"
            );
            assert!(evicted.response.body_evicted);
        }
    }

    /// La eviction preserva la metadata de la respuesta (status, tiempo,
    /// headers) y recuerda el tamaño original en `total_size_bytes`, para que
    /// la UI pueda seguir mostrando el resumen y decir cuánto pesaba el body.
    #[test]
    fn eviction_preserves_response_metadata_and_original_size() {
        let body = "x".repeat(4096);
        let mut tabs = vec![
            tab_with_response_body(&body),
            tab_with_response_body("otro"),
        ];

        evict_inactive_response_bodies(&mut tabs, 1);

        let evicted = tabs[0].response.as_ref().expect("metadata preservada");
        assert_eq!(evicted.response.status, 200);
        assert_eq!(evicted.response.duration_ms, 10);
        assert_eq!(evicted.response.final_url, "https://example.com");
        assert_eq!(
            evicted.response.total_size_bytes,
            Some(4096),
            "debe recordar cuánto pesaba el body liberado"
        );
    }

    /// Las tabs sin respuesta no se ven afectadas.
    #[test]
    fn eviction_ignores_tabs_without_response() {
        let mut tabs = vec![RequestTabState::blank(), tab_with_response_body("activo")];

        evict_inactive_response_bodies(&mut tabs, 1);

        assert!(tabs[0].response.is_none());
        assert_eq!(
            tabs[1]
                .response
                .as_ref()
                .expect("la tab activa conserva su respuesta")
                .response
                .body_text,
            "activo"
        );
    }

    /// Despacha `AssertionAdded` (Tarea 5.8) sobre el `Midway` de prueba y
    /// devuelve el `id` generado de la assertion agregada, para poder
    /// dirigirle los mensajes de configuración subsiguientes igual que lo
    /// haría la UI real de la tab Tests.
    fn add_test_assertion(midway: &mut Midway) -> String {
        let _ = update_request_composer(midway, RequestComposerMessage::AssertionAdded);
        midway.tabs[0]
            .draft
            .response_tests
            .last()
            .expect("AssertionAdded debe agregar una assertion al final de response_tests")
            .id
            .clone()
    }

    /// Configura la assertion identificada por `assertion_id` despachando
    /// únicamente los mensajes del editor de la tab Tests (Tarea 5.8):
    /// `AssertionSourceChanged`, `AssertionOperatorChanged`,
    /// `AssertionSelectorChanged` (solo si se pasa `Some`) y
    /// `AssertionExpectedChanged`. Deliberadamente NO construye el
    /// `ResponseAssertion` por literal de struct: el objetivo de la Tarea
    /// 5.9 es verificar que este camino de mensajes (el que usa la UI
    /// real) produce el valor correcto.
    fn configure_test_assertion(
        midway: &mut Midway,
        assertion_id: &str,
        source: AssertionSource,
        operator: AssertionOperator,
        selector: Option<&str>,
        expected: &str,
    ) {
        let _ = update_request_composer(
            midway,
            RequestComposerMessage::AssertionSourceChanged {
                assertion_id: assertion_id.to_string(),
                source,
            },
        );
        let _ = update_request_composer(
            midway,
            RequestComposerMessage::AssertionOperatorChanged {
                assertion_id: assertion_id.to_string(),
                operator,
            },
        );
        if let Some(selector_value) = selector {
            let _ = update_request_composer(
                midway,
                RequestComposerMessage::AssertionSelectorChanged {
                    assertion_id: assertion_id.to_string(),
                    selector: selector_value.to_string(),
                },
            );
        }
        let _ = update_request_composer(
            midway,
            RequestComposerMessage::AssertionExpectedChanged {
                assertion_id: assertion_id.to_string(),
                expected: expected.to_string(),
            },
        );
    }

    /// Combinación representativa: `Status` + `Equals`, sin selector
    /// (Status no lo necesita). El editor debe producir una assertion que,
    /// evaluada por el motor real contra una respuesta con `status = 200`,
    /// pasa.
    #[test]
    fn tests_tab_editor_status_equals_passes_via_real_engine() {
        let mut midway = build_test_midway(create_blank_draft());
        let assertion_id = add_test_assertion(&mut midway);
        configure_test_assertion(
            &mut midway,
            &assertion_id,
            AssertionSource::Status,
            AssertionOperator::Equals,
            None,
            "200",
        );

        let assertion = midway.tabs[0].draft.response_tests[0].clone();
        assert_eq!(assertion.source, AssertionSource::Status);
        assert_eq!(assertion.operator, AssertionOperator::Equals);
        assert_eq!(assertion.selector, None);
        assert_eq!(assertion.expected, "200");

        let response = build_test_response(200, vec![], "", "https://example.com");
        let report = evaluate_response_assertions(&response, &[assertion]);

        assert_eq!(report.total, 1);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
        assert!(report.results[0].passed);
    }

    /// Combinación representativa: `Header` + `Exists`, con selector (el
    /// nombre del header). El editor debe producir una assertion que,
    /// evaluada por el motor real contra una respuesta que sí trae ese
    /// header, pasa.
    #[test]
    fn tests_tab_editor_header_exists_passes_via_real_engine() {
        let mut midway = build_test_midway(create_blank_draft());
        let assertion_id = add_test_assertion(&mut midway);
        configure_test_assertion(
            &mut midway,
            &assertion_id,
            AssertionSource::Header,
            AssertionOperator::Exists,
            Some("Content-Type"),
            "",
        );

        let assertion = midway.tabs[0].draft.response_tests[0].clone();
        assert_eq!(assertion.source, AssertionSource::Header);
        assert_eq!(assertion.operator, AssertionOperator::Exists);
        assert_eq!(assertion.selector, Some("Content-Type".to_string()));

        let response = build_test_response(
            200,
            vec![midway_core::domain::http::ResolvedPair {
                key: "Content-Type".to_string(),
                value: "application/json".to_string(),
            }],
            "",
            "https://example.com",
        );
        let report = evaluate_response_assertions(&response, &[assertion]);

        assert_eq!(report.total, 1);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
        assert!(report.results[0].passed);
    }

    /// Combinación representativa: `BodyText` + `Contains`, sin selector.
    /// El editor debe producir una assertion que, evaluada por el motor
    /// real contra una respuesta cuyo body contiene el texto esperado,
    /// pasa.
    #[test]
    fn tests_tab_editor_body_text_contains_passes_via_real_engine() {
        let mut midway = build_test_midway(create_blank_draft());
        let assertion_id = add_test_assertion(&mut midway);
        configure_test_assertion(
            &mut midway,
            &assertion_id,
            AssertionSource::BodyText,
            AssertionOperator::Contains,
            None,
            "hello",
        );

        let assertion = midway.tabs[0].draft.response_tests[0].clone();
        assert_eq!(assertion.source, AssertionSource::BodyText);
        assert_eq!(assertion.operator, AssertionOperator::Contains);
        assert_eq!(assertion.selector, None);
        assert_eq!(assertion.expected, "hello");

        let response = build_test_response(200, vec![], "hello world", "https://example.com");
        let report = evaluate_response_assertions(&response, &[assertion]);

        assert_eq!(report.total, 1);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
        assert!(report.results[0].passed);
    }

    /// Combinación representativa: `JsonPointer` + `Equals`, con selector
    /// (el JSON Pointer). El editor debe producir una assertion que,
    /// evaluada por el motor real contra una respuesta cuyo body es JSON y
    /// contiene el valor esperado en ese pointer, pasa.
    #[test]
    fn tests_tab_editor_json_pointer_equals_passes_via_real_engine() {
        let mut midway = build_test_midway(create_blank_draft());
        let assertion_id = add_test_assertion(&mut midway);
        configure_test_assertion(
            &mut midway,
            &assertion_id,
            AssertionSource::JsonPointer,
            AssertionOperator::Equals,
            Some("/name"),
            "Ana",
        );

        let assertion = midway.tabs[0].draft.response_tests[0].clone();
        assert_eq!(assertion.source, AssertionSource::JsonPointer);
        assert_eq!(assertion.operator, AssertionOperator::Equals);
        assert_eq!(assertion.selector, Some("/name".to_string()));
        assert_eq!(assertion.expected, "Ana");

        let response = build_test_response(200, vec![], r#"{"name":"Ana"}"#, "https://example.com");
        let report = evaluate_response_assertions(&response, &[assertion]);

        assert_eq!(report.total, 1);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
        assert!(report.results[0].passed);
    }

    /// Combinación representativa: `FinalUrl` + `NotContains`, sin
    /// selector. A diferencia de los casos anteriores, esta respuesta se
    /// arma deliberadamente para que la assertion FALLE (el `final_url` SÍ
    /// contiene el texto que se espera que NO contenga), para verificar
    /// también el camino de fallo end-to-end (editor -> motor real).
    #[test]
    fn tests_tab_editor_final_url_not_contains_fails_via_real_engine() {
        let mut midway = build_test_midway(create_blank_draft());
        let assertion_id = add_test_assertion(&mut midway);
        configure_test_assertion(
            &mut midway,
            &assertion_id,
            AssertionSource::FinalUrl,
            AssertionOperator::NotContains,
            None,
            "localhost",
        );

        let assertion = midway.tabs[0].draft.response_tests[0].clone();
        assert_eq!(assertion.source, AssertionSource::FinalUrl);
        assert_eq!(assertion.operator, AssertionOperator::NotContains);
        assert_eq!(assertion.selector, None);
        assert_eq!(assertion.expected, "localhost");

        let response = build_test_response(200, vec![], "", "http://localhost:8080/api");
        let report = evaluate_response_assertions(&response, &[assertion]);

        assert_eq!(report.total, 1);
        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 1);
        assert!(!report.results[0].passed);
    }

    /// Caso límite (Requisito 3.9, "selector opcional"): el usuario deja el
    /// campo de selector vacío en una fuente que SÍ lo requiere (`Header`).
    /// Despachar `AssertionSelectorChanged` con un string vacío debe
    /// traducirse a `selector = None` en el dominio (semántica implementada
    /// en la Tarea 5.8), y el motor real (`extract_actual_value` en
    /// `domain/testing.rs`) debe reportar la assertion como fallida al no
    /// poder resolver un valor sin selector.
    #[test]
    fn tests_tab_editor_header_empty_selector_becomes_none_and_fails_via_real_engine() {
        let mut midway = build_test_midway(create_blank_draft());
        let assertion_id = add_test_assertion(&mut midway);
        configure_test_assertion(
            &mut midway,
            &assertion_id,
            AssertionSource::Header,
            AssertionOperator::Exists,
            Some("X-Test"),
            "",
        );

        // El usuario borra el selector que había escrito: string vacío.
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::AssertionSelectorChanged {
                assertion_id: assertion_id.clone(),
                selector: String::new(),
            },
        );

        let assertion = midway.tabs[0].draft.response_tests[0].clone();
        assert_eq!(assertion.selector, None);

        let response = build_test_response(
            200,
            vec![midway_core::domain::http::ResolvedPair {
                key: "X-Test".to_string(),
                value: "value".to_string(),
            }],
            "",
            "https://example.com",
        );
        let report = evaluate_response_assertions(&response, &[assertion]);

        assert_eq!(report.total, 1);
        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 1);
        assert!(!report.results[0].passed);
        assert!(report.results[0].message.contains("selector"));
    }

    // -----------------------------------------------------------------
    // Tarea 7.3 (Requisito 4.2): property test de invariantes de CRUD de
    // environments, centrado en `validate_environment_name` (Tarea 7.2),
    // el núcleo puro (sin I/O) de la validación local que se ejecuta antes
    // de invocar `save_environment` vía `SqliteRepository`. El flujo
    // asíncrono de guardado/borrado en sí delega en el repositorio real y
    // ya está cubierto por los tests propios de `midway-core` y por el
    // patrón de integración async usado en otras partes de este módulo;
    // esta property test verifica las invariantes de la Property 12 tal
    // como se manifiestan en el resultado de `validate_environment_name`
    // para cualquier combinación de environments existentes, nombre
    // candidato y modo (crear/editar).
    // -----------------------------------------------------------------

    /// Candidato de nombre a validar: la mayoría de los casos son un token
    /// "fresco" (posiblemente distinto de cualquier nombre existente),
    /// pero una fracción de los casos deliberadamente reutiliza el nombre
    /// de un environment ya existente (para ejercitar el rechazo de
    /// duplicados) y otra fracción genera un nombre de más de
    /// `MAX_ENVIRONMENT_NAME_LEN` caracteres (para ejercitar el rechazo
    /// por longitud).
    #[derive(Debug, Clone)]
    enum EnvironmentNameCandidate {
        Fresh(String),
        TooLong(String),
        DuplicateOfExisting(usize),
    }

    fn arb_environment_name_candidate() -> impl Strategy<Value = EnvironmentNameCandidate> {
        prop_oneof![
            3 => arb_preview_token().prop_map(EnvironmentNameCandidate::Fresh),
            1 => (101usize..=150usize).prop_map(|len| EnvironmentNameCandidate::TooLong("a".repeat(len))),
            2 => (0usize..20usize).prop_map(EnvironmentNameCandidate::DuplicateOfExisting),
        ]
    }

    /// Si se valida como creación (`editing_id = None`) o como edición de
    /// un environment existente (`editing_id = Some(id)`, resuelto por
    /// índice módulo el tamaño de la lista de environments existentes).
    #[derive(Debug, Clone)]
    enum EditingChoice {
        Create,
        EditExisting(usize),
    }

    fn arb_editing_choice() -> impl Strategy<Value = EditingChoice> {
        prop_oneof![
            2 => Just(EditingChoice::Create),
            1 => (0usize..20usize).prop_map(EditingChoice::EditExisting),
        ]
    }

    /// Lista de nombres de environments existentes, cada uno único dentro
    /// del conjunto generado (usando `hash_set` para garantizar la
    /// unicidad, dado que dos environments con el mismo nombre no podrían
    /// coexistir bajo las invariantes de la Property 12). La mayoría de
    /// los casos usan una lista pequeña (0 a 20), pero una fracción de los
    /// casos genera una lista cercana al límite `MAX_ENVIRONMENTS` (98 a
    /// 102 nombres) para ejercitar también el rechazo por límite de
    /// tamaño al crear un environment nuevo.
    fn arb_existing_environment_names() -> impl Strategy<Value = Vec<String>> {
        prop_oneof![
            3 => prop::collection::hash_set(arb_preview_token(), 0..=20)
                .prop_map(|set| set.into_iter().collect::<Vec<String>>()),
            1 => prop::collection::hash_set(arb_preview_token(), 98..=102)
                .prop_map(|set| set.into_iter().collect::<Vec<String>>()),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 12: Invariantes de
        /// CRUD de environments.
        /// Validates: Requirements 4.2
        ///
        /// Para cualquier lista de environments existentes (con nombres
        /// únicos entre sí), cualquier nombre candidato (fresco, duplicado
        /// de uno existente, o excesivamente largo) y cualquier modo
        /// (crear un environment nuevo o editar uno existente),
        /// `validate_environment_name` SHALL sostener las invariantes de
        /// la Property 12 sobre su resultado:
        ///
        /// - Si devuelve `Ok(name)`: `name` es el candidato "trimeado", no
        ///   vacío, de a lo sumo `MAX_ENVIRONMENT_NAME_LEN` caracteres, y
        ///   ningún OTRO environment existente (excluyendo el que se está
        ///   editando, si aplica) tiene ese mismo nombre — es decir, si la
        ///   validación pasa, el conjunto resultante (tras aplicar el
        ///   guardado) seguiría sin duplicados y dentro del límite de
        ///   longitud.
        /// - Si devuelve `Err(_)`: esto es consistente con un oráculo
        ///   independiente que recalcula, sin invocar la lógica real de
        ///   `validate_environment_name`, si la validación DEBERÍA haber
        ///   fallado (nombre vacío tras `trim`, o demasiado largo, o
        ///   duplicado de otro environment excluyendo el propio en
        ///   edición, o creación de un environment nuevo cuando ya se
        ///   alcanzó `MAX_ENVIRONMENTS`).
        #[test]
        fn property_12_environment_name_validation_invariants(
            existing_names in arb_existing_environment_names(),
            candidate in arb_environment_name_candidate(),
            editing_choice in arb_editing_choice(),
        ) {
            let environments: Vec<EnvironmentRecord> = existing_names
                .iter()
                .enumerate()
                .map(|(index, name)| EnvironmentRecord {
                    id: format!("env-{index}"),
                    name: name.clone(),
                    variables: vec![],
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                })
                .collect();

            let raw_name = match &candidate {
                EnvironmentNameCandidate::Fresh(token) => token.clone(),
                EnvironmentNameCandidate::TooLong(long_name) => long_name.clone(),
                EnvironmentNameCandidate::DuplicateOfExisting(index) => {
                    if environments.is_empty() {
                        // No hay ningún nombre existente que duplicar; se
                        // degrada a un nombre fresco arbitrario para no
                        // desperdiciar el caso generado.
                        "no-existing-to-duplicate".to_string()
                    } else {
                        environments[index % environments.len()].name.clone()
                    }
                }
            };

            let editing_id: Option<String> = match editing_choice {
                EditingChoice::Create => None,
                EditingChoice::EditExisting(index) => {
                    if environments.is_empty() {
                        None
                    } else {
                        Some(environments[index % environments.len()].id.clone())
                    }
                }
            };

            // Oráculo independiente (NO reimplementa `validate_environment_name`
            // llamando a la misma lógica; recalcula cada condición por
            // separado a partir de la definición del Requisito 4.2/4.9).
            let trimmed = raw_name.trim().to_string();
            let oracle_empty = trimmed.is_empty();
            let oracle_too_long = trimmed.chars().count() > MAX_ENVIRONMENT_NAME_LEN;
            let oracle_duplicate = environments.iter().any(|environment| {
                environment.name == trimmed && Some(environment.id.as_str()) != editing_id.as_deref()
            });
            let oracle_at_limit = editing_id.is_none() && environments.len() >= MAX_ENVIRONMENTS;
            let oracle_should_fail = oracle_empty || oracle_too_long || oracle_duplicate || oracle_at_limit;

            let result = validate_environment_name(&environments, editing_id.as_deref(), &raw_name);

            match result {
                Ok(name) => {
                    prop_assert!(!oracle_should_fail);
                    prop_assert_eq!(&name, &trimmed);
                    prop_assert!(!name.is_empty());
                    prop_assert!(name.chars().count() <= MAX_ENVIRONMENT_NAME_LEN);
                    let other_has_same_name = environments.iter().any(|environment| {
                        environment.name == name && Some(environment.id.as_str()) != editing_id.as_deref()
                    });
                    prop_assert!(!other_has_same_name);
                }
                Err(_) => {
                    prop_assert!(oracle_should_fail);
                }
            }
        }
    }

    // -----------------------------------------------------------------
    // Tarea 7.4 (Requisito 4.9): property test end-to-end (a través de
    // `handle_environment_submitted`, no solo de `validate_environment_name`
    // como en la Property 12/Tarea 7.3) del rechazo de un nombre de
    // environment duplicado. A diferencia de la Property 12 -que ejercita
    // duplicados solo en una fracción de los casos generados-, esta
    // property GARANTIZA que cada caso generado es un duplicado genuino
    // (el nombre candidato es siempre el de un environment YA existente),
    // y verifica el efecto observable sobre el `Midway` completo: el
    // formulario recibe un error, `environment_busy` nunca llega a
    // activarse (la validación falla de forma síncrona, antes de
    // despachar cualquier `Task::perform`) y, sobre todo,
    // `state.workspace.environments` permanece exactamente igual al
    // conjunto original.
    // -----------------------------------------------------------------

    /// Si el intento de guardado duplicado corresponde a la creación de un
    /// environment nuevo (`editing_id = None`) o a la edición de OTRO
    /// environment existente distinto del que aporta el nombre duplicado
    /// (`editing_id = Some(id)` de un environment cuyo índice es distinto
    /// del índice elegido como fuente del nombre duplicado). Ambos casos
    /// deben rechazarse por Requisito 4.9: el chequeo de duplicado excluye
    /// únicamente al environment que se está editando, no a los demás.
    #[derive(Debug, Clone)]
    enum DuplicateSubmissionMode {
        CreateNew,
        EditDifferentExisting,
    }

    fn arb_duplicate_submission_mode() -> impl Strategy<Value = DuplicateSubmissionMode> {
        prop_oneof![
            Just(DuplicateSubmissionMode::CreateNew),
            Just(DuplicateSubmissionMode::EditDifferentExisting),
        ]
    }

    /// Lista de al menos dos nombres de environments existentes, únicos
    /// entre sí, para poder elegir un nombre "fuente" del duplicado y (en
    /// el caso `EditDifferentExisting`) otro environment distinto que sea
    /// el que se está editando.
    fn arb_existing_environment_names_at_least_two() -> impl Strategy<Value = Vec<String>> {
        prop::collection::hash_set(arb_preview_token(), 2..=20)
            .prop_map(|set| set.into_iter().collect::<Vec<String>>())
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 15: Nombre de
        /// environment duplicado se rechaza sin mutar el conjunto
        /// existente.
        /// Validates: Requirements 4.9
        ///
        /// Para cualquier conjunto existente de environments (con nombres
        /// únicos entre sí) y cualquier intento de guardar (crear, o
        /// editar un environment DISTINTO al que aporta el nombre) un
        /// environment cuyo nombre duplica exactamente el de un
        /// environment YA existente (excluyendo el propio en edición),
        /// `handle_environment_submitted` SHALL:
        ///
        /// - Rechazar la operación (fijar `environment_form.error =
        ///   Some(_)`) sin llegar a invocar `save_environment` (verificado
        ///   indirectamente: `environment_busy` permanece `false`, ya que
        ///   solo se activa justo antes de despachar el `Task::perform`
        ///   real).
        /// - Dejar `state.workspace.environments` exactamente sin cambios
        ///   respecto al conjunto original (mismo largo, mismo contenido).
        #[test]
        fn property_15_duplicate_environment_name_rejected_without_mutation(
            existing_names in arb_existing_environment_names_at_least_two(),
            duplicate_source_index in 0usize..20usize,
            editing_target_offset in 1usize..20usize,
            submission_mode in arb_duplicate_submission_mode(),
        ) {
            let environments: Vec<EnvironmentRecord> = existing_names
                .iter()
                .enumerate()
                .map(|(index, name)| EnvironmentRecord {
                    id: format!("env-{index}"),
                    name: name.clone(),
                    variables: vec![],
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                })
                .collect();

            let source_index = duplicate_source_index % environments.len();
            let duplicate_name = environments[source_index].name.clone();

            let editing_id = match submission_mode {
                DuplicateSubmissionMode::CreateNew => None,
                DuplicateSubmissionMode::EditDifferentExisting => {
                    // Elige un environment distinto del que aporta el
                    // nombre duplicado: desplaza el índice fuente por un
                    // offset efectivo en el rango `1..=(len - 1)` (nunca un
                    // múltiplo del largo de la lista), garantizando que
                    // `target_index != source_index` para cualquier largo
                    // >= 2 (garantizado por el generador).
                    let effective_offset = 1 + (editing_target_offset % (environments.len() - 1));
                    let target_index = (source_index + effective_offset) % environments.len();
                    Some(environments[target_index].id.clone())
                }
            };

            let mut midway = build_test_midway(create_blank_draft());
            midway.workspace.environments = environments.clone();
            midway.workspace_panel.environment_form = EnvironmentFormState {
                editing_id: editing_id.clone(),
                name_input: duplicate_name.clone(),
                error: None,
            };

            let _task = handle_environment_submitted(&mut midway);

            prop_assert_eq!(
                serde_json::to_value(&environments).unwrap(),
                serde_json::to_value(&midway.workspace.environments).unwrap(),
                "el conjunto de environments existentes no debe mutar ante un intento de guardado con nombre duplicado"
            );
            prop_assert!(midway.workspace_panel.environment_form.error.is_some());
            prop_assert!(!midway.workspace_panel.environment_busy);
        }
    }

    // -----------------------------------------------------------------
    // Tarea 7.5 (Property 16, Requisito 4.10): eliminar el environment
    // activo limpia la selección activa.
    //
    // El "environment activo" de una tab de request es
    // `RequestTabState.draft.environment_id` (poblado por
    // `RequestComposerMessage::EnvironmentChanged`, Tarea 3.2, Requisito
    // 2.4). Esta property ejercita directamente `handle_environment_deleted_result`
    // (Tarea 7.2), la función real que implementa el Requisito 4.10: al
    // recibir el resultado exitoso de eliminar un environment, limpia a
    // `None` la selección de CUALQUIER tab que lo tuviera activo, y deja
    // sin cambios la selección de cualquier tab con un environment
    // distinto (o ninguno) seleccionado.
    // -----------------------------------------------------------------

    /// Operación abstracta sobre el ciclo de vida de environments y su
    /// selección activa por tab. A diferencia de la Property 12/15 (que
    /// validan una única operación de guardado), esta property ejercita
    /// SECUENCIAS de creación/selección/eliminación, verificando la
    /// invariante de la Property 16 después de cada eliminación.
    #[derive(Debug, Clone)]
    enum EnvironmentLifecycleOp {
        /// Crea un nuevo environment con id fresco, replicando el efecto
        /// observable de un `EnvironmentSavedResult(Ok(_))` exitoso (la
        /// validación previa ya está cubierta por la Property 12/Tarea
        /// 7.3; aquí se asume siempre exitosa).
        CreateEnvironment,
        /// Selecciona (o deselecciona, si `environment_choice` es `None`)
        /// un environment como activo en una tab de request dada (por
        /// posición módulo la cantidad de tabs existentes), replicando el
        /// efecto observable de `RequestComposerMessage::EnvironmentChanged`
        /// sobre dicha tab (Tarea 3.2, Requisito 2.4). El environment
        /// elegido se resuelve por posición módulo la cantidad de
        /// environments existentes en ese momento; si no hay ninguno
        /// existente, la selección resultante es siempre `None`.
        SelectActiveOnTab {
            tab_offset: usize,
            environment_choice: Option<usize>,
        },
        /// Elimina un environment existente (por posición módulo la
        /// cantidad de environments existentes en ese momento), invocando
        /// directamente `handle_environment_deleted_result` con un
        /// resultado exitoso (`Ok(())`) — la función bajo prueba de esta
        /// Property 16. Si no hay ningún environment existente, la
        /// operación se omite.
        DeleteEnvironment { environment_offset: usize },
    }

    fn arb_environment_lifecycle_op() -> impl Strategy<Value = EnvironmentLifecycleOp> {
        prop_oneof![
            2 => Just(EnvironmentLifecycleOp::CreateEnvironment),
            3 => (0usize..5usize, prop::option::of(0usize..20usize)).prop_map(
                |(tab_offset, environment_choice)| EnvironmentLifecycleOp::SelectActiveOnTab {
                    tab_offset,
                    environment_choice,
                }
            ),
            2 => (0usize..20usize).prop_map(|environment_offset| {
                EnvironmentLifecycleOp::DeleteEnvironment { environment_offset }
            }),
        ]
    }

    fn arb_environment_lifecycle_ops() -> impl Strategy<Value = Vec<EnvironmentLifecycleOp>> {
        prop::collection::vec(arb_environment_lifecycle_op(), 0..=40)
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 16: Eliminar el
        /// environment activo limpia la selección activa.
        /// Validates: Requirements 4.10
        ///
        /// Para cualquier secuencia de operaciones de creación de
        /// environments, selección de environment activo por tab, y
        /// eliminación de un environment existente, cada vez que se
        /// elimina un environment, `handle_environment_deleted_result`
        /// (la función real que implementa el Requisito 4.10) SHALL, para
        /// cada tab de request abierta:
        ///
        /// - Si dicha tab tenía seleccionado como activo el environment
        ///   justo eliminado, dejar su selección en `None` (sin ningún
        ///   environment activo).
        /// - Si dicha tab tenía seleccionado un environment DISTINTO del
        ///   eliminado (o ninguno), dejar su selección exactamente sin
        ///   cambios.
        ///
        /// El oráculo se calcula de forma independiente (sin invocar la
        /// lógica real de `handle_environment_deleted_result`) a partir
        /// del estado de selección de cada tab inmediatamente antes de la
        /// eliminación.
        #[test]
        fn property_16_deleting_active_environment_clears_selection(
            ops in arb_environment_lifecycle_ops(),
        ) {
            const NUM_TEST_TABS: usize = 5;

            let mut midway = build_test_midway(create_blank_draft());
            midway.tabs = (0..NUM_TEST_TABS).map(|_| RequestTabState::blank()).collect();
            midway.active_tab = Some(0);

            let mut environments: Vec<EnvironmentRecord> = Vec::new();
            let mut next_environment_index: usize = 0;

            for op in ops {
                match op {
                    EnvironmentLifecycleOp::CreateEnvironment => {
                        let id = format!("env-{next_environment_index}");
                        next_environment_index += 1;
                        let record = EnvironmentRecord {
                            id: id.clone(),
                            name: format!("Environment {id}"),
                            variables: vec![],
                            created_at: "2024-01-01T00:00:00Z".to_string(),
                            updated_at: "2024-01-01T00:00:00Z".to_string(),
                        };
                        environments.push(record.clone());
                        midway.workspace.environments.push(record);
                    }
                    EnvironmentLifecycleOp::SelectActiveOnTab {
                        tab_offset,
                        environment_choice,
                    } => {
                        let tab_index = tab_offset % midway.tabs.len();
                        let selected_id = environment_choice.and_then(|env_index| {
                            if environments.is_empty() {
                                None
                            } else {
                                Some(environments[env_index % environments.len()].id.clone())
                            }
                        });
                        midway.tabs[tab_index].draft.environment_id = selected_id;
                    }
                    EnvironmentLifecycleOp::DeleteEnvironment { environment_offset } => {
                        if environments.is_empty() {
                            continue;
                        }
                        let delete_index = environment_offset % environments.len();
                        let deleted_id = environments[delete_index].id.clone();

                        // Oráculo independiente, calculado ANTES de invocar
                        // la función real, a partir del estado de
                        // selección actual de cada tab.
                        let expected_selection_per_tab: Vec<Option<String>> = midway
                            .tabs
                            .iter()
                            .map(|tab| {
                                if tab.draft.environment_id.as_deref() == Some(deleted_id.as_str()) {
                                    None
                                } else {
                                    tab.draft.environment_id.clone()
                                }
                            })
                            .collect();

                        handle_environment_deleted_result(&mut midway, deleted_id.clone(), Ok(()));

                        for (tab, expected) in midway.tabs.iter().zip(expected_selection_per_tab.iter()) {
                            prop_assert_eq!(&tab.draft.environment_id, expected);
                        }

                        prop_assert!(
                            !midway
                                .workspace
                                .environments
                                .iter()
                                .any(|environment| environment.id == deleted_id)
                        );

                        environments.remove(delete_index);
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------
    // Tarea 7.8 (Property 13, Requisitos 4.4, 4.5): validación de import
    // por formato y tamaño en la sección Data del `Workspace_Panel`.
    //
    // El chequeo de tamaño (Requisito 4.5, "excede el tamaño máximo
    // permitido") ocurre de forma SÍNCRONA en `handle_import_submitted`
    // (Tarea 7.7), ANTES de invocar `detect_import_format` o cualquier
    // parser: por eso la primera property (`property_13_oversized_payload_
    // rejected_before_parsing`) ejercita directamente esa función,
    // independientemente de que el contenido del payload sea sintácticamente
    // válido o no (Requisito 4.4/4.5, "regardless of content validity").
    //
    // El chequeo de formato/contenido (Requisito 4.4, "no corresponde a
    // ninguno de los formatos soportados"; Requisito 4.5, "está corrupto")
    // ocurre de forma ASÍNCRONA dentro de `import_workspace_data` (Tarea
    // 7.7), vía `parse_json_or_yaml_payload` + `detect_import_format`: la
    // segunda property (`property_13_unsupported_or_corrupt_payload_
    // rejected_without_mutation`) ejercita esa función directamente contra
    // un `AppState` real (pero "inerte", igual que en la Property 8),
    // verificando que el snapshot completo del repositorio permanece
    // exactamente igual antes y después del intento fallido.
    // -----------------------------------------------------------------

    fn arb_workspace_import_format() -> impl Strategy<Value = WorkspaceImportFormat> {
        prop_oneof![
            Just(WorkspaceImportFormat::Auto),
            Just(WorkspaceImportFormat::NativeWorkspaceV1),
            Just(WorkspaceImportFormat::PostmanCollectionV21),
            Just(WorkspaceImportFormat::OpenApiV3),
        ]
    }

    /// Genera un payload que excede `MAX_IMPORT_PAYLOAD_BYTES` en una
    /// cantidad pequeña y variable de bytes extra (evita generar payloads
    /// arbitrariamente más grandes de lo necesario en cada caso), en dos
    /// variantes de contenido (Requisito 4.4/4.5, "regardless of content
    /// validity"): una sintácticamente válida como JSON (un objeto con un
    /// único campo de relleno) y otra claramente inválida (una secuencia de
    /// llaves de apertura sin cierre). El chequeo de tamaño de
    /// `handle_import_submitted` ocurre antes de intentar parsear nada, así
    /// que ambas variantes deben rechazarse de forma idéntica.
    fn arb_oversized_payload() -> impl Strategy<Value = String> {
        (0usize..=64usize, proptest::bool::ANY).prop_map(|(extra_bytes, looks_like_valid_json)| {
            let filler_len = MAX_IMPORT_PAYLOAD_BYTES + 1 + extra_bytes;
            if looks_like_valid_json {
                let filler = "a".repeat(filler_len);
                format!("{{\"padding\":\"{filler}\"}}")
            } else {
                "{".repeat(filler_len)
            }
        })
    }

    /// Genera un payload dentro del límite de tamaño, en dos variantes que
    /// un oráculo independiente (sin invocar `detect_import_format` ni
    /// ningún parser real) puede garantizar como no soportadas/corruptas:
    ///
    /// - Formato no soportado (Requisito 4.4): un objeto JSON válido con un
    ///   único campo arbitrario, sin la firma de ninguno de los tres
    ///   formatos reconocidos (`is_native_workspace_bundle` requiere
    ///   `format == NATIVE_WORKSPACE_FORMAT_ID`; `is_postman_collection`
    ///   requiere `info` + `item` no vacío; `is_openapi_document` requiere
    ///   `openapi` iniciando con `"3."`), por lo que `detect_import_format`
    ///   en modo `Auto` SHALL devolver siempre un error.
    /// - Contenido corrupto (Requisito 4.5): una secuencia de llaves de
    ///   apertura sin cierre correspondiente, que no puede parsearse
    ///   completamente ni como JSON ni como YAML.
    fn arb_unsupported_or_corrupt_payload() -> impl Strategy<Value = String> {
        prop_oneof![
            (arb_preview_token(), arb_preview_token())
                .prop_map(|(key, value)| format!("{{\"{key}\":\"{value}\"}}")),
            (2usize..=20usize).prop_map(|open_brace_count| "{".repeat(open_brace_count)),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 8, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 13: Validación de
        /// import por formato y tamaño (parte 1: tamaño).
        /// Validates: Requirements 4.4, 4.5
        ///
        /// Para cualquier payload cuyo tamaño en bytes UTF-8 exceda
        /// `MAX_IMPORT_PAYLOAD_BYTES` (10 MB) -sea o no sintácticamente
        /// válido como JSON- y para cualquier formato de import
        /// solicitado, `handle_import_submitted` SHALL rechazar la
        /// operación fijando `import_form.result_message = Some(_)` y
        /// `import_form.result_is_error = true`, SIN llegar a activar
        /// `import_form.busy` (que solo se activa justo antes de
        /// despachar el `Task::perform` real hacia `import_workspace_data`,
        /// nunca alcanzado por un payload sobredimensionado).
        #[test]
        fn property_13_oversized_payload_rejected_before_parsing(
            payload in arb_oversized_payload(),
            import_format in arb_workspace_import_format(),
        ) {
            prop_assert!(payload.len() > MAX_IMPORT_PAYLOAD_BYTES);

            let mut midway = build_test_midway(create_blank_draft());
            midway.workspace_panel.import_form.payload_input = payload;
            midway.workspace_panel.import_form.import_format = import_format;

            let _task = handle_import_submitted(&mut midway);

            prop_assert!(midway.workspace_panel.import_form.result_message.is_some());
            prop_assert!(midway.workspace_panel.import_form.result_is_error);
            prop_assert!(!midway.workspace_panel.import_form.busy);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 30, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 13: Validación de
        /// import por formato y tamaño (parte 2: formato/contenido).
        /// Validates: Requirements 4.4, 4.5
        ///
        /// Para cualquier payload dentro del límite de tamaño que un
        /// oráculo independiente determina como no correspondiente a
        /// ninguno de los formatos soportados o como contenido corrupto
        /// (irreconocible tanto en JSON como en YAML), `import_workspace_
        /// data` (Tarea 7.7, la orquestación async real invocada tras la
        /// validación de tamaño) SHALL devolver un error, y el snapshot
        /// completo del workspace persistido (`export_full_snapshot`)
        /// SHALL permanecer exactamente igual antes y después del intento
        /// fallido, sin haber mutado ningún environment, request o
        /// colección existente.
        #[test]
        fn property_13_unsupported_or_corrupt_payload_rejected_without_mutation(
            payload in arb_unsupported_or_corrupt_payload(),
        ) {
            prop_assert!(payload.len() <= MAX_IMPORT_PAYLOAD_BYTES);

            let app_state = Arc::new(build_test_app_state());
            let runtime = tokio::runtime::Runtime::new()
                .expect("no se pudo crear el runtime de tokio para el test de Property 13");

            let snapshot_before = runtime
                .block_on(app_state.repository.export_full_snapshot())
                .expect("export_full_snapshot no debería fallar contra el repositorio de prueba");

            let result = runtime.block_on(import_workspace_data(
                Arc::clone(&app_state),
                payload,
                WorkspaceImportFormat::Auto,
            ));

            prop_assert!(result.is_err());

            let snapshot_after = runtime
                .block_on(app_state.repository.export_full_snapshot())
                .expect("export_full_snapshot no debería fallar contra el repositorio de prueba");

            prop_assert_eq!(
                serde_json::to_value(&snapshot_before).unwrap(),
                serde_json::to_value(&snapshot_after).unwrap(),
                "el estado de datos existente no debe mutar ante un import rechazado por formato/contenido"
            );
        }
    }

    // -----------------------------------------------------------------
    // Tarea 7.10 (Property 17, Requisito 4.11): preservación de ambos
    // elementos ante colisión de nombre en import.
    //
    // Se ejercita en dos niveles, replicando el patrón de la Property 13
    // (Tarea 7.8: una parte "unitaria" sobre la función pura y una parte
    // "end-to-end" contra un `AppState` real):
    //
    // - Nivel 1 (`property_17_unique_name_against_never_collides_and_
    //   preserves_unchanged_candidate`): ejercita directamente
    //   `unique_name_against` (Tarea 7.9) con conjuntos de nombres
    //   existentes arbitrarios (incluyendo colisión exacta del candidato,
    //   colisión con sufijos ya usados de forma contigua o con huecos, y
    //   ningún tipo de colisión) mediante un oráculo independiente que NO
    //   reimplementa la función bajo prueba: solo verifica las dos
    //   invariantes que definen su contrato (nunca devuelve un nombre ya
    //   presente en el conjunto existente; si el candidato no colisiona,
    //   lo devuelve sin cambios).
    // - Nivel 2 (`property_17_import_name_collision_preserves_both_
    //   elements`): ejercita el flujo completo de import (`import_
    //   workspace_data`, Tarea 7.7/7.9) contra un `AppState` real,
    //   importando un bundle nativo con un environment cuyo nombre
    //   colisiona exactamente con uno ya existente, y verifica contra
    //   `export_full_snapshot` que ambos environments (preexistente e
    //   importado) sobreviven como registros distintos por `id`, con
    //   nombres distintos entre sí, y sin que ninguno de los dos pierda
    //   sus variables originales.
    // -----------------------------------------------------------------

    /// Genera un caso `(existing_names, candidate)` para el nivel 1:
    /// `unrelated_names` aporta nombres que no tienen relación con
    /// `candidate` (una fracción de los casos podría coincidir con él por
    /// azar, lo cual simplemente se suma como colisión adicional sin
    /// invalidar el caso); `candidate_present` decide si el candidato
    /// EXACTO ya está en el conjunto; y `used_suffixes` agrega, de forma
    /// independiente, un subconjunto arbitrario (posiblemente con huecos,
    /// p. ej. `{3, 5}` sin `{2, 4}`) de sufijos `" (N)"` ya usados sobre
    /// ese mismo candidato — el caso "near-collision" mencionado en la
    /// tarea (p. ej. ya existe `"Foo (2)"` pero no necesariamente `"Foo"`).
    fn arb_unique_name_against_case() -> impl Strategy<Value = (HashSet<String>, String)> {
        (
            arb_preview_token(),
            prop::collection::hash_set(arb_preview_token(), 0..=8),
            proptest::bool::ANY,
            prop::collection::hash_set(2usize..=12usize, 0..=6),
        )
            .prop_map(
                |(candidate, unrelated_names, candidate_present, used_suffixes)| {
                    let mut existing_names = unrelated_names;
                    if candidate_present {
                        existing_names.insert(candidate.clone());
                    }
                    for suffix in used_suffixes {
                        existing_names.insert(format!("{candidate} ({suffix})"));
                    }
                    (existing_names, candidate)
                },
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 17: Colisión de
        /// nombre en import preserva ambos elementos (nivel 1: función
        /// pura `unique_name_against`).
        /// Validates: Requirements 4.11
        ///
        /// Para cualquier conjunto existente de nombres y cualquier nombre
        /// candidato: el nombre devuelto por `unique_name_against` NUNCA
        /// está ya presente en el conjunto existente (así, al persistir el
        /// elemento importado bajo ese nombre, no puede sobrescribir ni
        /// colisionar con ningún elemento existente); y si el candidato NO
        /// colisionaba con el conjunto existente, el nombre devuelto es
        /// exactamente igual al candidato (sin sufijo añadido de forma
        /// innecesaria).
        #[test]
        fn property_17_unique_name_against_never_collides_and_preserves_unchanged_candidate(
            (existing_names, candidate) in arb_unique_name_against_case(),
        ) {
            let candidate_collides = existing_names.contains(&candidate);

            let result = unique_name_against(&existing_names, &candidate);

            prop_assert!(
                !existing_names.contains(&result),
                "el nombre devuelto ({result:?}) no debe colisionar con ningún nombre existente"
            );

            if !candidate_collides {
                prop_assert_eq!(&result, &candidate);
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 20, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 17: Colisión de
        /// nombre en import preserva ambos elementos (nivel 2: flujo
        /// completo de import contra un `AppState` real).
        /// Validates: Requirements 4.11
        ///
        /// Para cualquier workspace con un environment existente de nombre
        /// N y cualquier import de un bundle nativo que introduce un
        /// environment con ese mismo nombre N: tras la importación,
        /// `export_full_snapshot` SHALL reportar exactamente dos
        /// environments distintos por `id` — el original, sin ninguna
        /// modificación (mismo nombre N, mismas variables), y el
        /// importado, bajo un nombre diferenciado (distinto de N) que
        /// conserva sus propias variables — sin pérdida de datos de
        /// ninguno de los dos.
        #[test]
        fn property_17_import_name_collision_preserves_both_elements(
            collide_name in arb_preview_token(),
            existing_variables in arb_key_value_rows("existing-var"),
            imported_variables in arb_key_value_rows("imported-var"),
        ) {
            let app_state = Arc::new(build_test_app_state());
            let runtime = tokio::runtime::Runtime::new()
                .expect("no se pudo crear el runtime de tokio para el test de Property 17");

            let existing_environment = runtime
                .block_on(app_state.repository.save_environment(SaveEnvironmentInput {
                    environment_id: None,
                    name: collide_name.clone(),
                    variables: existing_variables.clone(),
                }))
                .expect("no se pudo guardar el environment existente de prueba");

            let bundle_snapshot = WorkspaceSnapshot {
                collections: Vec::new(),
                environments: vec![EnvironmentRecord {
                    id: "import-seed-id".to_string(),
                    name: collide_name.clone(),
                    variables: imported_variables.clone(),
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                }],
                history: Vec::new(),
                secrets: Vec::new(),
            };
            let bundle = make_native_bundle(bundle_snapshot, false, false);
            let payload_text = serde_json::to_string(&bundle)
                .expect("no se pudo serializar el bundle nativo de prueba");

            let import_result = runtime.block_on(import_workspace_data(
                Arc::clone(&app_state),
                payload_text,
                WorkspaceImportFormat::NativeWorkspaceV1,
            ));

            prop_assert!(
                import_result.is_ok(),
                "el import de un bundle nativo válido con colisión de nombre no debería fallar: {:?}",
                import_result.err()
            );

            let snapshot_after = runtime
                .block_on(app_state.repository.export_full_snapshot())
                .expect("export_full_snapshot no debería fallar contra el repositorio de prueba");

            prop_assert_eq!(
                snapshot_after.environments.len(),
                2,
                "deben coexistir el environment preexistente y el importado como dos registros distintos"
            );

            let original = snapshot_after
                .environments
                .iter()
                .find(|environment| environment.id == existing_environment.id);
            prop_assert!(original.is_some(), "el environment preexistente debe seguir presente por id");
            let original = original.unwrap();
            prop_assert_eq!(
                &original.name,
                &collide_name,
                "el environment preexistente no debe modificarse (mismo nombre)"
            );
            prop_assert_eq!(
                serde_json::to_value(&original.variables).unwrap(),
                serde_json::to_value(&existing_variables).unwrap(),
                "el environment preexistente no debe perder sus variables originales"
            );

            let imported = snapshot_after
                .environments
                .iter()
                .find(|environment| environment.id != existing_environment.id);
            prop_assert!(imported.is_some(), "el environment importado debe existir como un registro distinto");
            let imported = imported.unwrap();
            prop_assert_ne!(
                &imported.name,
                &collide_name,
                "el elemento importado debe recibir un nombre diferenciado, distinto del original"
            );
            prop_assert!(
                imported.name.starts_with(collide_name.as_str()),
                "el nombre diferenciado del importado debe derivar del nombre original colisionado"
            );
            prop_assert_eq!(
                serde_json::to_value(&imported.variables).unwrap(),
                serde_json::to_value(&imported_variables).unwrap(),
                "el environment importado no debe perder sus propias variables"
            );
        }
    }

    // -----------------------------------------------------------------
    // Tarea 11.2 (Requisitos 6.1, 6.2): apertura/cierre del Command
    // Palette y ejecución de ítems.
    //
    // Sin Property numerada asignada a esta tarea (Tarea 11.3, "Escribir
    // unit tests de apertura del Command_Palette y ejecución de ítems",
    // pide explícitamente un caso de camino nominal y un caso borde, no
    // property tests); se cubre con tests unitarios simples.
    // -----------------------------------------------------------------

    /// Construye un `WorkspaceSnapshot` de prueba con una única colección
    /// que contiene un único `SavedRequestRecord`, usado por los tests de
    /// `build_palette_items`/`execute_palette_item` que necesitan un
    /// request guardado navegable desde el palette.
    fn workspace_snapshot_with_one_saved_request(
    ) -> (midway_core::domain::workspace::WorkspaceSnapshot, String) {
        use midway_core::domain::http::RequestBodyDraft;
        use midway_core::domain::workspace::{
            CollectionSummary, CollectionWithRequests, SavedRequestRecord,
        };

        let request_id = "request-1".to_string();
        let draft = RequestDraft {
            id: Some(request_id.clone()),
            name: "Obtener usuario".to_string(),
            method: HttpMethod::GET,
            url: "https://api.ejemplo.com/users/1".to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            auth: AuthConfig::None,
            body: RequestBodyDraft {
                mode: BodyMode::None,
                value: String::new(),
                form_data: Vec::new(),
            },
            timeout_ms: 30_000,
            environment_id: None,
            response_tests: Vec::new(),
        };

        let snapshot = midway_core::domain::workspace::WorkspaceSnapshot {
            collections: vec![CollectionWithRequests {
                collection: CollectionSummary {
                    id: "collection-1".to_string(),
                    name: "Usuarios".to_string(),
                    request_count: 1,
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                },
                folders: Vec::new(),
                requests: vec![SavedRequestRecord {
                    id: request_id.clone(),
                    collection_id: "collection-1".to_string(),
                    folder_id: None,
                    name: "Obtener usuario".to_string(),
                    draft,
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                }],
            }],
            environments: Vec::new(),
            history: Vec::new(),
            secrets: Vec::new(),
        };

        (snapshot, request_id)
    }

    /// Camino nominal (Tarea 11.3): `PaletteMessage::Toggled` sobre un
    /// palette cerrado lo abre (`is_open = true`) con la query en blanco.
    #[test]
    fn palette_toggled_opens_palette_with_blank_query() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.query = "restos-de-una-busqueda-anterior".to_string();

        let _ = update_palette(&mut midway, PaletteMessage::Toggled);

        assert!(midway.palette.is_open);
        assert!(midway.palette.query.is_empty());
    }

    /// `PaletteMessage::Toggled` sobre un palette ya abierto lo cierra
    /// (comportamiento de toggle, ver doc de `PaletteMessage::Toggled`).
    #[test]
    fn palette_toggled_closes_an_already_open_palette() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.is_open = true;
        midway.palette.query = "algo".to_string();

        let _ = update_palette(&mut midway, PaletteMessage::Toggled);

        assert!(!midway.palette.is_open);
        assert!(midway.palette.query.is_empty());
    }

    /// `PaletteMessage::QueryChanged` almacena la query, y esa query se
    /// refleja en los resultados computados vía `search_palette_items`
    /// sobre `build_palette_items` (Tarea 11.2: los resultados se calculan
    /// on-the-fly, no se cachean en `PaletteState`).
    #[test]
    fn palette_query_changed_is_stored_and_reflected_in_search_results() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.is_open = true;

        let _ = update_palette(
            &mut midway,
            PaletteMessage::QueryChanged("nuevo".to_string()),
        );
        assert_eq!(midway.palette.query, "nuevo");

        let items = build_palette_items(&midway);
        let results = command_palette::search_palette_items(&items, &midway.palette.query, 14);
        assert!(
            results
                .iter()
                .any(|item| item.id == PALETTE_ACTION_NEW_REQUEST),
            "la acción fija 'Nuevo request' debe calzar con la query 'nuevo'"
        );
    }

    /// Selecciona la acción fija "Nuevo request" (Tarea 11.2, Requisito
    /// 6.2): debe abrir una tab en blanco adicional, activarla, y cerrar el
    /// palette (limpiando la query).
    #[test]
    fn palette_item_selected_new_request_action_opens_blank_tab_and_closes_palette() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.is_open = true;
        midway.palette.query = "nuevo".to_string();
        let tabs_before = midway.tabs.len();

        let _ = update_palette(
            &mut midway,
            PaletteMessage::ItemSelected {
                item_id: PALETTE_ACTION_NEW_REQUEST.to_string(),
            },
        );

        assert_eq!(midway.tabs.len(), tabs_before + 1);
        assert_eq!(midway.active_tab, Some(midway.tabs.len() - 1));
        assert!(!midway.palette.is_open);
        assert!(midway.palette.query.is_empty());
    }

    /// Selecciona un ítem `request:<id>` (Tarea 11.2, Requisito 6.2): debe
    /// abrir ese `SavedRequestRecord` en una tab nueva (draft equivalente,
    /// `saved_draft` fijado) y activarla, además de cerrar el palette.
    #[test]
    fn palette_item_selected_request_opens_saved_request_in_new_tab() {
        let mut midway = build_test_midway(create_blank_draft());
        let (snapshot, request_id) = workspace_snapshot_with_one_saved_request();
        midway.workspace = snapshot;
        midway.palette.is_open = true;

        let _ = update_palette(
            &mut midway,
            PaletteMessage::ItemSelected {
                item_id: format!("request:{request_id}"),
            },
        );

        assert!(!midway.palette.is_open);
        assert!(midway.palette.query.is_empty());

        let active_index = midway
            .active_tab
            .expect("debe haber una tab activa tras abrir el request");
        let active_tab = &midway.tabs[active_index];
        assert_eq!(active_tab.draft.url, "https://api.ejemplo.com/users/1");
        assert_eq!(
            active_tab
                .saved_draft
                .as_ref()
                .and_then(|draft| draft.id.clone()),
            Some(request_id)
        );
    }

    /// Reabrir el mismo request guardado dos veces reutiliza la tab ya
    /// abierta en lugar de duplicarla.
    #[test]
    fn palette_item_selected_request_reuses_already_open_tab() {
        let mut midway = build_test_midway(create_blank_draft());
        let (snapshot, request_id) = workspace_snapshot_with_one_saved_request();
        midway.workspace = snapshot;

        open_saved_request_in_tab(&mut midway, &request_id);
        let tabs_after_first_open = midway.tabs.len();

        // Cambia de tab activa antes de reabrir, para verificar que sí
        // vuelve a activar la tab existente (no solo que no duplica).
        midway.tabs.push(RequestTabState::blank());
        midway.active_tab = Some(midway.tabs.len() - 1);

        open_saved_request_in_tab(&mut midway, &request_id);

        assert_eq!(
            midway.tabs.len(),
            tabs_after_first_open + 1,
            "no debe crear una segunda tab para el mismo request"
        );
        let active_index = midway.active_tab.expect("debe haber una tab activa");
        assert_eq!(
            midway.tabs[active_index]
                .saved_draft
                .as_ref()
                .and_then(|draft| draft.id.clone()),
            Some(request_id)
        );
    }

    /// Caso borde (Tarea 11.3): búsqueda sin coincidencias. Una query que
    /// no calza con ninguna acción fija ni ningún ítem del workspace debe
    /// producir una lista de resultados vacía (sin entrar en pánico ni
    /// devolver ítems espurios), y `ItemSelected` con un id que no resuelve
    /// a ninguna acción/collection/request conocida no debe mutar el
    /// estado de tabs, aunque sí cierra el palette.
    #[test]
    fn palette_search_with_no_matches_returns_empty_results() {
        let midway = build_test_midway(create_blank_draft());

        let items = build_palette_items(&midway);
        let results = command_palette::search_palette_items(&items, "zzz-no-existe-zzz", 14);

        assert!(results.is_empty());
    }

    /// `ItemSelected` con un `item_id` desconocido (no calza con ningún
    /// prefijo/acción del esquema documentado en `build_palette_items`) no
    /// debe mutar `tabs`/`active_tab`, pero sí debe cerrar el palette (el
    /// cierre es incondicional al seleccionar, ver `update_palette`).
    #[test]
    fn palette_item_selected_with_unknown_id_closes_palette_without_mutating_tabs() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.is_open = true;
        let tabs_before = midway.tabs.len();
        let active_tab_before = midway.active_tab;

        let _ = update_palette(
            &mut midway,
            PaletteMessage::ItemSelected {
                item_id: "request:no-existe".to_string(),
            },
        );

        assert_eq!(midway.tabs.len(), tabs_before);
        assert_eq!(midway.active_tab, active_tab_before);
        assert!(!midway.palette.is_open);
    }

    /// `PaletteMessage::Dismissed` cierra el palette (limpiando la query)
    /// sin ejecutar ninguna acción, a diferencia de `ItemSelected`.
    #[test]
    fn palette_dismissed_closes_palette_without_executing_any_action() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.is_open = true;
        midway.palette.query = "algo".to_string();
        let tabs_before = midway.tabs.len();

        let _ = update_palette(&mut midway, PaletteMessage::Dismissed);

        assert!(!midway.palette.is_open);
        assert!(midway.palette.query.is_empty());
        assert_eq!(midway.tabs.len(), tabs_before);
    }

    // -----------------------------------------------------------------
    // Tarea 11.8 (Requisito 6.5): stack de tabs cerradas.
    // -----------------------------------------------------------------

    /// Cerrar una tab la quita de `tabs` y la empuja al FRENTE de
    /// `closed_tabs` (orden más-reciente-primero).
    #[test]
    fn closing_a_tab_pushes_it_to_the_front_of_closed_tabs() {
        let mut midway = build_test_midway(create_blank_draft());
        let closed_id = midway.tabs[0].id.clone();
        midway.tabs.push(RequestTabState::blank());
        midway.active_tab = Some(1);

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: closed_id.clone(),
            },
        );

        assert_eq!(midway.tabs.len(), 1, "la tab cerrada debe salir de `tabs`");
        assert!(!midway.tabs.iter().any(|tab| tab.id == closed_id));
        assert_eq!(midway.closed_tabs.len(), 1);
        assert_eq!(midway.closed_tabs.front().unwrap().id, closed_id);
        assert!(
            midway.session.dirty,
            "cerrar una tab debe marcar la sesión como dirty"
        );
    }

    /// Cerrar más de `CLOSED_TABS_LIMIT` (20) tabs mantiene el stack
    /// acotado exactamente a 20 elementos, descartando el más antiguo (el
    /// fondo del `VecDeque`) en cada exceso.
    #[test]
    fn closing_more_than_limit_tabs_keeps_stack_bounded_evicting_oldest() {
        let mut midway = build_test_midway(create_blank_draft());
        let num_extra_tabs = crate::session::CLOSED_TABS_LIMIT + 5;
        for _ in 0..num_extra_tabs {
            midway.tabs.push(RequestTabState::blank());
        }

        // Cierra todas las tabs salvo una, en orden, registrando los ids
        // en el orden en que se cerraron (primero cerrado -> último
        // cerrado).
        let mut closed_ids_in_order = Vec::new();
        while midway.tabs.len() > 1 {
            let tab_id = midway.tabs[0].id.clone();
            closed_ids_in_order.push(tab_id.clone());
            midway.active_tab = Some(0);
            let _ =
                update_request_composer(&mut midway, RequestComposerMessage::TabClosed { tab_id });
        }

        assert_eq!(midway.closed_tabs.len(), crate::session::CLOSED_TABS_LIMIT);

        // El frente debe ser el ÚLTIMO cerrado, y el fondo debe ser el
        // más antiguo que sobrevivió al recorte (los primeros 5 cerrados
        // deben haber sido descartados, ya que se cerraron 25 tabs con un
        // límite de 20).
        let most_recently_closed = closed_ids_in_order.last().unwrap();
        assert_eq!(
            &midway.closed_tabs.front().unwrap().id,
            most_recently_closed
        );

        let expected_oldest_surviving =
            &closed_ids_in_order[closed_ids_in_order.len() - crate::session::CLOSED_TABS_LIMIT];
        assert_eq!(
            &midway.closed_tabs.back().unwrap().id,
            expected_oldest_surviving
        );

        for discarded_id in &closed_ids_in_order[..5] {
            assert!(
                !midway
                    .closed_tabs
                    .iter()
                    .any(|snapshot| &snapshot.id == discarded_id),
                "los primeros cierres deben haber sido descartados del stack acotado"
            );
        }
    }

    /// Reabrir extrae la tab cerrada más recientemente (frente del
    /// `VecDeque`), la agrega al final de `tabs` y la activa.
    #[test]
    fn reopening_pops_most_recently_closed_tab_and_activates_it() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.tabs.push(RequestTabState::blank());
        let tab_id_to_close = midway.tabs[1].id.clone();
        midway.active_tab = Some(1);
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id_to_close.clone(),
            },
        );
        let tabs_before_reopen = midway.tabs.len();

        let _ = update_request_composer(&mut midway, RequestComposerMessage::ClosedTabReopened);

        assert_eq!(midway.tabs.len(), tabs_before_reopen + 1);
        assert!(midway.closed_tabs.is_empty());
        let active_index = midway
            .active_tab
            .expect("debe haber una tab activa tras reabrir");
        assert_eq!(active_index, midway.tabs.len() - 1);
        assert_eq!(midway.tabs[active_index].id, tab_id_to_close);
    }

    /// Reabrir con el stack de tabs cerradas vacío es un no-op: no debe
    /// entrar en pánico ni mutar `tabs`/`active_tab`.
    #[test]
    fn reopening_with_empty_closed_tabs_is_a_no_op() {
        let mut midway = build_test_midway(create_blank_draft());
        assert!(midway.closed_tabs.is_empty());
        let tabs_before = midway.tabs.len();
        let active_tab_before = midway.active_tab;

        let _ = update_request_composer(&mut midway, RequestComposerMessage::ClosedTabReopened);

        assert_eq!(midway.tabs.len(), tabs_before);
        assert_eq!(midway.active_tab, active_tab_before);
        assert!(midway.closed_tabs.is_empty());
    }

    /// El orden LIFO se preserva a través de múltiples ciclos de cierre y
    /// reapertura: cerrar A, luego cerrar B, y reabrir debe traer de
    /// vuelta B (el más recientemente cerrado), no A.
    #[test]
    fn lifo_order_is_preserved_across_close_and_reopen_cycles() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.tabs.push(RequestTabState::blank()); // tab B
        let tab_a_id = midway.tabs[0].id.clone();
        let tab_b_id = midway.tabs[1].id.clone();

        midway.active_tab = Some(0);
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_a_id.clone(),
            },
        );
        // Tras cerrar A, la única tab restante (B) queda activa en el
        // índice 0.
        midway.active_tab = Some(0);
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_b_id.clone(),
            },
        );

        assert_eq!(midway.closed_tabs.len(), 2);
        assert_eq!(
            midway.closed_tabs.front().unwrap().id,
            tab_b_id,
            "B se cerró más recientemente que A"
        );

        let _ = update_request_composer(&mut midway, RequestComposerMessage::ClosedTabReopened);

        let active_index = midway
            .active_tab
            .expect("debe haber una tab activa tras reabrir");
        assert_eq!(
            midway.tabs[active_index].id, tab_b_id,
            "reabrir debe traer B, no A"
        );
        assert_eq!(midway.closed_tabs.len(), 1);
        assert_eq!(
            midway.closed_tabs.front().unwrap().id,
            tab_a_id,
            "A sigue en el stack"
        );
    }

    // -----------------------------------------------------------------
    // Tarea 11.9 (Property 21, Requisito 6.5): stack acotado de tabs
    // cerradas con reapertura LIFO, sobre secuencias ARBITRARIAS de
    // cierres/reaperturas.
    //
    // Los unit tests de la Tarea 11.8 (arriba) cubren escenarios puntuales
    // (evicción exacta al superar 20, LIFO a través de 2 ciclos). Esta
    // property generaliza esos escenarios a cualquier secuencia de
    // operaciones abstractas `CloseTab`/`ReopenTab`, comparando el
    // comportamiento real (`handle_tab_closed`/`handle_closed_tab_reopened`,
    // invocadas directamente para no depender del aviso de unsaved changes
    // de la Tarea 11.10, que es responsabilidad de la Property 22/Tarea
    // 11.11) contra un oráculo independiente: un `VecDeque<String>` de ids
    // construido por el test mismo, replicando la lógica de
    // push-front/evict-desde-el-fondo-si-supera-el-límite/pop-front sin
    // llamar a ninguna función bajo prueba.
    // -----------------------------------------------------------------

    /// Operación abstracta sobre el stack de tabs cerradas.
    #[derive(Debug, Clone)]
    enum ClosedTabsStackOp {
        /// Cierra la tab abierta en la posición `tab_offset` módulo la
        /// cantidad de tabs actualmente abiertas, invocando directamente
        /// `handle_tab_closed`. Si no hay ninguna tab abierta, la
        /// operación es un no-op (consistente con el comportamiento
        /// existente: no hay tab que cerrar).
        CloseTab { tab_offset: usize },
        /// Reabre la tab cerrada más recientemente, invocando directamente
        /// `handle_closed_tab_reopened`. Si el stack de tabs cerradas está
        /// vacío, la operación es un no-op (comportamiento existente,
        /// cubierto por `reopening_with_empty_closed_tabs_is_a_no_op`).
        ReopenTab,
    }

    fn arb_closed_tabs_stack_op() -> impl Strategy<Value = ClosedTabsStackOp> {
        prop_oneof![
            3 => (0usize..30usize).prop_map(|tab_offset| ClosedTabsStackOp::CloseTab { tab_offset }),
            2 => Just(ClosedTabsStackOp::ReopenTab),
        ]
    }

    /// Secuencias de hasta 80 operaciones: suficientemente largas para
    /// ejercitar tanto la evicción acotada (más de 20 cierres sin
    /// suficientes reaperturas) como ciclos LIFO intercalados de cierre y
    /// reapertura.
    fn arb_closed_tabs_stack_ops() -> impl Strategy<Value = Vec<ClosedTabsStackOp>> {
        prop::collection::vec(arb_closed_tabs_stack_op(), 0..=80)
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 21: El stack de tabs
        /// cerradas está acotado y reabre en orden LIFO.
        /// Validates: Requirements 6.5
        ///
        /// Para cualquier secuencia de cierres (`CloseTab`) y reaperturas
        /// (`ReopenTab`) de tabs, aplicadas directamente sobre
        /// `handle_tab_closed`/`handle_closed_tab_reopened`:
        ///
        /// - `state.closed_tabs` SHALL nunca exceder
        ///   `crate::session::CLOSED_TABS_LIMIT` (20) elementos, incluso
        ///   tras más de 20 cierres sin reaperturas intermedias
        ///   (descartando la entrada más antigua al superar el límite).
        /// - Cada `ReopenTab` SHALL devolver (agregar a `state.tabs` y
        ///   activar) exactamente la tab cerrada más recientemente entre
        ///   las aún presentes en el stack, tal como lo determina un
        ///   oráculo independiente construido por el test.
        /// - Tras CADA operación, el contenido completo (no solo el
        ///   tamaño) de `state.closed_tabs` SHALL coincidir exactamente,
        ///   en el mismo orden, con el del oráculo independiente.
        #[test]
        fn property_21_closed_tabs_stack_is_bounded_and_reopens_lifo(
            ops in arb_closed_tabs_stack_ops(),
        ) {
            // Se necesitan más tabs abiertas que el límite del stack para
            // poder ejercitar secuencias largas de cierres sin agotar las
            // tabs disponibles.
            const NUM_SEED_TABS: usize = 25;

            let mut midway = build_test_midway(create_blank_draft());
            while midway.tabs.len() < NUM_SEED_TABS {
                midway.tabs.push(RequestTabState::blank());
            }
            midway.active_tab = Some(0);

            // Oráculo independiente: NO invoca `handle_tab_closed` ni
            // `handle_closed_tab_reopened` internamente, solo replica la
            // regla push-front/evict-desde-el-fondo/pop-front descrita en
            // el Requisito 6.5.
            let mut closed_oracle: VecDeque<String> = VecDeque::new();

            for op in ops {
                match op {
                    ClosedTabsStackOp::CloseTab { tab_offset } => {
                        if !midway.tabs.is_empty() {
                            let index = tab_offset % midway.tabs.len();
                            let tab_id = midway.tabs[index].id.clone();

                            handle_tab_closed(&mut midway, &tab_id);

                            closed_oracle.push_front(tab_id);
                            while closed_oracle.len() > crate::session::CLOSED_TABS_LIMIT {
                                closed_oracle.pop_back();
                            }
                        }
                    }
                    ClosedTabsStackOp::ReopenTab => {
                        if let Some(expected_id) = closed_oracle.pop_front() {
                            let tabs_len_before = midway.tabs.len();

                            handle_closed_tab_reopened(&mut midway);

                            prop_assert_eq!(midway.tabs.len(), tabs_len_before + 1);
                            let active_index = midway
                                .active_tab
                                .expect("debe haber una tab activa tras reabrir");
                            prop_assert_eq!(active_index, midway.tabs.len() - 1);
                            prop_assert_eq!(
                                &midway.tabs[active_index].id,
                                &expected_id,
                                "reabrir debe devolver exactamente la tab cerrada más recientemente"
                            );
                        } else {
                            let tabs_len_before = midway.tabs.len();

                            handle_closed_tab_reopened(&mut midway);

                            prop_assert_eq!(
                                midway.tabs.len(),
                                tabs_len_before,
                                "reabrir con el stack de tabs cerradas vacío debe ser un no-op"
                            );
                        }
                    }
                }

                // Invariante verificada tras CADA operación (no solo al
                // final de la secuencia): el stack nunca excede el límite,
                // y su contenido completo coincide exactamente con el
                // oráculo independiente.
                prop_assert!(midway.closed_tabs.len() <= crate::session::CLOSED_TABS_LIMIT);
                prop_assert_eq!(midway.closed_tabs.len(), closed_oracle.len());
                for (actual, expected) in midway.closed_tabs.iter().zip(closed_oracle.iter()) {
                    prop_assert_eq!(&actual.id, expected);
                }
            }
        }
    }

    // -----------------------------------------------------------------
    // Tarea 11.10 (Requisito 6.6): aviso de unsaved changes al cerrar una
    // tab con cambios sin guardar.
    // -----------------------------------------------------------------

    /// Cerrar una tab "dirty" (draft no vacío, nunca asociada a un
    /// `SavedRequestRecord`) muestra el aviso de unsaved changes en lugar
    /// de cerrarla de inmediato: la tab permanece en `state.tabs`, no se
    /// empuja a `closed_tabs`, y `state.unsaved_changes_prompt` queda
    /// `Some` apuntando a esa tab.
    #[test]
    fn closing_a_dirty_tab_shows_the_unsaved_changes_prompt_instead_of_closing() {
        let mut draft = create_blank_draft();
        draft.url = "https://example.com/unsaved".to_string();
        let mut midway = build_test_midway(draft);
        let tab_id = midway.tabs[0].id.clone();

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id.clone(),
            },
        );

        assert_eq!(
            midway.tabs.len(),
            1,
            "la tab dirty no debe cerrarse todavía"
        );
        assert!(
            midway.closed_tabs.is_empty(),
            "no debe empujarse al stack de tabs cerradas todavía"
        );
        let prompt = midway
            .unsaved_changes_prompt
            .as_ref()
            .expect("debe mostrarse el aviso de unsaved changes");
        assert_eq!(prompt.tab_id, tab_id);
        assert!(!prompt.saving);
        assert!(prompt.error.is_none());
    }

    /// Cerrar una tab limpia (no-dirty: draft vacío, o draft idéntico a su
    /// `saved_draft`) se cierra de inmediato, sin mostrar el aviso
    /// (comportamiento preexistente de la Tarea 11.8, que esta tarea NO
    /// debe alterar).
    #[test]
    fn closing_a_clean_tab_closes_immediately_without_showing_the_prompt() {
        let mut midway = build_test_midway(create_blank_draft());
        let tab_id = midway.tabs[0].id.clone();

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id.clone(),
            },
        );

        assert!(
            midway.tabs.is_empty(),
            "la tab limpia debe cerrarse de inmediato"
        );
        assert_eq!(midway.closed_tabs.len(), 1);
        assert_eq!(midway.closed_tabs.front().unwrap().id, tab_id);
        assert!(
            midway.unsaved_changes_prompt.is_none(),
            "no debe mostrarse ningún aviso para una tab limpia"
        );
    }

    /// Una tab con `saved_draft: Some(...)` cuyo draft actual es
    /// EXACTAMENTE igual a `saved_draft` (por ejemplo, se abrió desde el
    /// palette y no se editó nada) NO es dirty: cerrarla debe cerrarla de
    /// inmediato.
    #[test]
    fn closing_a_tab_with_draft_matching_saved_draft_closes_immediately() {
        let mut midway = build_test_midway(create_blank_draft());
        let saved = midway.tabs[0].draft.clone();
        midway.tabs[0].saved_draft = Some(saved);
        let tab_id = midway.tabs[0].id.clone();

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id.clone(),
            },
        );

        assert!(midway.tabs.is_empty());
        assert!(midway.unsaved_changes_prompt.is_none());
    }

    /// Una tab con `saved_draft: Some(...)` cuyo draft actual DIFIERE de
    /// `saved_draft` es dirty (Property 22: "draft distinto de su último
    /// saved_draft"), incluso si el draft en sí no está "vacío" en el
    /// sentido de `is_draft_empty`.
    #[test]
    fn closing_a_tab_with_draft_differing_from_saved_draft_shows_the_prompt() {
        let mut midway = build_test_midway(create_blank_draft());
        let saved = midway.tabs[0].draft.clone();
        midway.tabs[0].saved_draft = Some(saved);
        midway.tabs[0].draft.url = "https://example.com/edited-after-save".to_string();
        let tab_id = midway.tabs[0].id.clone();

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id.clone(),
            },
        );

        assert_eq!(midway.tabs.len(), 1);
        assert!(midway.unsaved_changes_prompt.is_some());
    }

    /// "Cancelar" descarta el aviso sin cerrar la tab ni mutar su
    /// draft/saved_draft de ninguna forma.
    #[test]
    fn unsaved_changes_cancel_leaves_the_tab_open_and_unchanged() {
        let mut draft = create_blank_draft();
        draft.url = "https://example.com/unsaved".to_string();
        let mut midway = build_test_midway(draft.clone());
        let tab_id = midway.tabs[0].id.clone();
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id.clone(),
            },
        );
        assert!(
            midway.unsaved_changes_prompt.is_some(),
            "precondición: el aviso debe estar abierto"
        );

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::UnsavedChangesCancelRequested,
        );

        assert!(
            midway.unsaved_changes_prompt.is_none(),
            "Cancelar debe descartar el aviso"
        );
        assert_eq!(midway.tabs.len(), 1, "Cancelar no debe cerrar la tab");
        assert_eq!(midway.tabs[0].id, tab_id);
        assert_eq!(
            midway.tabs[0].draft, draft,
            "Cancelar no debe mutar el draft"
        );
        assert!(
            midway.tabs[0].saved_draft.is_none(),
            "Cancelar no debe mutar saved_draft"
        );
    }

    /// "Descartar" cierra la tab de inmediato (misma lógica que
    /// `handle_tab_closed`) SIN invocar ningún flujo de persistencia:
    /// `saved_draft` no se altera (la tab entera se descarta), y la tab
    /// termina en el stack de tabs cerradas exactamente igual que un
    /// cierre no-dirty.
    #[test]
    fn unsaved_changes_discard_closes_the_tab_without_persisting() {
        let mut draft = create_blank_draft();
        draft.url = "https://example.com/unsaved".to_string();
        let mut midway = build_test_midway(draft);
        let tab_id = midway.tabs[0].id.clone();
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id.clone(),
            },
        );
        assert!(
            midway.unsaved_changes_prompt.is_some(),
            "precondición: el aviso debe estar abierto"
        );

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::UnsavedChangesDiscardRequested,
        );

        assert!(
            midway.unsaved_changes_prompt.is_none(),
            "Descartar debe cerrar el aviso"
        );
        assert!(midway.tabs.is_empty(), "Descartar debe cerrar la tab");
        assert_eq!(midway.closed_tabs.len(), 1);
        assert_eq!(midway.closed_tabs.front().unwrap().id, tab_id);
    }

    /// "Guardar" sobre el resultado exitoso de `UnsavedChangesSaveCompleted`
    /// (Tarea 11.10): fija `saved_draft` en el draft persistido y cierra
    /// la tab (misma lógica que un cierre no-dirty), descartando el
    /// aviso.
    ///
    /// Este test ejercita únicamente `handle_unsaved_changes_save_completed`
    /// (el handler síncrono del resultado), no el cuerpo async
    /// `persist_tab_draft` en sí (que requiere un `SqliteRepository` real
    /// y se cubre mejor con un test de integración si fuera necesario):
    /// simula el resultado exitoso directamente, igual que los tests
    /// existentes de `SendCompleted`/`PreviewLoaded` simulan sus
    /// resultados asíncronos sin pasar por `Task::perform`.
    #[test]
    fn unsaved_changes_save_completed_success_sets_saved_draft_and_closes_the_tab() {
        let mut draft = create_blank_draft();
        draft.url = "https://example.com/unsaved".to_string();
        let mut midway = build_test_midway(draft);
        let tab_id = midway.tabs[0].id.clone();
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id.clone(),
            },
        );
        assert!(
            midway.unsaved_changes_prompt.is_some(),
            "precondición: el aviso debe estar abierto"
        );

        let mut persisted_draft = midway.tabs[0].draft.clone();
        persisted_draft.id = Some("assigned-by-save-request".to_string());

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::UnsavedChangesSaveCompleted(Ok(SavedRequestSaveOutcome {
                tab_id: tab_id.clone(),
                saved_draft: persisted_draft.clone(),
            })),
        );

        assert!(
            midway.unsaved_changes_prompt.is_none(),
            "Guardar exitoso debe descartar el aviso"
        );
        assert!(midway.tabs.is_empty(), "Guardar exitoso debe cerrar la tab");
        assert_eq!(midway.closed_tabs.len(), 1);
        assert_eq!(midway.closed_tabs.front().unwrap().id, tab_id);
    }

    /// "Guardar" sobre un resultado de error (Tarea 11.10): el aviso
    /// permanece abierto con el mensaje de error visible, sin cerrar la
    /// tab ni mutar su draft/saved_draft.
    #[test]
    fn unsaved_changes_save_completed_error_keeps_the_prompt_open_without_closing_the_tab() {
        let mut draft = create_blank_draft();
        draft.url = "https://example.com/unsaved".to_string();
        let mut midway = build_test_midway(draft.clone());
        let tab_id = midway.tabs[0].id.clone();
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::TabClosed {
                tab_id: tab_id.clone(),
            },
        );
        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::UnsavedChangesSaveRequested,
        );

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::UnsavedChangesSaveCompleted(Err(
                "fallo simulado de guardado".to_string()
            )),
        );

        assert_eq!(
            midway.tabs.len(),
            1,
            "un error de Guardar no debe cerrar la tab"
        );
        assert_eq!(
            midway.tabs[0].draft, draft,
            "un error de Guardar no debe mutar el draft"
        );
        assert!(
            midway.tabs[0].saved_draft.is_none(),
            "un error de Guardar no debe mutar saved_draft"
        );
        let prompt = midway
            .unsaved_changes_prompt
            .as_ref()
            .expect("el aviso debe permanecer abierto tras un error");
        assert_eq!(prompt.tab_id, tab_id);
        assert!(!prompt.saving, "saving debe volver a false tras el error");
        assert!(
            prompt.error.is_some(),
            "debe quedar visible el mensaje de error"
        );
    }

    // -----------------------------------------------------------------
    // Tarea 11.11 (Property 22, Requisito 6.6): las tres acciones del
    // aviso de unsaved changes (Guardar/Descartar/Cancelar) son correctas
    // sobre CUALQUIER tab dirty, no solo los ejemplos puntuales de la
    // Tarea 11.10 (arriba).
    //
    // Generador de tab "dirty" (`arb_dirty_tab_state`): cubre ambos
    // sub-casos de `tab_is_dirty` (ver su doc) para no depender de un
    // único camino:
    // - `saved_draft: Some(saved)` con `draft` distinto de `saved`
    //   (se fuerza la diferencia editando la URL, en vez de generar dos
    //   drafts independientes y filtrar por desigualdad, para que el caso
    //   nunca se descarte por shrinking/colisión).
    // - `saved_draft: None` con un draft con contenido real
    //   (`arb_request_draft` siempre genera una URL no vacía, así que
    //   `is_draft_empty` es siempre `false` y este caso es dirty por
    //   construcción).
    //
    // Para "Guardar" se simula el resultado async exitoso directamente vía
    // `handle_unsaved_changes_save_completed` (mismo enfoque que los unit
    // tests de la Tarea 11.10: `persist_tab_draft` requiere un
    // `SqliteRepository` real y no es el objeto de esta property). Como
    // `TabSnapshot` no conserva `saved_draft` por separado y la tab se
    // quita de `state.tabs` al cerrarse, la observación de
    // "saved_draft == draft" en el momento del cierre se hace vía el
    // snapshot empujado a `closed_tabs`: su campo `draft` refleja
    // `tab.draft` YA mutado a `outcome.saved_draft` por
    // `handle_unsaved_changes_save_completed` antes de invocar
    // `handle_tab_closed`, así que comparar ese snapshot contra el draft
    // persistido es equivalente a verificar que ambos coincidían al cerrar.
    // -----------------------------------------------------------------

    /// Acción elegida por el usuario en el aviso de unsaved changes.
    #[derive(Debug, Clone)]
    enum UnsavedChangesPromptAction {
        Save,
        Discard,
        Cancel,
    }

    fn arb_unsaved_changes_prompt_action() -> impl Strategy<Value = UnsavedChangesPromptAction> {
        prop_oneof![
            Just(UnsavedChangesPromptAction::Save),
            Just(UnsavedChangesPromptAction::Discard),
            Just(UnsavedChangesPromptAction::Cancel),
        ]
    }

    /// Genera `(draft, saved_draft)` para una tab garantizada dirty según
    /// `tab_is_dirty` (ver doc de esa función para el porqué de los dos
    /// casos). Reutiliza `arb_request_draft` (Property 8, Tarea 3.21).
    fn arb_dirty_tab_state() -> impl Strategy<Value = (RequestDraft, Option<RequestDraft>)> {
        prop_oneof![
            // Caso A: la tab ya tenía un `saved_draft`, pero el draft
            // actual difiere (enunciado literal de la Property 22: "draft
            // distinto de su último saved_draft").
            arb_request_draft().prop_map(|saved| {
                let mut draft = saved.clone();
                draft.url = format!("{}-edited", draft.url);
                (draft, Some(saved))
            }),
            // Caso B: la tab nunca se asoció a un `SavedRequestRecord`
            // (`saved_draft: None`), pero tiene contenido real.
            arb_request_draft().prop_map(|draft| (draft, None)),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 22: Las acciones del
        /// aviso de unsaved changes son correctas.
        /// Validates: Requirements 6.6
        ///
        /// Para cualquier tab con un draft distinto de su último
        /// `saved_draft` (`arb_dirty_tab_state`), al mostrarse el aviso de
        /// unsaved changes (`close_tab_or_prompt_unsaved_changes`):
        ///
        /// - Guardar (simulando un resultado async exitoso) SHALL cerrar
        ///   la tab y persistir el draft: el snapshot empujado a
        ///   `closed_tabs` SHALL coincidir exactamente con el draft
        ///   persistido (`saved_draft == draft` en el momento del cierre).
        /// - Descartar SHALL cerrar la tab SIN persistir: el snapshot
        ///   empujado a `closed_tabs` SHALL conservar el draft ORIGINAL
        ///   (sin guardar), no ninguna versión hipotéticamente persistida.
        /// - Cancelar SHALL dejar la tab abierta, con `draft`,
        ///   `saved_draft` e `id` exactamente iguales a como estaban antes
        ///   del aviso, y SHALL descartar el aviso.
        #[test]
        fn property_22_unsaved_changes_prompt_actions_are_correct(
            (draft, saved_draft) in arb_dirty_tab_state(),
            action in arb_unsaved_changes_prompt_action(),
        ) {
            let mut midway = build_test_midway(draft.clone());
            midway.tabs[0].saved_draft = saved_draft.clone();
            let tab_id = midway.tabs[0].id.clone();

            prop_assert!(tab_is_dirty(&midway.tabs[0]), "precondición: la tab generada debe ser dirty");

            close_tab_or_prompt_unsaved_changes(&mut midway, tab_id.clone());
            prop_assert!(
                midway.tabs.iter().any(|tab| tab.id == tab_id),
                "la tab dirty no debe cerrarse antes de resolver el aviso"
            );
            prop_assert!(midway.unsaved_changes_prompt.is_some(), "debe mostrarse el aviso de unsaved changes");

            match action {
                UnsavedChangesPromptAction::Save => {
                    let mut persisted_draft = draft.clone();
                    persisted_draft.id = Some(format!("assigned-{tab_id}"));

                    handle_unsaved_changes_save_completed(
                        &mut midway,
                        Ok(SavedRequestSaveOutcome { tab_id: tab_id.clone(), saved_draft: persisted_draft.clone() }),
                    );

                    prop_assert!(midway.unsaved_changes_prompt.is_none(), "Guardar debe descartar el aviso");
                    prop_assert!(
                        !midway.tabs.iter().any(|tab| tab.id == tab_id),
                        "Guardar debe cerrar la tab"
                    );
                    let closed = midway.closed_tabs.front().expect("Guardar debe empujar la tab a closed_tabs");
                    prop_assert_eq!(&closed.id, &tab_id);
                    prop_assert_eq!(
                        &closed.draft,
                        &persisted_draft,
                        "el draft del snapshot cerrado debe ser EXACTAMENTE el draft persistido (saved_draft == draft)"
                    );
                }
                UnsavedChangesPromptAction::Discard => {
                    handle_unsaved_changes_discard_requested(&mut midway);

                    prop_assert!(midway.unsaved_changes_prompt.is_none(), "Descartar debe descartar el aviso");
                    prop_assert!(
                        !midway.tabs.iter().any(|tab| tab.id == tab_id),
                        "Descartar debe cerrar la tab"
                    );
                    let closed = midway.closed_tabs.front().expect("Descartar debe empujar la tab a closed_tabs");
                    prop_assert_eq!(&closed.id, &tab_id);
                    prop_assert_eq!(
                        &closed.draft,
                        &draft,
                        "Descartar no debe persistir: el snapshot cerrado debe conservar el draft ORIGINAL sin guardar"
                    );
                }
                UnsavedChangesPromptAction::Cancel => {
                    let _ = update_request_composer(&mut midway, RequestComposerMessage::UnsavedChangesCancelRequested);

                    prop_assert!(midway.unsaved_changes_prompt.is_none(), "Cancelar debe descartar el aviso");
                    let tab = midway
                        .tabs
                        .iter()
                        .find(|tab| tab.id == tab_id)
                        .expect("Cancelar no debe cerrar la tab");
                    prop_assert_eq!(&tab.id, &tab_id, "Cancelar no debe cambiar el id de la tab");
                    prop_assert_eq!(&tab.draft, &draft, "Cancelar no debe mutar el draft");
                    prop_assert_eq!(&tab.saved_draft, &saved_draft, "Cancelar no debe mutar saved_draft");
                }
            }
        }
    }

    // -----------------------------------------------------------------
    // Tarea 11.12 (Requisito 6.8): tabla global de shortcuts de teclado.
    //
    // No hay una property numerada asignada a esta tarea (la Tarea 11.13,
    // "Escribir unit tests de la tabla fija de shortcuts", pide
    // explícitamente unit tests, no property tests): se cubren aquí los
    // handlers de `update()` a los que despacha cada shortcut de la
    // tabla, siguiendo el mismo enfoque que la Tarea 11.3 usó para el
    // Command Palette (verificar el mensaje/acción despachado, no la
    // subscription de teclado en sí, que no es practicable de testear
    // unitariamente sin un backend de eventos real).
    //
    // De los shortcuts que se despachan DIRECTO como un mensaje de área ya
    // existente: Ctrl+Shift+T -> `ClosedTabReopened` (Tarea 11.8) y
    // Ctrl/Cmd+K -> `PaletteMessage::Toggled` (Tarea 11.2) ya tienen su
    // propio handler cubierto por tests de esas tareas, así que no se
    // duplican aquí. Ctrl+Enter -> `SendPressed`, Ctrl+Shift+P ->
    // `SettingsPressed` y Ctrl+. -> `WorkspaceMessage::ToggleCollapsed`
    // NO tenían, hasta esta tarea, ningún test que ejercitara el handler
    // en sí (los tests de las Tareas 3.18/3.19 y 3.20 solo cubren
    // `execute_send`/`handle_send_completed`/`compute_preview`, no
    // `handle_send_pressed`/`handle_settings_pressed`; el toggle de
    // `WorkspaceMessage::ToggleCollapsed` de la Tarea 7.1 no tenía ningún
    // test): se agregan aquí. El resto de esta sección cubre
    // específicamente `update_keyboard` (Ctrl+S, Ctrl+Shift+N, Ctrl+W,
    // Alt+1..9, Esc).
    // -----------------------------------------------------------------

    /// Ctrl+Enter ("Send"): despacha `RequestComposerMessage::SendPressed`,
    /// que debe marcar la tab activa como `sending` y asignarle un
    /// `execution_id` (la ejecución real de `execute_send` vía
    /// `Task::perform` está fuera de alcance de este test unitario; el
    /// flujo end-to-end contra un servidor mock ya está cubierto por
    /// `send_flow_end_to_end_hits_mock_server_and_populates_response`,
    /// Tarea 3.19).
    #[test]
    fn keyboard_ctrl_enter_send_pressed_marks_active_tab_as_sending() {
        let mut midway = build_test_midway(create_blank_draft());
        assert!(!midway.tabs[0].sending);

        let _ = update_request_composer(&mut midway, RequestComposerMessage::SendPressed);

        assert!(
            midway.tabs[0].sending,
            "Ctrl+Enter debe disparar el envío de la tab activa"
        );
        assert!(midway.tabs[0].execution_id.is_some());
    }

    /// Ctrl+Shift+P ("Preview"): despacha
    /// `RequestComposerMessage::SettingsPressed`, que en su camino de
    /// cierre (preview ya abierto) es sincrónico: alterna
    /// `tab.preview` a `None` sin disparar ningún `Task::perform`. Se
    /// ejercita ese camino (en lugar del de apertura, que requiere un
    /// draft resoluble y ya está cubierto por
    /// `property_8_preview_matches_current_draft_exactly`, Tarea 3.21)
    /// para verificar el toggle en sí de forma puramente sincrónica.
    #[test]
    fn keyboard_ctrl_shift_p_settings_pressed_closes_an_already_open_preview() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.tabs[0].preview = Some(dummy_request_preview());

        let _ = update_request_composer(&mut midway, RequestComposerMessage::SettingsPressed);

        assert!(
            midway.tabs[0].preview.is_none(),
            "Ctrl+Shift+P debe cerrar el preview ya abierto"
        );
    }

    /// Ctrl+. ("Tools/Workspace"): despacha
    /// `WorkspaceMessage::ToggleCollapsed`, que alterna
    /// `state.workspace_panel.collapsed` (Tarea 7.1, Requisito 4.1).
    #[test]
    fn keyboard_ctrl_dot_toggle_collapsed_flips_workspace_panel_collapsed() {
        let mut midway = build_test_midway(create_blank_draft());
        assert!(!midway.workspace_panel.collapsed);

        let _ = update_workspace(&mut midway, WorkspaceMessage::ToggleCollapsed);
        assert!(
            midway.workspace_panel.collapsed,
            "Ctrl+. debe colapsar el panel cuando estaba expandido"
        );

        let _ = update_workspace(&mut midway, WorkspaceMessage::ToggleCollapsed);
        assert!(
            !midway.workspace_panel.collapsed,
            "una segunda pulsación debe expandirlo de nuevo"
        );
    }

    /// Ctrl+S abre el mismo diálogo de guardado que el botón del composer.
    #[test]
    fn keyboard_save_requested_opens_save_dialog_for_active_tab() {
        let mut midway = build_test_midway(create_blank_draft());
        let expected_tab_id = midway.tabs[0].id.clone();

        let _ = update_keyboard(&mut midway, KeyboardMessage::SaveRequested);

        let prompt = midway
            .save_request_prompt
            .as_ref()
            .expect("Ctrl+S debe abrir el diálogo de guardado");
        assert_eq!(prompt.tab_id, expected_tab_id);
        assert_eq!(prompt.name_input, "Nueva petición");
    }

    #[test]
    fn keyboard_save_requested_without_active_tab_is_a_no_op() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.active_tab = None;

        let _ = update_keyboard(&mut midway, KeyboardMessage::SaveRequested);

        assert!(midway.save_request_prompt.is_none());
    }

    fn test_collection(
        id: &str,
        name: &str,
        folders: Vec<Folder>,
        requests: Vec<midway_core::domain::workspace::SavedRequestRecord>,
    ) -> CollectionWithRequests {
        CollectionWithRequests {
            collection: CollectionSummary {
                id: id.to_string(),
                name: name.to_string(),
                request_count: requests.len() as u64,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            folders,
            requests,
        }
    }

    #[test]
    fn save_dialog_defaults_to_active_collection() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.workspace.collections.push(test_collection(
            "collection-1",
            "My API",
            Vec::new(),
            Vec::new(),
        ));
        midway.active_collection_id = Some("collection-1".to_string());

        open_save_request_prompt(&mut midway);

        let prompt = midway.save_request_prompt.as_ref().unwrap();
        assert_eq!(prompt.collection_id.as_deref(), Some("collection-1"));
        assert!(prompt.folder_id.is_none());
    }

    #[test]
    fn changing_save_collection_resets_selected_folder() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.workspace.collections = vec![
            test_collection("collection-1", "One", Vec::new(), Vec::new()),
            test_collection("collection-2", "Two", Vec::new(), Vec::new()),
        ];
        open_save_request_prompt(&mut midway);
        midway.save_request_prompt.as_mut().unwrap().folder_id = Some("old-folder".to_string());

        let _ = update_request_composer(
            &mut midway,
            RequestComposerMessage::SaveCollectionChanged("collection-2".to_string()),
        );

        let prompt = midway.save_request_prompt.as_ref().unwrap();
        assert_eq!(prompt.collection_id.as_deref(), Some("collection-2"));
        assert!(prompt.folder_id.is_none());
    }

    #[test]
    fn save_completion_updates_tab_baseline_workspace_and_active_collection() {
        let mut midway = build_test_midway(create_blank_draft());
        open_save_request_prompt(&mut midway);
        let tab_id = midway.tabs[0].id.clone();
        let mut saved_draft = midway.tabs[0].draft.clone();
        saved_draft.id = Some("request-1".to_string());
        saved_draft.name = "List users".to_string();
        let record = midway_core::domain::workspace::SavedRequestRecord {
            id: "request-1".to_string(),
            collection_id: "collection-1".to_string(),
            folder_id: None,
            name: saved_draft.name.clone(),
            draft: saved_draft.clone(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let workspace = WorkspaceSnapshot {
            collections: vec![test_collection(
                "collection-1",
                "My API",
                Vec::new(),
                vec![record],
            )],
            environments: Vec::new(),
            history: Vec::new(),
            secrets: Vec::new(),
        };

        handle_save_request_completed(
            &mut midway,
            Ok(RequestSaveOutcome {
                tab_id,
                saved_draft: saved_draft.clone(),
                workspace,
                collection_id: "collection-1".to_string(),
            }),
        );

        assert_eq!(midway.tabs[0].draft, saved_draft);
        assert_eq!(midway.tabs[0].saved_draft, Some(saved_draft));
        assert_eq!(midway.active_collection_id.as_deref(), Some("collection-1"));
        assert_eq!(midway.workspace.collections[0].requests.len(), 1);
        assert!(midway.save_request_prompt.is_none());
        assert!(midway.session.dirty);
    }

    #[test]
    fn initial_workspace_load_restores_saved_baseline_without_overwriting_edits() {
        let mut current_draft = create_blank_draft();
        current_draft.id = Some("request-1".to_string());
        current_draft.name = "Edited locally".to_string();
        let mut midway = build_test_midway(current_draft.clone());
        midway.active_collection_id = Some("collection-1".to_string());

        let mut persisted_draft = current_draft.clone();
        persisted_draft.name = "Persisted name".to_string();
        let record = midway_core::domain::workspace::SavedRequestRecord {
            id: "request-1".to_string(),
            collection_id: "collection-1".to_string(),
            folder_id: None,
            name: persisted_draft.name.clone(),
            draft: persisted_draft.clone(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let snapshot = WorkspaceSnapshot {
            collections: vec![test_collection(
                "collection-1",
                "My API",
                Vec::new(),
                vec![record],
            )],
            environments: Vec::new(),
            history: Vec::new(),
            secrets: Vec::new(),
        };

        handle_workspace_snapshot_loaded(&mut midway, Ok(snapshot));

        assert_eq!(midway.tabs[0].draft, current_draft);
        assert_eq!(midway.tabs[0].saved_draft, Some(persisted_draft));
        assert_eq!(midway.active_collection_id.as_deref(), Some("collection-1"));
    }

    /// Ctrl+Shift+N ("Nuevo request"): abre una tab en blanco adicional y
    /// la activa, igual que la acción "Nuevo request" del Command
    /// Palette.
    #[test]
    fn keyboard_new_blank_tab_requested_opens_and_activates_blank_tab() {
        let mut midway = build_test_midway(create_blank_draft());
        let tabs_before = midway.tabs.len();

        let _ = update_keyboard(&mut midway, KeyboardMessage::NewBlankTabRequested);

        assert_eq!(midway.tabs.len(), tabs_before + 1);
        assert_eq!(midway.active_tab, Some(midway.tabs.len() - 1));
        assert!(
            midway.session.dirty,
            "abrir una tab nueva debe marcar la sesión como dirty"
        );
    }

    /// Ctrl+W ("Cerrar tab activa"): cierra la tab activa (identificada en
    /// `update_keyboard`, no en la subscription) y la empuja al stack de
    /// tabs cerradas, igual que `RequestComposerMessage::TabClosed`.
    #[test]
    fn keyboard_close_active_tab_shortcut_closes_the_active_tab() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.tabs.push(RequestTabState::blank());
        let active_tab_id = midway.tabs[1].id.clone();
        midway.active_tab = Some(1);

        let _ = update_keyboard(&mut midway, KeyboardMessage::CloseActiveTabShortcut);

        assert_eq!(midway.tabs.len(), 1);
        assert!(!midway.tabs.iter().any(|tab| tab.id == active_tab_id));
        assert_eq!(midway.closed_tabs.front().unwrap().id, active_tab_id);
    }

    /// Ctrl+W sin ninguna tab activa (`active_tab` es `None`) es un no-op:
    /// no debe entrar en pánico.
    #[test]
    fn keyboard_close_active_tab_shortcut_with_no_active_tab_is_a_no_op() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.active_tab = None;
        let tabs_before = midway.tabs.len();

        let _ = update_keyboard(&mut midway, KeyboardMessage::CloseActiveTabShortcut);

        assert_eq!(midway.tabs.len(), tabs_before);
        assert!(midway.closed_tabs.is_empty());
    }

    /// Alt+1..9 ("ir a tab N"): `GoToTabShortcut { index }` activa la tab
    /// en esa posición (0-based) cuando existe.
    #[test]
    fn keyboard_go_to_tab_shortcut_activates_tab_at_index() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.tabs.push(RequestTabState::blank());
        midway.tabs.push(RequestTabState::blank());
        midway.active_tab = Some(0);

        let _ = update_keyboard(&mut midway, KeyboardMessage::GoToTabShortcut { index: 2 });

        assert_eq!(midway.active_tab, Some(2));
    }

    /// Alt+9 con menos de 9 tabs abiertas (`index` fuera de rango) es un
    /// no-op: no debe mutar `active_tab` ni entrar en pánico.
    #[test]
    fn keyboard_go_to_tab_shortcut_out_of_range_is_a_no_op() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.tabs.push(RequestTabState::blank());
        midway.active_tab = Some(0);

        let _ = update_keyboard(&mut midway, KeyboardMessage::GoToTabShortcut { index: 8 });

        assert_eq!(midway.active_tab, Some(0));
    }

    /// Construye un `RequestPreview` mínimo de prueba (sin depender de
    /// `resolve_request` sobre un draft en blanco, que falla su validación
    /// de URL): estos tests solo necesitan que `tab.preview` sea `Some`,
    /// no un preview con contenido realista.
    fn dummy_request_preview() -> RequestPreview {
        RequestPreview {
            method: HttpMethod::GET,
            resolved_url: "https://ejemplo.com".to_string(),
            headers: Vec::new(),
            body_text: None,
            curl_command: "curl https://ejemplo.com".to_string(),
            environment_name: None,
            used_secret_aliases: Vec::new(),
            missing_secret_aliases: Vec::new(),
        }
    }

    /// Esc con el Command Palette abierto lo cierra PRIMERO (precedencia
    /// documentada en `KeyboardMessage::EscapePressed`), incluso si el
    /// preview de la tab activa también está abierto.
    #[test]
    fn keyboard_escape_closes_palette_first_when_both_palette_and_preview_are_open() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.is_open = true;
        midway.palette.query = "algo".to_string();
        midway.tabs[0].preview = Some(dummy_request_preview());

        let _ = update_keyboard(&mut midway, KeyboardMessage::EscapePressed);

        assert!(
            !midway.palette.is_open,
            "Esc debe cerrar el palette primero"
        );
        assert!(midway.palette.query.is_empty());
        assert!(
            midway.tabs[0].preview.is_some(),
            "el preview no debe cerrarse en esta pulsación de Esc"
        );
    }

    /// Esc con el palette cerrado pero el preview de la tab activa
    /// abierto cierra el preview (vía el mismo toggle que
    /// `SettingsPressed`).
    #[test]
    fn keyboard_escape_closes_active_tab_preview_when_palette_is_closed() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.is_open = false;
        midway.tabs[0].preview = Some(dummy_request_preview());

        let _ = update_keyboard(&mut midway, KeyboardMessage::EscapePressed);

        assert!(
            midway.tabs[0].preview.is_none(),
            "Esc debe cerrar el preview cuando el palette está cerrado"
        );
    }

    /// Esc sin ningún overlay abierto (ni palette ni preview) es un
    /// no-op.
    #[test]
    fn keyboard_escape_with_nothing_open_is_a_no_op() {
        let mut midway = build_test_midway(create_blank_draft());
        midway.palette.is_open = false;
        midway.tabs[0].preview = None;

        let _ = update_keyboard(&mut midway, KeyboardMessage::EscapePressed);

        assert!(!midway.palette.is_open);
        assert!(midway.tabs[0].preview.is_none());
    }

    // -----------------------------------------------------------------
    // Tarea 11.6 (Requisito 6.4): restauración de sesión al reabrir la
    // aplicación, vía `restore_pending_session`.
    //
    // `restore_pending_session` opera solo sobre `&mut SessionStoreState`
    // (ver su documentación), así que estos tests lo ejercitan
    // directamente sin pasar por `Midway::new`/`AppState::initialize` ni
    // por I/O real de `session.json`.
    // -----------------------------------------------------------------

    /// Construye un `TabSnapshot` de prueba con un `id` e `url` dados
    /// (el resto de los campos del `draft` quedan en su valor por
    /// defecto de una tab en blanco), y una `active_request_tab` fija.
    fn tab_snapshot(id: &str, url: &str) -> TabSnapshot {
        let mut draft = create_blank_draft();
        draft.url = url.to_string();
        TabSnapshot {
            id: id.to_string(),
            draft,
            active_request_tab: RequestTab::Headers,
        }
    }

    fn sample_panel_sizes() -> crate::session::PanelSizes {
        crate::session::PanelSizes {
            workspace_panel_width: 321.0,
            response_panel_height: 654.0,
            request_panel_width: 432.0,
        }
    }

    /// Restaurar una sesión con 2+ tabs abiertas restaura todas con su
    /// contenido/id correcto (preservados por `RequestTabState::from_snapshot`).
    #[test]
    fn restore_pending_session_restores_all_open_tabs_with_correct_content_and_ids() {
        let snapshot = crate::session::SessionSnapshot {
            version: crate::session::SESSION_SCHEMA_VERSION,
            active_tab_id: Some("tab-b".to_string()),
            open_tabs: vec![
                tab_snapshot("tab-a", "https://a.example.com"),
                tab_snapshot("tab-b", "https://b.example.com"),
                tab_snapshot("tab-c", "https://c.example.com"),
            ],
            closed_tabs: Vec::new(),
            panel_sizes: sample_panel_sizes(),
            theme_mode: ThemeMode::default(),
            saved_at: "2024-01-01T00:00:00Z".to_string(),
            active_collection_id: None,
        };
        let mut session = SessionStoreState {
            pending_restore: Some(snapshot),
            ..SessionStoreState::default()
        };

        let (tabs, _active_tab, _session_collection_id) = restore_pending_session(&mut session);

        assert_eq!(tabs.len(), 3);
        assert_eq!(tabs[0].id, "tab-a");
        assert_eq!(tabs[0].draft.url, "https://a.example.com");
        assert_eq!(tabs[0].active_request_tab, RequestTab::Headers);
        assert_eq!(tabs[1].id, "tab-b");
        assert_eq!(tabs[1].draft.url, "https://b.example.com");
        assert_eq!(tabs[2].id, "tab-c");
        assert_eq!(tabs[2].draft.url, "https://c.example.com");
    }

    /// `active_tab_id` se resuelve al índice de la tab restaurada cuyo
    /// `id` coincide.
    #[test]
    fn restore_pending_session_resolves_active_tab_id_to_matching_index() {
        let snapshot = crate::session::SessionSnapshot {
            version: crate::session::SESSION_SCHEMA_VERSION,
            active_tab_id: Some("tab-c".to_string()),
            open_tabs: vec![
                tab_snapshot("tab-a", "https://a.example.com"),
                tab_snapshot("tab-b", "https://b.example.com"),
                tab_snapshot("tab-c", "https://c.example.com"),
            ],
            closed_tabs: Vec::new(),
            panel_sizes: sample_panel_sizes(),
            theme_mode: ThemeMode::default(),
            saved_at: "2024-01-01T00:00:00Z".to_string(),
            active_collection_id: None,
        };
        let mut session = SessionStoreState {
            pending_restore: Some(snapshot),
            ..SessionStoreState::default()
        };

        let (_tabs, active_tab, _active_collection_id) = restore_pending_session(&mut session);

        assert_eq!(active_tab, Some(2));
    }

    /// Un `active_tab_id` que no coincide con ninguna tab restaurada
    /// (referencia obsoleta o corrupta) cae a `Some(0)`, ya que hay al
    /// menos una tab restaurada.
    #[test]
    fn restore_pending_session_falls_back_to_index_zero_on_stale_active_tab_id() {
        let snapshot = crate::session::SessionSnapshot {
            version: crate::session::SESSION_SCHEMA_VERSION,
            active_tab_id: Some("tab-does-not-exist".to_string()),
            open_tabs: vec![
                tab_snapshot("tab-a", "https://a.example.com"),
                tab_snapshot("tab-b", "https://b.example.com"),
            ],
            closed_tabs: Vec::new(),
            panel_sizes: sample_panel_sizes(),
            theme_mode: ThemeMode::default(),
            saved_at: "2024-01-01T00:00:00Z".to_string(),
            active_collection_id: None,
        };
        let mut session = SessionStoreState {
            pending_restore: Some(snapshot),
            ..SessionStoreState::default()
        };

        let (tabs, active_tab, _active_collection_id) = restore_pending_session(&mut session);

        assert_eq!(tabs.len(), 2);
        assert_eq!(active_tab, Some(0));
    }

    /// `panel_sizes` del snapshot restaurado se copia a
    /// `session.panel_sizes`.
    #[test]
    fn restore_pending_session_populates_session_panel_sizes() {
        let expected_panel_sizes = sample_panel_sizes();
        let snapshot = crate::session::SessionSnapshot {
            version: crate::session::SESSION_SCHEMA_VERSION,
            active_tab_id: None,
            open_tabs: vec![tab_snapshot("tab-a", "https://a.example.com")],
            closed_tabs: Vec::new(),
            panel_sizes: expected_panel_sizes.clone(),
            theme_mode: ThemeMode::default(),
            saved_at: "2024-01-01T00:00:00Z".to_string(),
            active_collection_id: None,
        };
        let mut session = SessionStoreState {
            pending_restore: Some(snapshot),
            ..SessionStoreState::default()
        };

        let _ = restore_pending_session(&mut session);

        assert_eq!(session.panel_sizes, expected_panel_sizes);
    }

    /// Una sesión válida guardada con `open_tabs` vacío cae al mismo
    /// fallback que el arranque sin sesión previa: una única tab en
    /// blanco con `active_tab: Some(0)`, en vez de arrancar con cero
    /// tabs.
    #[test]
    fn restore_pending_session_with_empty_open_tabs_falls_back_to_single_blank_tab() {
        let snapshot = crate::session::SessionSnapshot {
            version: crate::session::SESSION_SCHEMA_VERSION,
            active_tab_id: None,
            open_tabs: Vec::new(),
            closed_tabs: Vec::new(),
            panel_sizes: sample_panel_sizes(),
            theme_mode: ThemeMode::default(),
            saved_at: "2024-01-01T00:00:00Z".to_string(),
            active_collection_id: None,
        };
        let mut session = SessionStoreState {
            pending_restore: Some(snapshot),
            ..SessionStoreState::default()
        };

        let (tabs, active_tab, _active_collection_id) = restore_pending_session(&mut session);

        assert_eq!(tabs.len(), 1);
        assert!(is_draft_empty(&tabs[0].draft));
        assert_eq!(active_tab, Some(0));
    }

    /// Sin `pending_restore` (`None`), el comportamiento por defecto
    /// (una tab en blanco, `active_tab: Some(0)`) permanece sin cambios.
    #[test]
    fn restore_pending_session_with_no_pending_restore_uses_default_blank_tab() {
        let mut session = SessionStoreState::default();
        assert!(session.pending_restore.is_none());

        let (tabs, active_tab, _active_collection_id) = restore_pending_session(&mut session);

        assert_eq!(tabs.len(), 1);
        assert!(is_draft_empty(&tabs[0].draft));
        assert_eq!(active_tab, Some(0));
    }

    // -----------------------------------------------------------------
    // Tarea 11.5 (Property 19, Requisito 6.3): el autosave persiste dentro
    // de 2 segundos desde la última modificación.
    //
    // Nota sobre "reloj simulado": el timer real de autosave
    // (`iced::time::every(Duration::from_secs(1))`, ver `subscription()`,
    // Tarea 11.4) no expone ningún punto de inyección para un fake clock:
    // construye la subscription directamente contra el reloj de pared del
    // sistema, y `iced` no ofrece una API para sustituirlo en tests.
    // Introducir una capa de reloj inyectable ahí sería un refactor de la
    // infraestructura de subscriptions de `iced` ajeno al alcance de esta
    // tarea. Esto no es el primer caso en este archivo en el que se adapta
    // pragmáticamente el enfoque de testing frente a una limitación
    // similar: la Property 18 (Tarea 9.7, `collection_runner.rs`) usa un
    // delay simulado uniforme en lugar de mockear `RequestExecutorHandle`
    // por una razón análoga.
    //
    // Lo que SÍ se puede testear de forma significativa y aleatorizada de
    // la Property 19 es la parte que NO depende del reloj de pared en
    // absoluto: la máquina de estados dirty-tracking que `update_session`
    // implementa (dirty -> tick -> limpio; limpio -> tick -> sigue limpio
    // sin re-escritura), que es el mecanismo del que depende la garantía
    // de "persistir dentro de los 2 segundos". El propio intervalo de 1
    // segundo del timer -ya cómodamente por debajo del máximo de 2
    // segundos del Requisito 6.3- es una constante fija documentada en el
    // comentario de `subscription()` (Tarea 11.4): no hay ningún input
    // variable en "1 segundo <= 2 segundos" que se beneficie de un test de
    // propiedad con valores aleatorios.
    //
    // Por eso este test ejercita `update_session` (el handler real de
    // `AutosaveTick`, invocado directamente, sin pasar por la subscription
    // ni por ningún timer real o simulado) contra secuencias ARBITRARIAS
    // de operaciones mutantes (que marcan `dirty = true`, reutilizando
    // fuentes de mutación ya confirmadas por tareas anteriores:
    // `KeyboardMessage::NewBlankTabRequested`, Tarea 11.12;
    // `handle_tab_closed`/`handle_closed_tab_reopened`, Tarea 11.8)
    // intercaladas con ticks de autosave, verificando el invariante
    // central de la Property 19: inmediatamente después de CUALQUIER
    // `AutosaveTick`, `state.session.dirty` es `false` (proxy observable
    // de "el estado quedó persistido, o marcado como tal, hasta la última
    // modificación"; `Task<Message>` no expone una API de introspección
    // para verificar desde el test si se disparó o no una escritura real,
    // así que se usa la misma transición de `dirty` que ya documentan los
    // tests unitarios existentes de `update_session`, Tarea 11.4), y que
    // un tick sobre un estado ya limpio es un no-op observable (`dirty`
    // permanece `false`, evitando la re-escritura innecesaria exigida por
    // el Requisito 6.3).
    // -----------------------------------------------------------------

    /// Operación abstracta sobre el ciclo dirty-tracking del autosave.
    #[derive(Debug, Clone)]
    enum AutosaveOp {
        /// Marca la sesión como dirty mediante una fuente de mutación real
        /// ya confirmada por una tarea anterior (Tarea 11.12: crear una
        /// tab en blanco vía el shortcut de teclado). Elegida por ser la
        /// fuente de mutación más simple, siempre aplicable
        /// independientemente de cuántas tabs/tabs cerradas haya en ese
        /// momento.
        MarkDirty,
        /// Cierra la tab abierta en la posición `tab_offset` módulo la
        /// cantidad de tabs actualmente abiertas (no-op si no hay ninguna
        /// tab abierta), invocando `handle_tab_closed` directamente
        /// (también marca `dirty = true`, ver Tarea 11.8).
        CloseTab { tab_offset: usize },
        /// Reabre la tab cerrada más recientemente (no-op si el stack de
        /// tabs cerradas está vacío), invocando `handle_closed_tab_reopened`
        /// directamente (también marca `dirty = true` cuando reabre
        /// efectivamente una tab, ver Tarea 11.8).
        ReopenClosedTab,
        /// Dispara un tick del timer de autosave, invocando directamente
        /// `update_session(state, SessionMessage::AutosaveTick)` (el
        /// handler real bajo prueba, Tarea 11.4).
        AutosaveTick,
    }

    fn arb_autosave_op() -> impl Strategy<Value = AutosaveOp> {
        prop_oneof![
            3 => Just(AutosaveOp::MarkDirty),
            2 => (0usize..30usize).prop_map(|tab_offset| AutosaveOp::CloseTab { tab_offset }),
            2 => Just(AutosaveOp::ReopenClosedTab),
            4 => Just(AutosaveOp::AutosaveTick),
        ]
    }

    /// Secuencias de hasta 60 operaciones: suficientemente largas para
    /// ejercitar múltiples ciclos dirty -> tick -> limpio, así como ticks
    /// consecutivos sobre un estado ya limpio.
    fn arb_autosave_ops() -> impl Strategy<Value = Vec<AutosaveOp>> {
        prop::collection::vec(arb_autosave_op(), 0..=60)
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 19: El autosave
        /// persiste dentro de 2 segundos desde la última modificación.
        /// Validates: Requirements 6.3
        ///
        /// Ver el bloque de comentarios justo arriba de este módulo de
        /// tests para la justificación detallada de por qué esta property
        /// testea el ciclo dirty-tracking de `update_session` en lugar de
        /// inyectar un reloj simulado en la subscription de
        /// `iced::time::every` (que no expone ningún punto de inyección
        /// para eso).
        ///
        /// Para cualquier secuencia ARBITRARIA de operaciones mutantes
        /// (`MarkDirty`/`CloseTab`/`ReopenClosedTab`) intercaladas con
        /// ticks de autosave (`AutosaveTick`), aplicadas directamente
        /// sobre `update_session`/`update_keyboard`/`handle_tab_closed`/
        /// `handle_closed_tab_reopened`:
        ///
        /// - Inmediatamente después de CUALQUIER `AutosaveTick`,
        ///   `state.session.dirty` SHALL ser `false`, ya sea porque ya
        ///   estaba limpio y permanece así (evitando una re-escritura
        ///   innecesaria, Requisito 6.3 segunda mitad), o porque estaba
        ///   dirty y el tick lo dejó limpio (representando la
        ///   persistencia del estado de sesión, Requisito 6.3 primera
        ///   mitad).
        /// - Un `AutosaveTick` sobre un estado ya limpio (`dirty ==
        ///   false`) SHALL ser un no-op observable: `dirty` permanece
        ///   `false` sin que haya mediado ninguna modificación real.
        #[test]
        fn property_19_autosave_tick_clears_dirty_and_avoids_rewrites_when_clean(
            ops in arb_autosave_ops(),
        ) {
            const NUM_SEED_TABS: usize = 10;

            let mut midway = build_test_midway(create_blank_draft());
            while midway.tabs.len() < NUM_SEED_TABS {
                midway.tabs.push(RequestTabState::blank());
            }
            midway.active_tab = Some(0);
            // Estado inicial limpio: ninguna de las operaciones de setup
            // de arriba pasa por un handler que marque `dirty`.
            midway.session.dirty = false;

            for op in ops {
                match op {
                    AutosaveOp::MarkDirty => {
                        let _ = update_keyboard(&mut midway, KeyboardMessage::NewBlankTabRequested);
                        prop_assert!(
                            midway.session.dirty,
                            "NewBlankTabRequested debe marcar la sesión como dirty"
                        );
                    }
                    AutosaveOp::CloseTab { tab_offset } => {
                        if !midway.tabs.is_empty() {
                            let index = tab_offset % midway.tabs.len();
                            let tab_id = midway.tabs[index].id.clone();
                            handle_tab_closed(&mut midway, &tab_id);
                        }
                    }
                    AutosaveOp::ReopenClosedTab => {
                        handle_closed_tab_reopened(&mut midway);
                    }
                    AutosaveOp::AutosaveTick => {
                        let was_dirty_before_tick = midway.session.dirty;

                        let _ = update_session(&mut midway, SessionMessage::AutosaveTick);

                        prop_assert!(
                            !midway.session.dirty,
                            "AutosaveTick SHALL dejar la sesión limpia (persistida, o marcada \
                             como tal): estaba dirty antes del tick = {was_dirty_before_tick}"
                        );
                    }
                }
            }
        }
    }

    fn crud_saved_request(
        id: &str,
        collection_id: &str,
        folder_id: Option<&str>,
    ) -> midway_core::domain::workspace::SavedRequestRecord {
        let mut draft = create_blank_draft();
        draft.id = Some(id.to_string());
        draft.name = id.to_string();
        midway_core::domain::workspace::SavedRequestRecord {
            id: id.to_string(),
            collection_id: collection_id.to_string(),
            folder_id: folder_id.map(str::to_string),
            name: id.to_string(),
            draft,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn crud_collection(
        id: &str,
        folders: Vec<Folder>,
        requests: Vec<midway_core::domain::workspace::SavedRequestRecord>,
    ) -> CollectionWithRequests {
        CollectionWithRequests {
            collection: CollectionSummary {
                id: id.to_string(),
                name: format!("Colección {id}"),
                request_count: requests.len() as u64,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            folders,
            requests,
        }
    }

    #[test]
    fn reconcile_workspace_after_crud_cleans_deleted_request_references() {
        let deleted_folder = Folder {
            id: "folder-deleted".to_string(),
            collection_id: "collection-a".to_string(),
            parent_folder_id: None,
            name: "Borrada".to_string(),
        };
        let surviving_folder = Folder {
            id: "folder-keep".to_string(),
            collection_id: "collection-a".to_string(),
            parent_folder_id: None,
            name: "Conservar".to_string(),
        };
        let deleted_request =
            crud_saved_request("request-deleted", "collection-a", Some("folder-deleted"));
        let surviving_request = crud_saved_request("request-keep", "collection-a", None);

        let mut state = build_test_midway(create_blank_draft());
        state.workspace.collections = vec![crud_collection(
            "collection-a",
            vec![deleted_folder.clone(), surviving_folder.clone()],
            vec![deleted_request.clone(), surviving_request.clone()],
        )];
        state.active_collection_id = Some("collection-a".to_string());
        state.tabs = vec![
            RequestTabState::from_draft(deleted_request.draft.clone()),
            RequestTabState::from_draft(surviving_request.draft.clone()),
            RequestTabState::blank(),
        ];
        state.active_tab = Some(0);
        let deleted_tab_id = state.tabs[0].id.clone();
        state.closed_tabs.push_front(state.tabs[0].to_snapshot());
        state.save_request_prompt = Some(SaveRequestPromptState {
            tab_id: deleted_tab_id.clone(),
            name_input: "request-deleted".to_string(),
            collection_id: Some("collection-a".to_string()),
            folder_id: Some("folder-deleted".to_string()),
            saving: false,
            error: None,
        });
        state.unsaved_changes_prompt = Some(UnsavedChangesPromptState {
            tab_id: deleted_tab_id,
            saving: false,
            error: None,
        });
        state
            .tree
            .collapsed
            .extend(["folder-deleted".to_string(), "folder-keep".to_string()]);
        state.tree.collapsed_snapshot = Some(state.tree.collapsed.clone());
        state.workspace_crud_dialog = Some(WorkspaceCrudDialogState {
            kind: WorkspaceCrudKind::DeleteFolder {
                folder_id: "folder-deleted".to_string(),
            },
            entity_name: "Borrada".to_string(),
            name_input: String::new(),
            busy: true,
            error: None,
        });

        let new_workspace = WorkspaceSnapshot {
            collections: vec![crud_collection(
                "collection-a",
                vec![surviving_folder],
                vec![surviving_request],
            )],
            environments: Vec::new(),
            history: Vec::new(),
            secrets: Vec::new(),
        };
        reconcile_workspace_after_crud(&mut state, new_workspace);

        assert!(state
            .tabs
            .iter()
            .all(|tab| { tab.draft.id.as_deref() != Some("request-deleted") }));
        assert!(state
            .closed_tabs
            .iter()
            .all(|tab| { tab.draft.id.as_deref() != Some("request-deleted") }));
        assert!(state.save_request_prompt.is_none());
        assert!(state.unsaved_changes_prompt.is_none());
        assert_eq!(state.active_collection_id.as_deref(), Some("collection-a"));
        assert_eq!(
            state.tree.collapsed,
            HashSet::from(["folder-keep".to_string()])
        );
        assert_eq!(
            state.tree.collapsed_snapshot,
            Some(HashSet::from(["folder-keep".to_string()]))
        );
        assert!(state.workspace_crud_dialog.is_none());
        assert!(state.session.dirty);
    }

    #[test]
    fn reconcile_workspace_after_deleting_active_collection_selects_fallback() {
        let mut state = build_test_midway(create_blank_draft());
        state.workspace.collections = vec![
            crud_collection("collection-a", Vec::new(), Vec::new()),
            crud_collection("collection-b", Vec::new(), Vec::new()),
        ];
        state.active_collection_id = Some("collection-a".to_string());
        state.tree.filter = "filtro viejo".to_string();
        state.tree.collapsed.insert("stale-folder".to_string());

        reconcile_workspace_after_crud(
            &mut state,
            WorkspaceSnapshot {
                collections: vec![crud_collection("collection-b", Vec::new(), Vec::new())],
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
        );

        assert_eq!(state.active_collection_id.as_deref(), Some("collection-b"));
        assert!(state.tree.filter.is_empty());
        assert!(state.tree.collapsed.is_empty());
    }
}

#[cfg(test)]
mod error_boundary_tests {
    //! Feature: tauri-to-iced-migration, Tarea 11.14 (Requisito 6.9):
    //! unit tests de `guarded_update`/`guarded_view` (el `Error_Boundary`
    //! por componente). La property test que valida el aislamiento de
    //! panics de forma exhaustiva (Property 23) se implementa por
    //! separado en la Tarea 11.15.
    //!
    //! No es posible hacer panickear un handler EXISTENTE sin modificar su
    //! lógica de negocio (fuera de alcance de esta tarea): estos tests
    //! ejercitan `guarded_update`/`guarded_view` directamente con
    //! closures que panickean deliberadamente, que es el mismo patrón que
    //! usará la Property 23.
    //!
    //! `diagnostics::append_crash_record` escribe por defecto en el data
    //! dir real de la plataforma; `diagnostics::set_test_diagnostics_path_override`
    //! (seam de testabilidad exclusiva de `#[cfg(test)]`, ver su doc en
    //! `diagnostics.rs`) redirige esa escritura a un archivo dentro de un
    //! directorio temporal por la duración de cada test, evitando tanto
    //! tocar el data dir real como interferencia entre tests (el override
    //! es por hilo, y cada `#[test]` de Rust corre en su propio hilo).

    use super::tests::build_test_midway;
    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::TestCaseError;

    /// Redirige `diagnostics::append_crash_record`/`read_crash_records` a
    /// un archivo dentro de un directorio temporal para la duración de
    /// `body`, restaurando el comportamiento normal (data dir real) al
    /// finalizar, incluso si `body` entra en pánico.
    fn with_temp_diagnostics_path<T>(body: impl FnOnce() -> T) -> T {
        let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        let path = temp_dir.path().join("diagnostics.json");
        diagnostics::set_test_diagnostics_path_override(Some(path));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));

        diagnostics::set_test_diagnostics_path_override(None);

        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Camino nominal de `guarded_update`: un handler que NO panickea
    /// devuelve su `Task<Message>` sin cambios, y no se agrega ningún
    /// `CrashRecord`.
    #[test]
    fn guarded_update_passes_through_on_success() {
        with_temp_diagnostics_path(|| {
            let mut midway = build_test_midway(create_blank_draft());
            let records_before = diagnostics::read_crash_records().len();

            let _ = guarded_update("TestComponent", &mut midway, |state| {
                state.palette.query = "sin panic".to_string();
                Task::none()
            });

            assert_eq!(midway.palette.query, "sin panic");
            assert_eq!(diagnostics::read_crash_records().len(), records_before);
        });
    }

    /// Un handler que panickea es capturado por `guarded_update`: la
    /// llamada no propaga el panic, se agrega exactamente un
    /// `CrashRecord` nuevo con `source = ComponentBoundary` y el mensaje
    /// capturado, y el resto del estado de la aplicación permanece
    /// accesible (el `Midway` de prueba sigue siendo usable tras la
    /// llamada).
    #[test]
    fn guarded_update_catches_panic_and_records_crash() {
        with_temp_diagnostics_path(|| {
            let mut midway = build_test_midway(create_blank_draft());
            midway.palette.query = "estado previo".to_string();
            let records_before = diagnostics::read_crash_records();

            let task = guarded_update("TestComponent", &mut midway, |_state| {
                panic!("panic de prueba en guarded_update");
            });

            // No propaga el panic (si lo hiciera, el test entero abortaría
            // en lugar de llegar hasta aquí).
            let _ = task;

            // El resto del estado de la aplicación permanece sin alterar:
            // el handler panickeó antes de tocar `palette.query`.
            assert_eq!(midway.palette.query, "estado previo");

            let records_after = diagnostics::read_crash_records();
            assert_eq!(records_after.len(), records_before.len() + 1);

            let new_record = &records_after[0];
            assert_eq!(
                new_record.source,
                diagnostics::CrashSource::ComponentBoundary
            );
            assert!(
                new_record
                    .message
                    .contains("panic de prueba en guarded_update"),
                "el mensaje del CrashRecord debe incluir el mensaje del panic capturado, fue: {}",
                new_record.message
            );
            assert!(
                new_record.message.contains("TestComponent"),
                "el mensaje del CrashRecord debe identificar el componente, fue: {}",
                new_record.message
            );
        });
    }

    /// Camino nominal de `guarded_view`: un handler que NO panickea
    /// devuelve su `Element` sin cambios (verificado indirectamente: la
    /// llamada no agrega ningún `CrashRecord`).
    #[test]
    fn guarded_view_passes_through_on_success() {
        with_temp_diagnostics_path(|| {
            let records_before = diagnostics::read_crash_records().len();

            let _element: Element<'static, Message> =
                guarded_view("TestComponent", || text("contenido normal").into());

            assert_eq!(diagnostics::read_crash_records().len(), records_before);
        });
    }

    /// Un cierre de `view` que panickea es capturado por `guarded_view`:
    /// la llamada no propaga el panic, devuelve un `Element` de error en
    /// su lugar, y se agrega exactamente un `CrashRecord` nuevo con
    /// `source = ComponentBoundary`.
    #[test]
    fn guarded_view_catches_panic_and_records_crash() {
        with_temp_diagnostics_path(|| {
            let records_before = diagnostics::read_crash_records();

            let _element: Element<'static, Message> = guarded_view("TestComponent", || {
                panic!("panic de prueba en guarded_view");
            });

            let records_after = diagnostics::read_crash_records();
            assert_eq!(records_after.len(), records_before.len() + 1);

            let new_record = &records_after[0];
            assert_eq!(
                new_record.source,
                diagnostics::CrashSource::ComponentBoundary
            );
            assert!(
                new_record
                    .message
                    .contains("panic de prueba en guarded_view"),
                "el mensaje del CrashRecord debe incluir el mensaje del panic capturado, fue: {}",
                new_record.message
            );
        });
    }

    /// Genera un mensaje de panic arbitrario, razonablemente variado (letras,
    /// dígitos, espacios y algo de puntuación común) pero evitando
    /// caracteres patológicos (saltos de línea, comillas, backslashes) que
    /// podrían complicar las aserciones de `contains` sin aportar cobertura
    /// adicional a la propiedad en sí (el aislamiento de panics no depende
    /// de qué caracteres exactos contenga el mensaje).
    fn panic_message_strategy() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ,.:_-]{1,60}".prop_map(|s| s.to_string())
    }

    /// Genera un valor arbitrario de "estado de otra área de la
    /// aplicación", ajeno por completo al closure que panickea en las
    /// propiedades de abajo, usado para verificar que dicho estado
    /// permanece sin alterar (Property 23).
    fn other_state_value_strategy() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 ]{0,40}".prop_map(|s| s.to_string())
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 20, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 23: El Error_Boundary
        /// aísla panics de un componente sin derribar la aplicación (mitad
        /// `guarded_update`).
        /// Validates: Requirements 6.9
        ///
        /// Para cualquier mensaje de panic arbitrario y cualquier valor
        /// arbitrario sembrado en un área de la aplicación NO tocada por el
        /// handler (`palette.query`): al invocar `guarded_update` con un
        /// handler que panickea con ese mensaje, la llamada NO propaga el
        /// panic (si lo hiciera, este mismo test abortaría en lugar de
        /// llegar a las aserciones); se agrega EXACTAMENTE un
        /// `CrashRecord` nuevo con `source = ComponentBoundary` y un
        /// mensaje que contiene el texto del panic; y el valor sembrado en
        /// `palette.query` permanece exactamente igual.
        #[test]
        fn property_23_guarded_update_isolates_arbitrary_panics(
            panic_message in panic_message_strategy(),
            other_state_value in other_state_value_strategy(),
        ) {
            // `with_temp_diagnostics_path` es genérico sobre el valor de
            // retorno de `body`: se usa aquí con un cierre que devuelve
            // `Result<(), TestCaseError>` (en vez de `()`, como en los
            // tests de ejemplo fijo de arriba) porque `prop_assert!`/
            // `prop_assert_eq!` se expanden a `return Err(...)`, y ese
            // `return` necesita que el cierre que lo contiene tenga ese
            // tipo de retorno; el resultado se propaga con `?` al test de
            // proptest (que también espera ese mismo tipo).
            with_temp_diagnostics_path(|| -> Result<(), TestCaseError> {
                let mut midway = build_test_midway(create_blank_draft());
                midway.palette.query = other_state_value.clone();
                let records_before = diagnostics::read_crash_records();

                let panic_message_for_closure = panic_message.clone();
                let task = guarded_update("TestComponent", &mut midway, move |_state| {
                    panic!("{panic_message_for_closure}");
                });

                // No propaga el panic.
                let _ = task;

                // El estado de otras áreas de la aplicación permanece sin
                // alterar: el handler panickeó antes de tocar nada.
                prop_assert_eq!(&midway.palette.query, &other_state_value);

                // Se agregó EXACTAMENTE un `CrashRecord` nuevo.
                let records_after = diagnostics::read_crash_records();
                prop_assert_eq!(records_after.len(), records_before.len() + 1);

                let new_record = &records_after[0];
                prop_assert_eq!(new_record.source, diagnostics::CrashSource::ComponentBoundary);
                prop_assert!(
                    new_record.message.contains(&panic_message),
                    "el mensaje del CrashRecord debe incluir el mensaje del panic capturado, fue: {}",
                    new_record.message
                );

                Ok(())
            })?;
        }

        /// Feature: tauri-to-iced-migration, Property 23: El Error_Boundary
        /// aísla panics de un componente sin derribar la aplicación (mitad
        /// `guarded_view`).
        /// Validates: Requirements 6.9
        ///
        /// Para cualquier mensaje de panic arbitrario: al invocar
        /// `guarded_view` con un handler que panickea con ese mensaje, la
        /// llamada NO propaga el panic (si lo hiciera, este mismo test
        /// abortaría en lugar de llegar a las aserciones) y devuelve un
        /// `Element` de error en su lugar; y se agrega EXACTAMENTE un
        /// `CrashRecord` nuevo con `source = ComponentBoundary` y un
        /// mensaje que contiene el texto del panic.
        #[test]
        fn property_23_guarded_view_isolates_arbitrary_panics(
            panic_message in panic_message_strategy(),
        ) {
            with_temp_diagnostics_path(|| -> Result<(), TestCaseError> {
                let records_before = diagnostics::read_crash_records();

                let panic_message_for_closure = panic_message.clone();
                let _element: Element<'static, Message> = guarded_view("TestComponent", move || {
                    panic!("{panic_message_for_closure}");
                });

                let records_after = diagnostics::read_crash_records();
                prop_assert_eq!(records_after.len(), records_before.len() + 1);

                let new_record = &records_after[0];
                prop_assert_eq!(new_record.source, diagnostics::CrashSource::ComponentBoundary);
                prop_assert!(
                    new_record.message.contains(&panic_message),
                    "el mensaje del CrashRecord debe incluir el mensaje del panic capturado, fue: {}",
                    new_record.message
                );

                Ok(())
            })?;
        }
    }
}

// Feature: ux-flow-redesign, Property 7: Collection auto-selection at startup
#[cfg(test)]
mod resolve_startup_collection_tests {
    //! Feature: ux-flow-redesign, Property 7: Collection auto-selection at startup
    //! Validates: Requirements 6.1, 6.2, 6.3, 6.4

    use super::*;
    use midway_core::domain::workspace::{CollectionSummary, CollectionWithRequests};
    use proptest::prelude::*;

    /// Strategy to generate a valid collection id (non-empty alphanumeric string).
    fn collection_id_strategy() -> impl Strategy<Value = String> {
        "[a-z0-9]{1,20}".prop_map(|s| s.to_string())
    }

    /// Strategy to generate a CollectionWithRequests with a given id.
    fn collection_with_id(id: String) -> CollectionWithRequests {
        CollectionWithRequests {
            collection: CollectionSummary {
                id,
                name: "test-collection".to_string(),
                request_count: 0,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            folders: Vec::new(),
            requests: Vec::new(),
        }
    }

    /// Strategy to generate a non-empty Vec of CollectionWithRequests with unique ids.
    fn non_empty_collections_strategy() -> impl Strategy<Value = Vec<CollectionWithRequests>> {
        proptest::collection::vec(collection_id_strategy(), 1..=10)
            .prop_map(|ids| {
                // Deduplicate ids to ensure uniqueness
                let mut seen = std::collections::HashSet::new();
                ids.into_iter()
                    .filter(|id| seen.insert(id.clone()))
                    .map(collection_with_id)
                    .collect::<Vec<_>>()
            })
            .prop_filter("must have at least one collection", |v| !v.is_empty())
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// Property 7: For a non-empty list of collections, resolve_startup_collection
        /// returns session_id if it matches an existing collection's id, otherwise
        /// returns collections[0].id.
        #[test]
        fn property_7_non_empty_collections_returns_session_or_first(
            collections in non_empty_collections_strategy(),
        ) {
            let collection_ids: Vec<String> = collections.iter()
                .map(|c| c.collection.id.clone())
                .collect();

            // Test with session_id that matches one of the collections
            for id in &collection_ids {
                let result = resolve_startup_collection(&collections, Some(id));
                prop_assert_eq!(
                    result.as_deref(),
                    Some(id.as_str()),
                    "When session_id matches an existing collection, it should be returned"
                );
            }

            // Test with session_id that does NOT match any collection
            let non_existent_id = format!("{}-does-not-exist", collection_ids[0]);
            let result = resolve_startup_collection(&collections, Some(&non_existent_id));
            prop_assert_eq!(
                result.as_deref(),
                Some(collection_ids[0].as_str()),
                "When session_id does not match, should return first collection's id"
            );

            // Test with no session_id (None)
            let result = resolve_startup_collection(&collections, None);
            prop_assert_eq!(
                result.as_deref(),
                Some(collection_ids[0].as_str()),
                "When session_id is None, should return first collection's id"
            );
        }

        /// Property 7: For an empty collections list, resolve_startup_collection
        /// returns None regardless of session_id.
        #[test]
        fn property_7_empty_collections_returns_none(
            session_id in proptest::option::of("[a-z0-9]{1,20}"),
        ) {
            let empty: Vec<CollectionWithRequests> = Vec::new();
            let result = resolve_startup_collection(&empty, session_id.as_deref());
            prop_assert_eq!(
                result,
                None,
                "For an empty collections list, should always return None"
            );
        }
    }
}

// Feature: ux-flow-redesign, Property 4: Navigation toggle preserves Composer state
#[cfg(test)]
mod navigation_toggle_preserves_composer_state_tests {
    //! Feature: ux-flow-redesign, Property 4: Navigation toggle preserves Composer state
    //! Validates: Requirements 4.1, 4.2, 8.3

    use super::*;
    use crate::curl::create_blank_draft;
    use crate::state::AppState;
    use midway_core::domain::cookies::CookieJarHandle;
    use midway_core::infra::sqlite_repository::SqliteRepository;
    use midway_core::runtime::request_executor::RequestExecutorHandle;
    use midway_core::runtime::secret_executor::SecretExecutorHandle;
    use proptest::prelude::*;
    use std::collections::VecDeque;
    use std::sync::Arc;

    /// Build a minimal AppState for testing (opens a temp SQLite DB).
    fn build_test_app_state() -> AppState {
        let temp_file =
            tempfile::NamedTempFile::new().expect("failed to create temp file for test AppState");
        let db_path = temp_file.path().to_path_buf();

        let runtime = tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime for test AppState");

        let app_state = runtime.block_on(async {
            let repository = SqliteRepository::open(&db_path)
                .await
                .expect("failed to open SqliteRepository for test");

            let client = reqwest::Client::builder()
                .build()
                .expect("failed to build reqwest client for test");

            AppState {
                repository,
                request_executor: RequestExecutorHandle::spawn(client),
                secret_executor: SecretExecutorHandle::spawn("midway-test".to_string()),
                cookie_jar: CookieJarHandle::new(),
            }
        });

        drop(temp_file);
        app_state
    }

    /// Build a test Midway state with configurable focus, collection_id, tabs, and active_tab.
    fn build_test_midway_state(
        app_state: Arc<AppState>,
        focus: MainContentFocus,
        collection_id: Option<String>,
        tabs: Vec<RequestTabState>,
        active_tab: Option<usize>,
    ) -> Midway {
        Midway {
            app_state,
            workspace: midway_core::domain::workspace::WorkspaceSnapshot {
                collections: Vec::new(),
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
            tabs,
            active_tab,
            closed_tabs: VecDeque::new(),
            workspace_panel: WorkspacePanelState::default(),
            palette: PaletteState::default(),
            runner: None,
            session: SessionStoreState::default(),
            theme: ThemeSettingsState::default(),
            main_content_focus: focus,
            updater: UpdaterState::default(),
            crash_log: Vec::new(),
            unsaved_changes_prompt: None,
            active_collection_id: collection_id,
            top_bar_mode: TopBarMode::default(),
            tree: TreeViewState::default(),
            create_collection_prompt: None,
            save_request_prompt: None,
            workspace_crud_dialog: None,
            panel_dragging: None,
            panel_hovered: None,
        }
    }

    /// Strategy to generate an arbitrary `MainContentFocus`.
    fn arb_main_content_focus() -> impl Strategy<Value = MainContentFocus> {
        prop_oneof![
            Just(MainContentFocus::RequestTab),
            Just(MainContentFocus::WorkspaceSection),
        ]
    }

    /// Strategy to generate an arbitrary `Option<String>` for active_collection_id.
    fn arb_active_collection_id() -> impl Strategy<Value = Option<String>> {
        prop_oneof![
            3 => Just(None),
            7 => "[a-z0-9]{1,20}".prop_map(Some),
        ]
    }

    /// Strategy to generate a tab count (0 to 5) and an active_tab index.
    fn arb_tab_config() -> impl Strategy<Value = (usize, Option<usize>)> {
        (0usize..=5).prop_flat_map(|count| {
            let active = if count == 0 {
                Just(None).boxed()
            } else {
                (0..count).prop_map(Some).boxed()
            };
            active.prop_map(move |a| (count, a))
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// Property 4: Home toggle flips main_content_focus without modifying
        /// active_collection_id, tabs, or active_tab. Double-toggle returns
        /// main_content_focus to its original value.
        #[test]
        fn property_4_home_toggle_preserves_composer_state(
            focus in arb_main_content_focus(),
            collection_id in arb_active_collection_id(),
            (tab_count, active_tab_val) in arb_tab_config(),
        ) {
            // Share a single AppState across all iterations via lazy initialization
            use std::sync::LazyLock;
            static SHARED_APP_STATE: LazyLock<Arc<AppState>> = LazyLock::new(|| {
                Arc::new(build_test_app_state())
            });
            let app_state = Arc::clone(&SHARED_APP_STATE);

            // Build tabs from tab_count (each with a blank draft)
            let tabs: Vec<RequestTabState> = (0..tab_count)
                .map(|_| RequestTabState::from_draft(create_blank_draft()))
                .collect();

            // Capture pre-toggle snapshots of tabs for comparison
            let pre_tab_snapshots: Vec<TabSnapshot> = tabs.iter()
                .map(|t: &RequestTabState| t.to_snapshot())
                .collect();
            let pre_active_tab = active_tab_val;
            let pre_collection_id = collection_id.clone();
            let original_focus = focus;

            // Build test state
            let mut state = build_test_midway_state(
                app_state, focus, collection_id, tabs, active_tab_val,
            );

            // --- First toggle ---
            let _ = update_activity_bar(&mut state, ActivityBarMessage::HomePressed);

            // Assert main_content_focus flipped
            let expected_focus_after_first = match original_focus {
                MainContentFocus::RequestTab => MainContentFocus::WorkspaceSection,
                MainContentFocus::WorkspaceSection => MainContentFocus::RequestTab,
            };
            prop_assert_eq!(
                state.main_content_focus,
                expected_focus_after_first,
                "After first toggle, focus should have flipped"
            );

            // Assert active_collection_id unchanged
            prop_assert_eq!(
                &state.active_collection_id,
                &pre_collection_id,
                "active_collection_id must not change after HomePressed"
            );

            // Assert active_tab unchanged
            prop_assert_eq!(
                state.active_tab,
                pre_active_tab,
                "active_tab must not change after HomePressed"
            );

            // Assert tabs unchanged (compare via snapshots)
            let post_tab_snapshots: Vec<TabSnapshot> = state.tabs.iter()
                .map(|t: &RequestTabState| t.to_snapshot())
                .collect();
            prop_assert_eq!(
                &post_tab_snapshots,
                &pre_tab_snapshots,
                "tabs must not change after HomePressed"
            );

            // --- Second toggle (double-toggle) ---
            let _ = update_activity_bar(&mut state, ActivityBarMessage::HomePressed);

            // Assert main_content_focus returned to original
            prop_assert_eq!(
                state.main_content_focus,
                original_focus,
                "After double-toggle, focus should return to original value"
            );

            // Assert active_collection_id still unchanged
            prop_assert_eq!(
                &state.active_collection_id,
                &pre_collection_id,
                "active_collection_id must not change after double-toggle"
            );

            // Assert active_tab still unchanged
            prop_assert_eq!(
                state.active_tab,
                pre_active_tab,
                "active_tab must not change after double-toggle"
            );

            // Assert tabs still unchanged
            let post_double_tab_snapshots: Vec<TabSnapshot> = state.tabs.iter()
                .map(|t: &RequestTabState| t.to_snapshot())
                .collect();
            prop_assert_eq!(
                &post_double_tab_snapshots,
                &pre_tab_snapshots,
                "tabs must not change after double-toggle"
            );
        }
    }
}

// Feature: ux-flow-redesign, Property 5: Collection selection from WorkspaceSection transitions focus
#[cfg(test)]
mod collection_selection_workspace_property_tests {
    //! Feature: ux-flow-redesign, Property 5: Collection selection from WorkspaceSection transitions focus
    //! Validates: Requirements 4.3

    use super::*;
    use crate::state::AppState;
    use midway_core::domain::cookies::CookieJarHandle;
    use midway_core::domain::workspace::{CollectionSummary, CollectionWithRequests};
    use midway_core::infra::sqlite_repository::SqliteRepository;
    use midway_core::runtime::request_executor::RequestExecutorHandle;
    use midway_core::runtime::secret_executor::SecretExecutorHandle;
    use proptest::prelude::*;
    use std::collections::VecDeque;
    use std::sync::Arc;

    /// Build a minimal AppState for testing (opens a temp SQLite DB).
    fn build_test_app_state() -> AppState {
        let temp_file =
            tempfile::NamedTempFile::new().expect("failed to create temp file for test AppState");
        let db_path = temp_file.path().to_path_buf();

        let runtime = tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime for test AppState");

        let app_state = runtime.block_on(async {
            let repository = SqliteRepository::open(&db_path)
                .await
                .expect("failed to open SqliteRepository for test");

            let client = reqwest::Client::builder()
                .build()
                .expect("failed to build reqwest client for test");

            AppState {
                repository,
                request_executor: RequestExecutorHandle::spawn(client),
                secret_executor: SecretExecutorHandle::spawn("midway-test".to_string()),
                cookie_jar: CookieJarHandle::new(),
            }
        });

        drop(temp_file);
        app_state
    }

    /// Build a test Midway state with focus set to WorkspaceSection and the given collections.
    fn build_test_midway_workspace_focus(
        app_state: Arc<AppState>,
        collections: Vec<CollectionWithRequests>,
    ) -> Midway {
        Midway {
            app_state,
            workspace: midway_core::domain::workspace::WorkspaceSnapshot {
                collections,
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
            tabs: vec![RequestTabState::from_draft(
                crate::curl::create_blank_draft(),
            )],
            active_tab: Some(0),
            closed_tabs: VecDeque::new(),
            workspace_panel: WorkspacePanelState::default(),
            palette: PaletteState::default(),
            runner: None,
            session: SessionStoreState::default(),
            theme: ThemeSettingsState::default(),
            main_content_focus: MainContentFocus::WorkspaceSection,
            updater: UpdaterState::default(),
            crash_log: Vec::new(),
            unsaved_changes_prompt: None,
            active_collection_id: None,
            top_bar_mode: TopBarMode::default(),
            tree: TreeViewState::default(),
            create_collection_prompt: None,
            save_request_prompt: None,
            workspace_crud_dialog: None,
            panel_dragging: None,
            panel_hovered: None,
        }
    }

    /// Strategy to generate a valid collection id.
    fn collection_id_strategy() -> impl Strategy<Value = String> {
        "[a-z0-9]{1,20}".prop_map(|s| s.to_string())
    }

    /// Strategy to generate a CollectionWithRequests with a given id.
    fn collection_with_id(id: String) -> CollectionWithRequests {
        CollectionWithRequests {
            collection: CollectionSummary {
                id,
                name: "test-collection".to_string(),
                request_count: 0,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            folders: Vec::new(),
            requests: Vec::new(),
        }
    }

    /// Strategy to generate a non-empty Vec of CollectionWithRequests with unique ids,
    /// returning both the collections and the list of their ids.
    fn non_empty_collections_strategy() -> impl Strategy<Value = Vec<CollectionWithRequests>> {
        proptest::collection::vec(collection_id_strategy(), 1..=5)
            .prop_map(|ids| {
                let mut seen = std::collections::HashSet::new();
                ids.into_iter()
                    .filter(|id| seen.insert(id.clone()))
                    .map(collection_with_id)
                    .collect::<Vec<_>>()
            })
            .prop_filter("must have at least one collection", |v| !v.is_empty())
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// Property 5: Collection selection from WorkspaceSection transitions focus.
        ///
        /// **Validates: Requirements 4.3**
        ///
        /// For any application state where `main_content_focus` is `WorkspaceSection`
        /// and a valid collection id is selected, after handling
        /// `CollectionSelected(id)`, `main_content_focus == RequestTab` and
        /// `active_collection_id == Some(id)`.
        #[test]
        fn property_5_collection_selection_from_workspace_transitions_focus(
            collections in non_empty_collections_strategy(),
            index in 0usize..5,
        ) {
            let app_state = Arc::new(build_test_app_state());
            let valid_index = index % collections.len();
            let selected_id = collections[valid_index].collection.id.clone();

            let mut state = build_test_midway_workspace_focus(
                app_state,
                collections,
            );

            // Precondition: focus is WorkspaceSection
            prop_assert_eq!(
                state.main_content_focus,
                MainContentFocus::WorkspaceSection,
                "Precondition: state must start with WorkspaceSection focus"
            );

            // Act: handle CollectionSelected message
            let _task = update_activity_bar(
                &mut state,
                ActivityBarMessage::CollectionSelected(selected_id.clone()),
            );

            // Assert: focus transitions to RequestTab
            prop_assert_eq!(
                state.main_content_focus,
                MainContentFocus::RequestTab,
                "After CollectionSelected from WorkspaceSection, focus must be RequestTab"
            );

            // Assert: active_collection_id is set to the selected id
            prop_assert_eq!(
                state.active_collection_id.as_deref(),
                Some(selected_id.as_str()),
                "After CollectionSelected, active_collection_id must be Some(selected_id). \
                 Got: {:?}, expected: Some({:?})",
                state.active_collection_id,
                selected_id,
            );
        }
    }
}

// Feature: midway-baseline-audit-and-first-vertical, Tarea 4.1 (Requisitos 6.1, 6.2)
#[cfg(test)]
mod theme_toggle_characterization_tests {
    //! Feature: midway-baseline-audit-and-first-vertical, Tarea 4.1
    //! Requisitos 6.1, 6.2.
    //!
    //! Test_Caracterización del comportamiento observable del toggle de tema:
    //! `ThemeMessage::Toggled` alterna el modo de tema activo de `Midway`,
    //! cambia con él la paleta derivada que consume `view` y marca la sesión
    //! como sucia.
    //!
    //! Tras la Tarea 7.1 el modo activo se lee en `state.theme.mode()` en vez
    //! del campo suelto `Midway.theme_mode`, y la transición la aplica la
    //! vertical `ui/theme_settings`. Los valores esperados que este módulo
    //! fija NO cambiaron: es exactamente esa invariancia la que demuestra que
    //! el cableado de la vertical no alteró el comportamiento observable
    //! (Req 8.7).
    //!
    //! Estos tests son la referencia de comportamiento que protege la
    //! extracción de la vertical Tema/Ajustes (Tarea 7): tras el cableado,
    //! el mismo mensaje debe producir el mismo `ThemeMode`, la misma paleta
    //! derivada y la misma marca de sesión sucia, sin cambiar los valores
    //! esperados que se fijan aquí.
    //!
    //! Tarea 7.3 agregó la aserción de paleta derivada
    //! (`state.theme.design_system() == DesignSystem::for_mode(mode)`, la
    //! misma expresión que evalúa `app::view`) sin modificar ninguno de los
    //! valores esperados preexistentes.
    //!
    //! Headless por construcción: ni `update` ni `theme_settings::update`
    //! abren ventana o tocan el runtime gráfico de iced.

    use super::tests::build_test_midway;
    use super::*;

    /// Observación completa del comportamiento del toggle: modo activo,
    /// paleta derivada que consume `view` y marca de sesión sucia.
    struct Observed {
        mode: ThemeMode,
        design_system: DesignSystem,
        dirty: bool,
    }

    /// Aplica `ThemeMessage::Toggled` sobre un `Midway` cuyo modo de tema
    /// activo es `initial` y devuelve el `ThemeMode`, el `DesignSystem`
    /// derivado y el flag `session.dirty` observados después de la
    /// actualización.
    fn toggle_once(initial: ThemeMode) -> Observed {
        let mut state = build_test_midway(create_blank_draft());
        state.theme = ThemeSettingsState::new(initial);
        // Precondición explícita: la sesión arranca limpia, así que el
        // `dirty == true` observado más abajo solo puede provenir del
        // toggle.
        state.session.dirty = false;

        // Tarea 7.2: `update_theme` ya no existe; la ruta observable es
        // `Message::Theme` → `theme_settings::update`. Los valores esperados
        // que fijan los tests de abajo NO cambian (Req 8.7).
        let _task = update(&mut state, Message::Theme(ThemeMessage::Toggled));

        Observed {
            mode: state.theme.mode(),
            // Misma expresión que usa `app::view` para derivar la paleta.
            design_system: state.theme.design_system(),
            dirty: state.session.dirty,
        }
    }

    /// Desde `Dark` (el valor por defecto del baseline), el toggle deja el
    /// tema en `Light`, la paleta derivada de `Light` y la sesión sucia.
    #[test]
    fn toggled_from_dark_yields_light_and_marks_session_dirty() {
        let observed = toggle_once(ThemeMode::Dark);

        assert_eq!(observed.mode, ThemeMode::Light);
        // Tarea 7.3: la paleta que consume `view` sigue siendo exactamente la
        // que el baseline derivaba con `DesignSystem::for_mode(theme_mode)`.
        assert_eq!(
            observed.design_system,
            DesignSystem::for_mode(ThemeMode::Light)
        );
        assert!(
            observed.dirty,
            "el toggle de tema debe marcar la sesión como sucia"
        );
    }

    /// Desde `Light`, el toggle deja el tema en `Dark`, la paleta derivada de
    /// `Dark` y la sesión sucia.
    #[test]
    fn toggled_from_light_yields_dark_and_marks_session_dirty() {
        let observed = toggle_once(ThemeMode::Light);

        assert_eq!(observed.mode, ThemeMode::Dark);
        assert_eq!(
            observed.design_system,
            DesignSystem::for_mode(ThemeMode::Dark)
        );
        assert!(
            observed.dirty,
            "el toggle de tema debe marcar la sesión como sucia"
        );
    }

    /// Dos toggles consecutivos devuelven el tema al valor inicial y la
    /// sesión queda sucia: fija la involutividad observable del baseline.
    #[test]
    fn toggled_twice_returns_to_initial_mode_and_leaves_session_dirty() {
        let initial = ThemeMode::default();
        let mut state = build_test_midway(create_blank_draft());
        state.theme = ThemeSettingsState::new(initial);
        state.session.dirty = false;

        let _first = update(&mut state, Message::Theme(ThemeMessage::Toggled));
        let _second = update(&mut state, Message::Theme(ThemeMessage::Toggled));

        assert_eq!(state.theme.mode(), initial);
        assert_eq!(
            state.theme.design_system(),
            DesignSystem::for_mode(initial),
            "tras dos toggles la paleta derivada vuelve a la del modo inicial"
        );
        assert!(
            state.session.dirty,
            "tras dos toggles la sesión sigue marcada como sucia"
        );
    }
}

// Feature: midway-baseline-audit-and-first-vertical, Tarea 4.4 (Requisito 9.4)
#[cfg(test)]
mod session_restore_property_tests {
    //! Feature: midway-baseline-audit-and-first-vertical, Tarea 4.4
    //! Requisito 9.4.
    //!
    //! Test_Caracterización de propiedad sobre `restore_pending_session` en
    //! el baseline SIN CAMBIOS: restaurar un `SessionSnapshot` deja
    //! `session.panel_sizes` igual a `snapshot.panel_sizes` y el modo de
    //! tema activo igual a `snapshot.theme_mode`. No se modifica ningún
    //! código de producción; este módulo es solo `#[cfg(test)]`.
    //!
    //! El "modo de tema activo" se observa en `session.theme_mode`: es
    //! exactamente el valor con el que `Midway::new` construye
    //! `Midway.theme` justo después de llamar a `restore_pending_session`
    //! (`ThemeSettingsState::new(session.theme_mode)`), así que fijarlo aquí
    //! fija el tema que la aplicación muestra al reabrirse.
    //!
    //! Headless por construcción: `restore_pending_session` opera solo
    //! sobre `&mut SessionStoreState`, sin ventana, sin disco y sin red.
    //!
    //! Los generadores se definen localmente en vez de reutilizar los de
    //! `session.rs`: los de allí son `pub(super)` dentro de
    //! `session::tests`, no visibles desde este módulo.

    use super::*;
    use proptest::prelude::*;

    /// `ThemeMode` arbitrario: el enum tiene exactamente dos variantes en
    /// el baseline, así que se enumeran las dos en lugar de derivar una
    /// estrategia.
    fn arb_theme_mode() -> impl Strategy<Value = ThemeMode> {
        prop_oneof![Just(ThemeMode::Light), Just(ThemeMode::Dark)]
    }

    /// `PanelSizes` arbitrario con valores finitos, incluyendo cero y
    /// negativos: `restore_pending_session` copia el struct tal cual, así
    /// que la propiedad debe cumplirse incluso para tamaños que la UI
    /// nunca produciría. Se excluyen `NaN` e infinitos a propósito: con
    /// `NaN` la igualdad de `f32` es falsa por definición y la propiedad
    /// dejaría de hablar de preservación para hablar de aritmética de
    /// punto flotante.
    fn arb_panel_sizes() -> impl Strategy<Value = crate::session::PanelSizes> {
        (
            -1_000.0f32..5_000.0f32,
            -1_000.0f32..5_000.0f32,
            -1_000.0f32..5_000.0f32,
        )
            .prop_map(
                |(workspace_panel_width, response_panel_height, request_panel_width)| {
                    crate::session::PanelSizes {
                        workspace_panel_width,
                        response_panel_height,
                        request_panel_width,
                    }
                },
            )
    }

    fn arb_request_tab() -> impl Strategy<Value = RequestTab> {
        prop_oneof![
            Just(RequestTab::Params),
            Just(RequestTab::Headers),
            Just(RequestTab::Auth),
            Just(RequestTab::Body),
            Just(RequestTab::Tests),
        ]
    }

    /// `TabSnapshot` con un `draft` en blanco al que solo se le varía la
    /// URL: la propiedad no habla del contenido de las tabs, pero variar
    /// su cantidad (incluido el caso de cero tabs, que tiene un `return`
    /// temprano propio en `restore_pending_session`) sí importa para
    /// cubrir todas las ramas de la función.
    fn arb_tab_snapshot(id_prefix: &'static str) -> impl Strategy<Value = TabSnapshot> {
        ("[a-z]{1,8}", "[a-z]{1,8}", arb_request_tab()).prop_map(
            move |(id, host, active_request_tab)| {
                let mut draft = create_blank_draft();
                draft.url = format!("https://{host}.example.com/path");
                TabSnapshot {
                    id: format!("{id_prefix}-{id}"),
                    draft,
                    active_request_tab,
                }
            },
        )
    }

    fn arb_tab_snapshots(id_prefix: &'static str) -> impl Strategy<Value = Vec<TabSnapshot>> {
        proptest::collection::vec(arb_tab_snapshot(id_prefix), 0..=3)
    }

    /// `SessionSnapshot` arbitrario con la `version` actual del esquema:
    /// las versiones incompatibles se descartan antes de llegar acá
    /// (`load_session_or_default`), así que variar `version` no aportaría
    /// cobertura a esta propiedad.
    fn arb_session_snapshot() -> impl Strategy<Value = crate::session::SessionSnapshot> {
        (
            proptest::option::of("[a-z]{1,8}"),
            arb_tab_snapshots("open"),
            arb_tab_snapshots("closed"),
            arb_panel_sizes(),
            arb_theme_mode(),
            "[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z",
            proptest::option::of("[a-z]{1,8}"),
        )
            .prop_map(
                |(
                    active_tab_id,
                    open_tabs,
                    closed_tabs,
                    panel_sizes,
                    theme_mode,
                    saved_at,
                    active_collection_id,
                )| crate::session::SessionSnapshot {
                    version: crate::session::SESSION_SCHEMA_VERSION,
                    active_tab_id,
                    open_tabs,
                    closed_tabs,
                    panel_sizes,
                    theme_mode,
                    saved_at,
                    active_collection_id,
                },
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// Feature: midway-baseline-audit-and-first-vertical, Property 3: La
        /// restauración de sesión preserva tamaños de panel y tema.
        ///
        /// Para todo `SessionSnapshot`, restaurarlo sobre el estado de la
        /// aplicación deja `session.panel_sizes` igual a
        /// `snapshot.panel_sizes` y el modo de tema activo igual a
        /// `snapshot.theme_mode`.
        #[test]
        fn property_3_session_restore_preserves_panel_sizes_and_theme(
            snapshot in arb_session_snapshot(),
        ) {
            let expected_panel_sizes = snapshot.panel_sizes.clone();
            let expected_theme_mode = snapshot.theme_mode;

            let mut session = SessionStoreState {
                pending_restore: Some(snapshot),
                ..SessionStoreState::default()
            };

            let (tabs, active_tab, _active_collection_id) = restore_pending_session(&mut session);

            prop_assert_eq!(
                session.panel_sizes.clone(),
                expected_panel_sizes,
                "restaurar la sesión debe dejar session.panel_sizes igual al snapshot"
            );

            // El modo de tema activo es el que `Midway::new` lee de
            // `session.theme_mode` inmediatamente después de restaurar.
            let active_theme_mode = session.theme_mode;
            prop_assert_eq!(
                active_theme_mode,
                expected_theme_mode,
                "restaurar la sesión debe dejar el tema activo igual al del snapshot"
            );

            // Invariante estructural que acompaña a la restauración: nunca
            // se arranca con cero tabs, y la tab activa es significativa.
            prop_assert!(!tabs.is_empty(), "la restauración nunca debe dejar cero tabs");
            prop_assert!(active_tab.is_some(), "la restauración siempre resuelve una tab activa");
        }
    }
}
