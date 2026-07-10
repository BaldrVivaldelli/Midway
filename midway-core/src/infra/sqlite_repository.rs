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
            validate_folder_name, validate_folder_placement,
            validate_request_folder_association, CollectionSummary, CollectionWithRequests,
            EnvironmentRecord, Folder, FolderValidationError, HistoryEntry,
            SaveEnvironmentInput, SaveFolderInput, SaveRequestInput, SavedRequestRecord,
            SecretMetadata, WorkspaceSnapshot,
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

                    CREATE TABLE IF NOT EXISTS folders (
                        id TEXT PRIMARY KEY,
                        collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
                        parent_folder_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
                        name TEXT NOT NULL
                    );
                    "#,
                )?;

                // SQLite has no ADD COLUMN IF NOT EXISTS, so check via PRAGMA
                // before adding the folder_id column to requests (Req 4.2, 4.4).
                let has_folder_id = {
                    let mut stmt = conn.prepare("PRAGMA table_info(requests)")?;
                    let columns = stmt.query_map([], |row| {
                        row.get::<_, String>(1)
                    })?;
                    let mut found = false;
                    for col in columns {
                        if col? == "folder_id" {
                            found = true;
                            break;
                        }
                    }
                    found
                };

                if !has_folder_id {
                    conn.execute_batch(
                        "ALTER TABLE requests ADD COLUMN folder_id TEXT REFERENCES folders(id) ON DELETE CASCADE;"
                    )?;
                }

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
                conn.execute_batch("PRAGMA foreign_keys = ON;")?;
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
                    let CollectionWithRequests {
                        collection,
                        folders,
                        requests,
                    } = collection;

                    tx.execute(
                        "INSERT INTO collections (id, name, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT(id) DO UPDATE SET
                           name = excluded.name,
                           updated_at = excluded.updated_at",
                        params![
                            &collection.id,
                            &collection.name,
                            &collection.created_at,
                            &collection.updated_at
                        ],
                    )?;

                    let folders = validate_and_order_imported_folders(&collection.id, folders)?;
                    for folder in folders {
                        let existing_collection_id: Option<String> = tx
                            .query_row(
                                "SELECT collection_id FROM folders WHERE id = ?1",
                                params![&folder.id],
                                |row| row.get(0),
                            )
                            .optional()?;

                        if existing_collection_id
                            .as_deref()
                            .is_some_and(|id| id != collection.id)
                        {
                            return Err(import_validation_error(format!(
                                "El folder '{}' ya pertenece a otra colección.",
                                folder.id
                            )));
                        }

                        tx.execute(
                            "INSERT INTO folders (id, collection_id, parent_folder_id, name)
                             VALUES (?1, ?2, ?3, ?4)
                             ON CONFLICT(id) DO UPDATE SET
                               collection_id = excluded.collection_id,
                               parent_folder_id = excluded.parent_folder_id,
                               name = excluded.name",
                            params![
                                &folder.id,
                                &collection.id,
                                &folder.parent_folder_id,
                                &folder.name,
                            ],
                        )?;
                    }

                    for request in requests {
                        if request.collection_id != collection.id {
                            return Err(import_validation_error(format!(
                                "El request '{}' declara una colección distinta a la que lo contiene.",
                                request.id
                            )));
                        }

                        if let Some(ref folder_id) = request.folder_id {
                            let folder = tx
                                .query_row(
                                    "SELECT id, collection_id, parent_folder_id, name
                                     FROM folders WHERE id = ?1",
                                    params![folder_id],
                                    parse_folder_row,
                                )
                                .optional()?
                                .ok_or_else(|| {
                                    import_validation_error(format!(
                                        "El folder '{}' del request '{}' no existe.",
                                        folder_id, request.id
                                    ))
                                })?;

                            validate_request_folder_association(&collection.id, Some(&folder))
                                .map_err(folder_validation_to_rusqlite_error)?;
                        }

                        let draft_json = serde_json::to_string(&request.draft)
                            .map_err(|error| json_error(3, error))?;
                        tx.execute(
                            "INSERT INTO requests
                             (id, collection_id, name, draft_json, created_at, updated_at, folder_id)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                             ON CONFLICT(id) DO UPDATE SET
                               collection_id = excluded.collection_id,
                               name = excluded.name,
                               draft_json = excluded.draft_json,
                               updated_at = excluded.updated_at,
                               folder_id = excluded.folder_id",
                            params![
                                &request.id,
                                &collection.id,
                                &request.name,
                                &draft_json,
                                &request.created_at,
                                &request.updated_at,
                                &request.folder_id,
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

    /// Renames an existing collection while preserving its identity and contents.
    pub async fn rename_collection(
        &self,
        collection_id: String,
        name: String,
    ) -> AppResult<CollectionSummary> {
        let trimmed_name = name.trim().to_string();
        if trimmed_name.is_empty() {
            return Err(AppError::Validation(
                "La collection necesita un nombre.".to_string(),
            ));
        }
        let updated_at = now_rfc3339();

        self.connection
            .call(move |conn| {
                let changed = conn.execute(
                    "UPDATE collections SET name = ?2, updated_at = ?3 WHERE id = ?1",
                    params![&collection_id, &trimmed_name, &updated_at],
                )?;
                if changed == 0 {
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }

                let (created_at, request_count): (String, i64) = conn.query_row(
                    "SELECT c.created_at, COUNT(r.id)
                     FROM collections c
                     LEFT JOIN requests r ON r.collection_id = c.id
                     WHERE c.id = ?1
                     GROUP BY c.id, c.created_at",
                    params![&collection_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;

                Ok(CollectionSummary {
                    id: collection_id,
                    name: trimmed_name,
                    request_count: request_count as u64,
                    created_at,
                    updated_at,
                })
            })
            .await
            .map_err(map_db_err)
    }

    /// Deletes a collection and all of its folders and requests via FK cascades.
    pub async fn delete_collection(&self, collection_id: String) -> AppResult<()> {
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;
                let changed = tx.execute(
                    "DELETE FROM collections WHERE id = ?1",
                    params![&collection_id],
                )?;
                if changed == 0 {
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }
                tx.commit()?;
                Ok(())
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
                    "SELECT id, collection_id, name, draft_json, created_at, updated_at, folder_id
                     FROM requests
                     WHERE collection_id = ?1
                     ORDER BY updated_at DESC, name ASC",
                )?;

                let requests = request_stmt
                    .query_map(params![&collection.id], parse_saved_request_record_row)?
                    .collect::<Result<Vec<_>, _>>()?;

                let folders = load_folders_for_collection(conn, &collection.id)?;

                Ok(Some(CollectionWithRequests {
                    collection: CollectionSummary {
                        request_count: requests.len() as u64,
                        ..collection
                    },
                    folders,
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
        let folder_id = input.folder_id;
        let mut draft = input.draft;
        draft.id = Some(request_id.clone());
        let draft_json = serde_json::to_string(&draft)
            .map_err(|error| AppError::Serialization(error.to_string()))?;
        let created_at_seed = now_rfc3339();
        let updated_at = now_rfc3339();

        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;

                if let Some(ref folder_id) = folder_id {
                    let folder = tx
                        .query_row(
                            "SELECT id, collection_id, parent_folder_id, name FROM folders WHERE id = ?1",
                            params![folder_id],
                            parse_folder_row,
                        )
                        .optional()?
                        .ok_or(rusqlite::Error::QueryReturnedNoRows)?;

                    validate_request_folder_association(&collection_id, Some(&folder))
                        .map_err(folder_validation_to_rusqlite_error)?;
                }

                let existing_created_at: Option<String> = tx
                    .query_row(
                        "SELECT created_at FROM requests WHERE id = ?1",
                        params![&request_id],
                        |row| row.get(0),
                    )
                    .optional()?;

                let created_at = existing_created_at.unwrap_or_else(|| created_at_seed.clone());

                tx.execute(
                    "INSERT INTO requests (id, collection_id, name, draft_json, created_at, updated_at, folder_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(id) DO UPDATE SET
                         collection_id = excluded.collection_id,
                         name = excluded.name,
                         draft_json = excluded.draft_json,
                         folder_id = excluded.folder_id,
                         updated_at = excluded.updated_at",
                    params![
                        &request_id,
                        &collection_id,
                        &name,
                        &draft_json,
                        &created_at,
                        &updated_at,
                        &folder_id,
                    ],
                )?;

                tx.execute(
                    "UPDATE collections SET updated_at = ?2 WHERE id = ?1",
                    params![&collection_id, &updated_at],
                )?;

                tx.commit()?;

                Ok(SavedRequestRecord {
                    id: request_id,
                    collection_id,
                    folder_id,
                    name,
                    draft,
                    created_at,
                    updated_at,
                })
            })
            .await
            .map_err(map_db_err)
    }

    /// Deletes a saved request and updates its parent collection timestamp.
    pub async fn delete_request(&self, request_id: String) -> AppResult<()> {
        let updated_at = now_rfc3339();
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;
                let collection_id: String = tx
                    .query_row(
                        "SELECT collection_id FROM requests WHERE id = ?1",
                        params![&request_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .ok_or(rusqlite::Error::QueryReturnedNoRows)?;

                tx.execute("DELETE FROM requests WHERE id = ?1", params![&request_id])?;
                tx.execute(
                    "UPDATE collections SET updated_at = ?2 WHERE id = ?1",
                    params![&collection_id, &updated_at],
                )?;
                tx.commit()?;
                Ok(())
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

    /// Inserts imported folders into the `folders` table, assigning the given
    /// `collection_id` to all of them. Folders must be ordered so that a parent
    /// appears before its children (the order produced by `collect_postman_items`).
    pub async fn insert_imported_folders(
        &self,
        collection_id: String,
        folders: Vec<crate::domain::workspace::Folder>,
    ) -> AppResult<()> {
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;
                for folder in &folders {
                    tx.execute(
                        "INSERT INTO folders (id, collection_id, parent_folder_id, name)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![
                            &folder.id,
                            &collection_id,
                            &folder.parent_folder_id,
                            &folder.name,
                        ],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    /// Updates a request's folder while enforcing collection ownership.
    pub async fn update_request_folder(
        &self,
        request_id: &str,
        folder_id: Option<&str>,
    ) -> AppResult<()> {
        let request_id = request_id.to_string();
        let folder_id = folder_id.map(String::from);
        let updated_at = now_rfc3339();
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                let collection_id: String = tx.query_row(
                    "SELECT collection_id FROM requests WHERE id = ?1",
                    params![&request_id],
                    |row| row.get(0),
                )?;
                let folder = match folder_id.as_deref() {
                    Some(folder_id) => Some(
                        tx.query_row(
                            "SELECT id, collection_id, parent_folder_id, name
                             FROM folders WHERE id = ?1",
                            params![folder_id],
                            parse_folder_row,
                        )
                        .optional()?
                        .ok_or(rusqlite::Error::QueryReturnedNoRows)?,
                    ),
                    None => None,
                };
                validate_request_folder_association(&collection_id, folder.as_ref())
                    .map_err(folder_validation_to_rusqlite_error)?;

                tx.execute(
                    "UPDATE requests SET folder_id = ?2, updated_at = ?3 WHERE id = ?1",
                    params![&request_id, &folder_id, &updated_at],
                )?;
                tx.execute(
                    "UPDATE collections SET updated_at = ?2 WHERE id = ?1",
                    params![&collection_id, &updated_at],
                )?;
                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    /// Creates a new folder after validating name and placement (Req 3.5, 3.6, 4.5).
    pub async fn create_folder(&self, input: SaveFolderInput) -> AppResult<Folder> {
        // Validate name
        validate_folder_name(&input.name).map_err(folder_validation_to_app_error)?;

        let id = Uuid::new_v4().to_string();
        let trimmed_name = input.name.trim().to_string();
        let collection_id = input.collection_id;
        let parent_folder_id = input.parent_folder_id;

        // Build the folder entity for placement validation
        let folder = Folder {
            id: id.clone(),
            collection_id: collection_id.clone(),
            parent_folder_id: parent_folder_id.clone(),
            name: trimmed_name.clone(),
        };

        let folder_for_validation = folder.clone();

        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;

                // Load existing folders for validation context
                let existing_folders = load_folders_for_collection(&tx, &collection_id)?;

                // Validate placement
                validate_folder_placement(&folder_for_validation, &existing_folders)
                    .map_err(|e| folder_validation_to_rusqlite_error(e))?;

                tx.execute(
                    "INSERT INTO folders (id, collection_id, parent_folder_id, name)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        &folder_for_validation.id,
                        &collection_id,
                        &parent_folder_id,
                        &trimmed_name,
                    ],
                )?;

                tx.commit()?;
                Ok(folder)
            })
            .await
            .map_err(map_db_err)
    }

    /// Renames a folder after validating the new name (Req 3.1).
    pub async fn rename_folder(&self, folder_id: String, name: String) -> AppResult<Folder> {
        validate_folder_name(&name).map_err(folder_validation_to_app_error)?;

        let trimmed_name = name.trim().to_string();

        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;

                // Fetch the folder to ensure it exists
                let folder: Folder = tx
                    .query_row(
                        "SELECT id, collection_id, parent_folder_id, name FROM folders WHERE id = ?1",
                        params![&folder_id],
                        parse_folder_row,
                    )
                    .optional()?
                    .ok_or_else(|| {
                        rusqlite::Error::QueryReturnedNoRows
                    })?;

                tx.execute(
                    "UPDATE folders SET name = ?2 WHERE id = ?1",
                    params![&folder_id, &trimmed_name],
                )?;

                tx.commit()?;

                Ok(Folder {
                    name: trimmed_name,
                    ..folder
                })
            })
            .await
            .map_err(map_db_err)
    }

    /// Moves a folder to a new parent, validating placement (Req 3.5, 3.6).
    pub async fn move_folder(
        &self,
        folder_id: String,
        new_parent: Option<String>,
    ) -> AppResult<Folder> {
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;

                // Fetch the folder to ensure it exists
                let folder: Folder = tx
                    .query_row(
                        "SELECT id, collection_id, parent_folder_id, name FROM folders WHERE id = ?1",
                        params![&folder_id],
                        parse_folder_row,
                    )
                    .optional()?
                    .ok_or_else(|| {
                        rusqlite::Error::QueryReturnedNoRows
                    })?;

                // Build the folder with the new parent for validation
                let moved_folder = Folder {
                    parent_folder_id: new_parent.clone(),
                    ..folder.clone()
                };

                // Load existing folders (excluding the folder being moved, as it will change)
                let existing_folders = load_folders_for_collection(&tx, &folder.collection_id)?;

                // Validate placement with the new parent
                validate_folder_placement(&moved_folder, &existing_folders)
                    .map_err(|e| folder_validation_to_rusqlite_error(e))?;

                tx.execute(
                    "UPDATE folders SET parent_folder_id = ?2 WHERE id = ?1",
                    params![&folder_id, &new_parent],
                )?;

                tx.commit()?;

                Ok(moved_folder)
            })
            .await
            .map_err(map_db_err)
    }

    /// Deletes a folder with cascading delete of subfolders and requests (Req 4.5).
    /// Runs in a transaction with foreign_keys = ON; rollback on any failure.
    pub async fn delete_folder(&self, folder_id: String) -> AppResult<()> {
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;

                // Verify the folder exists
                let exists: bool = tx
                    .query_row(
                        "SELECT COUNT(*) FROM folders WHERE id = ?1",
                        params![&folder_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .map(|count| count > 0)?;

                if !exists {
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }

                // With ON DELETE CASCADE enabled, deleting the folder will
                // automatically cascade to subfolders and requests.
                tx.execute(
                    "DELETE FROM folders WHERE id = ?1",
                    params![&folder_id],
                )?;

                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }

    /// Associates/disassociates a request with a folder, validating the collection (Req 3.8).
    pub async fn set_request_folder(
        &self,
        request_id: String,
        folder_id: Option<String>,
    ) -> AppResult<()> {
        self.connection
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute_batch("PRAGMA foreign_keys = ON;")?;

                // Get the request's collection_id
                let request_collection_id: String = tx
                    .query_row(
                        "SELECT collection_id FROM requests WHERE id = ?1",
                        params![&request_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;

                // If folder_id is provided, fetch the folder and validate
                if let Some(ref fid) = folder_id {
                    let folder: Folder = tx
                        .query_row(
                            "SELECT id, collection_id, parent_folder_id, name FROM folders WHERE id = ?1",
                            params![fid],
                            parse_folder_row,
                        )
                        .optional()?
                        .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)?;

                    validate_request_folder_association(&request_collection_id, Some(&folder))
                        .map_err(|e| folder_validation_to_rusqlite_error(e))?;
                }

                tx.execute(
                    "UPDATE requests SET folder_id = ?2 WHERE id = ?1",
                    params![&request_id, &folder_id],
                )?;

                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(map_db_err)
    }
}

/// Validates a snapshot folder tree and returns it in parent-before-child order.
/// The import runs this inside its transaction so malformed trees cannot leave
/// partially imported collections behind.
fn validate_and_order_imported_folders(
    collection_id: &str,
    folders: Vec<Folder>,
) -> rusqlite::Result<Vec<Folder>> {
    let mut folder_ids = std::collections::HashSet::with_capacity(folders.len());

    for folder in &folders {
        validate_folder_name(&folder.name).map_err(folder_validation_to_rusqlite_error)?;

        if folder.collection_id != collection_id {
            return Err(import_validation_error(format!(
                "El folder '{}' declara una colección distinta a la que lo contiene.",
                folder.id
            )));
        }

        if !folder_ids.insert(folder.id.clone()) {
            return Err(import_validation_error(format!(
                "El snapshot contiene más de un folder con id '{}'.",
                folder.id
            )));
        }
    }

    let mut remaining = folders;
    let mut ordered = Vec::with_capacity(remaining.len());

    while !remaining.is_empty() {
        let ready_index = remaining.iter().position(|folder| {
            folder.parent_folder_id.as_ref().is_none_or(|parent_id| {
                ordered.iter().any(|parent: &Folder| parent.id == *parent_id)
            })
        });

        if let Some(index) = ready_index {
            let folder = remaining.remove(index);
            validate_folder_placement(&folder, &ordered)
                .map_err(folder_validation_to_rusqlite_error)?;
            ordered.push(folder);
            continue;
        }

        if let Some(folder) = remaining.iter().find(|folder| {
            folder
                .parent_folder_id
                .as_ref()
                .is_some_and(|parent_id| !folder_ids.contains(parent_id))
        }) {
            return Err(import_validation_error(format!(
                "El folder padre de '{}' no existe en el snapshot.",
                folder.id
            )));
        }

        return Err(folder_validation_to_rusqlite_error(
            FolderValidationError::Cycle {
                folder_id: remaining[0].id.clone(),
            },
        ));
    }

    Ok(ordered)
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
            "SELECT id, collection_id, name, draft_json, created_at, updated_at, folder_id
             FROM requests
             WHERE collection_id = ?1
             ORDER BY updated_at DESC, name ASC",
        )?;

        let requests = request_stmt
            .query_map(params![&collection.id], parse_saved_request_record_row)?
            .collect::<Result<Vec<_>, _>>()?;

        let folders = load_folders_for_collection(conn, &collection.id)?;

        out.push(CollectionWithRequests {
            collection: CollectionSummary {
                request_count: requests.len() as u64,
                ..collection
            },
            folders,
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
        folder_id: row.get(6)?,
        name: row.get(2)?,
        draft,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

/// Parses a folder row: (id, collection_id, parent_folder_id, name).
fn parse_folder_row(row: &Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        parent_folder_id: row.get(2)?,
        name: row.get(3)?,
    })
}

/// Loads all folders belonging to a collection.
fn load_folders_for_collection(
    conn: &rusqlite::Connection,
    collection_id: &str,
) -> rusqlite::Result<Vec<Folder>> {
    let mut stmt = conn.prepare(
        "SELECT id, collection_id, parent_folder_id, name
         FROM folders
         WHERE collection_id = ?1
         ORDER BY name ASC",
    )?;

    let rows = stmt.query_map(params![collection_id], parse_folder_row)?;
    rows.collect::<Result<Vec<_>, _>>()
}

/// Converts a FolderValidationError to AppError (for use outside of transactions).
fn folder_validation_to_app_error(error: FolderValidationError) -> AppError {
    let message = match error {
        FolderValidationError::NameLength { len } => {
            format!("El nombre del folder debe tener entre 1 y 100 caracteres (tiene {len}).")
        }
        FolderValidationError::Cycle { folder_id } => {
            format!("Mover el folder '{folder_id}' a ese destino generaría un ciclo.")
        }
        FolderValidationError::CrossCollectionParent => {
            "El folder padre pertenece a otra colección.".to_string()
        }
        FolderValidationError::CrossCollectionRequest => {
            "El request pertenece a una colección distinta a la del folder.".to_string()
        }
        FolderValidationError::ParentNotFound => {
            "El folder padre especificado no existe.".to_string()
        }
    };
    AppError::Validation(message)
}

/// Converts a FolderValidationError to rusqlite::Error (for use inside transactions).
fn folder_validation_to_rusqlite_error(error: FolderValidationError) -> rusqlite::Error {
    let message = match error {
        FolderValidationError::NameLength { len } => {
            format!("El nombre del folder debe tener entre 1 y 100 caracteres (tiene {len}).")
        }
        FolderValidationError::Cycle { folder_id } => {
            format!("Mover el folder '{folder_id}' a ese destino generaría un ciclo.")
        }
        FolderValidationError::CrossCollectionParent => {
            "El folder padre pertenece a otra colección.".to_string()
        }
        FolderValidationError::CrossCollectionRequest => {
            "El request pertenece a una colección distinta a la del folder.".to_string()
        }
        FolderValidationError::ParentNotFound => {
            "El folder padre especificado no existe.".to_string()
        }
    };
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, message)),
    )
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

fn import_validation_error(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        )),
    )
}

fn map_db_err(error: tokio_rusqlite::Error) -> AppError {
    match error {
        tokio_rusqlite::Error::Error(rusqlite::Error::QueryReturnedNoRows) => {
            AppError::NotFound("No se encontró el elemento solicitado.".to_string())
        }
        other => AppError::Database(other.to_string()),
    }
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

    fn request_draft(name: &str) -> RequestDraft {
        RequestDraft {
            id: None,
            name: name.to_string(),
            method: HttpMethod::GET,
            url: "https://example.com".to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            auth: crate::domain::http::AuthConfig::None,
            body: crate::domain::http::RequestBodyDraft {
                mode: crate::domain::http::BodyMode::None,
                value: String::new(),
                form_data: Vec::new(),
            },
            timeout_ms: 30_000,
            environment_id: None,
            response_tests: Vec::new(),
        }
    }

    #[tokio::test]
    async fn save_request_persists_selected_folder_atomically() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let folder = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Auth".to_string(),
            })
            .await
            .unwrap();

        let saved = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: Some(folder.id.clone()),
                draft: request_draft("Login"),
            })
            .await
            .unwrap();

        assert_eq!(saved.folder_id.as_deref(), Some(folder.id.as_str()));
        let snapshot = repository.export_full_snapshot().await.unwrap();
        assert_eq!(
            snapshot.collections[0].requests[0].folder_id.as_deref(),
            Some(folder.id.as_str())
        );
    }

    #[tokio::test]
    async fn save_request_rejects_folder_from_another_collection_without_inserting() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let first = repository
            .create_collection("First".to_string())
            .await
            .unwrap();
        let second = repository
            .create_collection("Second".to_string())
            .await
            .unwrap();
        let foreign_folder = repository
            .create_folder(SaveFolderInput {
                collection_id: first.id,
                parent_folder_id: None,
                name: "Foreign".to_string(),
            })
            .await
            .unwrap();

        let result = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: second.id.clone(),
                folder_id: Some(foreign_folder.id),
                draft: request_draft("Should fail"),
            })
            .await;

        assert!(result.is_err());
        let second_collection = repository
            .get_collection_with_requests(&second.id)
            .await
            .unwrap()
            .unwrap();
        assert!(second_collection.requests.is_empty());
    }

    #[tokio::test]
    async fn update_request_folder_rejects_cross_collection_without_mutation() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let first = repository
            .create_collection("First".to_string())
            .await
            .unwrap();
        let second = repository
            .create_collection("Second".to_string())
            .await
            .unwrap();
        let foreign_folder = repository
            .create_folder(SaveFolderInput {
                collection_id: second.id,
                parent_folder_id: None,
                name: "Foreign".to_string(),
            })
            .await
            .unwrap();
        let request = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: first.id.clone(),
                folder_id: None,
                draft: request_draft("Request"),
            })
            .await
            .unwrap();

        let result = repository
            .update_request_folder(&request.id, Some(&foreign_folder.id))
            .await;

        assert!(result.is_err());
        let loaded = repository
            .get_collection_with_requests(&first.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.requests[0].folder_id, None);
    }

    #[tokio::test]
    async fn update_request_folder_moves_between_folders_and_back_to_root() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let first = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "First".to_string(),
            })
            .await
            .unwrap();
        let second = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Second".to_string(),
            })
            .await
            .unwrap();
        let request = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: Some(first.id),
                draft: request_draft("Movable"),
            })
            .await
            .unwrap();

        repository
            .update_request_folder(&request.id, Some(&second.id))
            .await
            .unwrap();
        let moved = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(moved.requests[0].folder_id.as_deref(), Some(second.id.as_str()));

        repository
            .update_request_folder(&request.id, None)
            .await
            .unwrap();
        let rooted = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rooted.requests[0].folder_id, None);
    }

    #[tokio::test]
    async fn create_folder_persists_trimmed_root_and_nested_placement() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();

        let root = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "  Root  ".to_string(),
            })
            .await
            .unwrap();
        let child = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: Some(root.id.clone()),
                name: "  Child  ".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(root.name, "Root");
        assert_eq!(root.parent_folder_id, None);
        assert_eq!(child.name, "Child");
        assert_eq!(child.parent_folder_id.as_deref(), Some(root.id.as_str()));

        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.folders.len(), 2);
        assert!(loaded.folders.iter().any(|folder| folder == &root));
        assert!(loaded.folders.iter().any(|folder| folder == &child));
    }

    #[tokio::test]
    async fn create_folder_rejects_invalid_names_without_mutating_the_collection() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();

        for invalid_name in ["   ".to_string(), "x".repeat(101)] {
            let result = repository
                .create_folder(SaveFolderInput {
                    collection_id: collection.id.clone(),
                    parent_folder_id: None,
                    name: invalid_name,
                })
                .await;
            assert!(result.is_err());
        }

        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert!(loaded.folders.is_empty());
    }

    #[tokio::test]
    async fn create_folder_rejects_missing_and_cross_collection_parents_without_mutation() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let first = repository
            .create_collection("First".to_string())
            .await
            .unwrap();
        let second = repository
            .create_collection("Second".to_string())
            .await
            .unwrap();
        let foreign_parent = repository
            .create_folder(SaveFolderInput {
                collection_id: first.id.clone(),
                parent_folder_id: None,
                name: "Foreign parent".to_string(),
            })
            .await
            .unwrap();

        let missing_parent_result = repository
            .create_folder(SaveFolderInput {
                collection_id: second.id.clone(),
                parent_folder_id: Some("missing-folder".to_string()),
                name: "Missing parent child".to_string(),
            })
            .await;
        let cross_collection_result = repository
            .create_folder(SaveFolderInput {
                collection_id: second.id.clone(),
                parent_folder_id: Some(foreign_parent.id.clone()),
                name: "Cross collection child".to_string(),
            })
            .await;

        assert!(missing_parent_result.is_err());
        assert!(cross_collection_result.is_err());
        let first_loaded = repository
            .get_collection_with_requests(&first.id)
            .await
            .unwrap()
            .unwrap();
        let second_loaded = repository
            .get_collection_with_requests(&second.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first_loaded.folders, vec![foreign_parent]);
        assert!(second_loaded.folders.is_empty());
    }

    #[tokio::test]
    async fn rename_folder_persists_trimmed_name_and_preserves_identity_and_parent() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let parent = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Parent".to_string(),
            })
            .await
            .unwrap();
        let child = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: Some(parent.id.clone()),
                name: "Before".to_string(),
            })
            .await
            .unwrap();

        let renamed = repository
            .rename_folder(child.id.clone(), "  After  ".to_string())
            .await
            .unwrap();

        assert_eq!(renamed.id, child.id);
        assert_eq!(renamed.collection_id, collection.id);
        assert_eq!(renamed.parent_folder_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(renamed.name, "After");
        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert!(loaded.folders.iter().any(|folder| folder == &renamed));
        assert!(!loaded.folders.iter().any(|folder| folder.name == "Before"));
    }

    #[tokio::test]
    async fn rename_folder_rejects_invalid_name_and_unknown_id_without_mutation() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let original = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Original".to_string(),
            })
            .await
            .unwrap();

        let invalid_name_result = repository
            .rename_folder(original.id.clone(), "   ".to_string())
            .await;
        let unknown_id_result = repository
            .rename_folder("missing-folder".to_string(), "Valid".to_string())
            .await;

        assert!(invalid_name_result.is_err());
        assert!(unknown_id_result.is_err());
        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.folders, vec![original]);
    }

    #[tokio::test]
    async fn delete_folder_removes_only_its_subtree_and_preserves_unrelated_content() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let doomed_parent = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Doomed".to_string(),
            })
            .await
            .unwrap();
        let doomed_child = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: Some(doomed_parent.id.clone()),
                name: "Doomed child".to_string(),
            })
            .await
            .unwrap();
        let survivor = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Survivor".to_string(),
            })
            .await
            .unwrap();
        let doomed_request = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: Some(doomed_child.id),
                draft: request_draft("Doomed request"),
            })
            .await
            .unwrap();
        let root_request = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: None,
                draft: request_draft("Root request"),
            })
            .await
            .unwrap();

        repository.delete_folder(doomed_parent.id).await.unwrap();

        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.folders, vec![survivor]);
        assert_eq!(loaded.requests.len(), 1);
        assert_eq!(loaded.requests[0].id, root_request.id);
        assert!(!loaded
            .requests
            .iter()
            .any(|request| request.id == doomed_request.id));
    }

    #[tokio::test]
    async fn delete_folder_rolls_back_when_a_cascaded_request_delete_fails() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let folder = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Protected".to_string(),
            })
            .await
            .unwrap();
        let request = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: Some(folder.id.clone()),
                draft: request_draft("Protected request"),
            })
            .await
            .unwrap();

        repository
            .connection
            .call(|conn| {
                conn.execute_batch(
                    "CREATE TRIGGER reject_request_delete
                     BEFORE DELETE ON requests
                     BEGIN
                       SELECT RAISE(ABORT, 'forced cascade failure');
                     END;",
                )?;
                Ok::<(), rusqlite::Error>(())
            })
            .await
            .unwrap();

        let result = repository.delete_folder(folder.id.clone()).await;

        assert!(result.is_err());
        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.folders, vec![folder]);
        assert_eq!(loaded.requests.len(), 1);
        assert_eq!(loaded.requests[0].id, request.id);
    }

    #[tokio::test]
    async fn delete_folder_rejects_unknown_id_without_mutating_existing_tree() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let existing = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Existing".to_string(),
            })
            .await
            .unwrap();

        let result = repository
            .delete_folder("missing-folder".to_string())
            .await;

        assert!(result.is_err());
        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.folders, vec![existing]);
    }

    #[tokio::test]
    async fn rename_collection_preserves_requests_and_updates_name() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("Before".to_string())
            .await
            .unwrap();
        repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: None,
                draft: request_draft("List"),
            })
            .await
            .unwrap();

        let renamed = repository
            .rename_collection(collection.id.clone(), "After".to_string())
            .await
            .unwrap();

        assert_eq!(renamed.id, collection.id);
        assert_eq!(renamed.name, "After");
        assert_eq!(renamed.request_count, 1);
    }

    #[tokio::test]
    async fn delete_request_removes_only_selected_request() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let first = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: None,
                draft: request_draft("First"),
            })
            .await
            .unwrap();
        let second = repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: None,
                draft: request_draft("Second"),
            })
            .await
            .unwrap();

        repository.delete_request(first.id).await.unwrap();

        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.requests.len(), 1);
        assert_eq!(loaded.requests[0].id, second.id);
    }

    #[tokio::test]
    async fn delete_folder_cascades_to_children_and_requests() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("API".to_string())
            .await
            .unwrap();
        let parent = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Parent".to_string(),
            })
            .await
            .unwrap();
        let child = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: Some(parent.id.clone()),
                name: "Child".to_string(),
            })
            .await
            .unwrap();
        repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: Some(child.id),
                draft: request_draft("Nested"),
            })
            .await
            .unwrap();

        repository.delete_folder(parent.id).await.unwrap();

        let loaded = repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .unwrap();
        assert!(loaded.folders.is_empty());
        assert!(loaded.requests.is_empty());
    }

    #[tokio::test]
    async fn delete_collection_cascades_to_folders_and_requests() {
        let (repository, _temp_dir) = open_temp_repository().await;
        let collection = repository
            .create_collection("Disposable".to_string())
            .await
            .unwrap();
        let folder = repository
            .create_folder(SaveFolderInput {
                collection_id: collection.id.clone(),
                parent_folder_id: None,
                name: "Folder".to_string(),
            })
            .await
            .unwrap();
        repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: Some(folder.id),
                draft: request_draft("Nested"),
            })
            .await
            .unwrap();

        repository
            .delete_collection(collection.id.clone())
            .await
            .unwrap();

        assert!(repository
            .get_collection_with_requests(&collection.id)
            .await
            .unwrap()
            .is_none());
        assert!(repository.export_full_snapshot().await.unwrap().collections.is_empty());
    }

    #[tokio::test]
    async fn missing_entities_are_reported_as_not_found() {
        let (repository, _temp_dir) = open_temp_repository().await;

        let collection_error = repository
            .delete_collection("missing-collection".to_string())
            .await
            .unwrap_err();
        let folder_error = repository
            .delete_folder("missing-folder".to_string())
            .await
            .unwrap_err();
        let request_error = repository
            .delete_request("missing-request".to_string())
            .await
            .unwrap_err();

        assert!(matches!(collection_error, AppError::NotFound(_)));
        assert!(matches!(folder_error, AppError::NotFound(_)));
        assert!(matches!(request_error, AppError::NotFound(_)));
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
