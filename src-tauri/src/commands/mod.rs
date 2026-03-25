use std::{collections::{BTreeMap, BTreeSet}, path::PathBuf};

use chrono::Utc;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::{
    app::errors::{AppError, AppResult, CommandError},
    domain::{
        http::{RequestDraft, RequestPreview},
        interpolation::{resolve_request, SecretRenderMode},
        interop::{
            detect_import_format, export_postman_collection, import_openapi_document,
            import_postman_collection, make_native_bundle, parse_json_or_yaml_payload,
            parse_native_bundle, ExportWorkspaceInput, ExportWorkspaceResult,
            ImportWorkspaceInput, ImportWorkspacePayloadInput, ImportWorkspaceResult,
            WorkspaceExportFormat, WorkspaceImportFormat,
        },
        preview::make_preview,
        runner::{
            CancelRequestResult, CollectionRunItem, CollectionRunPhase,
            CollectionRunProgressEvent, CollectionRunReport, RequestExecutionOutcome,
            RunCollectionInput, COLLECTION_RUN_PROGRESS_EVENT,
        },
        secrets::collect_secret_aliases,
        testing::evaluate_response_assertions,
        workspace::{
            CollectionSummary, EnvironmentRecord, SaveEnvironmentInput, SaveRequestInput,
            SaveSecretInput, SavedRequestRecord, SecretMetadata, WorkspaceSnapshot,
        },
    },
    state::AppState,
};

const HISTORY_LIMIT: usize = 50;

#[tauri::command]
pub async fn preview_request(
    state: State<'_, AppState>,
    draft: RequestDraft,
) -> Result<RequestPreview, CommandError> {
    let environment = load_environment_record(&state, draft.environment_id.as_deref()).await?;
    let environment_name = environment.as_ref().map(|env| env.name.clone());
    let environment_rows = environment
        .as_ref()
        .map(|env| env.variables.as_slice())
        .unwrap_or(&[]);
    let secrets = load_secrets_for_request(&state, &draft, environment_rows).await?;
    let resolution = resolve_request(
        &draft,
        environment_name,
        environment_rows,
        &secrets,
        SecretRenderMode::Redact,
    )?;
    Ok(make_preview(resolution))
}

#[tauri::command]
pub async fn execute_request(
    state: State<'_, AppState>,
    draft: RequestDraft,
    execution_id: Option<String>,
) -> Result<RequestExecutionOutcome, CommandError> {
    let environment = load_environment_record(&state, draft.environment_id.as_deref()).await?;
    let environment_name = environment.as_ref().map(|env| env.name.clone());
    let execution_id = execution_id.unwrap_or_else(|| Uuid::new_v4().to_string());

    let result =
        execute_draft_with_environment(&state, &draft, environment.as_ref(), execution_id).await;

    match result {
        Ok((outcome, resolved_url, used_environment_name)) => {
            state
                .repository
                .append_history(
                    &draft,
                    used_environment_name.clone().or(environment_name),
                    resolved_url,
                    Some(outcome.response.clone()),
                    None,
                )
                .await?;
            Ok(outcome)
        }
        Err(error) => {
            let message = error.to_string();
            let _ = state
                .repository
                .append_history(
                    &draft,
                    environment_name,
                    draft.url.clone(),
                    None,
                    Some(message),
                )
                .await;
            Err(error.into())
        }
    }
}

#[tauri::command]
pub async fn cancel_request(
    state: State<'_, AppState>,
    execution_id: String,
) -> Result<CancelRequestResult, CommandError> {
    let canceled = state.request_executor.cancel(execution_id.clone()).await;
    Ok(CancelRequestResult {
        execution_id,
        canceled,
    })
}

#[tauri::command]
pub async fn run_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    input: RunCollectionInput,
) -> Result<CollectionRunReport, CommandError> {
    let collection = state
        .repository
        .get_collection_with_requests(&input.collection_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("No existe la collection {}", input.collection_id)))?;

    let override_environment =
        load_environment_record(&state, input.environment_override_id.as_deref()).await?;
    let started_at = Utc::now().to_rfc3339();
    let run_id = Uuid::new_v4().to_string();
    let total_requests = collection.requests.len() as u64;

    let mut items = Vec::new();
    let mut completed_requests = 0_u64;
    let mut errored_requests = 0_u64;
    let mut passed_assertions = 0_u64;
    let mut failed_assertions = 0_u64;

    emit_collection_progress(
        &app,
        CollectionRunProgressEvent {
            run_id: run_id.clone(),
            phase: CollectionRunPhase::Started,
            collection_id: collection.collection.id.clone(),
            collection_name: collection.collection.name.clone(),
            total_requests,
            processed_requests: 0,
            current_index: 0,
            completed_requests,
            errored_requests,
            passed_assertions,
            failed_assertions,
            request_id: None,
            request_name: None,
            environment_name: None,
            resolved_url: None,
            response_status: None,
            duration_ms: None,
            error_message: None,
            started_at: started_at.clone(),
            finished_at: None,
            emitted_at: Utc::now().to_rfc3339(),
        },
    );

    for (index, record) in collection.requests.iter().enumerate() {
        let effective_environment = match override_environment.as_ref() {
            Some(environment) => Some(environment.clone()),
            None => load_environment_record(&state, record.draft.environment_id.as_deref()).await?,
        };

        let environment_name = effective_environment.as_ref().map(|env| env.name.clone());
        let current_index = index as u64 + 1;
        let executed_at = Utc::now().to_rfc3339();

        emit_collection_progress(
            &app,
            CollectionRunProgressEvent {
                run_id: run_id.clone(),
                phase: CollectionRunPhase::RequestStarted,
                collection_id: collection.collection.id.clone(),
                collection_name: collection.collection.name.clone(),
                total_requests,
                processed_requests: completed_requests + errored_requests,
                current_index,
                completed_requests,
                errored_requests,
                passed_assertions,
                failed_assertions,
                request_id: Some(record.id.clone()),
                request_name: Some(record.name.clone()),
                environment_name: environment_name.clone(),
                resolved_url: None,
                response_status: None,
                duration_ms: None,
                error_message: None,
                started_at: started_at.clone(),
                finished_at: None,
                emitted_at: Utc::now().to_rfc3339(),
            },
        );

        match execute_draft_with_environment(
            &state,
            &record.draft,
            effective_environment.as_ref(),
            Uuid::new_v4().to_string(),
        )
        .await
        {
            Ok((outcome, resolved_url, used_environment_name)) => {
                completed_requests += 1;

                let response = outcome.response;
                let assertion_report = outcome.assertion_report;
                let response_status = response.status;
                let duration_ms = response.duration_ms;

                passed_assertions += assertion_report.passed;
                failed_assertions += assertion_report.failed;

                state
                    .repository
                    .append_history(
                        &record.draft,
                        used_environment_name.clone().or(environment_name.clone()),
                        resolved_url.clone(),
                        Some(response.clone()),
                        None,
                    )
                    .await?;

                let final_environment_name = used_environment_name.or(environment_name.clone());

                items.push(CollectionRunItem {
                    request_id: record.id.clone(),
                    request_name: record.name.clone(),
                    environment_name: final_environment_name.clone(),
                    resolved_url: Some(resolved_url.clone()),
                    response_status: Some(response_status),
                    duration_ms: Some(duration_ms),
                    error_message: None,
                    assertion_report: Some(assertion_report.clone()),
                    executed_at,
                });

                emit_collection_progress(
                    &app,
                    CollectionRunProgressEvent {
                        run_id: run_id.clone(),
                        phase: CollectionRunPhase::RequestFinished,
                        collection_id: collection.collection.id.clone(),
                        collection_name: collection.collection.name.clone(),
                        total_requests,
                        processed_requests: completed_requests + errored_requests,
                        current_index,
                        completed_requests,
                        errored_requests,
                        passed_assertions,
                        failed_assertions,
                        request_id: Some(record.id.clone()),
                        request_name: Some(record.name.clone()),
                        environment_name: final_environment_name,
                        resolved_url: Some(resolved_url),
                        response_status: Some(response_status),
                        duration_ms: Some(duration_ms),
                        error_message: None,
                        started_at: started_at.clone(),
                        finished_at: None,
                        emitted_at: Utc::now().to_rfc3339(),
                    },
                );
            }
            Err(error) => {
                errored_requests += 1;
                let message = error.to_string();

                let _ = state
                    .repository
                    .append_history(
                        &record.draft,
                        environment_name.clone(),
                        record.draft.url.clone(),
                        None,
                        Some(message.clone()),
                    )
                    .await;

                items.push(CollectionRunItem {
                    request_id: record.id.clone(),
                    request_name: record.name.clone(),
                    environment_name: environment_name.clone(),
                    resolved_url: None,
                    response_status: None,
                    duration_ms: None,
                    error_message: Some(message.clone()),
                    assertion_report: None,
                    executed_at,
                });

                emit_collection_progress(
                    &app,
                    CollectionRunProgressEvent {
                        run_id: run_id.clone(),
                        phase: CollectionRunPhase::RequestFinished,
                        collection_id: collection.collection.id.clone(),
                        collection_name: collection.collection.name.clone(),
                        total_requests,
                        processed_requests: completed_requests + errored_requests,
                        current_index,
                        completed_requests,
                        errored_requests,
                        passed_assertions,
                        failed_assertions,
                        request_id: Some(record.id.clone()),
                        request_name: Some(record.name.clone()),
                        environment_name: environment_name.clone(),
                        resolved_url: None,
                        response_status: None,
                        duration_ms: None,
                        error_message: Some(message.clone()),
                        started_at: started_at.clone(),
                        finished_at: None,
                        emitted_at: Utc::now().to_rfc3339(),
                    },
                );

                if input.stop_on_error {
                    break;
                }
            }
        }
    }

    let finished_at = Utc::now().to_rfc3339();
    let collection_id = collection.collection.id.clone();
    let collection_name = collection.collection.name.clone();

    emit_collection_progress(
        &app,
        CollectionRunProgressEvent {
            run_id,
            phase: CollectionRunPhase::Finished,
            collection_id: collection_id.clone(),
            collection_name: collection_name.clone(),
            total_requests,
            processed_requests: completed_requests + errored_requests,
            current_index: completed_requests + errored_requests,
            completed_requests,
            errored_requests,
            passed_assertions,
            failed_assertions,
            request_id: None,
            request_name: None,
            environment_name: None,
            resolved_url: None,
            response_status: None,
            duration_ms: None,
            error_message: None,
            started_at: started_at.clone(),
            finished_at: Some(finished_at.clone()),
            emitted_at: Utc::now().to_rfc3339(),
        },
    );

    Ok(CollectionRunReport {
        collection_id,
        collection_name,
        started_at,
        finished_at,
        total_requests,
        completed_requests,
        errored_requests,
        passed_assertions,
        failed_assertions,
        items,
    })
}

#[tauri::command]
pub async fn export_workspace_data(
    state: State<'_, AppState>,
    input: ExportWorkspaceInput,
) -> Result<ExportWorkspaceResult, CommandError> {
    let path = normalize_path(&input.path)?;

    match input.format {
        WorkspaceExportFormat::NativeWorkspaceV1 => {
            let snapshot = state.repository.export_full_snapshot().await?;
            let exporting_selected_collection = input.collection_id.is_some();
            let snapshot = if let Some(collection_id) = input.collection_id.as_deref() {
                snapshot_for_native_collection_export(
                    snapshot,
                    collection_id,
                    input.include_secret_metadata,
                )?
            } else {
                snapshot
            };
            let bundle = make_native_bundle(
                snapshot,
                input.include_history && !exporting_selected_collection,
                input.include_secret_metadata,
            );
            let payload = serde_json::to_string_pretty(&bundle)
                .map_err(|error| AppError::Serialization(error.to_string()))?;
            let bytes_written = write_text_file(&path, payload).await?;
            let snapshot = bundle.snapshot;
            let requests_exported = snapshot
                .collections
                .iter()
                .map(|collection| collection.requests.len() as u64)
                .sum::<u64>();

            Ok(ExportWorkspaceResult {
                path: path.to_string_lossy().to_string(),
                format: input.format,
                collections_exported: snapshot.collections.len() as u64,
                requests_exported,
                environments_exported: snapshot.environments.len() as u64,
                history_exported: snapshot.history.len() as u64,
                secret_metadata_exported: snapshot.secrets.len() as u64,
                bytes_written,
            })
        }
        WorkspaceExportFormat::PostmanCollectionV21 => {
            let collection_id = input.collection_id.ok_or_else(|| {
                AppError::Validation(
                    "Para exportar Postman v2.1 necesitás elegir una collection.".to_string(),
                )
            })?;
            let collection = state
                .repository
                .get_collection_with_requests(&collection_id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("No existe la collection {}", collection_id)))?;
            let payload = serde_json::to_string_pretty(&export_postman_collection(&collection))
                .map_err(|error| AppError::Serialization(error.to_string()))?;
            let bytes_written = write_text_file(&path, payload).await?;

            Ok(ExportWorkspaceResult {
                path: path.to_string_lossy().to_string(),
                format: input.format,
                collections_exported: 1,
                requests_exported: collection.requests.len() as u64,
                environments_exported: 0,
                history_exported: 0,
                secret_metadata_exported: 0,
                bytes_written,
            })
        }
    }
}

#[tauri::command]
pub async fn import_workspace_data(
    state: State<'_, AppState>,
    input: ImportWorkspaceInput,
) -> Result<ImportWorkspaceResult, CommandError> {
    let path = normalize_path(&input.path)?;
    let payload = read_payload_file(&path).await?;

    perform_workspace_import(
        &state,
        payload,
        input.format,
        input.merge,
        input.collection_name_override,
        path.to_string_lossy().to_string(),
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn import_workspace_payload(
    state: State<'_, AppState>,
    input: ImportWorkspacePayloadInput,
) -> Result<ImportWorkspaceResult, CommandError> {
    let source_label = input
        .source_label
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "payload pegado".to_string());

    perform_workspace_import(
        &state,
        input.payload,
        input.format,
        input.merge,
        input.collection_name_override,
        source_label,
    )
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn workspace_snapshot(
    state: State<'_, AppState>,
) -> Result<WorkspaceSnapshot, CommandError> {
    state
        .repository
        .workspace_snapshot(HISTORY_LIMIT)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    name: String,
) -> Result<CollectionSummary, CommandError> {
    state.repository.create_collection(name).await.map_err(Into::into)
}

#[tauri::command]
pub async fn save_request(
    state: State<'_, AppState>,
    input: SaveRequestInput,
) -> Result<SavedRequestRecord, CommandError> {
    state.repository.save_request(input).await.map_err(Into::into)
}

#[tauri::command]
pub async fn save_environment(
    state: State<'_, AppState>,
    input: SaveEnvironmentInput,
) -> Result<EnvironmentRecord, CommandError> {
    state
        .repository
        .save_environment(input)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn delete_environment(
    state: State<'_, AppState>,
    environment_id: String,
) -> Result<(), CommandError> {
    state
        .repository
        .delete_environment(environment_id)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn save_secret(
    state: State<'_, AppState>,
    input: SaveSecretInput,
) -> Result<SecretMetadata, CommandError> {
    let alias = input.alias.trim().to_string();

    if alias.is_empty() {
        return Err(CommandError {
            kind: "validation".to_string(),
            message: "El alias del secret no puede estar vacío.".to_string(),
        });
    }

    if input.value.trim().is_empty() {
        return Err(CommandError {
            kind: "validation".to_string(),
            message: "El valor del secret no puede estar vacío.".to_string(),
        });
    }

    state.secret_executor.set(alias.clone(), input.value).await?;
    state
        .repository
        .upsert_secret_metadata(alias)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn delete_secret(
    state: State<'_, AppState>,
    alias: String,
) -> Result<(), CommandError> {
    state.secret_executor.delete(alias.clone()).await?;
    state.repository.delete_secret_metadata(alias).await?;
    Ok(())
}

async fn perform_workspace_import(
    state: &AppState,
    payload_text: String,
    requested_format: WorkspaceImportFormat,
    merge: bool,
    collection_name_override: Option<String>,
    source_label: String,
) -> AppResult<ImportWorkspaceResult> {
    let payload = parse_json_or_yaml_payload(&payload_text)?;
    let detected_format = detect_import_format(&payload, requested_format)?;

    match detected_format {
        WorkspaceImportFormat::NativeWorkspaceV1 => {
            let bundle = parse_native_bundle(payload)?;
            let collection_ids = bundle
                .snapshot
                .collections
                .iter()
                .map(|collection| collection.collection.id.clone())
                .collect::<Vec<_>>();
            let requests_imported = bundle
                .snapshot
                .collections
                .iter()
                .map(|collection| collection.requests.len() as u64)
                .sum::<u64>();
            let collections_imported = bundle.snapshot.collections.len() as u64;
            let environments_imported = bundle.snapshot.environments.len() as u64;
            let history_imported = bundle.snapshot.history.len() as u64;
            let secret_metadata_imported = bundle.snapshot.secrets.len() as u64;

            state
                .repository
                .import_workspace_snapshot(bundle.snapshot, merge)
                .await?;

            Ok(ImportWorkspaceResult {
                path: source_label,
                detected_format,
                collections_imported,
                requests_imported,
                environments_imported,
                history_imported,
                secret_metadata_imported,
                collection_ids,
                warnings: if merge {
                    Vec::new()
                } else {
                    vec![
                        "Import nativo aplicado en modo replace: el workspace previo fue limpiado primero."
                            .to_string(),
                    ]
                },
            })
        }
        WorkspaceImportFormat::PostmanCollectionV21 => {
            let parsed = import_postman_collection(&payload)?;
            apply_imported_http_collection(
                state,
                parsed.collection_name,
                parsed.variables,
                parsed.requests,
                {
                    let mut warnings = parsed.warnings;
                    warnings.push(
                        "La importación Postman no convierte scripts/tests automáticamente; el runner y responseTests siguen siendo nativos de esta app.".to_string(),
                    );
                    warnings
                },
                source_label,
                detected_format,
                merge,
                collection_name_override,
            )
            .await
        }
        WorkspaceImportFormat::OpenApiV3 => {
            let parsed = import_openapi_document(&payload)?;
            apply_imported_http_collection(
                state,
                parsed.collection_name,
                parsed.variables,
                parsed.requests,
                parsed.warnings,
                source_label,
                detected_format,
                merge,
                collection_name_override,
            )
            .await
        }
        WorkspaceImportFormat::Auto => Err(AppError::Runtime(
            "El formato Auto debería resolverse antes del match.".to_string(),
        )),
    }
}

async fn apply_imported_http_collection(
    state: &AppState,
    default_collection_name: String,
    variables: Vec<crate::domain::http::KeyValueRow>,
    requests: Vec<RequestDraft>,
    mut warnings: Vec<String>,
    source_label: String,
    detected_format: WorkspaceImportFormat,
    merge: bool,
    collection_name_override: Option<String>,
) -> AppResult<ImportWorkspaceResult> {
    if !merge {
        state.repository.clear_workspace().await?;
    }

    let collection_name = collection_name_override
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(default_collection_name);
    let collection = state.repository.create_collection(collection_name).await?;
    let mut requests_imported = 0_u64;

    for draft in requests {
        state
            .repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                draft,
            })
            .await?;
        requests_imported += 1;
    }

    let environments_imported = if variables.is_empty() {
        0
    } else {
        state
            .repository
            .save_environment(SaveEnvironmentInput {
                environment_id: None,
                name: format!("{} vars", collection.name),
                variables,
            })
            .await?;
        1
    };

    if !merge {
        warnings.push(
            format!(
                "Import {} aplicado en modo replace: el workspace previo fue limpiado antes de crear la nueva collection.",
                match detected_format {
                    WorkspaceImportFormat::PostmanCollectionV21 => "Postman",
                    WorkspaceImportFormat::OpenApiV3 => "OpenAPI",
                    WorkspaceImportFormat::NativeWorkspaceV1 => "nativo",
                    WorkspaceImportFormat::Auto => "auto",
                }
            ),
        );
    }

    Ok(ImportWorkspaceResult {
        path: source_label,
        detected_format,
        collections_imported: 1,
        requests_imported,
        environments_imported,
        history_imported: 0,
        secret_metadata_imported: 0,
        collection_ids: vec![collection.id],
        warnings,
    })
}

fn snapshot_for_native_collection_export(
    snapshot: WorkspaceSnapshot,
    collection_id: &str,
    include_secret_metadata: bool,
) -> AppResult<WorkspaceSnapshot> {
    let WorkspaceSnapshot {
        collections,
        environments,
        history: _,
        secrets,
    } = snapshot;

    let selected_collection = collections
        .into_iter()
        .find(|collection| collection.collection.id == collection_id)
        .ok_or_else(|| AppError::NotFound(format!("No existe la collection {}", collection_id)))?;

    let environment_ids = selected_collection
        .requests
        .iter()
        .filter_map(|record| record.draft.environment_id.clone())
        .collect::<BTreeSet<_>>();

    let selected_environments = environments
        .into_iter()
        .filter(|environment| environment_ids.contains(&environment.id))
        .collect::<Vec<_>>();

    let selected_secrets = if include_secret_metadata {
        let environment_lookup = selected_environments
            .iter()
            .map(|environment| (environment.id.clone(), environment.variables.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut aliases = BTreeSet::new();

        for record in &selected_collection.requests {
            let environment_rows = record
                .draft
                .environment_id
                .as_ref()
                .and_then(|environment_id| environment_lookup.get(environment_id))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            aliases.extend(collect_secret_aliases(&record.draft, environment_rows));
        }

        secrets
            .into_iter()
            .filter(|metadata| aliases.contains(&metadata.alias))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    Ok(WorkspaceSnapshot {
        collections: vec![selected_collection],
        environments: selected_environments,
        history: Vec::new(),
        secrets: selected_secrets,
    })
}

async fn load_environment_record(
    state: &AppState,
    environment_id: Option<&str>,
) -> AppResult<Option<EnvironmentRecord>> {
    let Some(environment_id) = environment_id else {
        return Ok(None);
    };

    let environment = state.repository.get_environment_by_id(environment_id).await?;

    match environment {
        Some(environment) => Ok(Some(environment)),
        None => Err(AppError::NotFound(format!(
            "No existe el environment seleccionado: {environment_id}"
        ))),
    }
}

async fn execute_draft_with_environment(
    state: &AppState,
    draft: &RequestDraft,
    environment: Option<&EnvironmentRecord>,
    execution_id: String,
) -> AppResult<(RequestExecutionOutcome, String, Option<String>)> {
    let environment_name = environment.as_ref().map(|env| env.name.clone());
    let environment_rows = environment
        .as_ref()
        .map(|env| env.variables.as_slice())
        .unwrap_or(&[]);

    let secrets = load_secrets_for_request(state, draft, environment_rows).await?;
    let resolution = resolve_request(
        draft,
        environment_name.clone(),
        environment_rows,
        &secrets,
        SecretRenderMode::Resolve,
    )?;
    let resolved_url = resolution.request.url.clone();
    let response = state
        .request_executor
        .execute(execution_id, resolution.request)
        .await?;
    let assertion_report = evaluate_response_assertions(&response, &draft.response_tests);

    Ok((
        RequestExecutionOutcome {
            response,
            assertion_report,
        },
        resolved_url,
        environment_name,
    ))
}

async fn load_secrets_for_request(
    state: &AppState,
    draft: &RequestDraft,
    environment_rows: &[crate::domain::http::KeyValueRow],
) -> AppResult<BTreeMap<String, String>> {
    let aliases = collect_secret_aliases(draft, environment_rows);
    let mut secrets = BTreeMap::new();

    for alias in aliases {
        if let Some(value) = state.secret_executor.get(alias.clone()).await? {
            secrets.insert(alias, value);
        }
    }

    Ok(secrets)
}

fn emit_collection_progress(app: &AppHandle, payload: CollectionRunProgressEvent) {
    let _ = app.emit_to("main", COLLECTION_RUN_PROGRESS_EVENT, payload);
}

fn normalize_path(path: &str) -> AppResult<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "Necesitás indicar una ruta de archivo.".to_string(),
        ));
    }
    Ok(PathBuf::from(trimmed))
}

async fn write_text_file(path: &PathBuf, contents: String) -> AppResult<u64> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| AppError::Io(error.to_string()))?;
    }

    tokio::fs::write(path, contents.as_bytes())
        .await
        .map_err(|error| AppError::Io(error.to_string()))?;

    Ok(contents.as_bytes().len() as u64)
}

async fn read_payload_file(path: &PathBuf) -> AppResult<String> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(|error| AppError::Io(error.to_string()))
}
