//! Activity_Bar: barra angosta de iconos a la izquierda (Tarea 10.1).
//!
//! Renderiza Home + Activity fijos, un icono por colección en orden, y un
//! botón "+" al final para crear colecciones. Cada icono de colección es un
//! cuadrado de color con iniciales derivadas del nombre.

use iced::widget::tooltip::Position as TooltipPosition;
use iced::widget::{
    button, column, container, mouse_area, opaque, row, stack, text, text_input, tooltip,
};
use iced::{Border, Color, Element, Length};

use crate::app::{ActivityBarMessage, MainContentFocus, Message, Midway};
use crate::ui::design_system::DesignSystem;

// ─── Tooltip types and helpers ──────────────────────────────────────────────

/// Identifies which Activity_Bar item is being hovered.
#[derive(Debug, Clone, PartialEq)]
pub enum HoveredItem {
    Home,
    Activity,
    Collection(String), // collection name (for tooltip display)
    CreateButton,
}

/// Returns the tooltip text for the given hovered item.
///
/// - Home → "Workspace"
/// - Activity → "Activity"
/// - Collection → the full collection name
/// - CreateButton → "Nueva colección"
pub fn tooltip_text_for(item: &HoveredItem) -> &str {
    match item {
        HoveredItem::Home => "Workspace",
        HoveredItem::Activity => "Activity",
        HoveredItem::Collection(name) => name.as_str(),
        HoveredItem::CreateButton => "Nueva colección",
    }
}

// ─── Pure helpers ───────────────────────────────────────────────────────────

/// Returns the color for the Home icon based on the current `MainContentFocus`.
///
/// - `WorkspaceSection` → `ds.palette.accent` (active/highlighted)
/// - `RequestTab` → `ds.palette.text_secondary` (inactive/dimmed)
pub fn home_icon_color(focus: MainContentFocus, ds: &DesignSystem) -> Color {
    match focus {
        MainContentFocus::WorkspaceSection => ds.palette.accent,
        MainContentFocus::RequestTab => ds.palette.text_secondary,
    }
}

/// Derives up to 2 characters of initials from the collection name,
/// deterministically.
///
/// - If `name.trim()` is empty, returns "?" as fallback.
/// - If `name.trim()` has a single character, returns that single character.
/// - If `name.trim()` has ≥2 words (split on whitespace), returns the first
///   character of each of the first two words, uppercased.
/// - If `name.trim()` has only one word with ≥2 chars, returns the first char
///   uppercased + the second char lowercased.
pub fn collection_initials(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "?".to_string();
    }

    let mut chars = trimmed.chars();
    let first = chars.next().unwrap();

    // Single char after trim
    if chars.next().is_none() {
        return first.to_string();
    }

    // Multiple words?
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() >= 2 {
        let c1 = words[0]
            .chars()
            .next()
            .unwrap()
            .to_uppercase()
            .next()
            .unwrap();
        let c2 = words[1]
            .chars()
            .next()
            .unwrap()
            .to_uppercase()
            .next()
            .unwrap();
        format!("{c1}{c2}")
    } else {
        // Single word, ≥2 chars
        let c1 = first.to_uppercase().next().unwrap();
        let c2 = trimmed
            .chars()
            .nth(1)
            .unwrap()
            .to_lowercase()
            .next()
            .unwrap();
        format!("{c1}{c2}")
    }
}

/// Determines whether a collection should show the active indicator (accent border).
///
/// This derivation is independent of `MainContentFocus` — the accent border
/// shows whenever the collection is active, regardless of whether the user is
/// viewing RequestTab or WorkspaceSection.
pub fn is_collection_active(active_id: Option<&str>, collection_id: &str) -> bool {
    active_id == Some(collection_id)
}

/// Deterministic background color for the collection icon square, derived
/// from the name. Uses a simple hash to produce a hue with fixed
/// saturation/lightness.
pub fn collection_icon_color(name: &str) -> Color {
    // Simple deterministic hash: sum of byte values
    let hash: u32 = name.bytes().map(|b| b as u32).sum();
    let hue = (hash % 360) as f32;
    hsl_to_color(hue, 0.55, 0.45)
}

// ─── View ───────────────────────────────────────────────────────────────────

/// Renders the Activity Bar column.
///
/// Layout (top to bottom):
/// 1. Home icon (fixed)
/// 2. Activity icon (fixed)
/// 3. One icon per collection from `state.workspace.collections` (in list order)
/// 4. "+" button at the end for creating a new collection
pub fn view<'a>(state: &'a Midway, ds: &DesignSystem) -> Element<'a, Message> {
    let mut items = column![].spacing(ds.spacing.xs).padding(ds.spacing.xs);

    // 1. Home icon (fixed) — navigates to Workspace_Panel (Req 11.1)
    items = items.push(
        tooltip(
            home_icon_button(state, ds),
            text(tooltip_text_for(&HoveredItem::Home)).size(ds.typography.secondary.size),
            TooltipPosition::Right,
        )
        .gap(4),
    );

    // 2. Activity icon (fixed)
    items = items.push(
        tooltip(
            fixed_icon_button(
                "⚡",
                "Activity",
                ds,
                Message::ActivityBar(ActivityBarMessage::ActivityPressed),
            ),
            text(tooltip_text_for(&HoveredItem::Activity)).size(ds.typography.secondary.size),
            TooltipPosition::Right,
        )
        .gap(4),
    );

    // 3. One icon per collection
    for collection in &state.workspace.collections {
        let is_active = is_collection_active(
            state.active_collection_id.as_deref(),
            &collection.collection.id,
        );
        let tooltip_label =
            tooltip_text_for(&HoveredItem::Collection(collection.collection.name.clone()))
                .to_string();
        items = items.push(
            tooltip(
                collection_icon_button(&collection.collection, is_active, ds),
                text(tooltip_label).size(ds.typography.secondary.size),
                TooltipPosition::Right,
            )
            .gap(4),
        );
    }

    // 4. "+" button
    let plus_text_color = ds.palette.text_primary;
    let plus_radius = ds.radius.control;
    let plus_btn = button(
        container(
            text("+")
                .size(ds.typography.subtitle.size)
                .color(plus_text_color),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .width(Length::Fixed(36.0))
    .height(Length::Fixed(36.0))
    .padding(0)
    .style(move |_theme, _status| button::Style {
        text_color: plus_text_color,
        border: Border {
            radius: plus_radius.into(),
            width: 1.0,
            color: plus_text_color,
        },
        ..button::Style::default()
    })
    .on_press(Message::ActivityBar(
        ActivityBarMessage::CreateCollectionPressed,
    ));

    items = items.push(
        tooltip(
            plus_btn,
            text(tooltip_text_for(&HoveredItem::CreateButton)).size(ds.typography.secondary.size),
            TooltipPosition::Right,
        )
        .gap(4),
    );

    let background = ds.palette.background_secondary;
    let border_color = ds.palette.border;

    container(items)
        .width(Length::Fixed(48.0))
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(background.into()),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

/// Muestra el prompt de creación en una capa con ancho propio.
///
/// El Activity Bar permanece intencionalmente angosto (48 px), por lo que
/// incrustar aquí un campo de texto lo recortaría. El overlay permite escribir
/// y revisar el nombre completo sin alterar el ancho de la navegación.
pub fn with_create_collection_overlay<'a>(
    state: &'a Midway,
    main_content: Element<'a, Message>,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let Some(prompt) = &state.create_collection_prompt else {
        return main_content;
    };

    stack![main_content, opaque(create_collection_overlay(prompt, ds))].into()
}

fn create_collection_overlay<'a>(
    prompt: &'a crate::app::CreateCollectionPromptState,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let overlay_background = Color {
        a: 0.45,
        ..ds.palette.background_primary
    };

    mouse_area(
        container(opaque(create_collection_prompt_view(prompt, ds)))
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

// ─── Internal widget builders ───────────────────────────────────────────────

/// Home icon button: navigates to the Workspace_Panel (Req 11.1, 11.11).
/// Accessible in ≤1 interaction from both Debug and Test modes.
/// Color derived from `main_content_focus` (Req 7.1, 7.2):
/// - accent when WorkspaceSection (active)
/// - text_secondary when RequestTab (inactive)
fn home_icon_button<'a>(state: &Midway, ds: &DesignSystem) -> Element<'a, Message> {
    let text_color = home_icon_color(state.main_content_focus, ds);
    let radius = ds.radius.control;

    button(
        container(
            text("⌂")
                .size(ds.typography.subtitle.size)
                .color(text_color),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .width(Length::Fixed(36.0))
    .height(Length::Fixed(36.0))
    .padding(0)
    .style(move |_theme, _status| button::Style {
        text_color,
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .on_press(Message::ActivityBar(ActivityBarMessage::HomePressed))
    .into()
}

fn fixed_icon_button<'a>(
    icon: &'static str,
    _label: &'static str,
    ds: &DesignSystem,
    on_press: Message,
) -> Element<'a, Message> {
    let text_color = ds.palette.text_secondary;
    let radius = ds.radius.control;

    button(
        container(
            text(icon)
                .size(ds.typography.subtitle.size)
                .color(text_color),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .width(Length::Fixed(36.0))
    .height(Length::Fixed(36.0))
    .padding(0)
    .style(move |_theme, _status| button::Style {
        text_color,
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .on_press(on_press)
    .into()
}

fn collection_icon_button<'a>(
    collection: &'a midway_core::domain::workspace::CollectionSummary,
    is_active: bool,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let initials = collection_initials(&collection.name);
    let bg_color = collection_icon_color(&collection.name);
    let text_color = Color::WHITE;
    let radius = ds.radius.control;
    let accent = ds.palette.accent;

    let border_color = if is_active {
        accent
    } else {
        Color::TRANSPARENT
    };

    let id = collection.id.clone();

    button(
        container(
            text(initials)
                .size(ds.typography.secondary.size)
                .color(text_color),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(bg_color.into()),
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }),
    )
    .width(Length::Fixed(36.0))
    .height(Length::Fixed(36.0))
    .padding(2)
    .style(move |_theme, _status| button::Style {
        border: Border {
            radius: radius.into(),
            width: if is_active { 2.0 } else { 0.0 },
            color: border_color,
        },
        ..button::Style::default()
    })
    .on_press(Message::ActivityBar(
        ActivityBarMessage::CollectionSelected(id),
    ))
    .into()
}

fn create_collection_prompt_view<'a>(
    prompt: &'a crate::app::CreateCollectionPromptState,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let mut col = column![text("Nueva colección")
        .size(ds.typography.subtitle.size)
        .color(ds.palette.text_primary)]
    .spacing(ds.spacing.sm);

    // Name input
    let input = text_input("Nombre...", &prompt.name_input)
        .on_input(|s| Message::ActivityBar(ActivityBarMessage::CreateCollectionNameChanged(s)))
        .on_submit(Message::ActivityBar(
            ActivityBarMessage::CreateCollectionConfirmed,
        ))
        .size(ds.typography.secondary.size)
        .width(Length::Fill);
    col = col.push(input);

    // Error message (if any)
    if let Some(error) = &prompt.error {
        col = col.push(
            text(error.clone())
                .size(ds.typography.secondary.size)
                .color(ds.palette.status_error),
        );
    }

    // Confirm / Cancel buttons
    let confirm_text_color = ds.palette.text_primary;
    let cancel_text_color = ds.palette.text_secondary;
    let radius = ds.radius.control;
    let accent = ds.palette.accent;

    let buttons = row![
        button(
            text("Crear")
                .size(ds.typography.secondary.size)
                .color(confirm_text_color)
        )
        .padding([ds.spacing.xs, ds.spacing.sm])
        .style(move |_theme, _status| button::Style {
            background: Some(accent.into()),
            text_color: Color::WHITE,
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .on_press(Message::ActivityBar(
            ActivityBarMessage::CreateCollectionConfirmed
        )),
        button(
            text("Cancelar")
                .size(ds.typography.secondary.size)
                .color(cancel_text_color)
        )
        .padding([ds.spacing.xs, ds.spacing.sm])
        .style(move |_theme, _status| button::Style {
            text_color: cancel_text_color,
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .on_press(Message::ActivityBar(
            ActivityBarMessage::CreateCollectionCancelled
        )),
    ]
    .spacing(ds.spacing.xs);

    col = col.push(buttons);

    let bg = ds.palette.surface_elevated;
    let border_color = ds.palette.border;
    let panel_radius = ds.radius.card;

    container(col)
        .padding(ds.spacing.sm)
        .width(Length::Fill)
        .max_width(320.0)
        .style(move |_theme| container::Style {
            background: Some(bg.into()),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: panel_radius.into(),
            },
            ..container::Style::default()
        })
        .into()
}

// ─── HSL helper (local, avoids pub use from design_system) ──────────────────

fn hsl_to_color(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = l - c / 2.0;
    Color::from_rgb(r1 + m, g1 + m, b1 + m)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_single_char() {
        assert_eq!(collection_initials("A"), "A");
        assert_eq!(collection_initials(" x "), "x");
    }

    #[test]
    fn initials_single_word_multiple_chars() {
        assert_eq!(collection_initials("hello"), "He");
        assert_eq!(collection_initials("  Payments  "), "Pa");
    }

    #[test]
    fn initials_two_words() {
        assert_eq!(collection_initials("My Collection"), "MC");
        assert_eq!(collection_initials("auth service"), "AS");
    }

    #[test]
    fn initials_more_than_two_words() {
        assert_eq!(collection_initials("one two three"), "OT");
    }

    #[test]
    fn initials_empty_returns_fallback() {
        assert_eq!(collection_initials(""), "?");
        assert_eq!(collection_initials("   "), "?");
    }

    #[test]
    fn initials_deterministic() {
        let name = "Test Collection";
        let a = collection_initials(name);
        let b = collection_initials(name);
        assert_eq!(a, b);
    }

    #[test]
    fn initials_max_two_chars() {
        let names = ["A", "AB", "Hello World", "x y z", "LongestNameEver"];
        for name in names {
            let initials = collection_initials(name);
            assert!(
                initials.chars().count() <= 2,
                "initials for '{name}' = '{initials}' exceeds 2 chars"
            );
        }
    }

    #[test]
    fn icon_color_is_deterministic() {
        let name = "My API";
        let a = collection_icon_color(name);
        let b = collection_icon_color(name);
        assert_eq!(a, b);
    }

    #[test]
    fn icon_color_differs_for_different_names() {
        let c1 = collection_icon_color("Alpha");
        let c2 = collection_icon_color("Beta");
        // Not guaranteed to differ for all inputs, but these specific ones should
        assert_ne!(c1, c2);
    }

    // ─── Tooltip text tests ─────────────────────────────────────────────────

    #[test]
    fn tooltip_text_home() {
        assert_eq!(tooltip_text_for(&HoveredItem::Home), "Workspace");
    }

    #[test]
    fn tooltip_text_activity() {
        assert_eq!(tooltip_text_for(&HoveredItem::Activity), "Activity");
    }

    #[test]
    fn tooltip_text_collection_shows_full_name() {
        let name = "My Long Collection Name".to_string();
        assert_eq!(
            tooltip_text_for(&HoveredItem::Collection(name.clone())),
            "My Long Collection Name"
        );
    }

    #[test]
    fn tooltip_text_create_button() {
        assert_eq!(
            tooltip_text_for(&HoveredItem::CreateButton),
            "Nueva colección"
        );
    }

    // ─── Property-based tests ───────────────────────────────────────────────

    mod property_tests {
        use super::*;
        use crate::ui::design_system::{DesignSystem, ThemeMode};
        use proptest::prelude::*;

        /// Strategy to generate arbitrary `HoveredItem` values.
        fn arb_hovered_item() -> impl Strategy<Value = HoveredItem> {
            prop_oneof![
                Just(HoveredItem::Home),
                Just(HoveredItem::Activity),
                Just(HoveredItem::CreateButton),
                "[^\\x00]{1,100}".prop_map(|name| HoveredItem::Collection(name)),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

            // Feature: ux-flow-redesign, Property 2: Tooltip text correctness
            /// **Validates: Requirements 2.1, 2.2, 2.3, 2.4**
            ///
            /// For any Activity_Bar item, `tooltip_text_for` returns:
            /// - "Workspace" for Home
            /// - "Activity" for Activity
            /// - the full collection name for Collection items
            /// - "Nueva colección" for CreateButton
            #[test]
            fn property_2_tooltip_text_correctness(item in arb_hovered_item()) {
                let result = tooltip_text_for(&item);
                match &item {
                    HoveredItem::Home => prop_assert_eq!(result, "Workspace"),
                    HoveredItem::Activity => prop_assert_eq!(result, "Activity"),
                    HoveredItem::Collection(name) => prop_assert_eq!(result, name.as_str()),
                    HoveredItem::CreateButton => prop_assert_eq!(result, "Nueva colección"),
                }
            }
        }

        // Feature: ux-flow-redesign, Property 8: Home icon color derivation
        /// Strategy to generate arbitrary `MainContentFocus` values.
        fn arb_main_content_focus() -> impl Strategy<Value = MainContentFocus> {
            prop_oneof![
                Just(MainContentFocus::RequestTab),
                Just(MainContentFocus::WorkspaceSection),
            ]
        }

        /// Strategy to generate arbitrary `ThemeMode` values for broader coverage.
        fn arb_theme_mode() -> impl Strategy<Value = ThemeMode> {
            prop_oneof![Just(ThemeMode::Light), Just(ThemeMode::Dark),]
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

            // Feature: ux-flow-redesign, Property 8: Home icon color derivation
            /// **Validates: Requirements 7.1, 7.2**
            ///
            /// For any `MainContentFocus` value, `home_icon_color` returns:
            /// - `ds.palette.accent` when `WorkspaceSection`
            /// - `ds.palette.text_secondary` when `RequestTab`
            #[test]
            fn property_8_home_icon_color_derivation(
                focus in arb_main_content_focus(),
                mode in arb_theme_mode(),
            ) {
                let ds = DesignSystem::for_mode(mode);
                let color = home_icon_color(focus, &ds);
                match focus {
                    MainContentFocus::WorkspaceSection => {
                        prop_assert_eq!(color, ds.palette.accent);
                    }
                    MainContentFocus::RequestTab => {
                        prop_assert_eq!(color, ds.palette.text_secondary);
                    }
                }
            }
        }

        // Feature: ux-flow-redesign, Property 9: Active collection indicator independence from focus
        /// Strategy to generate arbitrary collection IDs.
        fn arb_collection_id() -> impl Strategy<Value = String> {
            "[a-z0-9\\-]{1,36}".prop_map(|s| s)
        }

        /// Strategy to generate an optional active_collection_id that may or may not
        /// match a given collection_id.
        fn arb_active_id_and_collection_id() -> impl Strategy<Value = (Option<String>, String)> {
            arb_collection_id().prop_flat_map(|col_id| {
                let col_id_clone = col_id.clone();
                let active_id_strategy = prop_oneof![
                    // Case: active_id matches collection_id (collection is active)
                    Just(Some(col_id.clone())),
                    // Case: active_id is None (no collection active)
                    Just(None),
                    // Case: active_id is some other id (different collection active)
                    "[a-z0-9\\-]{1,36}".prop_map(|other| Some(other)),
                ];
                active_id_strategy.prop_map(move |active_id| (active_id, col_id_clone.clone()))
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

            // Feature: ux-flow-redesign, Property 9: Active collection indicator independence from focus
            /// **Validates: Requirements 7.3**
            ///
            /// For any application state with an `active_collection_id` of `Some(id)`,
            /// the collection icon for `id` displays the accent border indicator
            /// regardless of whether `main_content_focus` is `RequestTab` or
            /// `WorkspaceSection`.
            #[test]
            fn property_9_active_collection_indicator_independence_from_focus(
                (active_id, collection_id) in arb_active_id_and_collection_id(),
                focus in arb_main_content_focus(),
            ) {
                // The is_active derivation must be independent of main_content_focus.
                // We verify by computing is_active and confirming it depends only on
                // whether active_id matches collection_id, not on focus.
                let is_active = is_collection_active(active_id.as_deref(), &collection_id);

                let expected = active_id.as_deref() == Some(collection_id.as_str());
                prop_assert_eq!(
                    is_active, expected,
                    "is_collection_active should depend only on id match, not focus. \
                     active_id={:?}, collection_id={:?}, focus={:?}",
                    active_id, collection_id, focus
                );

                // Verify that changing focus does NOT change the result.
                let other_focus = match focus {
                    MainContentFocus::RequestTab => MainContentFocus::WorkspaceSection,
                    MainContentFocus::WorkspaceSection => MainContentFocus::RequestTab,
                };
                // is_collection_active doesn't take focus as parameter — this is the
                // independence guarantee. We re-check with _ = other_focus to document
                // the invariant: the function signature proves focus-independence.
                let _ = other_focus;
                let is_active_again = is_collection_active(active_id.as_deref(), &collection_id);
                prop_assert_eq!(
                    is_active, is_active_again,
                    "is_collection_active must be deterministic and focus-independent"
                );
            }
        }
    }
}
