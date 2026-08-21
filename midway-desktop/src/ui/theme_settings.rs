//! Vertical Tema/Ajustes: estado propio, mensajes propios, `update` puro y
//! `control` (view) del toggle de tema.
//!
//! Tarea 6.1 — Requisitos: 8.2, 8.3, 8.5, 8.6, 8.8, 11.7.
//!
//! Esta es la primera vertical extraída de `app.rs` siguiendo el patrón de
//! extracción documentado en `docs/architecture.md`:
//!
//! - El estado que la vertical posee vive en [`ThemeSettingsState`]; la
//!   App_Raíz deja de tener el campo suelto de modo de tema (tarea 7.1).
//! - Los mensajes se declaran acá ([`ThemeMessage`]); el `Message` raíz los
//!   envuelve, no los define.
//! - [`update`] es total: no devuelve `Result`, no devuelve `Task`, no hace I/O
//!   y no toca estado global. Devuelve una lista de Evento_Ascendente
//!   ([`ThemeEvent`]) que la App_Raíz traduce en efecto (marcar la sesión
//!   sucia).
//! - [`control`] recibe todas sus dependencias por parámetro (el estado de la
//!   vertical por valor y el [`DesignSystem`] por referencia) y devuelve
//!   `Element<'a, ThemeMessage>`, de modo que el módulo **no** conoce el
//!   `Message` raíz. `top_bar.rs` hace `control(...).map(Message::Theme)`
//!   (tarea 7.2).
//!
//! Restricción estructural (Req 8.5, 8.6): las únicas dependencias de este
//! módulo son `iced` y `crate::ui::design_system`. No importa el estado
//! agregado de la aplicación, el módulo de sesión, el cliente HTTP ni la capa
//! de infraestructura de `midway-core`. Eso hace estructuralmente imposible el
//! acceso a disco, SQL o red desde la vertical, y es lo que verifica el test de
//! guardia `tests/vertical_import_guard.rs` (tarea 6.3).

use iced::widget::{button, text};
use iced::{Border, Element};

use crate::ui::design_system::{DesignSystem, ThemeMode};

/// Estado propio de la vertical Tema/Ajustes.
///
/// Es `Copy` a propósito: [`control`] lo recibe por valor y así no puede
/// mutarlo ni retener un préstamo del estado agregado de la aplicación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThemeSettingsState {
    mode: ThemeMode,
}

impl ThemeSettingsState {
    /// Construye el estado a partir de un [`ThemeMode`] ya conocido (por
    /// ejemplo, el restaurado desde `SessionSnapshot`).
    pub fn new(mode: ThemeMode) -> Self {
        Self { mode }
    }

    /// Modo de tema activo.
    pub fn mode(self) -> ThemeMode {
        self.mode
    }

    /// Sistema de diseño derivado del modo activo.
    pub fn design_system(self) -> DesignSystem {
        DesignSystem::for_mode(self.mode)
    }
}

/// Mensajes de la vertical. El `Message` raíz los envuelve; no los define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeMessage {
    /// El usuario activó el control de cambio de tema (Light/Dark).
    Toggled,
}

/// Evento_Ascendente: lo que la App_Raíz debe traducir en efecto.
///
/// La vertical nunca ejecuta el efecto. Para Tema/Ajustes el efecto es marcar
/// la sesión como sucia, lo que dispara el autosave existente que persiste
/// `theme_mode` dentro de `SessionSnapshot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeEvent {
    ThemeChanged { mode: ThemeMode },
}

/// `update` puro de la vertical: sin `Result`, sin `Task`, sin I/O.
///
/// [`ThemeMessage::Toggled`] aplica [`ThemeMode::toggled`] y emite
/// [`ThemeEvent::ThemeChanged`] con el modo resultante. No hay camino de error,
/// por lo que la función es total.
pub fn update(state: &mut ThemeSettingsState, message: ThemeMessage) -> Vec<ThemeEvent> {
    match message {
        ThemeMessage::Toggled => {
            state.mode = state.mode.toggled();
            vec![ThemeEvent::ThemeChanged { mode: state.mode }]
        }
    }
}

/// Icono del control según el modo activo.
///
/// Se mantiene idéntico al del baseline (`ui/top_bar.rs`) para que el
/// comportamiento observable no cambie (Req 8.7).
fn toggle_icon(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::Light => "☀",
        ThemeMode::Dark => "☾",
    }
}

/// `control` (view) del toggle de tema.
///
/// Recibe todas sus dependencias por parámetro y devuelve
/// `Element<'a, ThemeMessage>`: el módulo no conoce el `Message` raíz ni lee
/// estado global (Req 8.5).
pub fn control<'a>(state: ThemeSettingsState, ds: &DesignSystem) -> Element<'a, ThemeMessage> {
    let text_color = ds.palette.text_primary;
    let radius = ds.radius.control;

    button(
        text(toggle_icon(state.mode()))
            .size(ds.typography.body.size)
            .color(text_color),
    )
    .padding(ds.spacing.sm)
    .style(move |_theme, _status| button::Style {
        text_color,
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .on_press(ThemeMessage::Toggled)
    .into()
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// `ThemeMode` solo tiene dos variantes en el baseline
    /// (`design_system.rs`), así que se enumeran en vez de derivar una
    /// estrategia. Si se agrega una variante, este generador debe crecer y el
    /// compilador no lo va a avisar: el test de involutividad sí, porque
    /// `toggled` dejaría de ser una involución.
    fn arb_theme_mode() -> impl Strategy<Value = ThemeMode> {
        prop_oneof![Just(ThemeMode::Light), Just(ThemeMode::Dark)]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // Feature: midway-baseline-audit-and-first-vertical, Property 1: El toggle de tema cambia el modo, emite el evento y es involutivo
        //
        /// **Validates: Requirements 6.2, 8.4, 8.7**
        ///
        /// Para todo `ThemeMode` inicial, aplicar `ThemeMessage::Toggled` sobre
        /// el estado de la vertical produce un modo distinto del inicial, emite
        /// exactamente un `ThemeEvent::ThemeChanged` cuyo `mode` coincide con el
        /// modo resultante del estado, y aplicar el mensaje dos veces devuelve
        /// el modo original.
        ///
        /// El test corre sin ventana, sin disco y sin red: `update` es puro.
        #[test]
        fn property_1_theme_toggle_changes_mode_emits_event_and_is_involutive(
            initial in arb_theme_mode(),
        ) {
            let mut state = ThemeSettingsState::new(initial);

            // Primera aplicación: el modo cambia.
            let events = update(&mut state, ThemeMessage::Toggled);
            let after_first = state.mode();
            prop_assert_ne!(
                after_first,
                initial,
                "el toggle debe producir un modo distinto del inicial"
            );

            // Se emite exactamente un evento, y su `mode` coincide con el
            // estado resultante.
            prop_assert_eq!(
                events.len(),
                1,
                "el toggle debe emitir exactamente un Evento_Ascendente, emitió {:?}",
                events
            );
            prop_assert_eq!(
                events[0],
                ThemeEvent::ThemeChanged { mode: after_first },
                "el evento debe reportar el modo resultante del estado"
            );

            // El sistema de diseño derivado sigue al modo del estado.
            prop_assert_eq!(state.design_system(), DesignSystem::for_mode(after_first));

            // Segunda aplicación: involutividad.
            let second_events = update(&mut state, ThemeMessage::Toggled);
            prop_assert_eq!(
                state.mode(),
                initial,
                "aplicar el toggle dos veces debe devolver el modo original"
            );
            prop_assert_eq!(
                second_events,
                vec![ThemeEvent::ThemeChanged { mode: initial }],
                "la segunda aplicación también emite un solo evento con el modo resultante"
            );
        }
    }
}
