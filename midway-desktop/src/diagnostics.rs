//! `diagnostics.rs` (Tarea 7.12, Fase 3): persistencia del crash log
//! (`CrashRecord`) en un archivo JSON dentro del data dir de la app.
//!
//! Port de `src/lib/diagnostics.ts`, reemplazando `window.localStorage` por
//! un archivo `diagnostics.json` en el mismo directorio de datos que usa
//! `state::AppState::initialize` (resuelto vía el crate `dirs`, sin
//! depender de ningún runtime de UI). A diferencia de la referencia
//! TypeScript (`MAX_CRASH_RECORDS = 20`), este módulo usa el límite fijado
//! por el Requisito 4.7 / diseño ("Data Models > Formato de diagnostics"):
//! hasta 200 `CrashRecord`.
//!
//! Semántica preservada de la referencia TypeScript:
//! - `read_crash_records`: lee y deserializa el archivo; cualquier fallo de
//!   I/O o de parseo (archivo ausente, JSON inválido, forma inesperada)
//!   produce una lista vacía en lugar de propagar un error (Criterio de
//!   fallo silencioso análogo al `try/catch` de `readCrashRecords`).
//! - `append_crash_record`: antepone (`prepend`) el nuevo registro a los ya
//!   existentes (orden más-reciente-primero), trunca a `MAX_CRASH_RECORDS`
//!   y persiste; los fallos de escritura se ignoran (best-effort, igual que
//!   el `try/catch` de `appendCrashRecord` alrededor de
//!   `localStorage.setItem`), y el registro construido se devuelve en
//!   cualquier caso.
//! - `clear_crash_records`: elimina el archivo (equivalente a
//!   `localStorage.removeItem`); la ausencia previa del archivo no es un
//!   error.
//!
//! Ver diseño: "Components and Interfaces > Workspace_Panel lateral (Fase
//! 3)"; "Data Models > Formato de diagnostics".
//! Ver requisitos: 4.7.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use midway_core::app::errors::{AppError, AppResult};

/// Nombre de la aplicación usado para resolver el subdirectorio de datos,
/// igual que `state::AppState::initialize` (Requisito 4.7 / consistencia
/// con el resto de archivos persistidos por `midway-desktop`).
const APP_NAME: &str = "midway";

/// Nombre del archivo de persistencia del crash log dentro del data dir.
const DIAGNOSTICS_FILE_NAME: &str = "diagnostics.json";

/// Límite máximo de `CrashRecord` persistidos (Requisito 4.7): a diferencia
/// de la referencia TypeScript (`MAX_CRASH_RECORDS = 20` en
/// `src/lib/diagnostics.ts`), el diseño de `midway-desktop` fija este
/// límite en 200.
pub const MAX_CRASH_RECORDS: usize = 200;

/// Origen de un `CrashRecord` (Data Models > Formato de diagnostics):
/// equivalente a los literales `"window.error" | "unhandledrejection" |
/// "react-boundary"` de la referencia TypeScript, adaptados al modelo de
/// error de una app nativa `iced` (sin DOM ni promesas de JS):
/// - `WindowError`: error de ventana/sistema no capturado por ningún otro
///   mecanismo (análogo a `window.error`).
/// - `UnhandledPanic`: panic capturado por el hook de `std::panic::set_hook`
///   (Fase 5, Tarea 11.14), análogo a `unhandledrejection`.
/// - `ComponentBoundary`: panic capturado por el `Error_Boundary` de un
///   componente vía `std::panic::catch_unwind` (Fase 5, Tarea 11.14),
///   análogo a `react-boundary`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CrashSource {
    WindowError,
    UnhandledPanic,
    ComponentBoundary,
}

/// Registro de crash/diagnóstico persistido en `diagnostics.json` (Data
/// Models > Formato de diagnostics).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashRecord {
    pub id: String,
    pub source: CrashSource,
    pub message: String,
    pub stack: Option<String>,
    /// RFC3339.
    pub created_at: String,
}

/// Resuelve la ruta del archivo `diagnostics.json` dentro del data dir de
/// la aplicación, usando el mismo patrón que
/// `state::AppState::initialize` (crate `dirs`, subdirectorio `APP_NAME`).
///
/// No crea el directorio: los llamadores que necesiten escribir (
/// `append_crash_record`) son responsables de crearlo si no existe.
pub fn diagnostics_file_path() -> AppResult<PathBuf> {
    #[cfg(test)]
    if let Some(override_path) = test_diagnostics_path_override() {
        return Ok(override_path);
    }

    let data_dir = dirs::data_dir()
        .ok_or_else(|| AppError::Io("No se pudo resolver el directorio de datos del sistema.".to_string()))?
        .join(APP_NAME);

    Ok(data_dir.join(DIAGNOSTICS_FILE_NAME))
}

/// Seam de testabilidad exclusiva de tests (Tarea 11.14): permite que
/// código bajo `#[cfg(test)]` en OTROS módulos del mismo crate (por
/// ejemplo, los tests de `guarded_update`/`guarded_view` en `app.rs`) fije,
/// por hilo, una ruta alternativa para `diagnostics.json`, de modo que las
/// llamadas a la API pública (`append_crash_record`, `read_crash_records`)
/// hechas indirectamente a través de esos wrappers no toquen el data dir
/// real de la plataforma durante los tests. `None` restaura el
/// comportamiento normal (resolver vía `dirs::data_dir()`).
///
/// Nunca se compila en builds de release (todo el módulo está detrás de
/// `#[cfg(test)]`).
#[cfg(test)]
pub(crate) fn set_test_diagnostics_path_override(path: Option<PathBuf>) {
    TEST_DIAGNOSTICS_PATH_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = path;
    });
}

#[cfg(test)]
fn test_diagnostics_path_override() -> Option<PathBuf> {
    TEST_DIAGNOSTICS_PATH_OVERRIDE.with(|cell| cell.borrow().clone())
}

#[cfg(test)]
thread_local! {
    static TEST_DIAGNOSTICS_PATH_OVERRIDE: std::cell::RefCell<Option<PathBuf>> = std::cell::RefCell::new(None);
}

/// Lee y deserializa los `CrashRecord` persistidos en `diagnostics.json`.
///
/// Devuelve una lista vacía ante cualquier fallo (archivo ausente, error de
/// I/O, JSON inválido o de forma inesperada), replicando el comportamiento
/// de fallo silencioso de `readCrashRecords` (referencia TypeScript). El
/// orden devuelto es el mismo en que están persistidos (más-reciente-
/// primero, garantizado por `append_crash_record`).
pub fn read_crash_records() -> Vec<CrashRecord> {
    let Ok(path) = diagnostics_file_path() else {
        return Vec::new();
    };

    read_crash_records_at(&path)
}

/// Variante de [`read_crash_records`] parametrizada por `path`, usada tanto
/// por la API pública (con la ruta resuelta vía [`diagnostics_file_path`])
/// como por los tests unitarios (con una ruta dentro de un directorio
/// temporal), evitando así depender del data dir real de la plataforma en
/// los tests. Visibilidad `pub(crate)` (en vez de privada): los tests de
/// `app.rs` (Tarea 11.14) también necesitan verificar el efecto de
/// `guarded_update`/`guarded_view` sobre el crash log sin tocar el data dir
/// real de la plataforma.
pub(crate) fn read_crash_records_at(path: &PathBuf) -> Vec<CrashRecord> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    serde_json::from_str::<Vec<CrashRecord>>(&raw).unwrap_or_default()
}

/// Construye un nuevo `CrashRecord` (id fresco vía `uuid::Uuid::new_v4`,
/// `created_at` en RFC3339) y lo persiste anteponiéndolo (`prepend`) a los
/// registros existentes, truncando el resultado a `MAX_CRASH_RECORDS`.
///
/// La escritura es best-effort: cualquier fallo de I/O al crear el
/// directorio o escribir el archivo se ignora silenciosamente (mismo
/// comportamiento que el `try/catch` alrededor de `localStorage.setItem`
/// en `appendCrashRecord`), y el registro construido se devuelve en
/// cualquier caso, incluso si la persistencia falló.
pub fn append_crash_record(source: CrashSource, message: String, stack: Option<String>) -> CrashRecord {
    let Ok(path) = diagnostics_file_path() else {
        // Sin data dir resoluble no hay dónde persistir; se devuelve el
        // registro construido en memoria de todos modos (best-effort, ver
        // doc del módulo).
        return CrashRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            message,
            stack,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
    };

    append_crash_record_at(&path, source, message, stack)
}

/// Variante de [`append_crash_record`] parametrizada por `path`, usada
/// tanto por la API pública como por los tests unitarios (con una ruta
/// dentro de un directorio temporal). Visibilidad `pub(crate)`: ver nota de
/// [`read_crash_records_at`].
pub(crate) fn append_crash_record_at(path: &PathBuf, source: CrashSource, message: String, stack: Option<String>) -> CrashRecord {
    let record = CrashRecord {
        id: uuid::Uuid::new_v4().to_string(),
        source,
        message,
        stack,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut next = vec![record.clone()];
    next.extend(read_crash_records_at(path));
    next.truncate(MAX_CRASH_RECORDS);

    let _ = write_crash_records_at(path, &next);

    record
}

/// Elimina el archivo `diagnostics.json`, equivalente a
/// `localStorage.removeItem` en la referencia TypeScript. La ausencia
/// previa del archivo no se considera un error.
///
/// API pública del módulo de diagnostics, pensada para la acción "limpiar
/// diagnósticos" del `Workspace_Panel`; el botón que la dispara todavía no
/// está cableado en la UI, de ahí `#[allow(dead_code)]`.
#[allow(dead_code)]
pub fn clear_crash_records() -> AppResult<()> {
    let path = diagnostics_file_path()?;

    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Io(error.to_string())),
    }
}

/// Serializa y escribe `records` en `diagnostics.json` (en `path`), creando
/// el directorio contenedor si no existe. Usada por `append_crash_record_at`
/// y por los tests unitarios (con una ruta dentro de un directorio temporal).
fn write_crash_records_at(path: &PathBuf, records: &[CrashRecord]) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| AppError::Io(error.to_string()))?;
    }

    let serialized = serde_json::to_string(records).map_err(|error| AppError::Serialization(error.to_string()))?;

    std::fs::write(path, serialized).map_err(|error| AppError::Io(error.to_string()))
}

#[cfg(test)]
mod tests {
    //! Tests unitarios de `diagnostics.rs` (Tarea 7.14, Fase 3).
    //!
    //! Usan `append_crash_record_at` / `read_crash_records_at` con una ruta
    //! dentro de un directorio temporal (`tempfile::tempdir`) para evitar
    //! tocar el data dir real de la plataforma durante los tests.
    //!
    //! Ver requisitos: 10.4.

    use super::*;
    use proptest::prelude::*;

    fn diagnostics_path_in(dir: &std::path::Path) -> PathBuf {
        dir.join(DIAGNOSTICS_FILE_NAME)
    }

    /// Camino nominal: anteponer un `CrashRecord` típico lo persiste en el
    /// archivo temporal y puede leerse de vuelta, con el más reciente
    /// primero.
    #[test]
    fn append_crash_record_persists_and_reads_back_most_recent_first() {
        let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        let path = diagnostics_path_in(temp_dir.path());

        let first = append_crash_record_at(
            &path,
            CrashSource::WindowError,
            "primer error".to_string(),
            None,
        );
        let second = append_crash_record_at(
            &path,
            CrashSource::UnhandledPanic,
            "segundo error".to_string(),
            Some("stack trace".to_string()),
        );

        let persisted = read_crash_records_at(&path);

        assert_eq!(persisted.len(), 2);
        // Más-reciente-primero: `second` fue anteponer después de `first`.
        assert_eq!(persisted[0].id, second.id);
        assert_eq!(persisted[0].message, "segundo error");
        assert_eq!(persisted[0].source, CrashSource::UnhandledPanic);
        assert_eq!(persisted[0].stack, Some("stack trace".to_string()));
        assert_eq!(persisted[1].id, first.id);
        assert_eq!(persisted[1].message, "primer error");
        assert_eq!(persisted[1].source, CrashSource::WindowError);
        assert_eq!(persisted[1].stack, None);
    }

    /// Caso borde: al superar el límite de `MAX_CRASH_RECORDS` (200)
    /// entradas, el registro más antiguo se descarta y el más nuevo se
    /// conserva, manteniendo la lista acotada a 200.
    #[test]
    fn append_crash_record_evicts_oldest_at_max_capacity() {
        let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        let path = diagnostics_path_in(temp_dir.path());

        // Llena hasta el límite exacto (200 registros).
        let mut oldest = None;
        for i in 0..MAX_CRASH_RECORDS {
            let record = append_crash_record_at(
                &path,
                CrashSource::ComponentBoundary,
                format!("error {i}"),
                None,
            );
            if i == 0 {
                oldest = Some(record.id);
            }
        }

        let oldest = oldest.unwrap();

        let at_capacity = read_crash_records_at(&path);
        assert_eq!(at_capacity.len(), MAX_CRASH_RECORDS);
        assert_eq!(at_capacity[0].message, format!("error {}", MAX_CRASH_RECORDS - 1));
        assert_eq!(at_capacity[at_capacity.len() - 1].id, oldest);

        // Un registro adicional debe desalojar el más antiguo y seguir
        // acotado a MAX_CRASH_RECORDS.
        let newest = append_crash_record_at(
            &path,
            CrashSource::ComponentBoundary,
            "error mas nuevo".to_string(),
            None,
        );

        let after_overflow = read_crash_records_at(&path);
        assert_eq!(after_overflow.len(), MAX_CRASH_RECORDS);
        assert_eq!(after_overflow[0].id, newest.id);
        assert_eq!(after_overflow[0].message, "error mas nuevo");
        assert!(
            after_overflow.iter().all(|record| record.id != oldest),
            "el registro mas antiguo debe haber sido desalojado"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 50, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 14: Colecciones
        /// acotadas ordenadas de la más reciente a la más antigua
        /// (mitad Diagnostics).
        /// Validates: Requirements 4.7
        ///
        /// Para cualquier secuencia de N inserciones (`append_crash_record_at`)
        /// sobre un archivo `diagnostics.json` en un directorio temporal, el
        /// tamaño final de la colección persistida SHALL ser
        /// `min(N, MAX_CRASH_RECORDS)`, SHALL contener siempre las
        /// `MAX_CRASH_RECORDS` entradas más recientemente insertadas cuando
        /// N > `MAX_CRASH_RECORDS`, y SHALL estar ordenada de la más
        /// reciente a la más antigua.
        ///
        /// El oráculo se calcula de forma independiente de la
        /// implementación: se registra en memoria (fuera de
        /// `append_crash_record_at`) el orden real de inserción (mediante
        /// un índice secuencial embebido en el mensaje de cada registro) y
        /// se compara contra el sufijo esperado de las últimas
        /// `MAX_CRASH_RECORDS` inserciones, en orden inverso (más reciente
        /// primero).
        #[test]
        fn property_14_diagnostics_bounded_and_ordered_by_recency(
            insertion_count in 0usize..=250usize,
        ) {
            let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
            let path = diagnostics_path_in(temp_dir.path());

            // Oráculo independiente: mensajes con un índice secuencial
            // embebido, en el mismo orden en que se insertan.
            let mut inserted_messages: Vec<String> = Vec::with_capacity(insertion_count);

            for index in 0..insertion_count {
                let message = format!("crash-{index}");
                append_crash_record_at(&path, CrashSource::WindowError, message.clone(), None);
                inserted_messages.push(message);
            }

            let persisted = read_crash_records_at(&path);

            let expected_len = insertion_count.min(MAX_CRASH_RECORDS);
            prop_assert_eq!(persisted.len(), expected_len);

            // Las últimas `expected_len` inserciones, en orden inverso
            // (más reciente primero), calculadas de forma independiente de
            // la implementación bajo prueba.
            let mut expected_messages_most_recent_first: Vec<String> = inserted_messages
                [inserted_messages.len() - expected_len..]
                .to_vec();
            expected_messages_most_recent_first.reverse();

            let actual_messages: Vec<String> = persisted
                .iter()
                .map(|record| record.message.clone())
                .collect();

            prop_assert_eq!(actual_messages, expected_messages_most_recent_first);
        }
    }
}
