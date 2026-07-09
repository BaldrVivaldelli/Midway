//! `AppState`: estado compartido de infraestructura de `midway-desktop`.
//!
//! Análogo al `AppState` de `src-tauri/src/state.rs`, pero sin depender de
//! `tauri::AppHandle` para resolver el directorio de datos de la aplicación:
//! se usa el crate `dirs` (independiente de cualquier runtime de UI) para
//! obtener el directorio de datos estándar de la plataforma.

use midway_core::{
    app::errors::{AppError, AppResult},
    infra::sqlite_repository::SqliteRepository,
    runtime::{request_executor::RequestExecutorHandle, secret_executor::SecretExecutorHandle},
};

/// Nombre de la aplicación usado tanto para el subdirectorio de datos como
/// para el servicio de keyring del sistema (equivalente al `identifier` de
/// `tauri.conf.json` en la variante Tauri).
const APP_NAME: &str = "midway";

pub struct AppState {
    pub repository: SqliteRepository,
    pub request_executor: RequestExecutorHandle,
    pub secret_executor: SecretExecutorHandle,
}

impl AppState {
    pub async fn initialize() -> AppResult<Self> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| AppError::Io("No se pudo resolver el directorio de datos del sistema.".to_string()))?
            .join(APP_NAME);

        std::fs::create_dir_all(&data_dir).map_err(|error| AppError::Io(error.to_string()))?;

        let db_path = data_dir.join("workspace.sqlite3");
        let repository = SqliteRepository::open(&db_path).await?;

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .cookie_store(true)
            .build()
            .map_err(|error| AppError::Http(error.to_string()))?;

        Ok(Self {
            repository,
            request_executor: RequestExecutorHandle::spawn(client),
            secret_executor: SecretExecutorHandle::spawn(APP_NAME.to_string()),
        })
    }
}
