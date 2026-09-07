//! Vista de onboarding: se muestra cuando no existen colecciones y el
//! usuario está en `MainContentFocus::RequestTab`.
//!
//! Guía al usuario a crear su primera colección reutilizando el flujo
//! existente del Activity_Bar (botón "+").

use iced::widget::{button, column, container, text};
use iced::{Alignment, Border, Element, Length};

use crate::app::{ActivityBarMessage, MainContentFocus, Message, Midway};
use crate::ui::design_system::DesignSystem;

/// Determina si se debe mostrar la vista de onboarding.
///
/// Retorna `true` si y solo si no existen colecciones en el workspace
/// y el foco principal es `RequestTab`.
pub fn should_show_onboarding(state: &Midway) -> bool {
    state.workspace.collections.is_empty()
        && state.main_content_focus == MainContentFocus::RequestTab
}

/// Renderiza la vista de onboarding: un título de bienvenida y un botón
/// (Guided_Action) que emite `CreateCollectionPressed` para iniciar el
/// flujo de creación de colección.
pub fn view<'a>(ds: &DesignSystem) -> Element<'a, Message> {
    let title = text("Bienvenido a Midway")
        .size(ds.typography.title.size)
        .color(ds.palette.text_primary);

    let subtitle = text("Creá tu primera colección para empezar a trabajar con tus APIs.")
        .size(ds.typography.body.size)
        .color(ds.palette.text_secondary);

    let accent = ds.palette.accent;
    let radius = ds.radius.control;

    let create_button = button(
        text("Crear colección")
            .size(ds.typography.body.size)
            .color(iced::Color::WHITE),
    )
    .padding([ds.spacing.sm, ds.spacing.md])
    .style(move |_theme, _status| button::Style {
        background: Some(accent.into()),
        text_color: iced::Color::WHITE,
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .on_press(Message::ActivityBar(
        ActivityBarMessage::CreateCollectionPressed,
    ));

    let content = column![title, subtitle, create_button]
        .spacing(ds.spacing.md)
        .align_x(Alignment::Center);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

// Feature: ux-flow-redesign, Property 1: Onboarding visibility invariant
#[cfg(test)]
mod tests {
    //! Feature: ux-flow-redesign, Property 1: Onboarding visibility invariant
    //! Validates: Requirements 1.1, 1.4

    use super::*;
    use crate::app::{
        MainContentFocus, Midway, PaletteState, RequestTabState, SessionStoreState, TopBarMode,
        TreeViewState, UpdaterState, WorkspacePanelState,
    };
    use crate::curl::create_blank_draft;
    use crate::state::AppState;
    use midway_core::domain::cookies::CookieJarHandle;
    use midway_core::domain::workspace::{CollectionSummary, CollectionWithRequests};
    use midway_core::infra::sqlite_repository::SqliteRepository;
    use midway_core::runtime::request_executor::RequestExecutorHandle;
    use midway_core::runtime::secret_executor::SecretExecutorHandle;
    use proptest::prelude::*;
    use std::collections::VecDeque;
    use std::sync::Arc;

    /// Build a minimal AppState for testing (opens a temp SQLite DB).
    fn build_test_app_state() -> AppState {
        let temp_file =
            tempfile::NamedTempFile::new().expect("failed to create temp file for test AppState");
        let db_path = temp_file.path().to_path_buf();

        let runtime = tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime for test AppState");

        let app_state = runtime.block_on(async {
            let repository = SqliteRepository::open(&db_path)
                .await
                .expect("failed to open SqliteRepository for test");

            let client = reqwest::Client::builder()
                .build()
                .expect("failed to build reqwest client for test");

            AppState {
                repository,
                request_executor: RequestExecutorHandle::spawn(client),
                secret_executor: SecretExecutorHandle::spawn("midway-test".to_string()),
                cookie_jar: CookieJarHandle::new(),
            }
        });

        drop(temp_file);
        app_state
    }

    /// Build a test Midway state with the given collections and focus.
    fn build_test_midway_with(
        app_state: Arc<AppState>,
        collections: Vec<CollectionWithRequests>,
        focus: MainContentFocus,
    ) -> Midway {
        Midway {
            app_state,
            workspace: midway_core::domain::workspace::WorkspaceSnapshot {
                collections,
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
            tabs: vec![RequestTabState::from_draft(create_blank_draft())],
            active_tab: Some(0),
            closed_tabs: VecDeque::new(),
            workspace_panel: WorkspacePanelState::default(),
            palette: PaletteState::default(),
            runner: None,
            session: SessionStoreState::default(),
            theme: crate::ui::theme_settings::ThemeSettingsState::default(),
            main_content_focus: focus,
            updater: UpdaterState::default(),
            crash_log: Vec::new(),
            unsaved_changes_prompt: None,
            active_collection_id: None,
            top_bar_mode: TopBarMode::default(),
            tree: TreeViewState::default(),
            create_collection_prompt: None,
            save_request_prompt: None,
            workspace_crud_dialog: None,
            panel_dragging: None,
            panel_hovered: None,
        }
    }

    /// Strategy to generate a collection id.
    fn collection_id_strategy() -> impl Strategy<Value = String> {
        "[a-z0-9]{1,20}".prop_map(|s| s.to_string())
    }

    /// Strategy to generate a CollectionWithRequests with a given id.
    fn collection_with_id(id: String) -> CollectionWithRequests {
        CollectionWithRequests {
            collection: CollectionSummary {
                id,
                name: "test-collection".to_string(),
                request_count: 0,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
            },
            folders: Vec::new(),
            requests: Vec::new(),
        }
    }

    /// Strategy to generate a collections list that is either empty or non-empty.
    fn collections_strategy() -> impl Strategy<Value = Vec<CollectionWithRequests>> {
        prop_oneof![
            // Empty collections (50% of cases)
            5 => Just(Vec::new()),
            // Non-empty collections (50% of cases)
            5 => proptest::collection::vec(collection_id_strategy(), 1..=5)
                .prop_map(|ids| {
                    let mut seen = std::collections::HashSet::new();
                    ids.into_iter()
                        .filter(|id| seen.insert(id.clone()))
                        .map(collection_with_id)
                        .collect::<Vec<_>>()
                })
                .prop_filter("must have at least one collection", |v| !v.is_empty()),
        ]
    }

    /// Strategy to generate an arbitrary MainContentFocus.
    fn main_content_focus_strategy() -> impl Strategy<Value = MainContentFocus> {
        prop_oneof![
            Just(MainContentFocus::RequestTab),
            Just(MainContentFocus::WorkspaceSection),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        // Feature: ux-flow-redesign, Property 1: Onboarding visibility invariant
        /// **Validates: Requirements 1.1, 1.4**
        ///
        /// For any application state, `should_show_onboarding(state)` returns
        /// `true` if and only if `state.workspace.collections.is_empty()` AND
        /// `state.main_content_focus == MainContentFocus::RequestTab`.
        /// For all other states, it returns `false`.
        #[test]
        fn property_1_onboarding_visibility_invariant(
            collections in collections_strategy(),
            focus in main_content_focus_strategy(),
        ) {
            let app_state = Arc::new(build_test_app_state());
            let state = build_test_midway_with(
                app_state,
                collections.clone(),
                focus,
            );

            let result = should_show_onboarding(&state);

            let expected = collections.is_empty()
                && focus == MainContentFocus::RequestTab;

            prop_assert_eq!(
                result,
                expected,
                "should_show_onboarding must return true iff collections is empty AND focus is RequestTab. \
                 Got: {}, collections.len(): {}, focus: {:?}",
                result,
                collections.len(),
                focus,
            );
        }
    }
}
