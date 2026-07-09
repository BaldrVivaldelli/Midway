//! `midway-desktop`: binario de escritorio Midway sobre `iced`.
//!
//! Punto de entrada real de la Tarea 3.1 (Fase 1): construye el `AppState`
//! (repositorio SQLite, executor de requests, executor de secrets) y arranca
//! `iced::application(...)` con la arquitectura Elm definida en `app.rs`.

mod app;
mod collection_runner;
mod command_palette;
mod curl;
mod diagnostics;
mod session;
mod state;
mod ui;
mod updater;

use app::{update, view, Midway};
use state::AppState;

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .title("Midway Desktop")
        .subscription(app::subscription)
        .run()
}

/// Función de arranque (`boot`) de la aplicación.
///
/// `iced` 0.14 espera que `boot` devuelva de forma síncrona el estado
/// inicial (`(State, Task<Message>)`); como `AppState::initialize` es
/// `async` (abre la base de datos SQLite vía `tokio-rusqlite` y arranca los
/// executors de request/secrets), se resuelve de forma bloqueante sobre el
/// runtime de Tokio que `iced` ya deja como "current" en este punto (feature
/// `tokio` habilitado en `Cargo.toml`), evitando así mezclar dos runtimes o
/// tener que introducir un estado `Option<AppState>` transitorio.
fn boot() -> Midway {
    let app_state = tokio::runtime::Handle::current()
        .block_on(AppState::initialize())
        .expect("No se pudo inicializar el estado de la aplicación (AppState).");

    Midway::new(app_state)
}
