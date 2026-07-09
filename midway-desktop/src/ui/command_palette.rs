//! Vista del overlay del Command Palette (Tarea 11.2, Fase 5).
//!
//! El algoritmo de scoring/búsqueda (`search_palette_items`) vive en
//! `command_palette.rs` (Tarea 11.1); este módulo solo se encarga de
//! renderizar el overlay (vía `iced::widget::stack`, Requisito 6.1) sobre
//! el contenido principal cuando `state.palette.is_open` es `true`.
//!
//! Ver diseño: "Components and Interfaces > Command Palette, Session_Store
//! y shortcuts (Fase 5)".
//! Ver requisitos: 6.1, 6.2.

use iced::widget::{button, column, container, mouse_area, opaque, row, scrollable, stack, text, text_input};
use iced::{Color, Element, Length};

use crate::app::{build_palette_items, Message, Midway, PaletteMessage};
use crate::command_palette::{search_palette_items, CommandPaletteItem};

/// Número máximo de resultados mostrados a la vez en el overlay,
/// equivalente al `limit = 14` usado por la referencia TypeScript
/// (`searchPaletteItems(commandPaletteItems, commandPaletteQuery, 14)`).
const PALETTE_RESULT_LIMIT: usize = 14;

/// Envuelve `main_content` con el overlay del Command Palette cuando está
/// abierto (`state.palette.is_open`), usando `iced::widget::stack`
/// (Requisito 6.1). Cuando está cerrado, devuelve `main_content` sin
/// modificar (sin capa adicional de `stack`).
pub fn with_overlay<'a>(state: &'a Midway, main_content: Element<'a, Message>) -> Element<'a, Message> {
    if !state.palette.is_open {
        return main_content;
    }

    stack![main_content, opaque(palette_overlay(state))].into()
}

/// Construye la capa superior del overlay: un fondo semitransparente que
/// cierra el palette al hacer click fuera del cuadro de búsqueda/resultados
/// (`PaletteMessage::Dismissed`, Requisito 6.1), centrado sobre el cuadro
/// del palette en sí (`opaque`, para que los clicks dentro de él no se
/// propaguen al fondo y lo cierren accidentalmente).
fn palette_overlay(state: &Midway) -> Element<'_, Message> {
    mouse_area(
        container(opaque(palette_box(state)))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Top)
            .padding(80)
            .style(|_theme| container::Style {
                background: Some(Color { a: 0.5, ..Color::BLACK }.into()),
                ..container::Style::default()
            }),
    )
    .on_press(Message::Palette(PaletteMessage::Dismissed))
    .into()
}

/// Cuadro del palette en sí: `text_input` de búsqueda + lista de
/// resultados (Requisito 6.1, 6.2).
fn palette_box(state: &Midway) -> Element<'_, Message> {
    let items = build_palette_items(state);
    let results = search_palette_items(&items, &state.palette.query, PALETTE_RESULT_LIMIT);

    let search_input = text_input("Buscar acciones, colecciones o requests...", &state.palette.query)
        .on_input(|query| Message::Palette(PaletteMessage::QueryChanged(query)))
        .width(Length::Fill);

    let mut results_list = column![].spacing(4);
    if results.is_empty() {
        results_list = results_list.push(text("Sin coincidencias."));
    } else {
        for item in results {
            results_list = results_list.push(palette_result_row(item));
        }
    }

    container(column![search_input, scrollable(results_list).height(Length::Fixed(320.0))].spacing(8))
        .width(Length::Fixed(480.0))
        .padding(12)
        .style(container::rounded_box)
        .into()
}

/// Fila clickable de un resultado del palette (Requisito 6.2): título,
/// subtítulo (si existe) y sección; al presionarla, dispara
/// `PaletteMessage::ItemSelected` con el `id` del ítem.
fn palette_result_row<'a>(item: CommandPaletteItem) -> Element<'a, Message> {
    let mut item_column = column![row![
        text(item.title.clone()).width(Length::Fill),
        text(item.section.clone()).color(Color::from_rgb(0.55, 0.55, 0.55)),
    ]];

    if let Some(subtitle) = &item.subtitle {
        item_column = item_column.push(text(subtitle.clone()).color(Color::from_rgb(0.6, 0.6, 0.6)));
    }

    button(item_column)
        .width(Length::Fill)
        .on_press(Message::Palette(PaletteMessage::ItemSelected { item_id: item.id }))
        .into()
}
