//! Widget de tabs reutilizable (Tarea 5.1, Fase 2): envuelve la
//! construcción de `iced_aw::{TabBar, Tabs}` para evitar duplicar el mismo
//! wiring en las tres áreas del diseño que usan tabs:
//!
//! - Las tabs de configuración del `Request_Composer` (Params/Headers/Auth/
//!   Body/Tests, Tarea 5.1).
//! - Las tabs del `Response_Inspector` (Body/Headers/Tests, migradas desde
//!   botones simples en esta misma tarea).
//! - Las secciones del `Workspace_Panel` lateral (Environments/Data/
//!   History/Diagnostics/App updates, Fase 3, Tarea 7.1).
//!
//! Ver diseño: "Components and Interfaces > Tabs de configuración del
//! request (Fase 2)".
//! Ver requisitos: 3.1.

use iced::{Element, Length};
use iced_aw::widget::{TabLabel, Tabs};

/// Construye un widget de tabs (`iced_aw::Tabs`, que combina internamente
/// un `TabBar` con el contenido de la tab activa) a partir de una lista de
/// entradas `(id, etiqueta, contenido)`, el id de la tab actualmente activa
/// y la función que mapea el id seleccionado por el usuario a un `Message`.
///
/// `TabId` debe ser `Eq + Clone` (requisito de `iced_aw::Tabs`); los enums
/// de tab del diseño (`RequestTab`, `ResponseInspectorTab`, y los que se
/// agreguen para las secciones del `Workspace_Panel`) ya derivan
/// `PartialEq, Eq, Clone, Copy`, por lo que son compatibles sin envoltura
/// adicional.
pub fn tabs<'a, Message, TabId, F>(
    entries: Vec<(TabId, &'static str, Element<'a, Message>)>,
    active: &TabId,
    on_select: F,
) -> Element<'a, Message>
where
    Message: 'a,
    TabId: Eq + Clone + 'a,
    F: 'static + Fn(TabId) -> Message,
{
    let mut widget = Tabs::new(on_select).width(Length::Fill);

    for (id, label, content) in entries {
        widget = widget.push(id, TabLabel::Text(label.to_string()), content);
    }

    widget.set_active_tab(active).into()
}
