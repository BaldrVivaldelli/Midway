use tauri::Manager;

mod app;
mod commands;
mod domain;
mod infra;
mod runtime;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // El updater solo se inicializa cuando la config cargada trae
    // `plugins.updater` (por ejemplo, las overlays de release).
    let context = tauri::generate_context!();
    let has_updater_config = serde_json::to_value(context.config())
        .ok()
        .and_then(|config| config.get("plugins").and_then(|plugins| plugins.get("updater")).cloned())
        .map(|updater| updater.is_object())
        .unwrap_or(false);

    let mut builder = tauri::Builder::default().plugin(tauri_plugin_process::init());

    if has_updater_config {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(|app| {
            let handle = app.handle().clone();
            let state = tauri::async_runtime::block_on(async move {
                crate::state::AppState::initialize(&handle).await
            })
            .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?;

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::preview_request,
            commands::execute_request,
            commands::cancel_request,
            commands::run_collection,
            commands::export_workspace_data,
            commands::import_workspace_data,
            commands::import_workspace_payload,
            commands::workspace_snapshot,
            commands::create_collection,
            commands::save_request,
            commands::save_environment,
            commands::delete_environment,
            commands::save_secret,
            commands::delete_secret
        ])
        .run(context)
        .expect("error while running tauri application");
}
