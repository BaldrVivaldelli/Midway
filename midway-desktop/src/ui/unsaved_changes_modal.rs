//! Vista del overlay del aviso de unsaved changes (Tarea 11.10, Fase 5).
//!
//! Muestra un modal con tres opciones (Guardar/Descartar/Cancelar) cuando
//! el usuario intenta cerrar una tab con cambios sin guardar
//! (`Midway.unsaved_changes_prompt`), usando el mismo patrón de overlay
//! (`iced::widget::stack` + `opaque`/`mouse_area`) que el Command Palette
//! (Tarea 11.2, `ui/command_palette.rs`), para consistencia visual y de
//! implementación entre ambos overlays de `midway-desktop`.
//!
//! Ver diseño: "Components and Interfaces > Command Palette, Session_Store
//! y shortcuts (Fase 5)" (sección "Unsaved changes").
//! Ver requisitos: 6.6.

use iced::widget::{button, column, container, mouse_area, opaque, row, stack, text};
use iced::{Border, Color, Element, Length};

use crate::app::{Message, Midway, RequestComposerMessage};
use crate::ui::design_system::DesignSystem;

/// Envuelve `main_content` con el overlay del aviso de unsaved changes
/// cuando `state.unsaved_changes_prompt` es `Some` (Requisito 6.6). Cuando
/// es `None`, devuelve `main_content` sin modificar.
///
/// Este overlay se apila DESPUÉS del de `command_palette::with_overlay`
/// (ver `app::view`), de forma que quede por encima si ambos estuvieran
/// abiertos a la vez; en la práctica no hay ningún camino actual del
/// código que permita que el palette y este aviso estén abiertos
/// simultáneamente (el palette no dispara `TabClosed`), pero el orden de
/// apilado documenta la precedencia por si eso cambiara en el futuro.
pub fn with_overlay<'a>(
    state: &'a Midway,
    main_content: Element<'a, Message>,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let Some(prompt) = &state.unsaved_changes_prompt else {
        return main_content;
    };

    stack![main_content, opaque(unsaved_changes_overlay(prompt, ds))].into()
}

/// Construye la capa superior del overlay: un fondo semitransparente
/// centrado sobre el cuadro del aviso en sí (`opaque`, para que los clicks
/// dentro de él no se propaguen al fondo).
///
/// A diferencia del Command Palette, hacer click fuera del cuadro NO
/// descarta el aviso (no hay un `on_press` en el `mouse_area` de fondo):
/// un click accidental fuera del modal no debería descartar
/// silenciosamente cambios sin guardar sin que el usuario haya elegido
/// explícitamente una de las tres opciones (Guardar/Descartar/Cancelar).
fn unsaved_changes_overlay<'a>(
    prompt: &'a crate::app::UnsavedChangesPromptState,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let overlay_background = Color {
        a: 0.5,
        ..ds.palette.background_primary
    };

    mouse_area(
        container(opaque(unsaved_changes_box(prompt, ds)))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_theme| container::Style {
                background: Some(overlay_background.into()),
                ..container::Style::default()
            }),
    )
    .into()
}

/// Cuadro del aviso en sí: mensaje + botones Guardar/Descartar/Cancelar
/// (Requisito 6.6). Los tres botones se deshabilitan mientras
/// `prompt.saving` es `true` (una llamada de Guardar en curso), para
/// evitar disparar dos acciones a la vez sobre la misma tab.
fn unsaved_changes_box<'a>(
    prompt: &'a crate::app::UnsavedChangesPromptState,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let mut content = column![text(
        "Esta tab tiene cambios sin guardar. ¿Qué querés hacer?"
    )]
    .spacing(12);

    if let Some(error) = &prompt.error {
        content = content.push(text(error.clone()).color(ds.palette.status_error));
    }

    let save_label = if prompt.saving {
        "Guardando…"
    } else {
        "Guardar"
    };
    let mut save_button = button(text(save_label));
    let mut discard_button = button(text("Descartar"));
    let mut cancel_button = button(text("Cancelar"));

    if !prompt.saving {
        save_button = save_button.on_press(Message::RequestComposer(
            RequestComposerMessage::UnsavedChangesSaveRequested,
        ));
        discard_button = discard_button.on_press(Message::RequestComposer(
            RequestComposerMessage::UnsavedChangesDiscardRequested,
        ));
        cancel_button = cancel_button.on_press(Message::RequestComposer(
            RequestComposerMessage::UnsavedChangesCancelRequested,
        ));
    }

    content = content.push(row![save_button, discard_button, cancel_button].spacing(8));

    let background = ds.palette.surface_elevated;
    let border_color = ds.palette.border;
    let border_radius = ds.radius.panel;

    container(content)
        .width(Length::Fixed(420.0))
        .padding(16)
        .style(move |_theme| container::Style {
            background: Some(background.into()),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: border_radius.into(),
            },
            ..container::Style::default()
        })
        .into()
}
