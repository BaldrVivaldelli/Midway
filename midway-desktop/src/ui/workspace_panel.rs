//! `Workspace_Panel` lateral (Tarea 7.1, Fase 3): contenedor colapsable con
//! las secciones Environments/Data/History/Diagnostics/App updates.
//!
//! La Tarea 7.1 implementa el contenedor y el mecanismo de selección de
//! sección / colapsar-expandir (Requisito 4.1). La Tarea 7.2 agrega el
//! contenido real de la sección Environments (CRUD, Requisitos 4.2, 4.9,
//! 4.10). El resto de las secciones (export/import de datos, lista de
//! history, diagnostics, estado del updater) se agrega en tareas
//! posteriores (7.6/7.7, 7.11, 7.12, 7.15), reemplazando los placeholders
//! de `section_placeholder` uno a uno.
//!
//! Ver diseño: "Components and Interfaces > Workspace_Panel lateral (Fase 3)".
//! Ver requisitos: 4.1, 4.2, 4.9, 4.10.

use iced::widget::{button, column, pick_list, row, scrollable, text, text_input};
use iced::{Color, Element, Length};

use midway_core::domain::interop::{WorkspaceExportFormat, WorkspaceImportFormat};
use midway_core::domain::workspace::{EnvironmentRecord, HistoryEntry};

use crate::app::{
    ExportFormState, ImportFormState, Message, Midway, UpdaterStatus, WorkspaceMessage,
    WorkspacePanelSection,
};
use crate::diagnostics::CrashRecord;
use crate::ui::design_system::{DesignSystem, ThemeMode};
use crate::ui::tab_bar;

/// Envoltura de `domain::interop::WorkspaceExportFormat` para el
/// `pick_list` de formato de la sección Data (Tarea 7.6, Requisito 4.3):
/// `WorkspaceExportFormat` no implementa `Display` en `midway-core` (es un
/// tipo de dominio sin preocupaciones de presentación), mismo motivo que
/// `ApiKeyPlacementOption` en `ui::request_composer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExportFormatOption(WorkspaceExportFormat);

impl std::fmt::Display for ExportFormatOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.0 {
            WorkspaceExportFormat::NativeWorkspaceV1 => "Workspace v1 nativo",
            WorkspaceExportFormat::PostmanCollectionV21 => "Postman Collection v2.1",
        };
        f.write_str(label)
    }
}

const EXPORT_FORMAT_OPTIONS: [ExportFormatOption; 2] = [
    ExportFormatOption(WorkspaceExportFormat::NativeWorkspaceV1),
    ExportFormatOption(WorkspaceExportFormat::PostmanCollectionV21),
];

/// Envoltura de `domain::interop::WorkspaceImportFormat` para el
/// `pick_list` de formato de import de la sección Data (Tarea 7.7,
/// Requisito 4.4), mismo motivo que `ExportFormatOption`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImportFormatOption(WorkspaceImportFormat);

impl std::fmt::Display for ImportFormatOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.0 {
            WorkspaceImportFormat::Auto => "Detectar automáticamente",
            WorkspaceImportFormat::NativeWorkspaceV1 => "Workspace v1 nativo",
            WorkspaceImportFormat::PostmanCollectionV21 => "Postman Collection v2.1",
            WorkspaceImportFormat::OpenApiV3 => "OpenAPI v3 (JSON/YAML)",
        };
        f.write_str(label)
    }
}

const IMPORT_FORMAT_OPTIONS: [ImportFormatOption; 4] = [
    ImportFormatOption(WorkspaceImportFormat::Auto),
    ImportFormatOption(WorkspaceImportFormat::NativeWorkspaceV1),
    ImportFormatOption(WorkspaceImportFormat::PostmanCollectionV21),
    ImportFormatOption(WorkspaceImportFormat::OpenApiV3),
];

/// Construye el `Workspace_Panel` lateral para el estado de nivel superior.
///
/// Cuando el panel está colapsado (`state.workspace_panel.collapsed`), solo
/// se muestra un botón para expandirlo. Cuando está expandido, se muestra
/// un botón para colapsarlo junto con las tabs de sección (Environments/
/// Data/History/Diagnostics/App updates).
#[allow(dead_code)]
pub fn view(state: &Midway) -> Element<'_, Message> {
    if state.workspace_panel.collapsed {
        return button(text("▶"))
            .on_press(Message::Workspace(WorkspaceMessage::ToggleCollapsed))
            .into();
    }

    column![
        button(text("◀")).on_press(Message::Workspace(WorkspaceMessage::ToggleCollapsed)),
        section_tabs(state),
    ]
    .spacing(8)
    .into()
}

/// Devuelve el contenido de la sección activa del `Workspace_Panel` sin
/// incluir la franja de tabs usada para navegar entre secciones.
pub fn section_content<'a>(state: &'a Midway, _ds: &DesignSystem) -> Element<'a, Message> {
    match state.workspace_panel.active_section {
        WorkspacePanelSection::Environments => environments_section(state),
        WorkspacePanelSection::Data => data_section(state),
        WorkspacePanelSection::History => history_section(state),
        WorkspacePanelSection::Diagnostics => diagnostics_section(state),
        WorkspacePanelSection::AppUpdates => app_updates_section(state),
    }
}

/// Tabs de sección del `Workspace_Panel` (Requisito 4.1), usando el widget
/// compartido `ui::tab_bar` (envoltura de `iced_aw::{TabBar, Tabs}`), igual
/// que las tabs de configuración del `Request_Composer` y las del
/// `Response_Inspector`.
fn section_tabs<'a>(state: &'a Midway) -> Element<'a, Message> {
    let active = state.workspace_panel.active_section;

    let entries: Vec<(
        WorkspacePanelSection,
        &'static str,
        Box<dyn FnOnce() -> Element<'a, Message> + 'a>,
    )> = vec![
        (
            WorkspacePanelSection::Environments,
            "Environments",
            Box::new(move || environments_section(state)),
        ),
        (
            WorkspacePanelSection::Data,
            "Data",
            Box::new(move || data_section(state)),
        ),
        (
            WorkspacePanelSection::History,
            "History",
            Box::new(move || history_section(state)),
        ),
        (
            WorkspacePanelSection::Diagnostics,
            "Diagnostics",
            Box::new(move || diagnostics_section(state)),
        ),
        (
            WorkspacePanelSection::AppUpdates,
            "App updates",
            Box::new(move || app_updates_section(state)),
        ),
    ];

    let ds = DesignSystem::for_mode(ThemeMode::default());
    tab_bar::tabs(
        entries,
        &active,
        |section| Message::Workspace(WorkspaceMessage::SectionSelected(section)),
        &ds,
    )
}

/// Sección Environments del `Workspace_Panel` (Tarea 7.2, Requisitos 4.2,
/// 4.9, 4.10): lista de environments existentes (con acciones editar/
/// eliminar por fila) y formulario de creación/edición (nombre + botón
/// Crear/Guardar), junto con el mensaje de error de la última validación
/// fallida (duplicado, límite de 100 environments, límite de 100
/// caracteres) o del último fallo de guardado/eliminación.
fn environments_section(state: &Midway) -> Element<'_, Message> {
    let busy = state.workspace_panel.environment_busy;

    let mut list = column![].spacing(4);
    if state.workspace.environments.is_empty() {
        list = list.push(text("No hay environments todavía."));
    }
    for environment in &state.workspace.environments {
        list = list.push(environment_row(environment, busy));
    }

    let form = state.workspace_panel.environment_form.editing_id.is_some();
    let submit_label = if form { "Guardar" } else { "Crear" };

    let name_input = text_input(
        "Nombre del environment",
        &state.workspace_panel.environment_form.name_input,
    )
    .on_input(|name| Message::Workspace(WorkspaceMessage::EnvironmentNameInputChanged(name)))
    .width(Length::Fill);

    let mut submit_button = button(text(submit_label));
    if !busy {
        submit_button = submit_button.on_press(Message::Workspace(
            WorkspaceMessage::EnvironmentCreateOrUpdateSubmitted,
        ));
    }

    let mut form_row = row![name_input, submit_button].spacing(8);
    if form {
        let mut cancel_button = button(text("Cancelar"));
        if !busy {
            cancel_button = cancel_button.on_press(Message::Workspace(
                WorkspaceMessage::EnvironmentEditCancelled,
            ));
        }
        form_row = form_row.push(cancel_button);
    }

    let mut content = column![list, form_row].spacing(12);

    if let Some(error_message) = &state.workspace_panel.environment_form.error {
        content = content.push(text(error_message.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
    }

    content.into()
}

/// Fila de un environment existente en la lista de la sección Environments
/// (Tarea 7.2): nombre y botones de acción "Editar"/"Eliminar".
fn environment_row(environment: &EnvironmentRecord, busy: bool) -> Element<'_, Message> {
    let mut edit_button = button(text("Editar"));
    let mut delete_button = button(text("Eliminar"));

    if !busy {
        edit_button = edit_button.on_press(Message::Workspace(
            WorkspaceMessage::EnvironmentEditRequested(environment.id.clone()),
        ));
        delete_button = delete_button.on_press(Message::Workspace(
            WorkspaceMessage::EnvironmentDeleteRequested(environment.id.clone()),
        ));
    }

    row![
        text(environment.name.clone()).width(Length::Fill),
        edit_button,
        delete_button
    ]
    .spacing(8)
    .into()
}

/// Sección Data del `Workspace_Panel` (Tareas 7.6/7.7, Requisitos 4.3, 4.4,
/// 4.5): acciones de export en formato workspace v1 nativo o Postman
/// Collection v2.1, y de import en formato nativo v1, Postman v2.1 u
/// OpenAPI v3 (JSON/YAML), reutilizando `domain::interop` sin reescritura
/// (la orquestación de export/import vive en
/// `app::export_workspace_data`/`app::import_workspace_data`).
fn data_section(state: &Midway) -> Element<'_, Message> {
    column![
        export_section(&state.workspace_panel.export_form),
        import_section(&state.workspace_panel.import_form)
    ]
    .spacing(16)
    .into()
}

/// Subsección de export dentro de Data (Tarea 7.6, Requisito 4.3).
fn export_section(form: &ExportFormState) -> Element<'_, Message> {
    let busy = form.busy;

    let format_picker = pick_list(
        EXPORT_FORMAT_OPTIONS,
        Some(ExportFormatOption(form.export_format)),
        |option| Message::Workspace(WorkspaceMessage::ExportFormatChanged(option.0)),
    )
    .placeholder("Formato de export");

    let path_input = text_input(
        "Ruta de destino (ej. /home/user/export.json)",
        &form.path_input,
    )
    .on_input(|path| Message::Workspace(WorkspaceMessage::ExportPathInputChanged(path)))
    .width(Length::Fill);

    let mut content = column![text("Export de datos"), format_picker, path_input].spacing(8);

    // El id de collection solo es relevante para Postman v2.1 (que exporta
    // una única collection); el export nativo siempre exporta el snapshot
    // completo del workspace.
    if form.export_format == WorkspaceExportFormat::PostmanCollectionV21 {
        let collection_id_input =
            text_input("Id de la collection a exportar", &form.collection_id_input)
                .on_input(|collection_id| {
                    Message::Workspace(WorkspaceMessage::ExportCollectionIdInputChanged(
                        collection_id,
                    ))
                })
                .width(Length::Fill);
        content = content.push(collection_id_input);
    }

    let mut export_button = button(text("Exportar"));
    if !busy {
        export_button =
            export_button.on_press(Message::Workspace(WorkspaceMessage::ExportSubmitted));
    }
    content = content.push(export_button);

    if let Some(result_message) = &form.result_message {
        let color = if form.result_is_error {
            Color::from_rgb(0.8, 0.1, 0.1)
        } else {
            Color::from_rgb(0.1, 0.6, 0.1)
        };
        content = content.push(text(result_message.clone()).color(color));
    }

    content.into()
}

/// Subsección de import dentro de Data (Tarea 7.7, Requisitos 4.4, 4.5): el
/// contenido a importar se pega como texto plano (no hay diálogo nativo de
/// selección de archivo todavía); el formato puede detectarse
/// automáticamente o fijarse explícitamente. Al importar un environment,
/// request o colección con nombre ya existente, la orquestación async
/// (`app::import_workspace_data`) le asigna un nombre diferenciado antes de
/// persistirlo (Tarea 7.9, Requisito 4.11), preservando ambos elementos.
fn import_section(form: &ImportFormState) -> Element<'_, Message> {
    let busy = form.busy;

    let format_picker = pick_list(
        IMPORT_FORMAT_OPTIONS,
        Some(ImportFormatOption(form.import_format)),
        |option| Message::Workspace(WorkspaceMessage::ImportFormatChanged(option.0)),
    )
    .placeholder("Formato de import");

    let payload_input = text_input(
        "Pegá aquí el contenido a importar (JSON/YAML)",
        &form.payload_input,
    )
    .on_input(|payload| Message::Workspace(WorkspaceMessage::ImportPayloadInputChanged(payload)))
    .width(Length::Fill);

    let mut import_button = button(text("Importar"));
    if !busy {
        import_button =
            import_button.on_press(Message::Workspace(WorkspaceMessage::ImportSubmitted));
    }

    let mut content = column![
        text("Import de datos"),
        format_picker,
        payload_input,
        import_button
    ]
    .spacing(8);

    if let Some(result_message) = &form.result_message {
        let color = if form.result_is_error {
            Color::from_rgb(0.8, 0.1, 0.1)
        } else {
            Color::from_rgb(0.1, 0.6, 0.1)
        };
        content = content.push(text(result_message.clone()).color(color));
    }

    content.into()
}

/// Sección History del `Workspace_Panel` (Tarea 7.11, Requisito 4.6):
/// lista `state.workspace.history` (hasta `HISTORY_LIMIT` = 500
/// `HistoryEntry`, poblada por `workspace_snapshot` al seleccionar esta
/// sección por primera vez) del más reciente al más antiguo -orden ya
/// garantizado por la consulta SQL de `workspace_snapshot`
/// (`ORDER BY created_at DESC`)-, mostrando método/url/status (o mensaje
/// de error)/duración/fecha por entrada, igual de simple que la sección
/// Diagnostics (Tarea 7.12).
fn history_section(state: &Midway) -> Element<'_, Message> {
    if state.workspace_panel.history_loading {
        return text("Cargando historial...").into();
    }

    let mut content = column![].spacing(4);

    if let Some(error_message) = &state.workspace_panel.history_error {
        content = content.push(text(error_message.clone()).color(Color::from_rgb(0.8, 0.1, 0.1)));
    }

    if state.workspace.history.is_empty() {
        content = content.push(text("No hay requests en el historial todavía."));
    } else {
        let mut list = column![].spacing(4);
        for entry in &state.workspace.history {
            list = list.push(history_entry_row(entry));
        }
        content = content.push(scrollable(list).height(Length::Fill));
    }

    content.into()
}

/// Fila de un `HistoryEntry` en la sección History (Tarea 7.11): método,
/// url, status (o `error_message` si la request falló), duración y fecha
/// de creación.
fn history_entry_row(entry: &HistoryEntry) -> Element<'_, Message> {
    let outcome = match (entry.response_status, &entry.error_message) {
        (Some(status), _) => status.to_string(),
        (None, Some(error_message)) => error_message.clone(),
        (None, None) => "—".to_string(),
    };
    let duration = entry
        .duration_ms
        .map(|duration_ms| format!("{duration_ms} ms"))
        .unwrap_or_else(|| "—".to_string());

    column![
        row![
            text(entry.method.to_string()),
            text(entry.url.clone()).width(Length::Fill),
            text(entry.created_at.clone()),
        ]
        .spacing(8),
        row![text(outcome), text(duration)].spacing(8),
    ]
    .spacing(2)
    .into()
}

/// Sección Diagnostics del `Workspace_Panel` (Tarea 7.12, Requisito 4.7):
/// lista `state.crash_log` (hasta 200 `CrashRecord`, poblado por
/// `diagnostics::read_crash_records` al seleccionar esta sección) del más
/// reciente al más antiguo, mostrando `source`/`message`/`created_at` por
/// entrada, igual de simple que el resto de secciones actuales.
fn diagnostics_section(state: &Midway) -> Element<'_, Message> {
    if state.crash_log.is_empty() {
        return text("No hay cierres inesperados registrados.").into();
    }

    let mut list = column![].spacing(4);
    for record in &state.crash_log {
        list = list.push(crash_record_row(record));
    }

    scrollable(list).height(Length::Fill).into()
}

/// Fila de un `CrashRecord` en la sección Diagnostics (Tarea 7.12):
/// `source`, `message` y `created_at`, sin transformación adicional.
fn crash_record_row(record: &CrashRecord) -> Element<'_, Message> {
    column![
        row![
            text(crash_source_label(record.source)),
            text(record.created_at.clone())
        ]
        .spacing(8),
        text(record.message.clone()),
    ]
    .spacing(2)
    .into()
}

/// Etiqueta legible de un `CrashSource` (Tarea 7.12), usada por
/// `crash_record_row` para mostrar el origen del crash.
fn crash_source_label(source: crate::diagnostics::CrashSource) -> &'static str {
    match source {
        crate::diagnostics::CrashSource::WindowError => "window error",
        crate::diagnostics::CrashSource::UnhandledPanic => "unhandled panic",
        crate::diagnostics::CrashSource::ComponentBoundary => "component boundary",
    }
}

/// Sección App updates del `Workspace_Panel` (Tarea 7.15, Requisito 4.8):
/// card con el estado actual del Updater (`state.updater.status`),
/// reemplazo directo de `UpdateCenterCard.tsx`. Es una lectura pura del
/// estado (sin `WorkspaceMessage` ni botones funcionales): la lógica de red
/// que produce transiciones entre estados (comprobar/descargar/instalar)
/// se implementa en la Fase 6 (Tareas 13.1-13.10).
fn app_updates_section(state: &Midway) -> Element<'_, Message> {
    let status_text = match &state.updater.status {
        UpdaterStatus::NoUpdatesAvailable => "Estás en la última versión.".to_string(),
        UpdaterStatus::UpdateAvailable { version } => {
            format!("Actualización disponible: v{version}")
        }
        UpdaterStatus::Downloading { progress_percent } => {
            format!("Descargando actualización... {progress_percent}%")
        }
        UpdaterStatus::ReadyToInstall { version } => {
            format!("Actualización v{version} lista para instalar.")
        }
    };

    column![text("Actualizaciones de la app"), text(status_text)]
        .spacing(4)
        .into()
}
