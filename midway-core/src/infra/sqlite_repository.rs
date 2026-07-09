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

#[cfg(test)]
mod tests {
    //! Property test de la Tarea 7.13 (Fase 3, Property 14, Requisito
    //! 4.6): mitad History de "Colecciones acotadas ordenadas de la más
    //! reciente a la más antigua".
    //!
    //! `load_history` (usado por `workspace_snapshot`) es la capa que
    //! efectivamente acota y ordena la sección History: el `SELECT ...
    //! ORDER BY created_at DESC LIMIT ?1` es quien garantiza tanto el
    //! límite como el orden de más-reciente-a-más-antigua. La constante
    //! `HISTORY_LIMIT = 500` en sí misma vive en `midway-desktop`
    //! (Tarea 7.11) y solo se pasa como parámetro `history_limit` a
    //! `workspace_snapshot`; por eso esta property se ejercita aquí, al
    //! nivel de `SqliteRepository`, parametrizando el límite con el mismo
    //! valor (500) que usa la sección History real.
    //!
    //! Para controlar el orden de inserción de forma determinística (sin
    //! depender de la resolución del reloj del sistema al insertar N
    //! entradas muy rápido), este test inserta las `HistoryEntry`
    //! directamente vía `import_workspace_snapshot` (API pública existente
    //! que preserva el `created_at` provisto) en lugar de `append_history`
    //! (que genera `created_at` internamente con la hora actual).

    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::TestCaseError;

    async fn open_temp_repository() -> (SqliteRepository, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().expect("no se pudo crear el directorio temporal");
        let db_path = temp_dir.path().join("workspace.sqlite3");
        let repository = SqliteRepository::open(&db_path)
            .await
            .expect("no se pudo abrir la base de datos temporal");
        (repository, temp_dir)
    }

    /// Construye una `HistoryEntry` sintética cuyo `created_at` es un
    /// índice decimal con cero-padding (no un RFC3339 real: `created_at`
    /// se persiste y compara como TEXT sin parsearse en ningún punto de
    /// esta capa), de forma que el orden lexicográfico de `created_at`
    /// coincide exactamente con el orden numérico de `index`: a mayor
    /// índice, más reciente.
    fn history_entry_with_index(index: usize) -> HistoryEntry {
        HistoryEntry {
            id: format!("hist-{index}"),
            request_name: format!("request-{index}"),
            method: HttpMethod::GET,
            url: "https://example.com/".to_string(),
            environment_name: None,
            response_status: None,
            duration_ms: None,
            error_message: None,
            created_at: format!("{index:010}"),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 20, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 14: Colecciones
        /// acotadas ordenadas de la más reciente a la más antigua
        /// (mitad History).
        /// Validates: Requirements 4.6
        ///
        /// Para cualquier secuencia de N inserciones en History (límite
        /// L=500, espejo de `midway_desktop::app::HISTORY_LIMIT`), el
        /// tamaño final de `workspace_snapshot(L).history` SHALL ser
        /// `min(N, L)`, SHALL contener siempre las L entradas más
        /// recientemente insertadas cuando N > L, y SHALL estar ordenada
        /// de la más reciente a la más antigua.
        ///
        /// El oráculo (índices esperados en orden descendente) se calcula
        /// de forma independiente de la implementación, a partir
        /// únicamente de `insertion_count` (N).
        #[test]
        fn property_14_history_bounded_and_ordered_by_recency(
            insertion_count in 0usize..=520usize,
        ) {
            const HISTORY_LIMIT: usize = 500;

            let runtime = tokio::runtime::Runtime::new()
                .expect("no se pudo crear el runtime de tokio para el test");

            let result: Result<(), TestCaseError> = runtime.block_on(async move {
                let (repository, _temp_dir) = open_temp_repository().await;

                let history: Vec<HistoryEntry> = (0..insertion_count)
                    .map(history_entry_with_index)
                    .collect();

                let snapshot = WorkspaceSnapshot {
                    collections: vec![],
                    environments: vec![],
                    history,
                    secrets: vec![],
                };

                repository
                    .import_workspace_snapshot(snapshot, false)
                    .await
                    .expect("import_workspace_snapshot no debería fallar");

                let loaded = repository
                    .workspace_snapshot(HISTORY_LIMIT)
                    .await
                    .expect("workspace_snapshot no debería fallar");

                let expected_len = insertion_count.min(HISTORY_LIMIT);
                prop_assert_eq!(loaded.history.len(), expected_len);

                // Las últimas `expected_len` inserciones (mayor índice =
                // más reciente), en orden descendente.
                let expected_indices: Vec<usize> = (insertion_count.saturating_sub(expected_len)
                    ..insertion_count)
                    .rev()
                    .collect();

                let actual_indices: Vec<usize> = loaded
                    .history
                    .iter()
                    .map(|entry| {
                        entry
                            .request_name
                            .trim_start_matches("request-")
                            .parse::<usize>()
                            .expect("request_name debe contener el índice sintético")
                    })
                    .collect();

                prop_assert_eq!(actual_indices, expected_indices);

                Ok(())
            });

            result?;
        }
    }
}
