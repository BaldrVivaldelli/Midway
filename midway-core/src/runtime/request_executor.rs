use std::{collections::HashMap, sync::Arc};

use tokio::sync::{oneshot, Mutex};

use crate::{
    app::errors::{AppError, AppResult},
    domain::http::{ResolvedRequest, ResponseEnvelope},
    infra::http_reqwest,
};

#[derive(Clone)]
pub struct RequestExecutorHandle {
    client: reqwest::Client,
    inflight: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
}

impl RequestExecutorHandle {
    pub fn spawn(client: reqwest::Client) -> Self {
        Self {
            client,
            inflight: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn execute(
        &self,
        execution_id: String,
        request: ResolvedRequest,
    ) -> AppResult<ResponseEnvelope> {
        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        self.inflight
            .lock()
            .await
            .insert(execution_id.clone(), cancel_tx);

        let result = tokio::select! {
            _ = &mut cancel_rx => Err(AppError::Runtime("Request cancelado por el usuario.".to_string())),
            result = http_reqwest::execute_request(&self.client, request) => result,
        };

        self.inflight.lock().await.remove(&execution_id);
        result
    }

    pub async fn cancel(&self, execution_id: String) -> bool {
        let sender = self.inflight.lock().await.remove(&execution_id);
        if let Some(sender) = sender {
            let _ = sender.send(());
            true
        } else {
            false
        }
    }
}
