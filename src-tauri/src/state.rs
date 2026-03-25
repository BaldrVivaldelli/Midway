use tauri::{AppHandle, Manager};

use crate::{
    app::errors::{AppError, AppResult},
    infra::sqlite_repository::SqliteRepository,
    runtime::{request_executor::RequestExecutorHandle, secret_executor::SecretExecutorHandle},
};

pub struct AppState {
    pub repository: SqliteRepository,
    pub request_executor: RequestExecutorHandle,
    pub secret_executor: SecretExecutorHandle,
}

impl AppState {
    pub async fn initialize(app: &AppHandle) -> AppResult<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Io(error.to_string()))?;

        std::fs::create_dir_all(&data_dir).map_err(|error| AppError::Io(error.to_string()))?;

        let db_path = data_dir.join("workspace.sqlite3");
        let repository = SqliteRepository::open(&db_path).await?;

        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .cookie_store(true)
            .build()
            .map_err(|error| AppError::Http(error.to_string()))?;

        let secret_service_name = if app.config().identifier.trim().is_empty() {
            "midway".to_string()
        } else {
            app.config().identifier.clone()
        };

        Ok(Self {
            repository,
            request_executor: RequestExecutorHandle::spawn(client),
            secret_executor: SecretExecutorHandle::spawn(secret_service_name),
        })
    }
}
