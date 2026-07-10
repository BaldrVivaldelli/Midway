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

use app::{update, view, Message, Midway};
use state::AppState;

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .title("Midway Desktop")
        .theme(theme)
        .subscription(app::subscription)
        .run()
}

/// Devuelve el `iced::Theme` activo según el `ThemeMode` del estado.
fn theme(state: &Midway) -> iced::Theme {
    use crate::ui::design_system::{DesignSystem, ThemeMode};

    let ds = DesignSystem::for_mode(state.theme_mode);
    let p = ds.palette;

    let custom_palette = iced::theme::Palette {
        background: p.background_primary,
        text: p.text_primary,
        primary: p.accent,
        success: p.status_success,
        danger: p.status_error,
        warning: iced::Color::from_rgb8(0xEF, 0xA0, 0x4E),
    };

    match state.theme_mode {
        ThemeMode::Dark => iced::Theme::custom_with_fn(
            "Midway Dark".to_string(),
            custom_palette,
            |palette| iced::theme::palette::Extended::generate(palette),
        ),
        ThemeMode::Light => iced::Theme::custom_with_fn(
            "Midway Light".to_string(),
            custom_palette,
            |palette| iced::theme::palette::Extended::generate(palette),
        ),
    }
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
fn boot() -> (Midway, iced::Task<Message>) {
    let app_state = tokio::runtime::Handle::current()
        .block_on(AppState::initialize())
        .expect("No se pudo inicializar el estado de la aplicación (AppState).");

    let state = Midway::new(app_state);
    let task = app::initial_workspace_load(&state);
    (state, task)
}
