use std::path::Path;

use chrono::Utc;
use tokio_rusqlite::{
    params,
    rusqlite::{self, OptionalExtension, Row},
    Connection,
};
use uuid::Uuid;

use crate::{
    app::errors::{AppError, AppResult},
    domain::{
        http::{HttpMethod, RequestDraft, ResponseEnvelope},
        workspace::{
            CollectionSummary, CollectionWithRequests, EnvironmentRecord, HistoryEntry,
            SaveEnvironmentInput, SaveRequestInput, SavedRequestRecord, SecretMetadata,
            WorkspaceSnapshot,
        },
    },
};

#[derive(Clone)]
pub struct SqliteRepository {
    connection: Connection,
}

impl SqliteRepository {
    pub async fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let connection = Connection::open(path)
            .await
            .map_err(|error| AppError::Database(error.to_string()))?;

        let repository = Self { connection };
        repository.migrate().await?;
        Ok(repository)
    }

    pub async fn migrate(&self) -> AppResult<()> {
        self.connection
            .call(|conn| {
                conn.execute_batch(
                    r#"
                    PRAGMA foreign_keys = ON;
                    PRAGMA journal_mode = WAL;

                    CREATE TABLE IF NOT EXISTS collections (
                        id TEXT PRIMARY KEY,
                        name TEXT NOT NULL,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS requests (
                        id TEXT PRIMARY KEY,
                        collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
                        name TEXT NOT NULL,
                        draft_json TEXT NOT NULL,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS environments (
                        id TEXT PRIMARY KEY,
                        name TEXT NOT NULL,
                        variables_json TEXT NOT NULL,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS history (
                        id TEXT PRIMARY KEY,
                        request_name TEXT NOT NULL,
                        method TEXT NOT NULL,
                        url TEXT NOT NULL,
                        environment_name TEXT,
                        response_status INTEGER,
                        duration_ms INTEGER,
                        error_message TEXT,
                        created_at TEXT NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS secrets (
                        alias TEXT PRIMARY KEY,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );
                    "#,
                )?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn workspace_snapshot(&self, history_limit: usize) -> AppResult<WorkspaceSnapshot> {
        let history_limit = history_limit as i64;

        self.connection
            .call(move |conn| {
                let collections = load_collections_with_requests(conn)?;
                let environments = load_environments(conn)?;
                let secrets = load_secrets(conn)?;
                let history = load_history(conn, history_limit)?;

                Ok(WorkspaceSnapshot {
                    collections,
                    environments,
                    history,
                    secrets,
                })
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn export_full_snapshot(&self) -> AppResult<WorkspaceSnapshot> {
        self.connection
            .call(move |conn| {
                let collections = load_collections_with_requests(conn)?;
                let environments = load_environments(conn)?;
                let secrets = load_secrets(conn)?;
                let history = load_history(conn, i64::MAX)?;

                Ok(WorkspaceSnapshot {
                    collections,
                    environments,
                    history,
                    secrets,
                })
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn clear_workspace(&self) -> AppResult<()> {
        self.connection
            .call(|conn| {
                conn.execute_batch(
                    r#"
                    DELETE FROM history;
                    DELETE FROM requests;
                    DELETE FROM collections;
                    DELETE FROM environments;
                    DELETE FROM secrets;
                    "#,
                )?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn import_workspace_snapshot(
        &self,
        snapshot: WorkspaceSnapshot,
        merge: bool,
    ) -> AppResult<()> {
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;

                if !merge {
                    tx.execute_batch(
                        r#"
                        DELETE FROM history;
                        DELETE FROM requests;
                        DELETE FROM collections;
                        DELETE FROM environments;
                        DELETE FROM secrets;
                        "#,
                    )?;
                }

                for environment in snapshot.environments {
                    let variables_json = serde_json::to_string(&environment.variables)
                        .map_err(|error| json_error(2, error))?;
                    tx.execute(
                        "INSERT INTO environments (id, name, variables_json, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)
                         ON CONFLICT(id) DO UPDATE SET
                           name = excluded.name,
                           variables_json = excluded.variables_json,
                           updated_at = excluded.updated_at",
                        params![
                            &environment.id,
                            &environment.name,
                            &variables_json,
                            &environment.created_at,
                            &environment.updated_at
                        ],
                    )?;
                }

                for collection in snapshot.collections {
                    tx.execute(
                        "INSERT INTO collections (id, name, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(id) DO UPDATE SET
                           name = excluded.name,
                           updated_at = excluded.updated_at",
                        params![
                            &collection.collection.id,
                            &collection.collection.name,
                            &collection.collection.created_at,
                            &collection.collection.updated_at
                        ],
                    )?;

                    for request in collection.requests {
                        let draft_json = serde_json::to_string(&request.draft)
                            .map_err(|error| json_error(3, error))?;
                        tx.execute(
                            "INSERT INTO requests (id, collection_id, name, draft_json, created_at, updated_at)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                             ON CONFLICT(id) DO UPDATE SET
                               collection_id = excluded.collection_id,
                               name = excluded.name,
                               draft_json = excluded.draft_json,
                               updated_at = excluded.updated_at",
                            params![
                                &request.id,
                                &collection.collection.id,
                                &request.name,
                                &draft_json,
                                &request.created_at,
                                &request.updated_at
                            ],
                        )?;
                    }
                }

                for secret in snapshot.secrets {
                    tx.execute(
                        "INSERT INTO secrets (alias, created_at, updated_at)
                         VALUES (?1, ?2, ?3)
                         ON CONFLICT(alias) DO UPDATE SET updated_at = excluded.updated_at",
                        params![&secret.alias, &secret.created_at, &secret.updated_at],
                    )?;
                }

                for history in snapshot.history {
                    let response_status = history.response_status.map(|value| value as i64);
                    let duration_ms = history.duration_ms.map(|value| value as i64);
                    tx.execute(
                        "INSERT INTO history
                         (id, request_name, method, url, environment_name, response_status, duration_ms, error_message, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                         ON CONFLICT(id) DO UPDATE SET
                           request_name = excluded.request_name,
                           method = excluded.method,
                           url = excluded.url,
                           environment_name = excluded.environment_name,
                           response_status = excluded.response_status,
                           duration_ms = excluded.duration_ms,
                           error_message = excluded.error_message,
                           created_at = excluded.created_at",
                        params![
                            &history.id,
                            &history.request_name,
                            &method_to_string(&history.method),
                            &history.url,
                            &history.environment_name,
                            &response_status,
                            &duration_ms,
                            &history.error_message,
                            &history.created_at
                        ],
                    )?;
                }

                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn create_collection(&self, name: String) -> AppResult<CollectionSummary> {
        let trimmed_name = name.trim().to_string();

        if trimmed_name.is_empty() {
            return Err(AppError::Validation(
                "La collection necesita un nombre.".to_string(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        let now = now_rfc3339();

        self.connection
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO collections (id, name, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![&id, &trimmed_name, &now, &now],
                )?;

                Ok(CollectionSummary {
                    id,
                    name: trimmed_name,
                    request_count: 0,
                    created_at: now.clone(),
                    updated_at: now,
                })
            })
            .await
            .map_err(map_db_err)
    }



    pub async fn get_collection_with_requests(
        &self,
        collection_id: &str,
    ) -> AppResult<Option<CollectionWithRequests>> {
        let collection_id = collection_id.to_string();

        self.connection
            .call(move |conn| {
                let collection: Option<CollectionSummary> = conn
                    .query_row(
                        "SELECT id, name, created_at, updated_at FROM collections WHERE id = ?1",
                        params![&collection_id],
                        |row| {
                            Ok(CollectionSummary {
                                id: row.get(0)?,
                                name: row.get(1)?,
                                request_count: 0,
                                created_at: row.get(2)?,
                                updated_at: row.get(3)?,
                            })
                        },
                    )
                    .optional()?;

                let Some(collection) = collection else {
                    return Ok(None);
                };

                let mut request_stmt = conn.prepare(
                    "SELECT id, collection_id, name, draft_json, created_at, updated_at
                     FROM requests
                     WHERE collection_id = ?1
                     ORDER BY updated_at DESC, name ASC",
                )?;

                let requests = request_stmt
                    .query_map(params![&collection.id], parse_saved_request_record_row)?
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Some(CollectionWithRequests {
                    collection: CollectionSummary {
                        request_count: requests.len() as u64,
                        ..collection
                    },
                    requests,
                }))
            })
            .await
            .map_err(map_db_err)
    }
    pub async fn save_request(&self, input: SaveRequestInput) -> AppResult<SavedRequestRecord> {
        let request_id = input
            .request_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let name = input.draft.name.trim().to_string();

        if name.is_empty() {
            return Err(AppError::Validation(
                "El request necesita un nombre antes de guardarse.".to_string(),
            ));
        }

        let collection_id = input.collection_id;
        let mut draft = input.draft;
        draft.id = Some(request_id.clone());
        let draft_json = serde_json::to_string(&draft)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        let created_at_seed = now_rfc3339();
        let updated_at = now_rfc3339();

        self.connection
            .call(move |conn| {
                let existing_created_at: Option<String> = conn
                    .query_row(
                        "SELECT created_at FROM requests WHERE id = ?1",
                        params![&request_id],
                        |row| row.get(0),
                    )
                    .optional()?;

                let created_at = existing_created_at.unwrap_or_else(|| created_at_seed.clone());

                conn.execute(
                    "INSERT INTO requests (id, collection_id, name, draft_json, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(id) DO UPDATE SET
                         collection_id = excluded.collection_id,
                         name = excluded.name,
                         draft_json = excluded.draft_json,
                         updated_at = excluded.updated_at",
                    params![
                        &request_id,
                        &collection_id,
                        &name,
                        &draft_json,
                        &created_at,
                        &updated_at
                    ],
                )?;

                conn.execute(
                    "UPDATE collections SET updated_at = ?2 WHERE id = ?1",
                    params![&collection_id, &updated_at],
                )?;

                Ok(SavedRequestRecord {
                    id: request_id,
                    collection_id,
                    name,
                    draft,
                    created_at,
                    updated_at,
                })
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn save_environment(
        &self,
        input: SaveEnvironmentInput,
    ) -> AppResult<EnvironmentRecord> {
        let environment_id = input
            .environment_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let name = input.name.trim().to_string();

        if name.is_empty() {
            return Err(AppError::Validation(
                "El environment necesita un nombre.".to_string(),
            ));
        }

        let variables = input.variables;
        let variables_json = serde_json::to_string(&variables)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        let created_at_seed = now_rfc3339();
        let updated_at = now_rfc3339();

        self.connection
            .call(move |conn| {
                let existing_created_at: Option<String> = conn
                    .query_row(
                        "SELECT created_at FROM environments WHERE id = ?1",
                        params![&environment_id],
                        |row| row.get(0),
                    )
                    .optional()?;

                let created_at = existing_created_at.unwrap_or_else(|| created_at_seed.clone());

                conn.execute(
                    "INSERT INTO environments (id, name, variables_json, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(id) DO UPDATE SET
                         name = excluded.name,
                         variables_json = excluded.variables_json,
                         updated_at = excluded.updated_at",
                    params![
                        &environment_id,
                        &name,
                        &variables_json,
                        &created_at,
                        &updated_at
                    ],
                )?;

                Ok(EnvironmentRecord {
                    id: environment_id,
                    name,
                    variables,
                    created_at,
                    updated_at,
                })
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn delete_environment(&self, environment_id: String) -> AppResult<()> {
        self.connection
            .call(move |conn| {
                conn.execute(
                    "DELETE FROM environments WHERE id = ?1",
                    params![&environment_id],
                )?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn get_environment_by_id(
        &self,
        environment_id: &str,
    ) -> AppResult<Option<EnvironmentRecord>> {
        let environment_id = environment_id.to_string();

        self.connection
            .call(move |conn| {
                conn.query_row(
                    "SELECT id, name, variables_json, created_at, updated_at
                     FROM environments WHERE id = ?1",
                    params![&environment_id],
                    parse_environment_record_row,
                )
                .optional()
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn append_history(
        &self,
        draft: &RequestDraft,
        environment_name: Option<String>,
        url: String,
        response: Option<ResponseEnvelope>,
        error_message: Option<String>,
    ) -> AppResult<()> {
        let id = Uuid::new_v4().to_string();
        let name = draft.name.clone();
        let method = method_to_string(&draft.method);
        let response_status = response.as_ref().map(|response| response.status as i64);
        let duration_ms = response.as_ref().map(|response| response.duration_ms as i64);
        let created_at = now_rfc3339();

        self.connection
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO history
                     (id, request_name, method, url, environment_name, response_status, duration_ms, error_message, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        &id,
                        &name,
                        &method,
                        &url,
                        &environment_name,
                        &response_status,
                        &duration_ms,
                        &error_message,
                        &created_at
                    ],
                )?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn upsert_secret_metadata(&self, alias: String) -> AppResult<SecretMetadata> {
        let trimmed_alias = alias.trim().to_string();

        if trimmed_alias.is_empty() {
            return Err(AppError::Validation(
                "El alias del secret no puede estar vacío.".to_string(),
            ));
        }

        let created_at_seed = now_rfc3339();
        let updated_at = now_rfc3339();

        self.connection
            .call(move |conn| {
                let existing_created_at: Option<String> = conn
                    .query_row(
                        "SELECT created_at FROM secrets WHERE alias = ?1",
                        params![&trimmed_alias],
                        |row| row.get(0),
                    )
                    .optional()?;

                let created_at = existing_created_at.unwrap_or_else(|| created_at_seed.clone());

                conn.execute(
                    "INSERT INTO secrets (alias, created_at, updated_at)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(alias) DO UPDATE SET updated_at = excluded.updated_at",
                    params![&trimmed_alias, &created_at, &updated_at],
                )?;

                Ok(SecretMetadata {
                    alias: trimmed_alias,
                    created_at,
                    updated_at,
                })
            })
            .await
            .map_err(map_db_err)
    }

    pub async fn delete_secret_metadata(&self, alias: String) -> AppResult<()> {
        self.connection
            .call(move |conn| {
                conn.execute("DELETE FROM secrets WHERE alias = ?1", params![&alias])?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }
}

fn load_collections_with_requests(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<CollectionWithRequests>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, updated_at
         FROM collections
         ORDER BY updated_at DESC, name ASC",
    )?;

    let collections = stmt
        .query_map([], |row| {
            Ok(CollectionSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                request_count: 0,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut out = Vec::new();

    for collection in collections {
        let mut request_stmt = conn.prepare(
            "SELECT id, collection_id, name, draft_json, created_at, updated_at
             FROM requests
             WHERE collection_id = ?1
             ORDER BY updated_at DESC, name ASC",
        )?;

        let requests = request_stmt
            .query_map(params![&collection.id], parse_saved_request_record_row)?
            .collect::<Result<Vec<_>, _>>()?;

        out.push(CollectionWithRequests {
            collection: CollectionSummary {
                request_count: requests.len() as u64,
                ..collection
            },
            requests,
        });
    }

    Ok(out)
}

fn load_environments(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<EnvironmentRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, variables_json, created_at, updated_at
         FROM environments
         ORDER BY name ASC",
    )?;

    let rows = stmt.query_map([], parse_environment_record_row)?;
    let items = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

fn load_history(
    conn: &rusqlite::Connection,
    history_limit: i64,
) -> rusqlite::Result<Vec<HistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, request_name, method, url, environment_name, response_status, duration_ms, error_message, created_at
         FROM history
         ORDER BY created_at DESC
         LIMIT ?1",
    )?;

    let rows = stmt.query_map(params![history_limit], parse_history_entry_row)?;
    let items = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

fn load_secrets(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<SecretMetadata>> {
    let mut stmt = conn.prepare(
        "SELECT alias, created_at, updated_at
         FROM secrets
         ORDER BY alias ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(SecretMetadata {
            alias: row.get(0)?,
            created_at: row.get(1)?,
            updated_at: row.get(2)?,
        })
    })?;

    let items = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

fn parse_saved_request_record_row(row: &Row<'_>) -> rusqlite::Result<SavedRequestRecord> {
    let draft_json: String = row.get(3)?;
    let draft = serde_json::from_str::<RequestDraft>(&draft_json)
        .map_err(|error| json_error(3, error))?;

    Ok(SavedRequestRecord {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        name: row.get(2)?,
        draft,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn parse_environment_record_row(row: &Row<'_>) -> rusqlite::Result<EnvironmentRecord> {
    let variables_json: String = row.get(2)?;
    let variables = serde_json::from_str(&variables_json)
        .map_err(|error| json_error(2, error))?;

    Ok(EnvironmentRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        variables,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn parse_history_entry_row(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let method_text: String = row.get(2)?;
    let method = method_from_string(&method_text)
        .ok_or_else(|| json_error(2, format!("Método HTTP inválido: {method_text}")))?;

    Ok(HistoryEntry {
        id: row.get(0)?,
        request_name: row.get(1)?,
        method,
        url: row.get(3)?,
        environment_name: row.get(4)?,
        response_status: row.get::<_, Option<i64>>(5)?.map(|value| value as u16),
        duration_ms: row.get::<_, Option<i64>>(6)?.map(|value| value as u64),
        error_message: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn json_error(index: usize, error: impl ToString) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

fn map_db_err(error: tokio_rusqlite::Error) -> AppError {
    AppError::Database(error.to_string())
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn method_to_string(method: &HttpMethod) -> &'static str {
    match method {
        HttpMethod::GET => "GET",
        HttpMethod::POST => "POST",
        HttpMethod::PUT => "PUT",
        HttpMethod::PATCH => "PATCH",
        HttpMethod::DELETE => "DELETE",
        HttpMethod::HEAD => "HEAD",
        HttpMethod::OPTIONS => "OPTIONS",
    }
}

fn method_from_string(value: &str) -> Option<HttpMethod> {
    match value {
        "GET" => Some(HttpMethod::GET),
        "POST" => Some(HttpMethod::POST),
        "PUT" => Some(HttpMethod::PUT),
        "PATCH" => Some(HttpMethod::PATCH),
        "DELETE" => Some(HttpMethod::DELETE),
        "HEAD" => Some(HttpMethod::HEAD),
        "OPTIONS" => Some(HttpMethod::OPTIONS),
        _ => None,
    }
}
