//! Top_Bar: breadcrumb de ubicación, tabs de modo Debug/Test y control de tema.
//!
//! Tarea 12.1 — Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 12.1.
//!
//! Breadcrumb: "Midway / {colección}" con colección activa; solo "Midway" sin
//! colección (Req 6.1, 6.2). Exactamente dos tabs Debug/Test, sin "Design"
//! (Req 6.3, 12.1). Tab activa indicada con `Palette.accent` (Req 6.6). Test
//! deshabilitada sin colección activa (Req 6.7). Control de tema
//! (`ThemeMessage::Toggled`) discoverable en el Top_Bar (Req 1.6).

use iced::widget::{button, container, row, text};
use iced::{Border, Element, Length};

use crate::app::{
    MainContentFocus, Message, Midway, ThemeMessage, TopBarMessage, TopBarMode,
    WorkspacePanelSection,
};
use crate::ui::design_system::{DesignSystem, ThemeMode};

// ---------------------------------------------------------------------------
// Breadcrumb segment model (Task 4.1, Requirements 3.1–3.5)
// ---------------------------------------------------------------------------

/// A single segment in the breadcrumb trail rendered by the Top_Bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    pub text: String,
    pub clickable: bool,
}

/// Builds the breadcrumb segments based on the current navigation state.
///
/// Rules:
/// - `RequestTab` + active collection → `["Midway", col_name, mode_label]`
/// - `RequestTab` + no collection    → `["Midway", mode_label]`
/// - `WorkspaceSection`              → `["Midway", "Workspace", section_label]`
///
/// The first segment ("Midway") is always clickable.
pub fn breadcrumb_segments(state: &Midway) -> Vec<BreadcrumbSegment> {
    let root = BreadcrumbSegment {
        text: "Midway".to_string(),
        clickable: true,
    };

    match state.main_content_focus {
        MainContentFocus::RequestTab => {
            let mode_label = match state.top_bar_mode {
                TopBarMode::Debug => "Debug",
                TopBarMode::Test => "Test",
            };

            match active_collection_name(state) {
                Some(name) => vec![
                    root,
                    BreadcrumbSegment {
                        text: name.to_string(),
                        clickable: false,
                    },
                    BreadcrumbSegment {
                        text: mode_label.to_string(),
                        clickable: false,
                    },
                ],
                None => vec![
                    root,
                    BreadcrumbSegment {
                        text: mode_label.to_string(),
                        clickable: false,
                    },
                ],
            }
        }
        MainContentFocus::WorkspaceSection => {
            let section_label = section_display_label(state.workspace_panel.active_section);
            vec![
                root,
                BreadcrumbSegment {
                    text: "Workspace".to_string(),
                    clickable: false,
                },
                BreadcrumbSegment {
                    text: section_label.to_string(),
                    clickable: false,
                },
            ]
        }
    }
}

/// Returns the human-readable label for a workspace panel section.
fn section_display_label(section: WorkspacePanelSection) -> &'static str {
    match section {
        WorkspacePanelSection::Environments => "Environments",
        WorkspacePanelSection::Data => "Data",
        WorkspacePanelSection::History => "History",
        WorkspacePanelSection::Diagnostics => "Diagnostics",
        WorkspacePanelSection::AppUpdates => "App updates",
    }
}

/// Renders the Top Bar: breadcrumb + Back button + mode tabs + theme toggle.
///
/// Task 10.1 — Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 4.5
///
/// The breadcrumb is built dynamically from `breadcrumb_segments(state)`:
/// - Clickable segments are rendered as buttons.
/// - Non-clickable segments are rendered as plain text.
/// - Segments are separated by " / " dividers.
/// - When in `WorkspaceSection`, a "← Back" button is shown.
pub fn view<'a>(state: &'a Midway, ds: &DesignSystem) -> Element<'a, Message> {
    // --- Dynamic breadcrumb (Req 3.1–3.5) ---
    let segments = breadcrumb_segments(state);
    let breadcrumb = build_breadcrumb_row(&segments, ds);

    // --- Back button (Req 4.5): visible only in WorkspaceSection ---
    let back_button: Option<Element<'a, Message>> =
        if state.main_content_focus == MainContentFocus::WorkspaceSection {
            let text_color = ds.palette.text_secondary;
            let radius = ds.radius.control;
            Some(
                button(text("← Back").size(ds.typography.body.size).color(text_color))
                    .padding([ds.spacing.xs, ds.spacing.sm])
                    .style(move |_theme, _status| button::Style {
                        text_color,
                        border: Border {
                            radius: radius.into(),
                            ..Border::default()
                        },
                        ..button::Style::default()
                    })
                    .on_press(Message::TopBar(TopBarMessage::BackToComposer))
                    .into(),
            )
        } else {
            None
        };

    // --- Mode tabs: Debug and Test (no Design — Req 6.3, 12.1) ---
    let debug_tab = mode_tab(TopBarMode::Debug, "Debug", state, ds);
    let test_tab = mode_tab(TopBarMode::Test, "Test", state, ds);
    let tabs = row![debug_tab, test_tab].spacing(ds.spacing.sm);

    // --- Theme toggle (Req 1.6) ---
    let theme_icon = match state.theme_mode {
        ThemeMode::Light => "☀",
        ThemeMode::Dark => "☾",
    };
    let theme_text_color = ds.palette.text_primary;
    let theme_radius = ds.radius.control;
    let theme_toggle = button(text(theme_icon).size(ds.typography.body.size).color(theme_text_color))
        .padding(ds.spacing.sm)
        .style(move |_theme, _status| button::Style {
            text_color: theme_text_color,
            border: Border {
                radius: theme_radius.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .on_press(Message::Theme(ThemeMessage::Toggled));

    // --- Compose the Top Bar row ---
    let background = ds.palette.background_secondary;
    let border_color = ds.palette.border;

    let mut bar = row![breadcrumb]
        .spacing(ds.spacing.md)
        .align_y(iced::Alignment::Center);

    if let Some(back_btn) = back_button {
        bar = bar.push(back_btn);
    }

    let bar = bar.push(tabs).push(theme_toggle).width(Length::Fill);

    container(bar)
        .width(Length::Fill)
        .padding([ds.spacing.sm, ds.spacing.md])
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

/// Builds the breadcrumb row from segments, rendering clickable segments as
/// buttons and non-clickable ones as plain text, separated by " / " dividers.
fn build_breadcrumb_row<'a>(
    segments: &[BreadcrumbSegment],
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let mut breadcrumb_row = row![].spacing(0).align_y(iced::Alignment::Center);

    for (i, segment) in segments.iter().enumerate() {
        // Add divider before all segments except the first
        if i > 0 {
            let divider_color = ds.palette.text_secondary;
            breadcrumb_row = breadcrumb_row.push(
                text(" / ")
                    .size(ds.typography.subtitle.size)
                    .color(divider_color),
            );
        }

        if segment.clickable {
            // Clickable segment: rendered as a button
            let text_color = ds.palette.accent;
            let radius = ds.radius.control;
            let segment_text = segment.text.clone();
            let btn = button(
                text(segment_text)
                    .size(ds.typography.subtitle.size)
                    .color(text_color),
            )
            .padding(0)
            .style(move |_theme, _status| button::Style {
                text_color,
                border: Border {
                    radius: radius.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            })
            .on_press(Message::TopBar(TopBarMessage::BreadcrumbRootClicked));
            breadcrumb_row = breadcrumb_row.push(btn);
        } else {
            // Non-clickable segment: plain text
            let text_color = ds.palette.text_primary;
            let segment_text = segment.text.clone();
            breadcrumb_row = breadcrumb_row.push(
                text(segment_text)
                    .size(ds.typography.subtitle.size)
                    .color(text_color),
            );
        }
    }

    breadcrumb_row.into()
}

/// Constructs a single mode tab button. The active tab is highlighted with
/// `Palette.accent` (Req 6.6). Test is disabled when there's no active
/// collection (Req 6.7).
fn mode_tab<'a>(
    mode: TopBarMode,
    label: &'static str,
    state: &'a Midway,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let is_active = state.top_bar_mode == mode;
    let is_disabled = mode == TopBarMode::Test && state.active_collection_id.is_none();

    let accent = ds.palette.accent;
    let text_primary = ds.palette.text_primary;
    let text_secondary = ds.palette.text_secondary;
    let radius = ds.radius.control;

    let label_color = if is_disabled {
        text_secondary
    } else if is_active {
        accent
    } else {
        text_primary
    };

    let tab_background = if is_active { Some(accent) } else { None };
    let tab_text_color = if is_active {
        // White text on accent background for contrast
        iced::Color::WHITE
    } else {
        label_color
    };

    let btn = button(text(label).size(ds.typography.body.size).color(tab_text_color))
        .padding([ds.spacing.xs, ds.spacing.sm])
        .style(move |_theme, _status| button::Style {
            background: tab_background.map(Into::into),
            text_color: tab_text_color,
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });

    if is_disabled {
        btn.into()
    } else {
        btn.on_press(Message::TopBar(TopBarMessage::ModeSelected(mode)))
            .into()
    }
}

/// Returns the name of the active collection, if any.
fn active_collection_name(state: &Midway) -> Option<&str> {
    let active_id = state.active_collection_id.as_deref()?;
    state
        .workspace
        .collections
        .iter()
        .find(|c| c.collection.id == active_id)
        .map(|c| c.collection.name.as_str())
}

// Feature: ux-flow-redesign, Property 3: Breadcrumb format correctness
#[cfg(test)]
mod breadcrumb_property_tests {
    //! Feature: ux-flow-redesign, Property 3: Breadcrumb format correctness
    //! Validates: Requirements 3.1, 3.2, 3.3

    use super::*;
    use crate::app::{MainContentFocus, TopBarMode, WorkspacePanelSection};
    use midway_core::domain::workspace::{CollectionSummary, CollectionWithRequests};
    use proptest::prelude::*;

    /// Strategy to generate an arbitrary `MainContentFocus`.
    fn arb_main_content_focus() -> impl Strategy<Value = MainContentFocus> {
        prop_oneof![
            Just(MainContentFocus::RequestTab),
            Just(MainContentFocus::WorkspaceSection),
        ]
    }

    /// Strategy to generate an arbitrary `TopBarMode`.
    fn arb_top_bar_mode() -> impl Strategy<Value = TopBarMode> {
        prop_oneof![Just(TopBarMode::Debug), Just(TopBarMode::Test),]
    }

    /// Strategy to generate an arbitrary `WorkspacePanelSection`.
    fn arb_workspace_panel_section() -> impl Strategy<Value = WorkspacePanelSection> {
        prop_oneof![
            Just(WorkspacePanelSection::Environments),
            Just(WorkspacePanelSection::Data),
            Just(WorkspacePanelSection::History),
            Just(WorkspacePanelSection::Diagnostics),
            Just(WorkspacePanelSection::AppUpdates),
        ]
    }

    /// Strategy to generate a collection name (non-empty string with printable chars).
    fn arb_collection_name() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9 _-]{1,30}".prop_map(|s| s.to_string())
    }

    /// Strategy to generate an optional active collection (id + name pair, or None).
    /// When Some, also produces the matching `CollectionWithRequests` to add to state.
    fn arb_active_collection(
    ) -> impl Strategy<Value = (Option<String>, Option<CollectionWithRequests>)> {
        prop_oneof![
            // No active collection
            3 => Just((None, None)),
            // Active collection with a generated name
            7 => arb_collection_name().prop_map(|name| {
                let id = format!("col-{}", name.replace(' ', "-"));
                let collection = CollectionWithRequests {
                    collection: CollectionSummary {
                        id: id.clone(),
                        name: name.clone(),
                        request_count: 0,
                        created_at: "2024-01-01T00:00:00Z".to_string(),
                        updated_at: "2024-01-01T00:00:00Z".to_string(),
                    },
                    folders: Vec::new(),
                    requests: Vec::new(),
                };
                (Some(id), Some(collection))
            }),
        ]
    }

    /// Builds a minimal `Midway` test state with the given navigation parameters.
    /// Uses `build_test_midway` from the parent test helpers via a direct construction
    /// that mirrors it but without needing AppState (we only need the pure function).
    fn build_breadcrumb_test_state(
        focus: MainContentFocus,
        active_collection_id: Option<String>,
        collection: Option<CollectionWithRequests>,
        mode: TopBarMode,
        section: WorkspacePanelSection,
    ) -> Midway {
        use crate::state::AppState;
        use midway_core::domain::cookies::CookieJarHandle;
        use midway_core::runtime::request_executor::RequestExecutorHandle;
        use midway_core::runtime::secret_executor::SecretExecutorHandle;
        use std::collections::VecDeque;
        use std::sync::Arc;

        let temp_file = tempfile::NamedTempFile::new()
            .expect("no se pudo crear archivo temporal");
        let db_path = temp_file.path().to_path_buf();

        let runtime = tokio::runtime::Runtime::new()
            .expect("no se pudo crear runtime de tokio");

        let app_state = runtime.block_on(async {
            use midway_core::infra::sqlite_repository::SqliteRepository;
            let repository = SqliteRepository::open(&db_path)
                .await
                .expect("no se pudo abrir SqliteRepository");
            let client = reqwest::Client::builder()
                .build()
                .expect("no se pudo construir cliente reqwest");
            AppState {
                repository,
                request_executor: RequestExecutorHandle::spawn(client),
                secret_executor: SecretExecutorHandle::spawn("midway-test".to_string()),
                cookie_jar: CookieJarHandle::new(),
            }
        });

        drop(temp_file);

        let collections = match collection {
            Some(c) => vec![c],
            None => Vec::new(),
        };

        Midway {
            app_state: Arc::new(app_state),
            workspace: midway_core::domain::workspace::WorkspaceSnapshot {
                collections,
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
            tabs: vec![crate::app::RequestTabState::blank()],
            active_tab: Some(0),
            closed_tabs: VecDeque::new(),
            workspace_panel: crate::app::WorkspacePanelState {
                active_section: section,
                ..Default::default()
            },
            palette: crate::app::PaletteState::default(),
            runner: None,
            session: crate::app::SessionStoreState::default(),
            theme_mode: ThemeMode::default(),
            main_content_focus: focus,
            updater: crate::app::UpdaterState::default(),
            crash_log: Vec::new(),
            unsaved_changes_prompt: None,
            active_collection_id,
            top_bar_mode: mode,
            tree: crate::app::TreeViewState::default(),
            create_collection_prompt: None,
            save_request_prompt: None,
            workspace_crud_dialog: None,
            panel_dragging: None,
            panel_hovered: None,
        }
    }

    /// Returns the expected mode label for a given `TopBarMode`.
    fn expected_mode_label(mode: TopBarMode) -> &'static str {
        match mode {
            TopBarMode::Debug => "Debug",
            TopBarMode::Test => "Test",
        }
    }

    /// Returns the expected section label for a given `WorkspacePanelSection`.
    fn expected_section_label(section: WorkspacePanelSection) -> &'static str {
        match section {
            WorkspacePanelSection::Environments => "Environments",
            WorkspacePanelSection::Data => "Data",
            WorkspacePanelSection::History => "History",
            WorkspacePanelSection::Diagnostics => "Diagnostics",
            WorkspacePanelSection::AppUpdates => "App updates",
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// **Validates: Requirements 3.1, 3.2, 3.3**
        ///
        /// Property 3: For any valid navigation state, `breadcrumb_segments` produces:
        /// (a) ["Midway", col_name, mode_label] when focus is RequestTab and collection is active
        /// (b) ["Midway", mode_label] when focus is RequestTab with no active collection
        /// (c) ["Midway", "Workspace", section_label] when focus is WorkspaceSection
        /// The first segment is always clickable.
        #[test]
        fn property_3_breadcrumb_format_correctness(
            focus in arb_main_content_focus(),
            (active_id, collection) in arb_active_collection(),
            mode in arb_top_bar_mode(),
            section in arb_workspace_panel_section(),
        ) {
            let state = build_breadcrumb_test_state(
                focus,
                active_id.clone(),
                collection.clone(),
                mode,
                section,
            );

            let segments = breadcrumb_segments(&state);

            // First segment is always "Midway" and always clickable
            prop_assert!(!segments.is_empty(), "breadcrumb must have at least one segment");
            prop_assert_eq!(&segments[0].text, "Midway");
            prop_assert!(segments[0].clickable, "first segment 'Midway' must always be clickable");

            match focus {
                MainContentFocus::RequestTab => {
                    if active_id.is_some() && collection.is_some() {
                        // Case (a): RequestTab + active collection → 3 segments
                        let col_name = &collection.unwrap().collection.name;
                        prop_assert_eq!(
                            segments.len(), 3,
                            "RequestTab + active collection should produce 3 segments, got {:?}",
                            segments
                        );
                        prop_assert_eq!(&segments[1].text, col_name);
                        prop_assert_eq!(&segments[2].text, expected_mode_label(mode));
                        // Non-root segments are not clickable
                        prop_assert!(!segments[1].clickable);
                        prop_assert!(!segments[2].clickable);
                    } else {
                        // Case (b): RequestTab + no collection → 2 segments
                        prop_assert_eq!(
                            segments.len(), 2,
                            "RequestTab + no collection should produce 2 segments, got {:?}",
                            segments
                        );
                        prop_assert_eq!(&segments[1].text, expected_mode_label(mode));
                        prop_assert!(!segments[1].clickable);
                    }
                }
                MainContentFocus::WorkspaceSection => {
                    // Case (c): WorkspaceSection → 3 segments
                    prop_assert_eq!(
                        segments.len(), 3,
                        "WorkspaceSection should produce 3 segments, got {:?}",
                        segments
                    );
                    prop_assert_eq!(&segments[1].text, "Workspace");
                    prop_assert_eq!(&segments[2].text, expected_section_label(section));
                    prop_assert!(!segments[1].clickable);
                    prop_assert!(!segments[2].clickable);
                }
            }
        }
    }
}
