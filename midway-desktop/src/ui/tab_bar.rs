//! Flat tab bar — renders tabs as plain text with bold for active tab.
//! Matches the Insomnia-style: no borders, no backgrounds on tabs, just
//! text weight differentiation.

use iced::widget::{button, container, row, text};
use iced::{font, Border, Element, Font, Length};

use crate::ui::design_system::DesignSystem;

/// Entrada diferida de una tab: identificador, etiqueta y constructor de su
/// contenido. El alias mantiene legibles las firmas de los distintos paneles
/// que comparten este widget.
pub type TabEntry<'a, Message, TabId> = (
    TabId,
    &'static str,
    Box<dyn FnOnce() -> Element<'a, Message> + 'a>,
);

/// Renders a flat tab bar + the active tab's content below it.
///
/// El contenido de cada tab se recibe como closure y solo se invoca el de la
/// tab activa: construir los `Element` de las tabs ocultas para después
/// descartarlos desperdicia trabajo (y en la tab Body del
/// `Response_Inspector`, el shaping del payload completo).
pub fn tabs<'a, Message, TabId, F>(
    entries: Vec<TabEntry<'a, Message, TabId>>,
    active: &TabId,
    on_select: F,
    ds: &DesignSystem,
) -> Element<'a, Message>
where
    Message: 'a + Clone,
    TabId: Eq + Clone + 'a,
    F: 'static + Fn(TabId) -> Message,
{
    let body = ds.typography.body;
    let text_primary = ds.palette.text_primary;
    let text_secondary = ds.palette.text_secondary;

    let mut tabs_row = row![].spacing(ds.spacing.lg);

    let labels: Vec<(TabId, &'static str)> = entries
        .iter()
        .map(|(id, label, _)| (id.clone(), *label))
        .collect();

    for (id, label) in &labels {
        let is_active = id == active;
        let tab_color = if is_active {
            text_primary
        } else {
            text_secondary
        };
        let weight = if is_active {
            font::Weight::Bold
        } else {
            font::Weight::Normal
        };

        let tab_font = Font {
            family: font::Family::SansSerif,
            weight,
            ..Font::DEFAULT
        };

        let id_clone = id.clone();
        let tab_btn = button(text(*label).size(body.size).font(tab_font).color(tab_color))
            .padding([ds.spacing.xs, ds.spacing.sm])
            .style(move |_theme, _status| button::Style {
                background: None,
                text_color: tab_color,
                border: Border::default(),
                ..button::Style::default()
            })
            .on_press(on_select(id_clone));

        tabs_row = tabs_row.push(tab_btn);
    }

    // Solo se construye el contenido de la tab activa; el resto de los
    // closures se descarta sin ejecutarse.
    let active_content = entries
        .into_iter()
        .find(|(id, _, _)| id == active)
        .map(|(_, _, build)| build());

    let border_color = ds.palette.border;
    let separator = container(text(""))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_theme| iced::widget::container::Style {
            background: Some(border_color.into()),
            ..iced::widget::container::Style::default()
        });

    let mut col = iced::widget::column![tabs_row, separator]
        .spacing(ds.spacing.xs)
        .width(Length::Fill);

    if let Some(content) = active_content {
        col = col.push(content);
    }

    col.into()
}
