use tokio::sync::{mpsc, oneshot};

use crate::{
    app::errors::{AppError, AppResult},
    infra::secret_store::SecretStore,
};

#[derive(Clone)]
pub struct SecretExecutorHandle {
    sender: mpsc::Sender<SecretJob>,
}

enum SecretJob {
    Get {
        alias: String,
        respond_to: oneshot::Sender<AppResult<Option<String>>>,
    },
    Set {
        alias: String,
        value: String,
        respond_to: oneshot::Sender<AppResult<()>>,
    },
    Delete {
        alias: String,
        respond_to: oneshot::Sender<AppResult<()>>,
    },
}

impl SecretExecutorHandle {
    pub fn spawn(service_name: String) -> Self {
        let (sender, mut receiver) = mpsc::channel::<SecretJob>(32);
        let store = SecretStore::new(service_name);

        tauri::async_runtime::spawn(async move {
            while let Some(job) = receiver.recv().await {
                match job {
                    SecretJob::Get { alias, respond_to } => {
                        let store = store.clone();
                        let result = tokio::task::spawn_blocking(move || store.get(&alias))
                            .await
                            .map_err(|error| AppError::Runtime(error.to_string()))
                            .and_then(|result| result);
                        let _ = respond_to.send(result);
                    }
                    SecretJob::Set {
                        alias,
                        value,
                        respond_to,
                    } => {
                        let store = store.clone();
                        let result = tokio::task::spawn_blocking(move || store.set(&alias, &value))
                            .await
                            .map_err(|error| AppError::Runtime(error.to_string()))
                            .and_then(|result| result);
                        let _ = respond_to.send(result);
                    }
                    SecretJob::Delete { alias, respond_to } => {
                        let store = store.clone();
                        let result = tokio::task::spawn_blocking(move || store.delete(&alias))
                            .await
                            .map_err(|error| AppError::Runtime(error.to_string()))
                            .and_then(|result| result);
                        let _ = respond_to.send(result);
                    }
                }
            }
        });

        Self { sender }
    }

    pub async fn get(&self, alias: String) -> AppResult<Option<String>> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(SecretJob::Get { alias, respond_to })
            .await
            .map_err(|_| AppError::Runtime("El executor de secrets se cerró.".to_string()))?;

        response
            .await
            .map_err(|_| AppError::Runtime("No llegó respuesta del executor de secrets.".to_string()))?
    }

    pub async fn set(&self, alias: String, value: String) -> AppResult<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(SecretJob::Set {
                alias,
                value,
                respond_to,
            })
            .await
            .map_err(|_| AppError::Runtime("El executor de secrets se cerró.".to_string()))?;

        response
            .await
            .map_err(|_| AppError::Runtime("No llegó respuesta del executor de secrets.".to_string()))?
    }

    pub async fn delete(&self, alias: String) -> AppResult<()> {
        let (respond_to, response) = oneshot::channel();
        self.sender
            .send(SecretJob::Delete { alias, respond_to })
            .await
            .map_err(|_| AppError::Runtime("El executor de secrets se cerró.".to_string()))?;

        response
            .await
            .map_err(|_| AppError::Runtime("No llegó respuesta del executor de secrets.".to_string()))?
    }
}
