//! `Collection_Runner` (Fase 4): ejecución secuencial de los requests
//! guardados de una collection.
//!
//! Puerto de la lógica de orquestación del comando `run_collection` de
//! `src-tauri/src/commands/mod.rs` (Tarea 9.1, Requisito 5.1), reutilizando
//! `domain::runner` (`CollectionRunReport`, `CollectionRunItem`,
//! `RequestExecutionOutcome`, `RunCollectionInput`) sin reescritura de su
//! lógica.
//!
//! Alcance de esta tarea: cargar la collection con sus requests, ejecutarlos
//! en el orden en que están guardados (el orden ya devuelto por
//! `SqliteRepository::get_collection_with_requests`) y acumular el reporte
//! consolidado final, reportando además el progreso incremental (Tarea 9.2,
//! Requisito 5.2) mediante un canal `mpsc` (`iced::futures::channel::mpsc`,
//! consumido en `midway-desktop::app` por una `iced::subscription`) en lugar
//! de `app.emit_to(..., COLLECTION_RUN_PROGRESS_EVENT, ...)`. Los cuatro
//! puntos de emisión (inicio de la ejecución, inicio de cada request, fin
//! de cada request, fin de la ejecución) y la forma exacta del payload de
//! cada evento (`CollectionRunProgressEvent`) replican los del comando
//! Tauri original.
//!
//! Cubre además el manejo de fallos por request (Tarea 9.4), el caso
//! explícito de colección vacía (Tarea 9.5, que ya funciona de forma
//! natural con este bucle: con `total_requests == 0` se emiten únicamente
//! los eventos `Started`/`Finished`) y la cancelación de una ejecución en
//! curso (Tarea 9.6, Requisito 5.8) mediante un `oneshot::Receiver<()>`
//! comprobado al inicio de cada iteración.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::Utc;
use iced::futures::channel::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

use midway_core::app::errors::{AppError, AppResult};
use midway_core::domain::http::RequestDraft;
use midway_core::domain::interpolation::{resolve_request, SecretRenderMode};
use midway_core::domain::runner::{
    CollectionRunItem, CollectionRunPhase, CollectionRunProgressEvent, CollectionRunReport,
    RequestExecutionOutcome, RunCollectionInput,
};
use midway_core::domain::secrets::collect_secret_aliases;
use midway_core::domain::testing::evaluate_response_assertions;
use midway_core::domain::workspace::EnvironmentRecord;

use crate::state::AppState;

/// Envía `event` por el canal de progreso, ignorando el error si el extremo
/// receptor ya fue soltado (por ejemplo, porque la `iced::subscription` que
/// lo consumía ya no está activa). Espejo de `emit_collection_progress` de
/// `src-tauri/src/commands/mod.rs`, que de la misma forma ignora el
/// resultado de `app.emit_to`.
fn emit_progress(
    progress_tx: &mpsc::UnboundedSender<CollectionRunProgressEvent>,
    event: CollectionRunProgressEvent,
) {
    let _ = progress_tx.unbounded_send(event);
}

/// Ejecuta todos los requests guardados de la collection indicada por
/// `input.collection_id`, en el orden en que están guardados, acumulando un
/// `CollectionRunReport` (Tarea 9.1, Requisito 5.1) y reportando progreso
/// incremental por `progress_tx` en cada uno de los cuatro puntos de la
/// ejecución (Tarea 9.2, Requisito 5.2): inicio de la ejecución, inicio de
/// cada request, fin de cada request (éxito o error) y fin de la ejecución.
///
/// Puerto directo de `run_collection` de `src-tauri/src/commands/mod.rs`,
/// sustituyendo `app.emit_to` por el envío al canal `progress_tx`. Con
/// `collection.requests` vacío, el bucle no itera y se devuelve de
/// inmediato un reporte con cero requests ejecutados tras emitir únicamente
/// `Started` y `Finished` (comportamiento base del Criterio 5.7, refinado en
/// la Tarea 9.5).
///
/// Cancelación (Tarea 9.6, Requisito 5.8): `cancel_rx` recibe una señal
/// (`()`) enviada desde el `oneshot::Sender<()>` correspondiente almacenado
/// en `CollectionRunnerState` (`midway_desktop::app`), análogo al
/// `oneshot::Sender<()>` que `RequestExecutorHandle` usa por cada request
/// en curso. Dado que los requests de una collection se ejecutan
/// secuencialmente (no multiplexados), un único cancel token por ejecución
/// basta. La señal se comprueba (`try_recv`) al INICIO de cada iteración,
/// antes de arrancar el siguiente request: si ya llegó, el bucle se
/// interrumpe de inmediato sin esperar ni incluir el resultado de ningún
/// request adicional, de forma que el reporte consolidado final refleja
/// únicamente los requests que ya habían completado antes de observarse la
/// cancelación. Una ejecución cancelada no es un `AppError`: se sigue
/// emitiendo el evento `Finished` (con los contadores tal como quedaron en
/// el momento de la cancelación) y se devuelve `Ok(...)` con un reporte
/// parcial, pues un reporte incompleto es, igualmente, un reporte válido.
pub async fn run_collection(
    app_state: Arc<AppState>,
    input: RunCollectionInput,
    progress_tx: mpsc::UnboundedSender<CollectionRunProgressEvent>,
    mut cancel_rx: oneshot::Receiver<()>,
) -> AppResult<CollectionRunReport> {
    let collection = app_state
        .repository
        .get_collection_with_requests(&input.collection_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("No existe la collection {}", input.collection_id))
        })?;

    let override_environment =
        load_environment_record(&app_state, input.environment_override_id.as_deref()).await?;

    let run_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    let total_requests = collection.requests.len() as u64;

    let mut items = Vec::with_capacity(collection.requests.len());
    let mut completed_requests = 0_u64;
    let mut errored_requests = 0_u64;
    let mut passed_assertions = 0_u64;
    let mut failed_assertions = 0_u64;

    emit_progress(
        &progress_tx,
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
        // Comprobación de cancelación (Tarea 9.6, Requisito 5.8): se chequea
        // al inicio de cada iteración, antes de arrancar el siguiente
        // request, de forma que el reporte consolidado final refleje
        // únicamente los requests ya completados hasta este punto.
        if cancel_rx.try_recv().is_ok() {
            break;
        }

        let effective_environment = match override_environment.as_ref() {
            Some(environment) => Some(environment.clone()),
            None => load_environment_record(&app_state, record.draft.environment_id.as_deref()).await?,
        };

        let environment_name = effective_environment.as_ref().map(|environment| environment.name.clone());
        let current_index = index as u64 + 1;
        let executed_at = Utc::now().to_rfc3339();

        emit_progress(
            &progress_tx,
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
            &app_state,
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

                let final_environment_name = used_environment_name.or(environment_name);

                let _ = app_state
                    .repository
                    .append_history(
                        &record.draft,
                        final_environment_name.clone(),
                        resolved_url.clone(),
                        Some(response.clone()),
                        None,
                    )
                    .await;

                emit_progress(
                    &progress_tx,
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
                        environment_name: final_environment_name.clone(),
                        resolved_url: Some(resolved_url.clone()),
                        response_status: Some(response_status),
                        duration_ms: Some(duration_ms),
                        error_message: None,
                        started_at: started_at.clone(),
                        finished_at: None,
                        emitted_at: Utc::now().to_rfc3339(),
                    },
                );

                items.push(CollectionRunItem {
                    request_id: record.id.clone(),
                    request_name: record.name.clone(),
                    environment_name: final_environment_name,
                    resolved_url: Some(resolved_url),
                    response_status: Some(response_status),
                    duration_ms: Some(duration_ms),
                    error_message: None,
                    assertion_report: Some(assertion_report),
                    executed_at,
                });
            }
            Err(error) => {
                errored_requests += 1;
                let message = error.to_string();

                let _ = app_state
                    .repository
                    .append_history(
                        &record.draft,
                        environment_name.clone(),
                        record.draft.url.clone(),
                        None,
                        Some(message.clone()),
                    )
                    .await;

                emit_progress(
                    &progress_tx,
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

                items.push(CollectionRunItem {
                    request_id: record.id.clone(),
                    request_name: record.name.clone(),
                    environment_name,
                    resolved_url: None,
                    response_status: None,
                    duration_ms: None,
                    error_message: Some(message),
                    assertion_report: None,
                    executed_at,
                });
            }
        }
    }

    let finished_at = Utc::now().to_rfc3339();

    emit_progress(
        &progress_tx,
        CollectionRunProgressEvent {
            run_id,
            phase: CollectionRunPhase::Finished,
            collection_id: collection.collection.id.clone(),
            collection_name: collection.collection.name.clone(),
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
        collection_id: collection.collection.id,
        collection_name: collection.collection.name,
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

/// Carga el `EnvironmentRecord` indicado por `environment_id`, si está
/// presente. Puerto directo de `load_environment_record` de
/// `src-tauri/src/commands/mod.rs`.
async fn load_environment_record(
    app_state: &AppState,
    environment_id: Option<&str>,
) -> AppResult<Option<EnvironmentRecord>> {
    let Some(environment_id) = environment_id else {
        return Ok(None);
    };

    let environment = app_state.repository.get_environment_by_id(environment_id).await?;

    match environment {
        Some(environment) => Ok(Some(environment)),
        None => Err(AppError::NotFound(format!(
            "No existe el environment seleccionado: {environment_id}"
        ))),
    }
}

/// Carga los secrets necesarios, resuelve el draft (interpolación + auth) y
/// ejecuta la request HTTP resultante, evaluando además las assertions
/// configuradas. Puerto directo de `execute_draft_with_environment` /
/// `load_secrets_for_request` de `src-tauri/src/commands/mod.rs` (misma
/// lógica que `execute_send` en `app.rs` para el flujo de Send).
async fn execute_draft_with_environment(
    app_state: &AppState,
    draft: &RequestDraft,
    environment: Option<&EnvironmentRecord>,
    execution_id: String,
) -> AppResult<(RequestExecutionOutcome, String, Option<String>)> {
    let environment_name = environment.map(|environment| environment.name.clone());
    let environment_rows = environment
        .map(|environment| environment.variables.as_slice())
        .unwrap_or(&[]);

    let aliases = collect_secret_aliases(draft, environment_rows);
    let mut secrets = BTreeMap::new();
    for alias in aliases {
        if let Some(value) = app_state.secret_executor.get(alias.clone()).await? {
            secrets.insert(alias, value);
        }
    }

    let resolution = resolve_request(
        draft,
        environment_name.clone(),
        environment_rows,
        &secrets,
        SecretRenderMode::Resolve,
    )?;
    let resolved_url = resolution.request.url.clone();

    let response = app_state
        .request_executor
        .execute(execution_id, resolution.request)
        .await?;
    let assertion_report = evaluate_response_assertions(&response, &draft.response_tests);

    Ok((
        RequestExecutionOutcome { response, assertion_report },
        resolved_url,
        resolution.environment_name.or(environment_name),
    ))
}

#[cfg(test)]
mod tests {
    //! Feature: tauri-to-iced-migration, Tarea 9.2 (Requisito 5.2).
    //!
    //! Verifica el mecanismo de progreso incremental de `run_collection`
    //! (envío por `mpsc::UnboundedSender<CollectionRunProgressEvent>`) contra
    //! un `AppState` real (`SqliteRepository` en un archivo SQLite temporal
    //! + `RequestExecutorHandle` real) y un servidor HTTP mock local
    //! (`wiremock`), sin mockear `run_collection` en sí. No repite la
    //! cobertura de las fases 9.3-9.6 (reporte consolidado detallado,
    //! environment de override, fallos por request, colección vacía,
    //! cancelación), que se cubre en tareas posteriores (Property 18, Tarea
    //! 9.7); este test cubre específicamente que el canal recibe los cuatro
    //! eventos de progreso (`Started`, `RequestStarted`, `RequestFinished`,
    //! `Finished`) en el orden correcto para una collection con más de un
    //! request, con `total_requests`/`current_index`/`completed_requests`
    //! consistentes en cada evento.

    use super::*;
    use midway_core::domain::cookies::CookieJarHandle;
    use midway_core::domain::http::HttpMethod;
    use midway_core::domain::testing::{AssertionOperator, AssertionSource, ResponseAssertion};
    use midway_core::domain::workspace::SaveRequestInput;
    use midway_core::infra::sqlite_repository::SqliteRepository;
    use midway_core::runtime::request_executor::RequestExecutorHandle;
    use midway_core::runtime::secret_executor::SecretExecutorHandle;
    use proptest::prelude::*;
    use proptest::test_runner::TestCaseError;
    use wiremock::matchers::method as http_method_matcher;
    use wiremock::matchers::path as wiremock_path_matcher;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn build_test_app_state() -> AppState {
        let temp_file = tempfile::NamedTempFile::new()
            .expect("no se pudo crear un archivo temporal para el AppState de prueba");
        let db_path = temp_file.path().to_path_buf();

        let repository = SqliteRepository::open(&db_path)
            .await
            .expect("no se pudo abrir el SqliteRepository de prueba");

        let client = reqwest::Client::builder()
            .build()
            .expect("no se pudo construir el cliente reqwest de prueba");

        let app_state = AppState {
            repository,
            request_executor: RequestExecutorHandle::spawn(client),
            secret_executor: SecretExecutorHandle::spawn("midway-test-collection-runner".to_string()),
            cookie_jar: CookieJarHandle::new(),
        };

        drop(temp_file);
        app_state
    }

    fn blank_draft_for(url: String) -> RequestDraft {
        let mut draft = crate::curl::create_blank_draft();
        draft.method = HttpMethod::GET;
        draft.url = url;
        draft
    }

    /// Con una collection de 2 requests, `run_collection` SHALL emitir por
    /// `progress_tx`, en orden, exactamente: `Started`, luego para cada
    /// request `RequestStarted` seguido de `RequestFinished`, y finalmente
    /// `Finished`; con contadores (`total_requests`, `current_index`,
    /// `completed_requests`) consistentes con la posición de cada request
    /// en la secuencia (Tarea 9.2, Requisito 5.2).
    #[tokio::test]
    async fn run_collection_emits_progress_events_in_expected_phase_order() {
        let mock_server = MockServer::start().await;
        Mock::given(http_method_matcher("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let app_state = Arc::new(build_test_app_state().await);

        let collection = app_state
            .repository
            .create_collection("Progress test collection".to_string())
            .await
            .expect("no se pudo crear la collection de prueba");

        for path in ["/a", "/b"] {
            app_state
                .repository
                .save_request(SaveRequestInput {
                    request_id: None,
                    collection_id: collection.id.clone(),
                    folder_id: None,
                    draft: blank_draft_for(format!("{}{}", mock_server.uri(), path)),
                })
                .await
                .expect("no se pudo guardar el request de prueba");
        }

        let (progress_tx, mut progress_rx) = mpsc::unbounded();
        let (_cancel_tx, cancel_rx) = oneshot::channel();

        let report = run_collection(
            Arc::clone(&app_state),
            RunCollectionInput {
                collection_id: collection.id.clone(),
                environment_override_id: None,
                stop_on_error: false,
            },
            progress_tx,
            cancel_rx,
        )
        .await
        .expect("run_collection debería completar exitosamente contra el mock server");

        assert_eq!(report.total_requests, 2);
        assert_eq!(report.completed_requests, 2);
        assert_eq!(report.errored_requests, 0);

        // El `Sender` fue movido dentro de `run_collection` y se soltó al
        // finalizar, así que el canal ya está cerrado: se puede drenar por
        // completo con `try_recv` sin bloquear.
        let mut events = Vec::new();
        while let Ok(event) = progress_rx.try_recv() {
            events.push(event);
        }

        let phases: Vec<CollectionRunPhase> = events.iter().map(|event| event.phase).collect();
        assert!(
            matches!(
                phases.as_slice(),
                [
                    CollectionRunPhase::Started,
                    CollectionRunPhase::RequestStarted,
                    CollectionRunPhase::RequestFinished,
                    CollectionRunPhase::RequestStarted,
                    CollectionRunPhase::RequestFinished,
                    CollectionRunPhase::Finished,
                ]
            ),
            "orden de fases inesperado: {phases:?}"
        );

        // `Started`: sin progreso todavía.
        assert_eq!(events[0].total_requests, 2);
        assert_eq!(events[0].current_index, 0);
        assert_eq!(events[0].completed_requests, 0);
        assert_eq!(events[0].finished_at, None);

        // Primer request: `RequestStarted` (índice 1, 0 completados
        // todavía) seguido de `RequestFinished` (índice 1, 1 completado).
        assert_eq!(events[1].current_index, 1);
        assert_eq!(events[1].completed_requests, 0);
        assert_eq!(events[2].current_index, 1);
        assert_eq!(events[2].completed_requests, 1);
        assert_eq!(events[2].response_status, Some(200));

        // Segundo request: análogo, con índice 2.
        assert_eq!(events[3].current_index, 2);
        assert_eq!(events[3].completed_requests, 1);
        assert_eq!(events[4].current_index, 2);
        assert_eq!(events[4].completed_requests, 2);
        assert_eq!(events[4].response_status, Some(200));

        // `Finished`: refleja el total final y trae `finished_at`.
        assert_eq!(events[5].completed_requests, 2);
        assert!(events[5].finished_at.is_some());
    }

    /// Con una collection sin requests guardados (N=0), `run_collection`
    /// SHALL emitir únicamente `Started` seguido de `Finished`, sin ningún
    /// evento `RequestStarted`/`RequestFinished` intermedio y sin realizar
    /// ninguna llamada HTTP (comportamiento base del Criterio 5.7, ver
    /// también Tarea 9.5).
    #[tokio::test]
    async fn run_collection_with_no_requests_emits_only_started_and_finished() {
        let app_state = Arc::new(build_test_app_state().await);

        let collection = app_state
            .repository
            .create_collection("Empty progress test collection".to_string())
            .await
            .expect("no se pudo crear la collection de prueba");

        let (progress_tx, mut progress_rx) = mpsc::unbounded();
        let (_cancel_tx, cancel_rx) = oneshot::channel();

        let report = run_collection(
            Arc::clone(&app_state),
            RunCollectionInput {
                collection_id: collection.id.clone(),
                environment_override_id: None,
                stop_on_error: false,
            },
            progress_tx,
            cancel_rx,
        )
        .await
        .expect("run_collection debería completar exitosamente sin requests");

        assert_eq!(report.total_requests, 0);
        assert_eq!(report.completed_requests, 0);
        assert_eq!(report.errored_requests, 0);
        assert_eq!(report.passed_assertions, 0);
        assert_eq!(report.failed_assertions, 0);
        assert!(report.items.is_empty());

        let mut events = Vec::new();
        while let Ok(event) = progress_rx.try_recv() {
            events.push(event);
        }

        let phases: Vec<CollectionRunPhase> = events.iter().map(|event| event.phase).collect();
        assert!(
            matches!(phases.as_slice(), [CollectionRunPhase::Started, CollectionRunPhase::Finished]),
            "orden de fases inesperado: {phases:?}"
        );

        // Ningún evento intermedio referencia un request: no hubo ninguna
        // llamada HTTP (no existe ningún request guardado del cual derivar
        // una URL a la cual llamar).
        assert!(events.iter().all(|event| event.request_id.is_none()));
    }

    /// Feature: tauri-to-iced-migration, Tarea 9.4 (Requisito 5.5).
    ///
    /// Con una collection de 3 requests donde el primero tiene éxito HTTP
    /// pero falla una assertion, el segundo falla al nivel de la llamada
    /// HTTP (error de red/conexión) y el tercero tiene éxito HTTP y pasa su
    /// assertion, `run_collection` SHALL ejecutar los tres requests sin
    /// detener la secuencia ante ningún fallo, registrando cada fallo junto
    /// con su motivo en el reporte consolidado:
    /// - El request con assertion fallida SHALL contarse como completado
    ///   (no como errored), con su `assertion_report` reflejando el fallo
    ///   (`failed > 0`) en el item correspondiente.
    /// - El request con fallo de red SHALL contarse como errored, con
    ///   `error_message` poblado y sin `assertion_report` en su item.
    /// - El tercer request SHALL ejecutarse con normalidad, demostrando que
    ///   la secuencia continuó tras ambos fallos anteriores.
    #[tokio::test]
    async fn run_collection_continues_after_network_error_and_assertion_failure() {
        let mock_server = MockServer::start().await;
        Mock::given(http_method_matcher("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let app_state = Arc::new(build_test_app_state().await);

        let collection = app_state
            .repository
            .create_collection("Mixed outcomes test collection".to_string())
            .await
            .expect("no se pudo crear la collection de prueba");

        // Request 1: éxito HTTP, assertion fallida (status esperado 404,
        // pero el mock responde 200).
        //
        // Se le da a cada draft un `name` distinto (en vez de dejar el
        // nombre por defecto de `blank_draft_for`) porque
        // `get_collection_with_requests` ordena por `updated_at DESC, name
        // ASC`: con el mismo nombre por defecto y guardados dentro del
        // mismo segundo, el orden entre ellos no sería determinístico. Los
        // items del reporte se identifican más abajo por `request_name`,
        // no por posición, para no depender de los detalles de ese orden.
        let mut failing_assertion_draft = blank_draft_for(format!("{}/ok-but-assertion-fails", mock_server.uri()));
        failing_assertion_draft.name = "1 - assertion fallida".to_string();
        failing_assertion_draft.response_tests.push(ResponseAssertion {
            id: "assertion-1".to_string(),
            name: "status debería ser 404".to_string(),
            enabled: true,
            source: AssertionSource::Status,
            operator: AssertionOperator::Equals,
            selector: None,
            expected: "404".to_string(),
        });
        app_state
            .repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: None,
                draft: failing_assertion_draft,
            })
            .await
            .expect("no se pudo guardar el request con assertion fallida");

        // Request 2: fallo de red (puerto sin nada escuchando, timeout
        // corto para mantener el test rápido y determinístico).
        let mut network_error_draft = blank_draft_for("http://127.0.0.1:1/unreachable".to_string());
        network_error_draft.name = "2 - fallo de red".to_string();
        network_error_draft.timeout_ms = 2_000;
        app_state
            .repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: None,
                draft: network_error_draft,
            })
            .await
            .expect("no se pudo guardar el request con fallo de red");

        // Request 3: éxito HTTP y assertion exitosa (status esperado 200).
        let mut succeeding_draft = blank_draft_for(format!("{}/ok", mock_server.uri()));
        succeeding_draft.name = "3 - exito".to_string();
        succeeding_draft.response_tests.push(ResponseAssertion {
            id: "assertion-2".to_string(),
            name: "status debería ser 200".to_string(),
            enabled: true,
            source: AssertionSource::Status,
            operator: AssertionOperator::Equals,
            selector: None,
            expected: "200".to_string(),
        });
        app_state
            .repository
            .save_request(SaveRequestInput {
                request_id: None,
                collection_id: collection.id.clone(),
                folder_id: None,
                draft: succeeding_draft,
            })
            .await
            .expect("no se pudo guardar el tercer request");

        let (progress_tx, _progress_rx) = mpsc::unbounded();
        let (_cancel_tx, cancel_rx) = oneshot::channel();

        let report = run_collection(
            Arc::clone(&app_state),
            RunCollectionInput {
                collection_id: collection.id.clone(),
                environment_override_id: None,
                stop_on_error: false,
            },
            progress_tx,
            cancel_rx,
        )
        .await
        .expect("run_collection debería completar los 3 requests sin detenerse");

        // Los 3 requests se ejecutaron: la secuencia no se detuvo ante
        // ningún fallo (Requisito 5.5).
        assert_eq!(report.total_requests, 3);
        assert_eq!(report.items.len(), 3);

        // El request con assertion fallida cuenta como completado (no como
        // errored): fallar una assertion no es un error de ejecución HTTP.
        // El request con fallo de red cuenta como errored.
        assert_eq!(report.completed_requests, 2);
        assert_eq!(report.errored_requests, 1);
        assert_eq!(report.failed_assertions, 1);
        assert_eq!(report.passed_assertions, 1);

        let find_item = |name: &str| {
            report
                .items
                .iter()
                .find(|item| item.request_name == name)
                .unwrap_or_else(|| panic!("no se encontró el item para el request \"{name}\""))
        };

        let assertion_failure_item = find_item("1 - assertion fallida");
        assert!(assertion_failure_item.error_message.is_none());
        assert_eq!(assertion_failure_item.response_status, Some(200));
        let assertion_report = assertion_failure_item
            .assertion_report
            .as_ref()
            .expect("el item con assertion fallida debería traer su assertion_report");
        assert_eq!(assertion_report.failed, 1);
        assert_eq!(assertion_report.passed, 0);

        let network_error_item = find_item("2 - fallo de red");
        assert!(network_error_item.error_message.is_some());
        assert!(network_error_item.response_status.is_none());
        assert!(network_error_item.assertion_report.is_none());

        let success_item = find_item("3 - exito");
        assert!(success_item.error_message.is_none());
        assert_eq!(success_item.response_status, Some(200));
        let success_assertion_report = success_item
            .assertion_report
            .as_ref()
            .expect("el tercer item debería traer su assertion_report");
        assert_eq!(success_assertion_report.passed, 1);
        assert_eq!(success_assertion_report.failed, 0);
    }

    /// Feature: tauri-to-iced-migration, Tarea 9.6 (Requisito 5.8).
    ///
    /// Si la cancelación ya se envió ANTES de llamar a `run_collection`
    /// (caso límite: el `oneshot::Sender<()>` envía `()` antes de que el
    /// bucle procese la primera iteración), la comprobación de
    /// `cancel_rx.try_recv()` al inicio de la primera iteración SHALL
    /// detectarla de inmediato: no se ejecuta ningún request, el reporte
    /// consolidado queda vacío (`items` vacío, `completed_requests` y
    /// `errored_requests` en 0) mientras que `total_requests` SHALL seguir
    /// reflejando el tamaño completo de la collection.
    #[tokio::test]
    async fn run_collection_cancelled_before_starting_produces_empty_report() {
        let mock_server = MockServer::start().await;
        Mock::given(http_method_matcher("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let app_state = Arc::new(build_test_app_state().await);

        let collection = app_state
            .repository
            .create_collection("Cancelled before starting test collection".to_string())
            .await
            .expect("no se pudo crear la collection de prueba");

        for path in ["/a", "/b", "/c"] {
            app_state
                .repository
                .save_request(SaveRequestInput {
                    request_id: None,
                    collection_id: collection.id.clone(),
                    folder_id: None,
                    draft: blank_draft_for(format!("{}{}", mock_server.uri(), path)),
                })
                .await
                .expect("no se pudo guardar el request de prueba");
        }

        let (progress_tx, mut progress_rx) = mpsc::unbounded();
        let (cancel_tx, cancel_rx) = oneshot::channel();
        // Se cancela antes de siquiera invocar `run_collection`.
        cancel_tx.send(()).expect("no se pudo enviar la señal de cancelación");

        let report = run_collection(
            Arc::clone(&app_state),
            RunCollectionInput {
                collection_id: collection.id.clone(),
                environment_override_id: None,
                stop_on_error: false,
            },
            progress_tx,
            cancel_rx,
        )
        .await
        .expect("una ejecución cancelada SHALL devolver Ok con un reporte parcial, no un AppError");

        assert_eq!(report.total_requests, 3);
        assert!(report.items.is_empty());
        assert_eq!(report.completed_requests, 0);
        assert_eq!(report.errored_requests, 0);
        assert!(report.completed_requests + report.errored_requests < report.total_requests);

        let mut events = Vec::new();
        while let Ok(event) = progress_rx.try_recv() {
            events.push(event);
        }

        // Únicamente `Started` y `Finished`: ningún request llegó a
        // arrancar (comparable a una collection vacía en cuanto a fases
        // emitidas, aunque aquí `total_requests` sí refleja los 3 guardados).
        let phases: Vec<CollectionRunPhase> = events.iter().map(|event| event.phase).collect();
        assert!(
            matches!(phases.as_slice(), [CollectionRunPhase::Started, CollectionRunPhase::Finished]),
            "orden de fases inesperado: {phases:?}"
        );
        assert!(events.last().unwrap().finished_at.is_some());
    }

    /// Feature: tauri-to-iced-migration, Tarea 9.6 (Requisito 5.8).
    ///
    /// Con una collection de 4 requests cuyo mock responde con un retraso
    /// (`ResponseTemplate::set_delay`) suficientemente mayor al de una tarea
    /// concurrente que envía la cancelación poco después de arrancar
    /// `run_collection`, la cancelación SHALL observarse recién al inicio de
    /// la segunda iteración del bucle (justo después de que el primer
    /// request ya completó): el reporte consolidado SHALL contener
    /// únicamente el item del primer request (`items.len() == 1`,
    /// `completed_requests == 1`), con `total_requests` reflejando
    /// igualmente el tamaño completo de la collection (4) y
    /// `completed_requests + errored_requests < total_requests`. El retraso
    /// del mock hace que la ventana entre "la cancelación ya se envió" y
    /// "el bucle llega a comprobarla" sea holgada, evitando que el test
    /// dependa de una carrera ajustada entre tareas concurrentes.
    #[tokio::test]
    async fn run_collection_cancelled_mid_sequence_reports_only_completed_items() {
        let mock_server = MockServer::start().await;
        Mock::given(http_method_matcher("GET"))
            .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(200)))
            .mount(&mock_server)
            .await;

        let app_state = Arc::new(build_test_app_state().await);

        let collection = app_state
            .repository
            .create_collection("Cancelled mid-sequence test collection".to_string())
            .await
            .expect("no se pudo crear la collection de prueba");

        for (index, path) in ["/a", "/b", "/c", "/d"].into_iter().enumerate() {
            let mut draft = blank_draft_for(format!("{}{}", mock_server.uri(), path));
            draft.name = format!("{index} - request");
            app_state
                .repository
                .save_request(SaveRequestInput {
                    request_id: None,
                    collection_id: collection.id.clone(),
                    folder_id: None,
                    draft,
                })
                .await
                .expect("no se pudo guardar el request de prueba");
        }

        let (progress_tx, mut progress_rx) = mpsc::unbounded::<CollectionRunProgressEvent>();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        // Tarea concurrente: envía la cancelación casi de inmediato (mucho
        // antes de que el primer request, retrasado 200ms por el mock,
        // termine). Como la comprobación de `cancel_rx` ocurre únicamente
        // al INICIO de cada iteración, esto no aborta el primer request en
        // curso: solo garantiza que la señal ya esté disponible para la
        // comprobación del inicio de la segunda iteración.
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            let _ = cancel_tx.send(());
        });

        let report = run_collection(
            Arc::clone(&app_state),
            RunCollectionInput {
                collection_id: collection.id.clone(),
                environment_override_id: None,
                stop_on_error: false,
            },
            progress_tx,
            cancel_rx,
        )
        .await
        .expect("una ejecución cancelada SHALL devolver Ok con un reporte parcial, no un AppError");

        assert_eq!(report.total_requests, 4);
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.completed_requests, 1);
        assert_eq!(report.errored_requests, 0);
        assert!(report.completed_requests + report.errored_requests < report.total_requests);
        // No se asume un orden específico entre los 4 requests guardados
        // (`get_collection_with_requests` ordena por `updated_at DESC, name
        // ASC`, y el orden exacto entre ellos no es relevante para esta
        // propiedad): solo importa que se completó exactamente uno de los
        // cuatro nombres guardados.
        let completed_names: std::collections::HashSet<&str> =
            ["0 - request", "1 - request", "2 - request", "3 - request"].into_iter().collect();
        assert!(completed_names.contains(report.items[0].request_name.as_str()));

        // El primer request SHALL haber completado normalmente antes de la
        // cancelación: su `RequestFinished` fue emitido.
        let mut events = Vec::new();
        while let Ok(event) = progress_rx.try_recv() {
            events.push(event);
        }
        assert!(events
            .iter()
            .any(|event| matches!(event.phase, CollectionRunPhase::RequestFinished)));
        assert!(matches!(events.last().unwrap().phase, CollectionRunPhase::Finished));
    }

    // -----------------------------------------------------------------
    // Tarea 9.7 (Property 18, Requisitos 5.1, 5.2, 5.3, 5.5, 5.7, 5.8):
    // ejecución secuencial y reporte consolidado del Collection_Runner,
    // contra un mock HTTP local (`wiremock`) parametrizado por una
    // secuencia generada de resultados simulados por request y un punto
    // de cancelación k.
    //
    // Decisión de diseño: en lugar de introducir un seam de inyección de
    // mock para `RequestExecutorHandle` (que no es un trait hoy, ver
    // `midway_core::runtime::request_executor`, y cuyo refactor sería
    // invasivo sobre código de las Tareas 9.1-9.6 ya probado), esta
    // property usa un `MockServer` real pero local (loopback, sin
    // latencia de red externa) con dos rutas fijas ("/success" y
    // "/timeout") y un retraso uniforme (`SIMULATED_DELAY_MS`) para
    // TODOS los resultados simulados (éxito, éxito con assertion,
    // assertion fallida, fallo de red), de forma que el tiempo de pared
    // por request sea aproximadamente constante sin importar el
    // resultado. Esto permite:
    // - Mantener el costo por caso acotado (loopback, delay pequeño), y
    // - Determinar de forma DETERMINÍSTICA (no basada en temporización
    //   de reloj de pared) en qué momento cancelar (Requisito 5.8): para
    //   un punto k > 0, una tarea concurrente consume `progress_rx`
    //   evento por evento (`StreamExt::next`) y envía la señal de
    //   cancelación en cuanto observa el k-ésimo evento
    //   `RequestFinished` (`current_index == k`), justo cuando el
    //   request `k-1` (0-indexado) terminó de completarse. Como
    //   `run_collection` comprueba `cancel_rx` de forma síncrona al
    //   inicio de la siguiente iteración, esto garantiza que la señal
    //   llegue después de que el request `k-1` complete y antes de que
    //   arranque el request `k`, sin ninguna carrera de temporización
    //   (a diferencia de una versión anterior de este test, que agendaba
    //   la señal vía `tokio::time::sleep` estimando el instante en
    //   función de `SIMULATED_DELAY_MS`, y que era flaky bajo variación
    //   de scheduler/carga). Para k=0 se envía la señal ANTES de invocar
    //   `run_collection` (igual que en el test dedicado de la Tarea 9.6),
    //   sin depender de temporización.
    //
    // Cobertura de Requisitos: 5.1 (orden de ejecución), 5.2 (progreso),
    // 5.3 (reporte consolidado detallado), 5.5 (continuar tras fallos),
    // 5.7 (N=0), 5.8 (cancelación con punto k generado en 0..=N o
    // "sin cancelar"). El Requisito 5.6 (environment de override) se
    // deja fuera de esta property: ya está cubierto por la revisión
    // dedicada de la Tarea 9.3, y variarlo aquí habría requerido mockear
    // además la resolución de environments sin aportar cobertura
    // adicional sobre la lógica bajo prueba (el bucle secuencial y el
    // reporte consolidado).

    use std::time::Duration;

    use iced::futures::stream::StreamExt;

    /// Resultado simulado por request, mapeado a una combinación fija de
    /// URL (dentro del `MockServer` local) + `response_tests` que
    /// garantiza dicho resultado de forma determinística.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SimulatedOutcome {
        /// Éxito HTTP (200), sin ninguna assertion configurada.
        SuccessNoAssertion,
        /// Éxito HTTP (200) con una assertion que pasa (status == 200).
        SuccessPassingAssertion,
        /// Éxito HTTP (200) con una assertion que falla (se espera 404).
        AssertionFailure,
        /// Fallo de red: el draft usa un timeout menor al delay
        /// configurado en la ruta "/timeout" del mock, por lo que
        /// `RequestExecutorHandle::execute` SHALL devolver un error de
        /// timeout tras, aproximadamente, `SIMULATED_DELAY_MS`.
        NetworkError,
    }

    fn arb_simulated_outcome() -> impl Strategy<Value = SimulatedOutcome> {
        prop_oneof![
            Just(SimulatedOutcome::SuccessNoAssertion),
            Just(SimulatedOutcome::SuccessPassingAssertion),
            Just(SimulatedOutcome::AssertionFailure),
            Just(SimulatedOutcome::NetworkError),
        ]
    }

    /// De 0 a 4 resultados simulados (incluyendo el caso N=0, Requisito
    /// 5.7), junto con un punto de cancelación `k` (`Some(0..=N)`) o
    /// `None` (sin cancelar en absoluto), generado en función del N
    /// efectivamente producido.
    fn arb_scenario() -> impl Strategy<Value = (Vec<SimulatedOutcome>, Option<usize>)> {
        prop::collection::vec(arb_simulated_outcome(), 0..=4).prop_flat_map(|outcomes| {
            let total = outcomes.len();
            (Just(outcomes), proptest::option::of(0..=total))
        })
    }

    /// Retraso uniforme (ms) aplicado a TODAS las respuestas del mock,
    /// sin importar el resultado simulado (ver comentario de diseño más
    /// arriba). Un valor bajo mantiene el costo por caso bajo, dado que
    /// esta property realiza llamadas HTTP reales (aunque locales).
    const SIMULATED_DELAY_MS: u64 = 100;

    fn draft_for_outcome(index: usize, outcome: SimulatedOutcome, mock_server: &MockServer) -> RequestDraft {
        let mut draft = crate::curl::create_blank_draft();
        draft.method = HttpMethod::GET;
        draft.name = format!("property-18-request-{index}");

        match outcome {
            SimulatedOutcome::SuccessNoAssertion => {
                draft.url = format!("{}/success", mock_server.uri());
                draft.timeout_ms = 5_000;
            }
            SimulatedOutcome::SuccessPassingAssertion => {
                draft.url = format!("{}/success", mock_server.uri());
                draft.timeout_ms = 5_000;
                draft.response_tests.push(ResponseAssertion {
                    id: format!("property-18-pass-{index}"),
                    name: "status debería ser 200".to_string(),
                    enabled: true,
                    source: AssertionSource::Status,
                    operator: AssertionOperator::Equals,
                    selector: None,
                    expected: "200".to_string(),
                });
            }
            SimulatedOutcome::AssertionFailure => {
                draft.url = format!("{}/success", mock_server.uri());
                draft.timeout_ms = 5_000;
                draft.response_tests.push(ResponseAssertion {
                    id: format!("property-18-fail-{index}"),
                    name: "status debería ser 404".to_string(),
                    enabled: true,
                    source: AssertionSource::Status,
                    operator: AssertionOperator::Equals,
                    selector: None,
                    expected: "404".to_string(),
                });
            }
            SimulatedOutcome::NetworkError => {
                draft.url = format!("{}/timeout", mock_server.uri());
                // El mock en "/timeout" responde con un delay mucho mayor
                // (ver más abajo); este timeout, menor a ese delay, es el
                // que efectivamente dispara el error de red, y coincide
                // aproximadamente con `SIMULATED_DELAY_MS` para mantener
                // el tiempo de pared uniforme entre resultados.
                draft.timeout_ms = SIMULATED_DELAY_MS;
            }
        }

        draft
    }

    /// Datos esperados de un item del reporte, calculados de forma
    /// independiente de `run_collection` a partir únicamente del
    /// `SimulatedOutcome` (el oráculo de esta property).
    struct ExpectedItem {
        completed: bool,
        has_error_message: bool,
        response_status: Option<u16>,
        assertion_total: Option<u64>,
        assertion_passed: Option<u64>,
        assertion_failed: Option<u64>,
    }

    fn expected_item_for(outcome: SimulatedOutcome) -> ExpectedItem {
        match outcome {
            SimulatedOutcome::SuccessNoAssertion => ExpectedItem {
                completed: true,
                has_error_message: false,
                response_status: Some(200),
                assertion_total: Some(0),
                assertion_passed: Some(0),
                assertion_failed: Some(0),
            },
            SimulatedOutcome::SuccessPassingAssertion => ExpectedItem {
                completed: true,
                has_error_message: false,
                response_status: Some(200),
                assertion_total: Some(1),
                assertion_passed: Some(1),
                assertion_failed: Some(0),
            },
            SimulatedOutcome::AssertionFailure => ExpectedItem {
                completed: true,
                has_error_message: false,
                response_status: Some(200),
                assertion_total: Some(1),
                assertion_passed: Some(0),
                assertion_failed: Some(1),
            },
            SimulatedOutcome::NetworkError => ExpectedItem {
                completed: false,
                has_error_message: true,
                response_status: None,
                assertion_total: None,
                assertion_passed: None,
                assertion_failed: None,
            },
        }
    }

    /// Siembra la collection directamente vía `import_workspace_snapshot`
    /// (en lugar de `save_request`, usado por el resto de los tests de
    /// este archivo) para controlar explícitamente `updated_at` de cada
    /// request: se le asigna un valor estrictamente decreciente con el
    /// índice generado, de forma que el `ORDER BY updated_at DESC, name
    /// ASC` de `get_collection_with_requests` (Tarea 9.1) devuelva los
    /// requests EXACTAMENTE en el orden generado, sin depender de la
    /// resolución del reloj del sistema ni de desempates por nombre.
    async fn seed_collection(
        app_state: &AppState,
        collection_id: &str,
        outcomes: &[SimulatedOutcome],
        mock_server: &MockServer,
    ) {
        let total = outcomes.len();
        let requests = outcomes
            .iter()
            .enumerate()
            .map(|(index, outcome)| {
                let draft = draft_for_outcome(index, *outcome, mock_server);
                midway_core::domain::workspace::SavedRequestRecord {
                    id: format!("property-18-req-{index}"),
                    collection_id: collection_id.to_string(),
                    folder_id: None,
                    name: draft.name.clone(),
                    draft,
                    created_at: format!("{:010}", total - index),
                    updated_at: format!("{:010}", total - index),
                }
            })
            .collect::<Vec<_>>();

        let snapshot = midway_core::domain::workspace::WorkspaceSnapshot {
            collections: vec![midway_core::domain::workspace::CollectionWithRequests {
                collection: midway_core::domain::workspace::CollectionSummary {
                    id: collection_id.to_string(),
                    name: "Property 18 scenario collection".to_string(),
                    request_count: total as u64,
                    created_at: "0000000000".to_string(),
                    updated_at: "0000000000".to_string(),
                },
                folders: Vec::new(),
                requests,
            }],
            environments: vec![],
            history: vec![],
            secrets: vec![],
        };

        app_state
            .repository
            .import_workspace_snapshot(snapshot, false)
            .await
            .expect("import_workspace_snapshot no debería fallar al sembrar el escenario");
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 20, ..ProptestConfig::default() })]

        /// Feature: tauri-to-iced-migration, Property 18: Ejecución
        /// secuencial y reporte consolidado del Collection_Runner.
        /// Validates: Requirements 5.1, 5.2, 5.3, 5.5, 5.7, 5.8
        ///
        /// Para cualquier colección generada de N requests (incluyendo
        /// N=0) con una secuencia arbitraria de resultados simulados por
        /// request (éxito sin assertions, éxito con assertion que pasa,
        /// éxito con assertion que falla, fallo de red) y cualquier punto
        /// de cancelación k (`Some(0..=N)` o `None` para no cancelar):
        ///
        /// - Los requests SHALL ejecutarse en el mismo orden en que están
        ///   guardados (Requisito 5.1).
        /// - Las fases de progreso emitidas SHALL ser exactamente
        ///   `Started`, luego un par (`RequestStarted`, `RequestFinished`)
        ///   por cada request efectivamente ejecutado, y finalmente
        ///   `Finished`, con `total_requests` igual a N en todos los
        ///   eventos (Requisito 5.2).
        /// - El reporte consolidado SHALL contener exactamente una
        ///   entrada por request efectivamente ejecutado (N si no se
        ///   cancela a tiempo, k si se cancela tras k requests),
        ///   reflejando fielmente el resultado simulado de cada uno:
        ///   indicador de éxito/fallo, status HTTP, y resultado de
        ///   assertions (Requisitos 5.3, 5.5).
        /// - Los conteos agregados de éxito/fallo y de assertions
        ///   pasadas/falladas del reporte SHALL coincidir con los
        ///   calculados de forma independiente a partir de la secuencia
        ///   generada (Requisito 5.3).
        /// - Con N=0, el reporte consolidado SHALL reflejar de inmediato
        ///   cero requests ejecutados (Requisito 5.7).
        #[test]
        fn property_18_sequential_execution_and_consolidated_report(
            (outcomes, cancel_point) in arb_scenario(),
        ) {
            let total = outcomes.len();

            let runtime = tokio::runtime::Runtime::new()
                .expect("no se pudo crear el runtime de tokio para el test de Property 18");

            let result: Result<(), TestCaseError> = runtime.block_on(async move {
                let mock_server = MockServer::start().await;
                Mock::given(http_method_matcher("GET"))
                    .and(wiremock_path_matcher("/success"))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_delay(Duration::from_millis(SIMULATED_DELAY_MS)),
                    )
                    .mount(&mock_server)
                    .await;
                Mock::given(http_method_matcher("GET"))
                    .and(wiremock_path_matcher("/timeout"))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_delay(Duration::from_millis(SIMULATED_DELAY_MS * 20)),
                    )
                    .mount(&mock_server)
                    .await;

                let app_state = Arc::new(build_test_app_state().await);
                let collection_id = "property-18-collection".to_string();
                seed_collection(&app_state, &collection_id, &outcomes, &mock_server).await;

                let (progress_tx, progress_rx) = mpsc::unbounded();
                let (cancel_tx, cancel_rx) = oneshot::channel();

                // Mecanismo de cancelación determinístico, sin depender
                // de temporización de reloj de pared (evita la carrera
                // que hacía flaky este test cuando se agendaba la señal
                // vía `tokio::time::sleep`, ver Tarea 9.7 fix): para un
                // punto de cancelación `Some(k)` con `k > 0`, la señal se
                // envía desde la propia tarea que consume `progress_rx`
                // EN CUANTO observa el k-ésimo evento
                // `CollectionRunPhase::RequestFinished` (es decir,
                // `current_index == k`), justo cuando el request `k-1`
                // (0-indexado) terminó de completarse. Como la
                // comprobación de `cancel_rx` en `run_collection` ocurre
                // de forma síncrona al inicio de la siguiente iteración,
                // sin ningún punto de espera intermedio, esto garantiza
                // que la señal llegue estrictamente después de que el
                // request `k-1` complete y antes de que arranque el
                // request `k`, sin ninguna ventana de temporización.
                let cancel_tx_for_forwarder = match cancel_point {
                    Some(0) => {
                        // Cancelación ANTES de invocar `run_collection`
                        // (igual que el caso límite k=0 del test dedicado
                        // de la Tarea 9.6): no depende de temporización.
                        let _ = cancel_tx.send(());
                        None
                    }
                    Some(k) => Some((cancel_tx, k)),
                    None => {
                        // Sin cancelar: se descarta el sender sin enviar
                        // nada; `cancel_rx.try_recv()` seguirá devolviendo
                        // `Err(_)` (vacío o cerrado, según el caso) en
                        // cada iteración, nunca `Ok(())`.
                        drop(cancel_tx);
                        None
                    }
                };

                // Tarea concurrente que reenvía cada evento de
                // `progress_rx` A MEDIDA QUE LLEGA (en vez de drenar el
                // canal solo después de que `run_collection` termine),
                // observando el k-ésimo `RequestStarted` para disparar la
                // cancelación de forma determinística. Se dispara sobre
                // `RequestStarted` (no sobre `RequestFinished`) porque la
                // comprobación de `cancel_rx` ocurre de forma síncrona,
                // sin ningún punto de espera intermedio, justo después de
                // que el request anterior completa: disparar sobre el
                // `RequestFinished` del request `k-1` deja una ventana
                // efectivamente nula frente a esa comprobación síncrona
                // (confirmado empíricamente: fallaba de forma consistente,
                // no solo ocasionalmente, al intentarlo). Disparar en
                // cambio sobre el `RequestStarted` del request `k` deja
                // como margen la duración COMPLETA de ese request (~
                // `SIMULATED_DELAY_MS`) antes de que el bucle vuelva a
                // comprobar `cancel_rx` al inicio de la iteración
                // siguiente, garantizando que el request `k` complete con
                // normalidad y que el reporte final contenga exactamente
                // `k` items.
                let forwarder = tokio::spawn(async move {
                    let mut progress_rx: mpsc::UnboundedReceiver<CollectionRunProgressEvent> = progress_rx;
                    let mut cancel_tx_for_forwarder = cancel_tx_for_forwarder;
                    let mut events: Vec<CollectionRunProgressEvent> = Vec::new();
                    while let Some(event) = progress_rx.next().await {
                        if let Some((_, k)) = cancel_tx_for_forwarder.as_ref() {
                            if matches!(event.phase, CollectionRunPhase::RequestStarted)
                                && event.current_index as usize == *k
                            {
                                if let Some((cancel_tx, _)) = cancel_tx_for_forwarder.take() {
                                    let _ = cancel_tx.send(());
                                }
                            }
                        }
                        events.push(event);
                    }
                    events
                });

                let report = run_collection(
                    Arc::clone(&app_state),
                    RunCollectionInput {
                        collection_id: collection_id.clone(),
                        environment_override_id: None,
                        stop_on_error: false,
                    },
                    progress_tx,
                    cancel_rx,
                )
                .await
                .expect("run_collection SHALL devolver Ok incluso si la ejecución fue cancelada");

                // La tarea de reenvío consume `progress_rx` por completo
                // (el `Sender` fue movido dentro de `run_collection` y se
                // soltó al finalizar, cerrando el canal), así que al
                // completar `run_collection` el `Stream` ya terminó y
                // `forwarder` SHALL resolver con todos los eventos
                // reenviados.
                let events = forwarder
                    .await
                    .expect("la tarea de reenvío de progreso no debería panicar");

                // Oráculo: cuántos requests se ejecutaron efectivamente.
                // Si k=N o no se cancela, se ejecutan los N; si se
                // cancela tras k < N, se ejecutan exactamente k (la
                // cancelación tardía, con k >= N, no tiene efecto
                // observable porque ya no queda ninguna iteración
                // restante que la compruebe).
                let effective_count = cancel_point.unwrap_or(total).min(total);

                prop_assert_eq!(report.total_requests, total as u64);
                prop_assert_eq!(report.items.len(), effective_count);

                let expected_items: Vec<ExpectedItem> = outcomes[..effective_count]
                    .iter()
                    .map(|outcome| expected_item_for(*outcome))
                    .collect();

                let expected_completed = expected_items.iter().filter(|item| item.completed).count() as u64;
                let expected_errored = expected_items.iter().filter(|item| !item.completed).count() as u64;
                let expected_passed_assertions: u64 = expected_items
                    .iter()
                    .filter_map(|item| item.assertion_passed)
                    .sum();
                let expected_failed_assertions: u64 = expected_items
                    .iter()
                    .filter_map(|item| item.assertion_failed)
                    .sum();

                prop_assert_eq!(report.completed_requests, expected_completed);
                prop_assert_eq!(report.errored_requests, expected_errored);
                prop_assert_eq!(report.passed_assertions, expected_passed_assertions);
                prop_assert_eq!(report.failed_assertions, expected_failed_assertions);

                // Cada item del reporte SHALL reflejar fielmente su
                // resultado simulado, EN EL MISMO ORDEN en que fueron
                // generados (Requisito 5.1): el índice `i` del reporte
                // corresponde al request `i`-ésimo generado.
                for (i, (actual_item, expected_item)) in report.items.iter().zip(expected_items.iter()).enumerate() {
                    prop_assert_eq!(
                        actual_item.request_name.as_str(),
                        format!("property-18-request-{i}"),
                        "el item en la posición {} no corresponde al request generado en esa posición",
                        i
                    );
                    prop_assert_eq!(actual_item.response_status, expected_item.response_status);
                    prop_assert_eq!(actual_item.error_message.is_some(), expected_item.has_error_message);
                    prop_assert_eq!(actual_item.duration_ms.is_some(), expected_item.completed);

                    match (&actual_item.assertion_report, expected_item.assertion_total) {
                        (Some(report), Some(expected_total)) => {
                            prop_assert_eq!(report.total, expected_total);
                            prop_assert_eq!(report.passed, expected_item.assertion_passed.unwrap());
                            prop_assert_eq!(report.failed, expected_item.assertion_failed.unwrap());
                        }
                        (None, None) => {}
                        (actual, expected) => {
                            return Err(TestCaseError::fail(format!(
                                "presencia de assertion_report inesperada en la posición {i}: actual={actual:?}, expected_total={expected:?}"
                            )));
                        }
                    }
                }

                // Fases de progreso: `Started`, un par
                // (`RequestStarted`, `RequestFinished`) por cada request
                // efectivamente ejecutado, y `Finished` (Requisito 5.2).
                let mut expected_phases = vec![CollectionRunPhase::Started];
                for _ in 0..effective_count {
                    expected_phases.push(CollectionRunPhase::RequestStarted);
                    expected_phases.push(CollectionRunPhase::RequestFinished);
                }
                expected_phases.push(CollectionRunPhase::Finished);

                let actual_phases: Vec<CollectionRunPhase> = events.iter().map(|event| event.phase).collect();
                prop_assert_eq!(format!("{actual_phases:?}"), format!("{expected_phases:?}"));
                prop_assert!(events.iter().all(|event| event.total_requests == total as u64));

                let finished_event = events
                    .last()
                    .expect("siempre SHALL emitirse al menos el evento Finished");
                prop_assert_eq!(finished_event.completed_requests, expected_completed);
                prop_assert_eq!(finished_event.errored_requests, expected_errored);
                prop_assert!(finished_event.finished_at.is_some());

                Ok(())
            });

            result?;
        }
    }
}
