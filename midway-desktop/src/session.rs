//! `session.rs` (Tarea 11.4, Fase 5): `Session_Store` — `SessionSnapshot`,
//! escritura atómica en `session.json` y el mecanismo de autosave.
//!
//! Sigue el mismo patrón de resolución de ruta y de funciones `_at`
//! parametrizadas por `path` (para tests con un directorio temporal) que
//! `diagnostics.rs` (Tarea 7.12). A diferencia de `diagnostics.rs`
//! (`std::fs::write` directo), la escritura aquí es ATÓMICA: se serializa a
//! un archivo temporal en el mismo directorio y luego se hace `rename` sobre
//! la ruta final. `std::fs::rename` es una operación atómica dentro del
//! mismo sistema de archivos: un crash a mitad de la escritura del archivo
//! temporal nunca deja `session.json` en un estado a medio escribir, porque
//! el único punto en que el archivo final cambia de contenido es el
//! `rename` mismo (una operación indivisible a nivel de sistema operativo).
//!
//! `TabSnapshot`/`RequestTab` se reutilizan tal cual desde `crate::app`
//! (coinciden exactamente con la forma descrita en el diseño, "Data Models
//! > Formato de sesión persistida"), en lugar de duplicar un tipo idéntico
//! aquí.
//!
//! El descarte seguro de una sesión corrupta o con `version` incompatible
//! (Criterio 6.10, Tarea 11.16) se construye sobre la primitiva de lectura
//! de la Tarea 11.4 ([`read_session_snapshot_at`]) en
//! [`load_session_or_default`]/[`SessionLoadOutcome`]: nunca propaga un
//! error hacia el arranque, y distingue "no hay sesión todavía" (primer
//! arranque, sin notificación) de "había una sesión pero se descartó por
//! estar corrupta o ser incompatible" (sí amerita notificar al usuario).
//!
//! Ver diseño: "Components and Interfaces > Command Palette, Session_Store
//! y shortcuts (Fase 5)"; "Error Handling".
//! Ver requisitos: 6.3, 6.10.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use midway_core::app::errors::{AppError, AppResult};

use crate::{app::TabSnapshot, ui::design_system::ThemeMode};

/// Nombre de la aplicación usado para resolver el subdirectorio de datos.
///
/// Se duplica localmente en lugar de reexportar la constante privada de
/// `diagnostics.rs` (o hacerla `pub(crate)` allí): es un `&str` trivial de
/// una sola línea, y `diagnostics.rs`/`state.rs` ya duplican esta misma
/// constante entre sí (mismo estilo de duplicación ya establecido en el
/// codebase para este identificador concreto), así que mantener `session.rs`
/// desacoplado de los detalles internos de `diagnostics.rs` es la opción
/// más simple y consistente.
const APP_NAME: &str = "midway";

/// Nombre del archivo de persistencia de la sesión dentro del data dir.
const SESSION_FILE_NAME: &str = "session.json";

/// Sufijo del archivo temporal usado para la escritura atómica (Tarea
/// 11.4): se escribe primero a `session.json.tmp` y luego se hace `rename`
/// sobre `session.json`.
const SESSION_TMP_FILE_NAME: &str = "session.json.tmp";

/// Versión actual del esquema de `SessionSnapshot` (Data Models > Formato
/// de sesión persistida). La Tarea 11.16 usará este valor para decidir si
/// una sesión persistida previamente sigue siendo compatible.
pub const SESSION_SCHEMA_VERSION: u32 = 1;

/// Límite del stack de tabs cerradas (Requisito 6.5, Tarea 11.8): se
/// declara aquí (junto al formato persistido que lo transporta) aunque la
/// lógica de evicción LIFO se implemente en `app.rs` en una tarea
/// posterior.
pub const CLOSED_TABS_LIMIT: usize = 20;

/// Tamaños de los paneles redimensionables (Workspace_Panel, Response
/// Inspector), persistidos como parte de la sesión.
///
/// Ver diseño: "Data Models > Formato de sesión persistida".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelSizes {
    pub workspace_panel_width: f32,
    pub response_panel_height: f32,
    /// Ancho del editor de request cuando editor y respuesta están lado a lado.
    /// `serde(default)` mantiene compatibles las sesiones creadas antes de
    /// que este divisor fuese redimensionable.
    #[serde(default = "default_request_panel_width")]
    pub request_panel_width: f32,
}

fn default_request_panel_width() -> f32 {
    360.0
}

impl Default for PanelSizes {
    /// Valores por defecto razonables, usados mientras no exista lógica de
    /// redimensionamiento de paneles todavía (esa lógica llega en tareas
    /// posteriores de la Fase 5).
    fn default() -> Self {
        Self {
            workspace_panel_width: 280.0,
            response_panel_height: 320.0,
            request_panel_width: default_request_panel_width(),
        }
    }
}

/// Snapshot completo de la sesión persistido en `session.json` (Data Models
/// > Formato de sesión persistida).
///
/// `open_tabs`/`closed_tabs` reutilizan `crate::app::TabSnapshot` (idéntico
/// en forma al `TabSnapshot` descrito en el diseño); `closed_tabs` se
/// persiste como `Vec` (no `VecDeque`, que no deriva `Serialize`/
/// `Deserialize` directamente en todas las versiones de `serde` sin
/// features adicionales) en el mismo orden en memoria (más reciente
/// primero), acotado a [`CLOSED_TABS_LIMIT`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub version: u32,
    pub active_tab_id: Option<String>,
    pub open_tabs: Vec<TabSnapshot>,
    pub closed_tabs: Vec<TabSnapshot>,
    pub panel_sizes: PanelSizes,
    /// Tema seleccionado por el usuario. `default` mantiene compatibles
    /// las sesiones guardadas antes de que este campo existiera, que se
    /// restauran con el tema claro en vez de descartarse como corruptas.
    #[serde(default)]
    pub theme_mode: ThemeMode,
    /// RFC3339.
    pub saved_at: String,
    /// ID de la colección activa al momento del autosave. `default`
    /// garantiza retrocompatibilidad con snapshots anteriores que no
    /// incluyen este campo (se deserializan como `None`).
    #[serde(default)]
    pub active_collection_id: Option<String>,
}

/// Resuelve la ruta del archivo `session.json` dentro del data dir de la
/// aplicación, usando el mismo patrón que `diagnostics::diagnostics_file_path`
/// / `state::AppState::initialize` (crate `dirs`, subdirectorio `APP_NAME`).
///
/// No crea el directorio: [`write_session_snapshot`] es responsable de
/// crearlo si no existe.
pub fn session_file_path() -> AppResult<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| {
            AppError::Io("No se pudo resolver el directorio de datos del sistema.".to_string())
        })?
        .join(APP_NAME);

    Ok(data_dir.join(SESSION_FILE_NAME))
}

/// Escribe `snapshot` en el `session.json` real del data dir de la
/// aplicación, de forma atómica (ver [`write_session_snapshot_at`]).
pub fn write_session_snapshot(snapshot: &SessionSnapshot) -> AppResult<()> {
    let path = session_file_path()?;

    write_session_snapshot_at(&path, snapshot)
}

/// Variante de [`write_session_snapshot`] parametrizada por `path`, usada
/// tanto por la API pública (con la ruta resuelta vía [`session_file_path`])
/// como por los tests unitarios (con una ruta dentro de un directorio
/// temporal).
///
/// Escritura atómica: serializa `snapshot`, escribe el resultado en un
/// archivo temporal (`<nombre>.tmp`) en el MISMO directorio que `path` (
/// `std::fs::rename` solo es atómico dentro del mismo sistema de archivos;
/// usar un directorio distinto, como `std::env::temp_dir()`, podría forzar
/// una copia entre dispositivos en algunas plataformas y perder la
/// atomicidad) y luego renombra ese archivo temporal sobre `path`. Crea el
/// directorio contenedor si no existe.
fn write_session_snapshot_at(path: &Path, snapshot: &SessionSnapshot) -> AppResult<()> {
    let parent = path.parent().ok_or_else(|| {
        AppError::Io("La ruta de session.json no tiene un directorio contenedor.".to_string())
    })?;

    std::fs::create_dir_all(parent).map_err(|error| AppError::Io(error.to_string()))?;

    let serialized = serde_json::to_string(snapshot)
        .map_err(|error| AppError::Serialization(error.to_string()))?;

    let tmp_path = parent.join(SESSION_TMP_FILE_NAME);

    std::fs::write(&tmp_path, serialized).map_err(|error| AppError::Io(error.to_string()))?;

    // El `rename` es el único punto en que `path` cambia de contenido
    // observable: si el proceso se cae durante el `write` anterior, el
    // `session.json` previo (si existía) permanece intacto y el `.tmp`
    // parcialmente escrito queda huérfano, pero nunca corrupto a mitad de
    // lectura.
    std::fs::rename(&tmp_path, path).map_err(|error| AppError::Io(error.to_string()))
}

/// Lee y deserializa el `SessionSnapshot` persistido en el `session.json`
/// real del data dir de la aplicación.
///
/// Primitiva de lectura simple para esta tarea (11.4): propaga cualquier
/// fallo de I/O o de deserialización como `Err`. El descarte seguro de una
/// sesión corrupta o con `version` incompatible (Criterio 6.10, "iniciar
/// una sesión vacía sin abortar el arranque") se construye sobre esta
/// función en la Tarea 11.16, sin adelantarlo aquí.
///
/// API pública de lectura directa; el arranque usa `load_session_or_default`
/// (más seguro), por lo que este primitivo queda expuesto pero sin llamador
/// interno todavía, de ahí `#[allow(dead_code)]`.
#[allow(dead_code)]
pub fn read_session_snapshot() -> AppResult<SessionSnapshot> {
    let path = session_file_path()?;

    read_session_snapshot_at(&path)
}

/// Variante de [`read_session_snapshot`] parametrizada por `path`, usada
/// tanto por la API pública como por los tests unitarios.
fn read_session_snapshot_at(path: &Path) -> AppResult<SessionSnapshot> {
    let raw = std::fs::read_to_string(path).map_err(|error| AppError::Io(error.to_string()))?;

    serde_json::from_str(&raw).map_err(|error| AppError::Serialization(error.to_string()))
}

/// Resultado de un intento de carga "segura" de la sesión persistida
/// (Tarea 11.16, Criterio 6.10).
///
/// Distingue tres casos, porque solo uno de ellos amerita una notificación
/// visible al usuario:
///
/// - [`SessionLoadOutcome::Loaded`]: `session.json` existe, se pudo
///   deserializar y su `version` coincide con [`SESSION_SCHEMA_VERSION`].
///   Caso normal, sin notificación.
/// - [`SessionLoadOutcome::NotFound`]: `session.json` no existe todavía
///   (primer arranque, o la app se ejecuta con un data dir nuevo). Esto NO
///   es una sesión "corrupta o incompatible" en el sentido del Criterio
///   6.10 (que enumera explícitamente solo "falla la deserialización" o
///   "`version` no es compatible"), así que tampoco amerita notificación:
///   mostrar "tu sesión estaba corrupta" en una instalación nueva sería un
///   falso positivo confuso para el usuario.
/// - [`SessionLoadOutcome::DiscardedCorruptOrIncompatible`]: `session.json`
///   existe pero no se pudo leer/deserializar, o se pudo deserializar pero
///   su `version` no coincide con [`SESSION_SCHEMA_VERSION`]. Este es el
///   único caso que amerita notificar al usuario (Criterio 6.10): el
///   contenido se descarta y `reason` describe por qué, para que el
///   llamador construya el mensaje visible.
#[derive(Debug)]
pub enum SessionLoadOutcome {
    /// Sesión cargada con éxito, compatible con la versión actual del
    /// esquema.
    Loaded(SessionSnapshot),
    /// No existe ningún `session.json` todavía (primer arranque). No es un
    /// descarte: no hay nada que notificar.
    NotFound,
    /// `session.json` existía pero se descartó por estar corrupto
    /// (deserialización fallida) o por tener una `version` incompatible
    /// con [`SESSION_SCHEMA_VERSION`]. `reason` es un mensaje legible
    /// pensado para mostrarse al usuario.
    DiscardedCorruptOrIncompatible { reason: String },
}

/// Carga la sesión persistida en el `session.json` real del data dir de la
/// aplicación de forma segura (Tarea 11.16, Criterio 6.10): nunca entra en
/// pánico ni aborta el arranque, sin importar lo que haya (o no) en disco.
///
/// Si no se puede ni siquiera resolver la ruta del data dir (caso
/// extremadamente improbable, ver [`session_file_path`]), se trata igual
/// que "no existe sesión" ([`SessionLoadOutcome::NotFound`]): no hay una
/// `version` incompatible ni contenido corrupto que reportar, así que no
/// amerita una notificación de descarte, solo arrancar con una sesión
/// vacía.
pub fn load_session_or_default() -> SessionLoadOutcome {
    match session_file_path() {
        Ok(path) => load_session_or_default_at(&path),
        Err(_) => SessionLoadOutcome::NotFound,
    }
}

/// Variante de [`load_session_or_default`] parametrizada por `path`, usada
/// tanto por la API pública como por los tests unitarios.
///
/// Comprueba primero la existencia del archivo (para distinguir "primer
/// arranque" de "existe pero está corrupto") y solo entonces reutiliza
/// [`read_session_snapshot_at`], la primitiva de lectura + deserialización
/// directa de la Tarea 11.4, para el intento de lectura real.
fn load_session_or_default_at(path: &Path) -> SessionLoadOutcome {
    if !path.exists() {
        return SessionLoadOutcome::NotFound;
    }

    match read_session_snapshot_at(path) {
        Err(error) => SessionLoadOutcome::DiscardedCorruptOrIncompatible {
            reason: format!("No se pudo leer o interpretar session.json: {error}"),
        },
        Ok(snapshot) if snapshot.version != SESSION_SCHEMA_VERSION => {
            SessionLoadOutcome::DiscardedCorruptOrIncompatible {
                reason: format!(
                    "La sesión guardada usa una versión de esquema incompatible (encontrada {}, esperada {}).",
                    snapshot.version, SESSION_SCHEMA_VERSION
                ),
            }
        }
        Ok(snapshot) => SessionLoadOutcome::Loaded(snapshot),
    }
}

#[cfg(test)]
mod tests {
    //! Tests unitarios de `session.rs` (Tarea 11.4).
    //!
    //! Usan `write_session_snapshot_at` / `read_session_snapshot_at` con una
    //! ruta dentro de un directorio temporal (`tempfile::tempdir`) para
    //! evitar tocar el data dir real de la plataforma durante los tests.

    use super::*;
    use crate::app::RequestTab;
    use midway_core::domain::http::{
        AuthConfig, BodyMode, HttpMethod, RequestBodyDraft, RequestDraft,
    };

    fn session_path_in(dir: &Path) -> PathBuf {
        dir.join(SESSION_FILE_NAME)
    }

    fn sample_draft(url: &str) -> RequestDraft {
        RequestDraft {
            id: None,
            name: "sample".to_string(),
            method: HttpMethod::GET,
            url: url.to_string(),
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
        }
    }

    fn sample_tab(id: &str, url: &str) -> TabSnapshot {
        TabSnapshot {
            id: id.to_string(),
            draft: sample_draft(url),
            active_request_tab: RequestTab::Params,
        }
    }

    fn sample_snapshot() -> SessionSnapshot {
        SessionSnapshot {
            version: SESSION_SCHEMA_VERSION,
            active_tab_id: Some("tab-1".to_string()),
            open_tabs: vec![
                sample_tab("tab-1", "https://example.com/a"),
                sample_tab("tab-2", "https://example.com/b"),
            ],
            closed_tabs: vec![sample_tab("tab-closed-1", "https://example.com/closed")],
            panel_sizes: PanelSizes {
                workspace_panel_width: 280.0,
                response_panel_height: 320.0,
                request_panel_width: 360.0,
            },
            theme_mode: ThemeMode::default(),
            saved_at: "2024-01-01T00:00:00Z".to_string(),
            active_collection_id: Some("collection-1".to_string()),
        }
    }

    #[test]
    fn panel_sizes_from_older_session_default_request_panel_width() {
        let panel_sizes: PanelSizes =
            serde_json::from_str(r#"{"workspacePanelWidth":280.0,"responsePanelHeight":320.0}"#)
                .expect("el formato anterior debe seguir siendo compatible");

        assert_eq!(panel_sizes.workspace_panel_width, 280.0);
        assert_eq!(panel_sizes.response_panel_height, 320.0);
        assert_eq!(panel_sizes.request_panel_width, 360.0);
    }

    /// Round-trip: escribir un `SessionSnapshot` de forma atómica y volver a
    /// leerlo produce datos estructuralmente idénticos a los originales.
    #[test]
    fn write_then_read_round_trips_exact_snapshot() {
        let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        let path = session_path_in(temp_dir.path());
        let snapshot = sample_snapshot();

        write_session_snapshot_at(&path, &snapshot)
            .expect("la escritura atómica no debería fallar");

        let read_back = read_session_snapshot_at(&path).expect("la lectura no debería fallar");

        assert_eq!(read_back.version, snapshot.version);
        assert_eq!(read_back.active_tab_id, snapshot.active_tab_id);
        assert_eq!(read_back.panel_sizes, snapshot.panel_sizes);
        assert_eq!(read_back.saved_at, snapshot.saved_at);
        assert_eq!(read_back.open_tabs.len(), snapshot.open_tabs.len());
        assert_eq!(read_back.closed_tabs.len(), snapshot.closed_tabs.len());
        for (actual, expected) in read_back.open_tabs.iter().zip(snapshot.open_tabs.iter()) {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.draft.url, expected.draft.url);
            assert_eq!(actual.active_request_tab, expected.active_request_tab);
        }
        for (actual, expected) in read_back
            .closed_tabs
            .iter()
            .zip(snapshot.closed_tabs.iter())
        {
            assert_eq!(actual.id, expected.id);
            assert_eq!(actual.draft.url, expected.draft.url);
        }
    }

    /// El mecanismo de escritura atómica (temp file + rename) no deja ningún
    /// archivo temporal residual tras una escritura exitosa, y el archivo
    /// final resultante es exactamente el esperado (no hay una etapa
    /// intermedia observable desde fuera del proceso).
    #[test]
    fn atomic_write_leaves_no_residual_tmp_file_and_final_file_is_correct() {
        let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        let path = session_path_in(temp_dir.path());
        let snapshot = sample_snapshot();

        write_session_snapshot_at(&path, &snapshot)
            .expect("la escritura atómica no debería fallar");

        let tmp_path = temp_dir.path().join(SESSION_TMP_FILE_NAME);
        assert!(
            !tmp_path.exists(),
            "el archivo temporal no debería sobrevivir a un rename exitoso"
        );
        assert!(
            path.exists(),
            "el archivo final debería existir tras la escritura"
        );

        let read_back = read_session_snapshot_at(&path).expect("la lectura no debería fallar");
        assert_eq!(read_back.version, snapshot.version);
        assert_eq!(read_back.open_tabs.len(), snapshot.open_tabs.len());
    }

    /// Una segunda escritura atómica sobre un `session.json` ya existente
    /// reemplaza por completo el contenido anterior (el `rename` sobrescribe
    /// el destino), sin mezclar datos de ambas escrituras.
    #[test]
    fn second_atomic_write_fully_replaces_previous_content() {
        let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        let path = session_path_in(temp_dir.path());

        let first = sample_snapshot();
        write_session_snapshot_at(&path, &first).expect("la primera escritura no debería fallar");

        let mut second = sample_snapshot();
        second.active_tab_id = Some("tab-2".to_string());
        second.open_tabs = vec![sample_tab("tab-2", "https://example.com/only-second")];
        write_session_snapshot_at(&path, &second).expect("la segunda escritura no debería fallar");

        let read_back = read_session_snapshot_at(&path).expect("la lectura no debería fallar");
        assert_eq!(read_back.active_tab_id, Some("tab-2".to_string()));
        assert_eq!(read_back.open_tabs.len(), 1);
        assert_eq!(read_back.open_tabs[0].id, "tab-2");
    }

    /// Leer una ruta inexistente propaga un error en lugar de entrar en
    /// pánico (la Tarea 11.16 construye el descarte seguro sobre este
    /// comportamiento).
    #[test]
    fn read_missing_file_returns_err() {
        let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        let path = session_path_in(temp_dir.path());

        let result = read_session_snapshot_at(&path);
        assert!(result.is_err());
    }

    /// Tests unitarios de [`load_session_or_default_at`] (Tarea 11.16,
    /// Criterio 6.10): el descarte seguro de una sesión corrupta o
    /// incompatible.
    mod load_session_or_default {
        use super::*;

        /// (a) Un archivo inexistente (primer arranque) produce
        /// `NotFound`, NO `DiscardedCorruptOrIncompatible`: no hay ninguna
        /// sesión previa que haya sido descartada, así que no amerita
        /// notificar al usuario.
        #[test]
        fn missing_file_returns_not_found_without_notification() {
            let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
            let path = session_path_in(temp_dir.path());

            let outcome = load_session_or_default_at(&path);

            match outcome {
                SessionLoadOutcome::NotFound => {}
                other => panic!("se esperaba NotFound, se obtuvo {other:?}"),
            }
        }

        /// (b) Un archivo con JSON inválido produce
        /// `DiscardedCorruptOrIncompatible` con un `reason` no vacío,
        /// nunca entra en pánico.
        #[test]
        fn invalid_json_returns_discarded_corrupt_with_reason() {
            let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
            let path = session_path_in(temp_dir.path());
            std::fs::write(&path, "{ esto no es JSON válido")
                .expect("no se pudo escribir el archivo de prueba");

            let outcome = load_session_or_default_at(&path);

            match outcome {
                SessionLoadOutcome::DiscardedCorruptOrIncompatible { reason } => {
                    assert!(
                        !reason.is_empty(),
                        "el motivo del descarte no debería estar vacío"
                    );
                }
                other => panic!("se esperaba DiscardedCorruptOrIncompatible, se obtuvo {other:?}"),
            }
        }

        /// (c) Un archivo con JSON válido pero `version` distinta de
        /// `SESSION_SCHEMA_VERSION` produce `DiscardedCorruptOrIncompatible`
        /// con un `reason` no vacío, sin propagar un error.
        #[test]
        fn incompatible_version_returns_discarded_incompatible_with_reason() {
            let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
            let path = session_path_in(temp_dir.path());
            let mut snapshot = sample_snapshot();
            snapshot.version = SESSION_SCHEMA_VERSION + 1;
            write_session_snapshot_at(&path, &snapshot).expect("la escritura no debería fallar");

            let outcome = load_session_or_default_at(&path);

            match outcome {
                SessionLoadOutcome::DiscardedCorruptOrIncompatible { reason } => {
                    assert!(
                        !reason.is_empty(),
                        "el motivo del descarte no debería estar vacío"
                    );
                }
                other => panic!("se esperaba DiscardedCorruptOrIncompatible, se obtuvo {other:?}"),
            }
        }

        /// (d) Un archivo válido y compatible produce `Loaded` con el
        /// snapshot exacto que se había escrito.
        #[test]
        fn valid_compatible_file_returns_loaded_with_exact_snapshot() {
            let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
            let path = session_path_in(temp_dir.path());
            let snapshot = sample_snapshot();
            write_session_snapshot_at(&path, &snapshot).expect("la escritura no debería fallar");

            let outcome = load_session_or_default_at(&path);

            match outcome {
                SessionLoadOutcome::Loaded(loaded) => {
                    assert_eq!(loaded.version, snapshot.version);
                    assert_eq!(loaded.active_tab_id, snapshot.active_tab_id);
                    assert_eq!(loaded.panel_sizes, snapshot.panel_sizes);
                    assert_eq!(loaded.saved_at, snapshot.saved_at);
                    assert_eq!(loaded.open_tabs.len(), snapshot.open_tabs.len());
                }
                other => panic!("se esperaba Loaded, se obtuvo {other:?}"),
            }
        }
    }

    /// Property test de [`load_session_or_default_at`] (Tarea 11.17,
    /// Property 24, Criterio 6.10): generaliza los tests unitarios fijos
    /// de `invalid_json_returns_discarded_corrupt_with_reason` e
    /// `incompatible_version_returns_discarded_incompatible_with_reason`
    /// (arriba) a contenido de `session.json` ARBITRARIO.
    ///
    /// Esta propiedad se enfoca exclusivamente en el contrato propio de
    /// `load_session_or_default_at`: para cualquier contenido que no
    /// deserialice como `SessionSnapshot` válido o cuya `version` no sea
    /// [`SESSION_SCHEMA_VERSION`], la función SHALL resolver siempre a
    /// `DiscardedCorruptOrIncompatible { reason }` con un `reason` no
    /// vacío, sin entrar en pánico ni devolver un `Err` que pudiera
    /// abortar el arranque (si algo de esto ocurriera, el test de
    /// propiedad abortaría, ya que `load_session_or_default_at` devuelve
    /// `SessionLoadOutcome`, no un `Result`). Verificar que el llamador
    /// (`Midway::new`, Tarea 11.16) efectivamente inicia una sesión vacía
    /// y notifica al usuario a partir de esta variante es responsabilidad
    /// de la Tarea 11.16, no de esta propiedad.
    ///
    /// El caso complementario -sesión VÁLIDA y COMPATIBLE con campos
    /// arbitrarios produce `Loaded(_)`, no un descarte- ya está cubierto
    /// de forma adecuada por el test unitario fijo
    /// `valid_compatible_file_returns_loaded_with_exact_snapshot` (arriba,
    /// con `sample_snapshot()`): ese test ya ejercita un `SessionSnapshot`
    /// completo (con tabs abiertas/cerradas, `active_tab_id`, etc.) por lo
    /// que duplicarlo aquí como una propiedad no aportaría cobertura
    /// adicional relevante para el criterio 6.10; esta propiedad se
    /// concentra en el lado de detección de corrupción/incompatibilidad.
    mod load_session_or_default_property_tests {
        use super::*;
        use proptest::prelude::*;
        use proptest::test_runner::TestCaseError;

        /// Genera contenido que NO deserializa como `SessionSnapshot`
        /// válido, en dos variantes (mismo espíritu que
        /// `arb_unsupported_or_corrupt_payload` en `app.rs`, Property 13,
        /// Tarea 7.8, adaptado localmente a este módulo):
        ///
        /// - Un objeto JSON sintácticamente VÁLIDO pero con un único campo
        ///   arbitrario ajeno a la forma de `SessionSnapshot` (le faltan
        ///   todos los campos requeridos: `version`, `activeTabId`,
        ///   `openTabs`, `closedTabs`, `panelSizes`, `savedAt`), por lo que
        ///   `serde_json::from_str::<SessionSnapshot>` SHALL fallar por
        ///   campos faltantes.
        /// - Una secuencia de llaves de apertura sin cierre
        ///   correspondiente, que ni siquiera es JSON sintácticamente
        ///   válido.
        fn arb_corrupt_or_malformed_content() -> impl Strategy<Value = String> {
            prop_oneof![
                ("[a-zA-Z0-9_-]{1,12}", "[a-zA-Z0-9_-]{0,20}")
                    .prop_map(|(key, value)| format!("{{\"{key}\":\"{value}\"}}")),
                (2usize..=20usize).prop_map(|open_brace_count| "{".repeat(open_brace_count)),
            ]
        }

        /// Genera contenido sintácticamente VÁLIDO como `SessionSnapshot`
        /// (serializa una instancia real, con algunos campos variados de
        /// forma arbitraria) pero con `version` distinta de
        /// [`SESSION_SCHEMA_VERSION`], para ejercitar el criterio de
        /// incompatibilidad de esquema por sí solo (sin mezclarlo con
        /// corrupción sintáctica).
        ///
        /// `open_tabs`/`closed_tabs` se dejan vacíos deliberadamente: la
        /// fidelidad de esos campos no es relevante para esta propiedad
        /// (que solo verifica la detección de `version` incompatible, no
        /// el contenido de las tabs), y mantenerlos vacíos evita acoplar
        /// este generador a la forma completa de `RequestDraft`.
        fn arb_incompatible_version_content() -> impl Strategy<Value = String> {
            (
                1u32..=1_000u32,
                proptest::option::of("[a-zA-Z0-9_-]{1,12}"),
                "[a-zA-Z0-9 _-]{0,20}",
            )
                .prop_map(|(version_offset, active_tab_id, saved_at)| {
                    let snapshot = SessionSnapshot {
                        version: SESSION_SCHEMA_VERSION + version_offset,
                        active_tab_id,
                        open_tabs: Vec::new(),
                        closed_tabs: Vec::new(),
                        panel_sizes: PanelSizes::default(),
                        theme_mode: ThemeMode::default(),
                        saved_at,
                        active_collection_id: None,
                    };

                    serde_json::to_string(&snapshot)
                        .expect("serde_json::to_string no debería fallar para un SessionSnapshot construido")
                })
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

            /// Feature: tauri-to-iced-migration, Property 24: Sesión
            /// corrupta o incompatible se descarta de forma segura.
            /// Validates: Requirements 6.10
            ///
            /// Para cualquier contenido arbitrario de `session.json` que
            /// no deserialice como `SessionSnapshot` válido (corrupto) o
            /// cuya `version` no coincida con `SESSION_SCHEMA_VERSION`
            /// (incompatible), `load_session_or_default_at` SHALL
            /// resolver siempre a `DiscardedCorruptOrIncompatible` con un
            /// `reason` no vacío, sin propagar el fallo de deserialización
            /// como pánico ni como un `Err` que pudiera abortar el
            /// arranque.
            #[test]
            fn property_24_corrupt_or_incompatible_content_is_discarded_safely(
                content in prop_oneof![
                    arb_corrupt_or_malformed_content(),
                    arb_incompatible_version_content(),
                ],
            ) {
                let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
                let path = session_path_in(temp_dir.path());
                std::fs::write(&path, &content).expect("no se pudo escribir el archivo de prueba");

                let outcome = load_session_or_default_at(&path);

                match outcome {
                    SessionLoadOutcome::DiscardedCorruptOrIncompatible { reason } => {
                        prop_assert!(!reason.is_empty(), "el motivo del descarte no debería estar vacío");
                    }
                    other => {
                        return Err(TestCaseError::fail(format!(
                            "se esperaba DiscardedCorruptOrIncompatible, se obtuvo {other:?}"
                        )));
                    }
                }
            }
        }
    }

    /// Property test de round-trip de la sesión persistida (Tarea 11.7,
    /// Property 20, Criterios 6.4, 6.7): generaliza el test unitario fijo
    /// `write_then_read_round_trips_exact_snapshot` (arriba, con
    /// `sample_snapshot()`) a un `SessionSnapshot` ARBITRARIO -tabs
    /// abiertas y cerradas (con `RequestDraft` variado), `active_tab_id`,
    /// `panel_sizes` y `saved_at`-, sobre la primitiva de persistencia ya
    /// implementada en la Tarea 11.4
    /// (`write_session_snapshot_at`/`read_session_snapshot_at`).
    ///
    /// Esta propiedad se concentra exclusivamente en la fidelidad del
    /// round-trip de serialización en sí (escribir y volver a leer
    /// reproduce EXACTAMENTE el mismo valor): es la base sobre la que se
    /// apoyan la restauración de sesión al arrancar (Criterio 6.4, Tarea
    /// 11.6, todavía no implementada) y la recuperación tras un cierre
    /// inesperado (Criterio 6.7), pero esta propiedad no ejercita esa
    /// lógica de restauración/recuperación en sí -solo la primitiva de
    /// persistencia de la que ambas dependen-.
    ///
    /// El generador de `RequestDraft` (`arb_request_draft`, abajo) es
    /// deliberadamente más simple que `arb_request_draft` en
    /// `app.rs` (Property 8, Tarea 3.21): esta propiedad verifica
    /// fidelidad ESTRUCTURAL de la serialización, no un comportamiento
    /// semántico/de dominio (como resolución de templates o preview), así
    /// que no necesita valores "realistas" (por ejemplo, URLs válidas o
    /// bodies JSON bien formados) -basta con variar la FORMA de cada
    /// campo (algunas filas de query/headers habilitadas y otras no, las
    /// 4 variantes de `AuthConfig`, un subconjunto de `BodyMode`, 0 a 2
    /// `response_tests`- para ejercitar cada rama de (de)serialización
    /// involucrada.
    mod session_snapshot_round_trip_property_tests {
        use super::*;
        use midway_core::domain::http::{ApiKeyPlacement, KeyValueRow};
        use midway_core::domain::testing::{AssertionOperator, AssertionSource, ResponseAssertion};
        use proptest::prelude::*;

        /// Alfabeto simple para strings generados en este módulo: no hay
        /// re-parseo ni interpolación de templates involucrados en el
        /// round-trip de serialización, así que no es necesario evitar
        /// ninguna sintaxis en particular (a diferencia de
        /// `arb_preview_token` en `app.rs`, Property 8).
        pub(super) fn arb_token() -> impl Strategy<Value = String> {
            "[a-zA-Z][a-zA-Z0-9_-]{0,9}".prop_map(|s| s.to_string())
        }

        fn arb_key_value_row(id_prefix: &'static str) -> impl Strategy<Value = KeyValueRow> {
            (arb_token(), arb_token(), any::<bool>()).prop_map(move |(key, value, enabled)| {
                KeyValueRow {
                    id: format!("{id_prefix}-{key}"),
                    key,
                    value,
                    enabled,
                }
            })
        }

        fn arb_key_value_rows(id_prefix: &'static str) -> impl Strategy<Value = Vec<KeyValueRow>> {
            proptest::collection::vec(arb_key_value_row(id_prefix), 0..=3)
        }

        /// Cubre las 4 variantes de `AuthConfig` (Tarea 5.4); a diferencia
        /// del generador equivalente en `app.rs`, `ApiKey` siempre usa
        /// `ApiKeyPlacement::Header` (variar `placement` no aporta
        /// cobertura adicional para un round-trip puramente estructural,
        /// que ya varía todos los demás campos del enum).
        fn arb_auth_config() -> impl Strategy<Value = AuthConfig> {
            prop_oneof![
                Just(AuthConfig::None),
                arb_token().prop_map(|token| AuthConfig::Bearer { token }),
                (arb_token(), arb_token())
                    .prop_map(|(username, password)| AuthConfig::Basic { username, password }),
                (arb_token(), arb_token()).prop_map(|(key, value)| AuthConfig::ApiKey {
                    key,
                    value,
                    placement: ApiKeyPlacement::Header,
                }),
            ]
        }

        /// Subconjunto de `BodyMode` (None/Json/Text/FormData): cubre
        /// cada variante del enum al menos una vez, sin generar bodies
        /// "realistas" (JSON bien formado, etc.), consistente con el
        /// enfoque puramente estructural de esta propiedad.
        fn arb_body() -> impl Strategy<Value = RequestBodyDraft> {
            prop_oneof![
                Just(RequestBodyDraft {
                    mode: BodyMode::None,
                    value: String::new(),
                    form_data: Vec::new(),
                }),
                arb_token().prop_map(|value| RequestBodyDraft {
                    mode: BodyMode::Json,
                    value,
                    form_data: Vec::new(),
                }),
                arb_token().prop_map(|value| RequestBodyDraft {
                    mode: BodyMode::Text,
                    value,
                    form_data: Vec::new(),
                }),
            ]
        }

        fn arb_response_assertion() -> impl Strategy<Value = ResponseAssertion> {
            (arb_token(), any::<bool>(), arb_token()).prop_map(|(name, enabled, expected)| {
                ResponseAssertion {
                    id: format!("assertion-{name}"),
                    name,
                    enabled,
                    source: AssertionSource::BodyText,
                    operator: AssertionOperator::Equals,
                    selector: None,
                    expected,
                }
            })
        }

        fn arb_response_tests() -> impl Strategy<Value = Vec<ResponseAssertion>> {
            proptest::collection::vec(arb_response_assertion(), 0..=2)
        }

        /// Generador local de `RequestDraft` para este módulo (ver
        /// comentario del módulo, arriba, sobre por qué es más simple que
        /// el de `app.rs`): varía método, URL, query/headers (incluyendo
        /// filas deshabilitadas), auth, body y `response_tests`.
        fn arb_request_draft() -> impl Strategy<Value = RequestDraft> {
            (
                prop_oneof![
                    Just(HttpMethod::GET),
                    Just(HttpMethod::POST),
                    Just(HttpMethod::PUT),
                    Just(HttpMethod::PATCH),
                    Just(HttpMethod::DELETE),
                    Just(HttpMethod::HEAD),
                    Just(HttpMethod::OPTIONS),
                ],
                arb_token().prop_map(|host| format!("https://{host}.example.com/path")),
                arb_key_value_rows("query"),
                arb_key_value_rows("header"),
                arb_auth_config(),
                arb_body(),
                arb_response_tests(),
            )
                .prop_map(
                    |(method, url, query, headers, auth, body, response_tests)| RequestDraft {
                        id: None,
                        name: "round-trip draft".to_string(),
                        method,
                        url,
                        query,
                        headers,
                        auth,
                        body,
                        timeout_ms: 30_000,
                        environment_id: None,
                        response_tests,
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

        fn arb_tab_snapshot(id_prefix: &'static str) -> impl Strategy<Value = TabSnapshot> {
            (arb_token(), arb_request_draft(), arb_request_tab()).prop_map(
                move |(id, draft, active_request_tab)| TabSnapshot {
                    id: format!("{id_prefix}-{id}"),
                    draft,
                    active_request_tab,
                },
            )
        }

        pub(super) fn arb_tab_snapshots(
            id_prefix: &'static str,
        ) -> impl Strategy<Value = Vec<TabSnapshot>> {
            proptest::collection::vec(arb_tab_snapshot(id_prefix), 0..=3)
        }

        /// Rango finito y acotado, para evitar `NaN`/infinito (que
        /// romperían la comparación de igualdad estructural, ya que
        /// `NaN != NaN`).
        ///
        /// `pub(super)` para que el módulo hermano
        /// `session_snapshot_serde_round_trip_property_tests` (Property 2
        /// del spec `midway-baseline-audit-and-first-vertical`) reutilice
        /// este generador en lugar de duplicarlo.
        pub(super) fn arb_panel_sizes() -> impl Strategy<Value = PanelSizes> {
            (50.0f32..2000.0f32, 50.0f32..2000.0f32, 50.0f32..2000.0f32).prop_map(
                |(workspace_panel_width, response_panel_height, request_panel_width)| PanelSizes {
                    workspace_panel_width,
                    response_panel_height,
                    request_panel_width,
                },
            )
        }

        /// Formato fijo tipo RFC3339 con dígitos variados: `saved_at` no
        /// se valida semánticamente al leer (Tarea 11.4), así que basta
        /// con variar el contenido, no con generar fechas válidas de
        /// calendario.
        pub(super) fn arb_saved_at() -> impl Strategy<Value = String> {
            "[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z".prop_map(|s| s.to_string())
        }

        /// Genera un `SessionSnapshot` arbitrario completo. `version` se
        /// mantiene siempre en `SESSION_SCHEMA_VERSION`: esta propiedad
        /// verifica fidelidad de round-trip, no detección de
        /// incompatibilidad de esquema (eso ya lo cubre el Property 24,
        /// arriba), así que variar `version` no aportaría cobertura
        /// adicional relevante aquí.
        fn arb_session_snapshot() -> impl Strategy<Value = SessionSnapshot> {
            (
                proptest::option::of(arb_token()),
                arb_tab_snapshots("open"),
                arb_tab_snapshots("closed"),
                arb_panel_sizes(),
                arb_saved_at(),
                proptest::option::of(arb_token()),
            )
                .prop_map(
                    |(
                        active_tab_id,
                        open_tabs,
                        closed_tabs,
                        panel_sizes,
                        saved_at,
                        active_collection_id,
                    )| SessionSnapshot {
                        version: SESSION_SCHEMA_VERSION,
                        active_tab_id,
                        open_tabs,
                        closed_tabs,
                        panel_sizes,
                        theme_mode: ThemeMode::default(),
                        saved_at,
                        active_collection_id,
                    },
                )
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 30, ..ProptestConfig::default() })]

            /// Feature: tauri-to-iced-migration, Property 20: Round-trip
            /// de la sesión persistida.
            /// Validates: Requirements 6.4, 6.7
            ///
            /// Para cualquier `SessionSnapshot` generado (tabs abiertas y
            /// cerradas con `RequestDraft` variado, `active_tab_id`,
            /// `panel_sizes` y `saved_at`), escribirlo de forma atómica
            /// vía `write_session_snapshot_at` y volver a leerlo vía
            /// `read_session_snapshot_at` SHALL producir un
            /// `SessionSnapshot` estructuralmente igual al original.
            /// `SessionSnapshot`, `TabSnapshot` y sus tipos anidados
            /// (`RequestDraft`, `RequestTab`, `PanelSizes`, etc.) derivan
            /// `PartialEq`, así que la comparación se hace directamente
            /// con `prop_assert_eq!`, sin necesidad de recurrir a
            /// `serde_json::to_value` (el patrón usado en `app.rs` para
            /// tipos sin `PartialEq`, por ejemplo Property 8/13/17).
            #[test]
            fn property_20_write_then_read_round_trips_arbitrary_snapshot(
                snapshot in arb_session_snapshot(),
            ) {
                let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
                let path = session_path_in(temp_dir.path());

                write_session_snapshot_at(&path, &snapshot)
                    .expect("la escritura atómica no debería fallar para un SessionSnapshot arbitrario");

                let read_back = read_session_snapshot_at(&path)
                    .expect("la lectura no debería fallar tras una escritura exitosa");

                prop_assert_eq!(read_back, snapshot);
            }
        }
    }

    // Feature: ux-flow-redesign, Property 10: Session snapshot round-trip for active_collection_id
    /// Property test para el round-trip de `active_collection_id` en
    /// `SessionSnapshot` (Property 10, Requirements 9.1, 9.4):
    ///
    /// 1. Para cualquier `Option<String>` arbitrario usado como
    ///    `active_collection_id`, serializar un `SessionSnapshot` a JSON
    ///    y deserializarlo de vuelta SHALL preservar el valor exactamente.
    /// 2. Deserializar un JSON que NO contiene el campo
    ///    `active_collection_id` SHALL producir `None`
    ///    (retrocompatibilidad vía `#[serde(default)]`).
    ///
    /// **Validates: Requirements 9.1, 9.4**
    mod active_collection_id_round_trip_property_tests {
        use super::*;
        use proptest::prelude::*;

        /// Genera un `Option<String>` arbitrario para `active_collection_id`:
        /// `None` o un string no vacío con caracteres alfanuméricos y guiones.
        fn arb_active_collection_id() -> impl Strategy<Value = Option<String>> {
            proptest::option::of("[a-zA-Z0-9_-]{1,30}".prop_map(|s| s.to_string()))
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, ..ProptestConfig::default() })]

            /// Feature: ux-flow-redesign, Property 10: Session snapshot
            /// round-trip for active_collection_id.
            /// Validates: Requirements 9.1, 9.4
            ///
            /// Para cualquier `Option<String>` arbitrario usado como
            /// `active_collection_id`, serializar a JSON y deserializar
            /// de vuelta SHALL preservar el valor exactamente.
            #[test]
            fn property_10_active_collection_id_round_trip(
                active_collection_id in arb_active_collection_id(),
            ) {
                let snapshot = SessionSnapshot {
                    version: SESSION_SCHEMA_VERSION,
                    active_tab_id: None,
                    open_tabs: Vec::new(),
                    closed_tabs: Vec::new(),
                    panel_sizes: PanelSizes::default(),
                    theme_mode: ThemeMode::default(),
                    saved_at: "2024-01-01T00:00:00Z".to_string(),
                    active_collection_id: active_collection_id.clone(),
                };

                let json = serde_json::to_string(&snapshot)
                    .expect("serialización no debería fallar");
                let deserialized: SessionSnapshot = serde_json::from_str(&json)
                    .expect("deserialización no debería fallar");

                prop_assert_eq!(
                    deserialized.active_collection_id,
                    active_collection_id,
                    "active_collection_id debe preservarse exactamente tras round-trip"
                );
            }

            /// Feature: ux-flow-redesign, Property 10: Session snapshot
            /// round-trip for active_collection_id (retrocompatibility).
            /// Validates: Requirements 9.1, 9.4
            ///
            /// Para cualquier `SessionSnapshot` serializado cuyo JSON se
            /// modifique eliminando el campo `activeCollectionId`,
            /// deserializar SHALL producir `active_collection_id == None`
            /// gracias a `#[serde(default)]`.
            #[test]
            fn property_10_missing_active_collection_id_deserializes_to_none(
                active_collection_id in arb_active_collection_id(),
            ) {
                let snapshot = SessionSnapshot {
                    version: SESSION_SCHEMA_VERSION,
                    active_tab_id: None,
                    open_tabs: Vec::new(),
                    closed_tabs: Vec::new(),
                    panel_sizes: PanelSizes::default(),
                    theme_mode: ThemeMode::default(),
                    saved_at: "2024-01-01T00:00:00Z".to_string(),
                    active_collection_id,
                };

                let json = serde_json::to_string(&snapshot)
                    .expect("serialización no debería fallar");

                // Remover el campo "activeCollectionId" del JSON para
                // simular un snapshot antiguo que no lo incluye.
                let mut json_value: serde_json::Value = serde_json::from_str(&json)
                    .expect("JSON debería ser válido");
                json_value.as_object_mut()
                    .expect("root debería ser un objeto")
                    .remove("activeCollectionId");

                let modified_json = serde_json::to_string(&json_value)
                    .expect("re-serialización no debería fallar");
                let deserialized: SessionSnapshot = serde_json::from_str(&modified_json)
                    .expect("deserialización sin activeCollectionId no debería fallar");

                prop_assert_eq!(
                    deserialized.active_collection_id,
                    None,
                    "active_collection_id debe ser None cuando el campo está ausente del JSON"
                );
            }
        }
    }

    /// Test de caracterización del baseline SIN CAMBIOS (spec
    /// `midway-baseline-audit-and-first-vertical`, Tarea 4.2, Property 2):
    /// el ida y vuelta de serialización de `SessionSnapshot` a través de
    /// `serde_json`, con foco explícito en `theme_mode` y en TODOS los
    /// campos de `panel_sizes` (Criterio 6.3).
    ///
    /// Complementa (no duplica) el `property_20_...` de arriba: aquel
    /// ejercita la persistencia en disco (`write_session_snapshot_at` +
    /// `read_session_snapshot_at`) manteniendo `theme_mode` fijo en su
    /// valor por defecto, mientras que esta propiedad ejercita la
    /// (de)serialización pura y VARÍA `theme_mode` sobre sus dos
    /// variantes, que es exactamente el campo cuyo comportamiento
    /// observable debe quedar fijado antes de extraer la vertical de tema
    /// (Requisitos 13.7, 13.11: el estado del usuario se persiste, se
    /// recupera tras reiniciar y los datos existentes se preservan).
    ///
    /// Reutiliza los generadores ya existentes en
    /// [`session_snapshot_round_trip_property_tests`] (`arb_panel_sizes`,
    /// `arb_tab_snapshots`, `arb_saved_at`, `arb_token`) en lugar de
    /// duplicarlos.
    mod session_snapshot_serde_round_trip_property_tests {
        use super::session_snapshot_round_trip_property_tests::{
            arb_panel_sizes, arb_saved_at, arb_tab_snapshots, arb_token,
        };
        use super::*;
        use proptest::prelude::*;

        /// Cubre las dos variantes de `ThemeMode` del baseline.
        fn arb_theme_mode() -> impl Strategy<Value = ThemeMode> {
            prop_oneof![Just(ThemeMode::Light), Just(ThemeMode::Dark)]
        }

        /// `SessionSnapshot` arbitrario con `theme_mode` variable.
        /// `version` se mantiene en [`SESSION_SCHEMA_VERSION`]: esta
        /// propiedad verifica fidelidad del ida y vuelta, no detección de
        /// esquema incompatible.
        fn arb_session_snapshot_with_theme() -> impl Strategy<Value = SessionSnapshot> {
            (
                proptest::option::of(arb_token()),
                arb_tab_snapshots("open"),
                arb_tab_snapshots("closed"),
                arb_panel_sizes(),
                arb_theme_mode(),
                arb_saved_at(),
                proptest::option::of(arb_token()),
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
                    )| SessionSnapshot {
                        version: SESSION_SCHEMA_VERSION,
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

            // Feature: midway-baseline-audit-and-first-vertical, Property 2: Para todo
            // `SessionSnapshot` válido, serializarlo a JSON y deserializarlo produce un
            // snapshot igual al original, preservando en particular `theme_mode` y todos
            // los campos de `panel_sizes`.
            /// **Validates: Requirements 6.3, 13.7, 13.11**
            #[test]
            fn property_2_session_snapshot_json_round_trip_preserves_theme_and_panel_sizes(
                snapshot in arb_session_snapshot_with_theme(),
            ) {
                let json = serde_json::to_string(&snapshot)
                    .expect("serializar un SessionSnapshot construido no debería fallar");

                let deserialized: SessionSnapshot = serde_json::from_str(&json)
                    .expect("deserializar el JSON producido por serde no debería fallar");

                prop_assert_eq!(
                    deserialized.theme_mode,
                    snapshot.theme_mode,
                    "theme_mode debe preservarse exactamente tras el ida y vuelta"
                );
                prop_assert_eq!(
                    deserialized.panel_sizes.workspace_panel_width,
                    snapshot.panel_sizes.workspace_panel_width,
                    "panel_sizes.workspace_panel_width debe preservarse exactamente"
                );
                prop_assert_eq!(
                    deserialized.panel_sizes.response_panel_height,
                    snapshot.panel_sizes.response_panel_height,
                    "panel_sizes.response_panel_height debe preservarse exactamente"
                );
                prop_assert_eq!(
                    deserialized.panel_sizes.request_panel_width,
                    snapshot.panel_sizes.request_panel_width,
                    "panel_sizes.request_panel_width debe preservarse exactamente"
                );
                prop_assert_eq!(
                    deserialized,
                    snapshot,
                    "el snapshot completo debe ser igual al original tras el ida y vuelta"
                );
            }
        }
    }

    /// Tests de ejemplo de caracterización del ESQUEMA de sesión del
    /// baseline SIN CAMBIOS (spec `midway-baseline-audit-and-first-vertical`,
    /// Tarea 4.3; Criterios 6.3, 11.5, 13.11).
    ///
    /// Fijan tres hechos observables del esquema persistido que el
    /// incremento de UX asume vigentes y no debe alterar:
    ///
    /// 1. La clave JSON de la altura del panel de respuesta es
    ///    EXACTAMENTE `panelSizes.responsePanelHeight` (el arrastre del
    ///    divisor de respuesta escribe ese campo ya existente; no lo crea
    ///    ni lo renombra).
    /// 2. Un `session.json` que solo trae los campos requeridos -sin
    ///    ninguno de los campos opcionales con `serde(default)`- sigue
    ///    deserializando, con los valores por defecto del baseline
    ///    (Criterio 13.11: los datos existentes del usuario se preservan;
    ///    una sesión escrita por una versión anterior no se descarta como
    ///    corrupta).
    /// 3. Tripwire: [`SESSION_SCHEMA_VERSION`] vale `1`. Si un cambio
    ///    futuro toca el esquema sin declararlo, este test falla y obliga
    ///    a aplicar el Req 11.5 completo (versión de esquema, respaldo
    ///    previo, migración transaccional, validación posterior, rollback
    ///    y tests desde versiones anteriores).
    ///
    /// Son tests de ejemplo (sin Property numerada) y no modifican código
    /// de producción.
    mod session_schema_characterization_tests {
        use super::*;

        /// La altura del panel de respuesta serializa bajo la clave
        /// `responsePanelHeight` dentro del objeto `panelSizes`, con ese
        /// nombre exacto (y no bajo el nombre `snake_case` del campo de
        /// Rust).
        #[test]
        fn panel_sizes_json_key_is_exactly_response_panel_height() {
            let mut snapshot = sample_snapshot();
            snapshot.panel_sizes.response_panel_height = 275.0;

            let json: serde_json::Value = serde_json::to_value(&snapshot)
                .expect("serializar un SessionSnapshot construido no debería fallar");

            let panel_sizes = json
                .get("panelSizes")
                .expect("el snapshot debe serializar la clave `panelSizes`")
                .as_object()
                .expect("`panelSizes` debe serializar como objeto JSON");

            assert!(
                panel_sizes.contains_key("responsePanelHeight"),
                "`panelSizes` debe contener la clave exacta `responsePanelHeight`, claves presentes: {:?}",
                panel_sizes.keys().collect::<Vec<_>>()
            );
            assert!(
                !panel_sizes.contains_key("response_panel_height"),
                "`panelSizes` no debe usar el nombre snake_case del campo de Rust"
            );
            assert_eq!(
                panel_sizes
                    .get("responsePanelHeight")
                    .and_then(serde_json::Value::as_f64),
                Some(275.0),
                "`responsePanelHeight` debe transportar el valor de `response_panel_height`"
            );
        }

        /// Un `session.json` con solo los campos requeridos deserializa:
        /// los campos con `serde(default)` (`panelSizes.requestPanelWidth`,
        /// `themeMode`, `activeCollectionId`) toman su valor por defecto
        /// en lugar de hacer fallar la lectura.
        ///
        /// El `theme_mode` esperado es [`ThemeMode::default()`], que en el
        /// baseline es `ThemeMode::Dark`.
        #[test]
        fn session_json_without_optional_fields_deserializes_with_defaults() {
            let json = r#"{
                "version": 1,
                "activeTabId": null,
                "openTabs": [],
                "closedTabs": [],
                "panelSizes": {
                    "workspacePanelWidth": 280.0,
                    "responsePanelHeight": 320.0
                },
                "savedAt": "2024-01-01T00:00:00Z"
            }"#;

            let snapshot: SessionSnapshot = serde_json::from_str(json)
                .expect("un session.json sin campos opcionales debe seguir deserializando");

            assert_eq!(snapshot.version, SESSION_SCHEMA_VERSION);
            assert_eq!(snapshot.active_tab_id, None);
            assert!(snapshot.open_tabs.is_empty());
            assert!(snapshot.closed_tabs.is_empty());
            assert_eq!(snapshot.panel_sizes.workspace_panel_width, 280.0);
            assert_eq!(snapshot.panel_sizes.response_panel_height, 320.0);
            assert_eq!(
                snapshot.panel_sizes.request_panel_width,
                default_request_panel_width(),
                "`requestPanelWidth` ausente debe tomar su valor por defecto"
            );
            assert_eq!(
                snapshot.theme_mode,
                ThemeMode::default(),
                "`themeMode` ausente debe tomar el tema por defecto del baseline"
            );
            assert_eq!(
                snapshot.active_collection_id, None,
                "`activeCollectionId` ausente debe deserializar como None"
            );
            assert_eq!(snapshot.saved_at, "2024-01-01T00:00:00Z");
        }

        /// Tripwire de esquema: `SESSION_SCHEMA_VERSION` vale `1` en el
        /// baseline. Este spec no modifica la persistencia, así que este
        /// valor no debe cambiar; si un cambio futuro lo requiere, aplica
        /// el Req 11.5 completo antes de tocarlo.
        #[test]
        fn session_schema_version_tripwire_is_one() {
            assert_eq!(
                SESSION_SCHEMA_VERSION, 1,
                "cambiar SESSION_SCHEMA_VERSION exige el Req 11.5 completo: versión de esquema, \
                 respaldo previo, migración transaccional, validación posterior, rollback y tests \
                 desde versiones anteriores"
            );
        }
    }
}
