//! Request_Tree_Pane: árbol colapsable de folders y requests de la colección
//! activa con filtro de texto (Tarea 11.1, Requisitos 5.1–5.11).
//!
//! Expone funciones puras `build_tree` y `filter_tree` para construir y
//! filtrar el árbol (testeables sin UI), y una función `view` que renderiza
//! el componente completo con estados vacíos, filtro y expansión/colapso.
#![allow(dead_code)]

use std::collections::HashSet;

use iced::widget::{
    button, column, container, mouse_area, row, scrollable, space, text, text_input, tooltip,
};
use iced::{font, Border, Element, Font, Length};

use midway_core::domain::workspace::{Folder, SavedRequestRecord};

use crate::app::{
    ActivityBarMessage, Message, Midway, RequestComposerMessage, RequestDragState,
    RequestDropTarget, TreeHoveredItem, TreeMessage, TreeViewState, WorkspaceCrudMessage,
};
use crate::ui::design_system::{method_color, selectable_item_style, DesignSystem, TextStyle};

// ─── Tree pane state derivation ─────────────────────────────────────────────

/// High-level state the tree pane should render. Derived from `Midway` state
/// by [`determine_tree_state`] (UX Flow Redesign, Requirements 5.1, 5.3, 5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreePaneState {
    /// Active collection has content (normal tree).
    Normal,
    /// Active collection is empty (no requests/folders).
    EmptyCollection,
    /// No collection is active but collections exist.
    NoActiveCollection,
    /// No collections exist (onboarding handles this in main panel).
    NoCollections,
}

/// Determines which state the tree pane should render based on application state.
///
/// Rules (evaluated in order):
/// - `NoCollections`: `workspace.collections.is_empty()`
/// - `NoActiveCollection`: `active_collection_id.is_none()` AND collections is non-empty
/// - `EmptyCollection`: `active_collection_id` is Some, the corresponding collection has
///   zero requests and folders, AND the filter text is empty
/// - `Normal`: otherwise (collection has content, or filter is active)
pub fn determine_tree_state(state: &Midway) -> TreePaneState {
    if state.workspace.collections.is_empty() {
        return TreePaneState::NoCollections;
    }

    let active_id = match &state.active_collection_id {
        Some(id) => id,
        None => return TreePaneState::NoActiveCollection,
    };

    // Find the active collection
    let collection = state
        .workspace
        .collections
        .iter()
        .find(|c| c.collection.id == *active_id);

    match collection {
        Some(col) => {
            let is_empty = col.requests.is_empty() && col.folders.is_empty();
            let filter_empty = state.tree.filter.trim().is_empty();

            if is_empty && filter_empty {
                TreePaneState::EmptyCollection
            } else {
                TreePaneState::Normal
            }
        }
        // If active_collection_id references a non-existent collection, treat as no active
        None => TreePaneState::NoActiveCollection,
    }
}

// ─── Pure tree types and functions ──────────────────────────────────────────

/// Nodo del árbol de folders/requests. La raíz tiene `folder: None`.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub folder: Option<Folder>,
    pub child_folders: Vec<TreeNode>,
    pub requests: Vec<SavedRequestRecord>,
}

/// Construye el árbol ordenado: en cada nivel, primero folders (alfabético
/// case-insensitive por nombre) y luego requests (alfabético case-insensitive
/// por nombre) (Req 5.1).
pub fn build_tree(folders: &[Folder], requests: &[SavedRequestRecord]) -> TreeNode {
    // Collect root-level folders (parent_folder_id == None)
    let mut root_folders: Vec<&Folder> = folders
        .iter()
        .filter(|f| f.parent_folder_id.is_none())
        .collect();
    root_folders.sort_by_key(|folder| folder.name.to_lowercase());

    // Collect root-level requests (folder_id == None)
    let mut root_requests: Vec<SavedRequestRecord> = requests
        .iter()
        .filter(|r| r.folder_id.is_none())
        .cloned()
        .collect();
    root_requests.sort_by_key(|request| request.name.to_lowercase());

    let child_folder_nodes = root_folders
        .into_iter()
        .map(|f| build_subtree(f, folders, requests))
        .collect();

    TreeNode {
        folder: None,
        child_folders: child_folder_nodes,
        requests: root_requests,
    }
}

/// Recursively builds a subtree rooted at the given folder.
fn build_subtree(
    folder: &Folder,
    all_folders: &[Folder],
    all_requests: &[SavedRequestRecord],
) -> TreeNode {
    let mut children: Vec<&Folder> = all_folders
        .iter()
        .filter(|f| f.parent_folder_id.as_deref() == Some(&folder.id))
        .collect();
    children.sort_by_key(|child| child.name.to_lowercase());

    let mut folder_requests: Vec<SavedRequestRecord> = all_requests
        .iter()
        .filter(|r| r.folder_id.as_deref() == Some(&folder.id))
        .cloned()
        .collect();
    folder_requests.sort_by_key(|request| request.name.to_lowercase());

    let child_folder_nodes = children
        .into_iter()
        .map(|f| build_subtree(f, all_folders, all_requests))
        .collect();

    TreeNode {
        folder: Some(folder.clone()),
        child_folders: child_folder_nodes,
        requests: folder_requests,
    }
}

/// Filtra el árbol dejando solo requests cuyo nombre contenga `query`
/// (case-insensitive) más sus folders ancestras (Req 5.2).
///
/// Retorna `None` si `query` no es vacío y no hay coincidencias, señalizando
/// el estado "sin resultados" (Req 5.10).
pub fn filter_tree(tree: &TreeNode, query: &str) -> Option<TreeNode> {
    let query = query.trim();
    if query.is_empty() {
        return Some(tree.clone());
    }

    let query_lower = query.to_lowercase();
    let filtered = filter_node(tree, &query_lower);

    match filtered {
        Some(node) if node.child_folders.is_empty() && node.requests.is_empty() => None,
        Some(node) => Some(node),
        None => None,
    }
}

/// Recursively filters a node: keeps requests matching the query plus their
/// ancestor folders.
fn filter_node(node: &TreeNode, query_lower: &str) -> Option<TreeNode> {
    // Filter child folders recursively
    let filtered_children: Vec<TreeNode> = node
        .child_folders
        .iter()
        .filter_map(|child| filter_node(child, query_lower))
        .collect();

    // Filter requests by name containing query (case-insensitive)
    let filtered_requests: Vec<SavedRequestRecord> = node
        .requests
        .iter()
        .filter(|r| r.name.to_lowercase().contains(query_lower))
        .cloned()
        .collect();

    // Keep this node if it has any matching content (either matching requests
    // or child folders that contain matches)
    if filtered_children.is_empty() && filtered_requests.is_empty() {
        None
    } else {
        Some(TreeNode {
            folder: node.folder.clone(),
            child_folders: filtered_children,
            requests: filtered_requests,
        })
    }
}

/// Collects all folder ids from the tree (used to force-expand ancestors
/// when filtering).
pub fn collect_all_folder_ids(tree: &TreeNode) -> HashSet<String> {
    let mut ids = HashSet::new();
    collect_folder_ids_recursive(tree, &mut ids);
    ids
}

fn collect_folder_ids_recursive(node: &TreeNode, ids: &mut HashSet<String>) {
    if let Some(ref folder) = node.folder {
        ids.insert(folder.id.clone());
    }
    for child in &node.child_folders {
        collect_folder_ids_recursive(child, ids);
    }
}

// ─── Update logic for TreeMessage ───────────────────────────────────────────

/// Handles `TreeMessage` variants, mutating `Midway` state and returning a
/// `Task` for async operations.
pub fn update_tree(state: &mut Midway, message: TreeMessage) -> iced::Task<Message> {
    match message {
        TreeMessage::FilterChanged(new_text) => {
            let was_empty = state.tree.filter.trim().is_empty();
            let is_empty = new_text.trim().is_empty();

            if was_empty && !is_empty {
                // Going from empty to non-empty: snapshot current collapsed
                // state before forcing expansion (Req 5.11).
                state.tree.collapsed_snapshot = Some(state.tree.collapsed.clone());
                // Force-expand all folders (clear collapsed set so all are visible)
                state.tree.collapsed.clear();
            } else if !was_empty && is_empty {
                // Going from non-empty to empty: restore the snapshot (Req 5.11).
                if let Some(snapshot) = state.tree.collapsed_snapshot.take() {
                    state.tree.collapsed = snapshot;
                }
            }

            state.tree.filter = new_text;
            state.tree.error = None;
            iced::Task::none()
        }
        TreeMessage::FolderToggled(folder_id) => {
            // Toggle: if collapsed, expand; if expanded, collapse (involución, Req 5.4).
            if state.tree.collapsed.contains(&folder_id) {
                state.tree.collapsed.remove(&folder_id);
            } else {
                state.tree.collapsed.insert(folder_id);
            }
            iced::Task::none()
        }
        TreeMessage::RequestOpened(request_id) => {
            // Try to open the request in a composer tab (reuse empty or create new, Req 5.6).
            handle_request_opened(state, request_id)
        }
        TreeMessage::RequestEditRequested(request_id) => {
            // Reuse/open the persisted request, then expose its name/location editor.
            let _ = handle_request_opened(state, request_id);
            if state.tree.error.is_none() {
                iced::Task::done(Message::RequestComposer(
                    RequestComposerMessage::SaveRequested,
                ))
            } else {
                iced::Task::none()
            }
        }
        TreeMessage::RequestDragStarted(request_id) => {
            if state.tree.moving_request_id.is_some() {
                return iced::Task::none();
            }
            let exists = state
                .active_collection_id
                .as_deref()
                .and_then(|collection_id| {
                    state
                        .workspace
                        .collections
                        .iter()
                        .find(|collection| collection.collection.id == collection_id)
                })
                .is_some_and(|collection| {
                    collection
                        .requests
                        .iter()
                        .any(|request| request.id == request_id)
                });
            if !exists {
                state.tree.error =
                    Some("No se pudo iniciar el movimiento del request.".to_string());
                return iced::Task::none();
            }

            state.tree.request_drag = Some(RequestDragState {
                request_id,
                target: None,
            });
            state.tree.error = None;
            iced::Task::none()
        }
        TreeMessage::RequestDropTargetEntered(target) => {
            if let RequestDropTarget::Folder(folder_id) = &target {
                state.tree.hovered_item = Some(TreeHoveredItem::Folder(folder_id.clone()));
            }
            if state.tree.request_drag.is_none() {
                return iced::Task::none();
            }
            let valid = match &target {
                RequestDropTarget::Root => state.active_collection_id.is_some(),
                RequestDropTarget::Folder(folder_id) => state
                    .active_collection_id
                    .as_deref()
                    .and_then(|collection_id| {
                        state
                            .workspace
                            .collections
                            .iter()
                            .find(|collection| collection.collection.id == collection_id)
                    })
                    .is_some_and(|collection| {
                        collection
                            .folders
                            .iter()
                            .any(|folder| folder.id == *folder_id)
                    }),
            };
            if valid {
                if let Some(drag) = state.tree.request_drag.as_mut() {
                    drag.target = Some(target);
                }
                state.tree.error = None;
            } else {
                state.tree.error = Some("Ese destino ya no está disponible.".to_string());
            }
            iced::Task::none()
        }
        TreeMessage::RequestDropTargetLeft(target) => {
            if let RequestDropTarget::Folder(folder_id) = &target {
                let hovered = TreeHoveredItem::Folder(folder_id.clone());
                if state.tree.hovered_item.as_ref() == Some(&hovered) {
                    state.tree.hovered_item = None;
                }
            }
            if state
                .tree
                .request_drag
                .as_ref()
                .is_some_and(|drag| drag.target.as_ref() == Some(&target))
            {
                if let Some(drag) = state.tree.request_drag.as_mut() {
                    drag.target = None;
                }
            }
            iced::Task::none()
        }
        TreeMessage::ItemHovered(item) => {
            state.tree.hovered_item = Some(item);
            iced::Task::none()
        }
        TreeMessage::ItemUnhovered(item) => {
            if state.tree.hovered_item.as_ref() == Some(&item) {
                state.tree.hovered_item = None;
            }
            iced::Task::none()
        }
        TreeMessage::RequestDragReleased => finish_request_drag(state),
        TreeMessage::RequestDragCancelled => {
            state.tree.request_drag = None;
            iced::Task::none()
        }
        TreeMessage::RequestMoveCompleted {
            request_id,
            destination_folder_id,
            result,
        } => {
            if state.tree.moving_request_id.as_deref() != Some(request_id.as_str()) {
                return iced::Task::none();
            }
            state.tree.moving_request_id = None;
            match result {
                Ok(()) => {
                    let moved_request = state
                        .workspace
                        .collections
                        .iter_mut()
                        .flat_map(|collection| collection.requests.iter_mut())
                        .find(|request| request.id == request_id);
                    let Some(moved_request) = moved_request else {
                        state.tree.error = Some(
                            "El request se movió, pero ya no está en el workspace actual."
                                .to_string(),
                        );
                        return iced::Task::none();
                    };
                    moved_request.folder_id = destination_folder_id.clone();
                    if let Some(folder_id) = destination_folder_id {
                        state.tree.collapsed.remove(&folder_id);
                    }
                    state.tree.error = None;
                    state.session.dirty = true;
                }
                Err(error) => state.tree.error = Some(error),
            }
            iced::Task::none()
        }
        TreeMessage::CreateFirstRequestPressed => {
            // Open a new blank tab (Req 5.9), same as existing new request behavior.
            use crate::app::KeyboardMessage;
            iced::Task::done(Message::Keyboard(KeyboardMessage::NewBlankTabRequested))
        }
    }
}

/// Attempts to open a saved request by id in a composer tab.
/// On failure, sets tree.error and keeps the current active tab (Req 5.7).
fn handle_request_opened(state: &mut Midway, request_id: String) -> iced::Task<Message> {
    // Find the request in the active collection
    let request_record = state
        .active_collection_id
        .as_ref()
        .and_then(|collection_id| {
            state
                .workspace
                .collections
                .iter()
                .find(|c| c.collection.id == *collection_id)
        })
        .and_then(|collection| collection.requests.iter().find(|r| r.id == request_id))
        .cloned();

    let Some(record) = request_record else {
        state.tree.error = Some("No se pudo encontrar el request.".to_string());
        return iced::Task::none();
    };

    // Si el request ya está abierto, sólo enfocamos esa tab. Esto evita
    // duplicados al hacer click repetidamente o al usar la acción Editar.
    if let Some(index) = state
        .tabs
        .iter()
        .position(|tab| tab.draft.id.as_deref() == Some(record.id.as_str()))
    {
        state.active_tab = Some(index);
        state.main_content_focus = crate::app::MainContentFocus::RequestTab;
        state.tree.error = None;
        return iced::Task::none();
    }

    // Check if there's an empty tab to reuse (same as existing behavior)
    let empty_tab_index = state.tabs.iter().position(|tab| {
        tab.draft.url.trim().is_empty() && tab.saved_draft.is_none() && !tab.sending
    });

    if let Some(index) = empty_tab_index {
        // Reuse empty tab
        state.tabs[index].draft = record.draft.clone();
        state.tabs[index].saved_draft = Some(record.draft);
        state.tabs[index].body_editor =
            crate::ui::text_editor::TextEditorState::new(&state.tabs[index].draft.body.value);
        state.active_tab = Some(index);
        state.main_content_focus = crate::app::MainContentFocus::RequestTab;
    } else {
        // Create a new tab
        use crate::app::RequestTabState;
        let mut new_tab = RequestTabState::from_draft(record.draft.clone());
        new_tab.saved_draft = Some(record.draft);
        state.tabs.push(new_tab);
        state.active_tab = Some(state.tabs.len() - 1);
        state.main_content_focus = crate::app::MainContentFocus::RequestTab;
    }

    state.tree.error = None;
    iced::Task::none()
}

// ─── View ───────────────────────────────────────────────────────────────────

fn font_for(style: &TextStyle) -> Font {
    Font {
        family: if style.monospace {
            font::Family::Monospace
        } else {
            font::Family::SansSerif
        },
        weight: style.weight,
        ..Font::DEFAULT
    }
}

/// Envuelve el contenido del explorador en su nivel de elevación
/// (Req 9.1, diseño §8.2).
///
/// El explorador es el nivel intermedio de los tres: se apoya en
/// `background_secondary` —entre el `background_primary` del editor de request
/// y el `surface_elevated` del inspector de respuesta— y se separa del área de
/// contenido con un borde derecho de 1 px en `border`.
///
/// El borde es una franja propia en vez de `container::Style::border` porque
/// `iced::Border` es uniforme en los cuatro lados: un `width: 1.0` dibujaría
/// también arriba, abajo y a la izquierda, encajonando el panel. La franja va
/// dentro del ancho del panel, así que el divisor de arrastre que `app::view`
/// coloca a continuación conserva su hitbox y su comportamiento intactos
/// (Req 9.7).
///
/// Todos los colores salen de la escala existente de `DesignSystem`: no se
/// introduce ningún color de marca nuevo (Req 9.8).
fn pane_shell<'a>(content: Element<'a, Message>, ds: &DesignSystem) -> Element<'a, Message> {
    let background = ds.palette.background_secondary;
    let border_color = ds.palette.border;

    let surface = container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(background.into()),
            ..container::Style::default()
        });

    let right_border = container(column![])
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(border_color.into()),
            ..container::Style::default()
        });

    row![surface, right_border]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Renderiza el Request_Tree_Pane completo: filtro + árbol o estados vacíos.
/// Uses `determine_tree_state` to decide which state to render (Task 11.1).
pub fn view<'a>(state: &'a Midway, ds: &DesignSystem) -> Element<'a, Message> {
    let body = ds.typography.body;
    let secondary = ds.typography.secondary;
    let body_font = font_for(&body);
    let secondary_font = font_for(&secondary);

    match determine_tree_state(state) {
        TreePaneState::NoCollections => {
            // Show nothing — onboarding in main panel handles this case
            pane_shell(column![].into(), ds)
        }
        TreePaneState::NoActiveCollection => {
            // Show "Seleccioná o creá una colección" with a Guided_Action
            let message = text("Seleccioná o creá una colección")
                .size(body.size)
                .font(body_font)
                .color(ds.palette.text_secondary);

            let create_button = button(
                text("Crear colección")
                    .size(secondary.size)
                    .font(secondary_font),
            )
            .padding([ds.spacing.sm, ds.spacing.md])
            .style(|_theme, _status| button::Style {
                border: Border {
                    color: iced::Color::TRANSPARENT,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..button::Style::default()
            })
            .on_press(Message::ActivityBar(
                ActivityBarMessage::CreateCollectionPressed,
            ));

            let content = column![message, create_button]
                .spacing(ds.spacing.md)
                .padding(ds.spacing.md)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center);

            pane_shell(
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_y(Length::Fill)
                    .into(),
                ds,
            )
        }
        TreePaneState::EmptyCollection => {
            let collection_id = state.active_collection_id.clone().unwrap_or_default();
            let collection_name = state
                .active_collection_id
                .as_deref()
                .and_then(|id| {
                    state
                        .workspace
                        .collections
                        .iter()
                        .find(|collection| collection.collection.id == id)
                })
                .map(|collection| collection.collection.name.as_str())
                .unwrap_or("Colección");
            // Enhanced empty state with informational text about importing
            let empty_message = text("La colección está vacía.")
                .size(body.size)
                .font(body_font)
                .color(ds.palette.text_secondary);

            let import_hint = text("También podés importar una colección existente.")
                .size(secondary.size)
                .font(secondary_font)
                .color(ds.palette.text_secondary);

            let create_request_button =
                button(text("+ Request").size(secondary.size).font(secondary_font))
                    .on_press(Message::Tree(TreeMessage::CreateFirstRequestPressed));
            let create_folder_button =
                button(text("+ Carpeta").size(secondary.size).font(secondary_font)).on_press(
                    Message::WorkspaceCrud(WorkspaceCrudMessage::CreateFolderRequested {
                        parent_folder_id: None,
                    }),
                );
            let rename_collection_button =
                button(text("Renombrar").size(secondary.size)).on_press(Message::WorkspaceCrud(
                    WorkspaceCrudMessage::RenameCollectionRequested(collection_id.clone()),
                ));
            let delete_collection_button =
                button(text("Borrar").size(secondary.size)).on_press(Message::WorkspaceCrud(
                    WorkspaceCrudMessage::DeleteCollectionRequested(collection_id),
                ));

            let content = column![
                text(collection_name)
                    .size(ds.typography.subtitle.size)
                    .font(body_font)
                    .color(ds.palette.text_primary),
                row![rename_collection_button, delete_collection_button].spacing(ds.spacing.xs),
                empty_message,
                import_hint,
                row![create_request_button, create_folder_button].spacing(ds.spacing.xs),
            ]
            .spacing(ds.spacing.sm)
            .padding(ds.spacing.md)
            .width(Length::Fill);

            pane_shell(content.into(), ds)
        }
        TreePaneState::Normal => {
            // Render tree as before: filter + tree content
            view_normal(state, ds)
        }
    }
}

/// Renders the normal tree state: filter input + tree or "no results" state.
fn view_normal<'a>(state: &'a Midway, ds: &DesignSystem) -> Element<'a, Message> {
    let body = ds.typography.body;
    let secondary = ds.typography.secondary;
    let body_font = font_for(&body);
    let secondary_font = font_for(&secondary);

    // Get the active collection's folders and requests
    let active_collection = state.active_collection_id.as_ref().and_then(|id| {
        state
            .workspace
            .collections
            .iter()
            .find(|c| c.collection.id == *id)
    });
    let (folders, requests) = match active_collection {
        Some(collection) => (
            collection.folders.as_slice(),
            collection.requests.as_slice(),
        ),
        None => (&[] as &[Folder], &[] as &[SavedRequestRecord]),
    };

    // Build the tree
    let full_tree = build_tree(folders, requests);

    // Filter input
    let filter_input = text_input("Filtrar requests...", &state.tree.filter)
        .on_input(|text| Message::Tree(TreeMessage::FilterChanged(text)))
        .size(body.size)
        .padding(ds.spacing.sm);

    // Check for filter
    let filter_active = !state.tree.filter.trim().is_empty();

    // Apply filter
    let filtered_tree = if filter_active {
        filter_tree(&full_tree, &state.tree.filter)
    } else {
        Some(full_tree)
    };

    // Flatten the tree into a list of render items (owned data) to avoid
    // lifetime issues with the locally-built filtered tree.
    let render_items = match filtered_tree.as_ref() {
        Some(tree) => flatten_tree_for_render(tree, &state.tree, 0, filter_active),
        None => Vec::new(),
    };

    // Determine which content to show
    let active_request_id = state
        .active_tab
        .and_then(|index| state.tabs.get(index))
        .and_then(|tab| tab.draft.id.as_deref())
        .map(str::to_string);
    let content: Element<'_, Message> = if filter_active && filtered_tree.is_none() {
        // "No results" empty state (Req 5.10, takes precedence over empty collection)
        column![text("No se encontraron resultados para el filtro.")
            .size(body.size)
            .font(body_font)
            .color(ds.palette.text_secondary),]
        .spacing(ds.spacing.sm)
        .padding(ds.spacing.md)
        .width(Length::Fill)
        .into()
    } else {
        // Render the tree from flattened items
        let mut tree_column = column![].spacing(ds.spacing.xs);
        for item in render_items {
            match item {
                RenderItem::Folder {
                    id,
                    name,
                    collapsed,
                    depth,
                } => {
                    let indent = (depth as f32) * ds.spacing.lg;
                    let toggle_icon = if collapsed { "▸" } else { "▾" };

                    let folder_button = button(
                        row![
                            text(toggle_icon)
                                .size(secondary.size)
                                .font(body_font)
                                .color(ds.palette.text_secondary),
                            text(name)
                                .size(body.size)
                                .font(body_font)
                                .color(ds.palette.text_primary),
                        ]
                        .spacing(ds.spacing.xs)
                        .align_y(iced::alignment::Vertical::Center),
                    )
                    .width(Length::Fill)
                    .padding([ds.spacing.xs, ds.spacing.sm])
                    .style(|_theme, _status| button::Style::default())
                    .on_press_maybe(
                        state
                            .tree
                            .request_drag
                            .is_none()
                            .then(|| Message::Tree(TreeMessage::FolderToggled(id.clone()))),
                    );
                    let folder_hovered = state.tree.hovered_item.as_ref()
                        == Some(&TreeHoveredItem::Folder(id.clone()));
                    let folder_actions: Element<'_, Message> = if folder_hovered
                        && state.tree.request_drag.is_none()
                        && state.tree.moving_request_id.is_none()
                    {
                        row![
                            compact_action(
                                "+",
                                "Nueva subcarpeta",
                                Message::WorkspaceCrud(
                                    WorkspaceCrudMessage::CreateFolderRequested {
                                        parent_folder_id: Some(id.clone()),
                                    },
                                ),
                                ds,
                            ),
                            compact_action(
                                "✎",
                                "Renombrar carpeta",
                                Message::WorkspaceCrud(
                                    WorkspaceCrudMessage::RenameFolderRequested(id.clone()),
                                ),
                                ds,
                            ),
                            compact_action(
                                "×",
                                "Eliminar carpeta",
                                Message::WorkspaceCrud(
                                    WorkspaceCrudMessage::DeleteFolderRequested(id.clone()),
                                ),
                                ds,
                            ),
                        ]
                        .spacing(TREE_ACTION_GAP)
                        .into()
                    } else {
                        space()
                            .width(Length::Fixed(THREE_ACTIONS_WIDTH))
                            .height(Length::Fixed(TREE_ACTION_SIZE))
                            .into()
                    };
                    let folder_row = row![folder_button, folder_actions]
                        .spacing(ds.spacing.xs)
                        .width(Length::Fill)
                        .align_y(iced::alignment::Vertical::Center);

                    let folder_target = RequestDropTarget::Folder(id.clone());
                    let is_drop_target = state
                        .tree
                        .request_drag
                        .as_ref()
                        .is_some_and(|drag| drag.target.as_ref() == Some(&folder_target));
                    let drop_background = if is_drop_target {
                        Some(iced::Color {
                            a: 0.18,
                            ..ds.palette.accent
                        })
                    } else {
                        None
                    };
                    let drop_accent = ds.palette.accent;
                    let drop_radius = ds.radius.control;
                    let folder_surface =
                        container(folder_row)
                            .width(Length::Fill)
                            .style(move |_theme| container::Style {
                                background: drop_background.map(Into::into),
                                border: Border {
                                    color: if is_drop_target {
                                        drop_accent
                                    } else {
                                        iced::Color::TRANSPARENT
                                    },
                                    width: if is_drop_target { 1.0 } else { 0.0 },
                                    radius: drop_radius.into(),
                                },
                                ..container::Style::default()
                            });
                    let folder_drop_area = mouse_area(folder_surface)
                        .on_enter(Message::Tree(TreeMessage::RequestDropTargetEntered(
                            folder_target.clone(),
                        )))
                        .on_exit(Message::Tree(TreeMessage::RequestDropTargetLeft(
                            folder_target,
                        )))
                        .interaction(if state.tree.request_drag.is_some() {
                            iced::mouse::Interaction::Move
                        } else {
                            iced::mouse::Interaction::None
                        });

                    tree_column =
                        tree_column.push(container(folder_drop_area).padding(iced::Padding {
                            top: 0.0,
                            right: 0.0,
                            bottom: 0.0,
                            left: indent,
                        }));
                }
                RenderItem::Request {
                    id,
                    name,
                    method,
                    depth,
                } => {
                    let indent = (depth as f32) * ds.spacing.lg;
                    let m_color = method_color(method);
                    let method_label = method.to_string();
                    let display_name = request_name_without_method_prefix(&name, method);
                    let is_active = active_request_id.as_deref() == Some(id.as_str());
                    let item_style = selectable_item_style(ds, is_active);

                    let request_button = button(
                        row![
                            text(method_label)
                                .size(secondary.size)
                                .font(secondary_font)
                                .color(m_color),
                            text(display_name)
                                .size(body.size)
                                .font(body_font)
                                .color(item_style.text_color),
                        ]
                        .spacing(ds.spacing.sm)
                        .align_y(iced::alignment::Vertical::Center),
                    )
                    .width(Length::Fill)
                    .padding([ds.spacing.xs, ds.spacing.sm])
                    .style(move |_theme, _status| button::Style {
                        background: item_style.background.map(Into::into),
                        text_color: item_style.text_color,
                        ..button::Style::default()
                    })
                    .on_press(Message::Tree(TreeMessage::RequestOpened(id.clone())));
                    let dragging_this = state
                        .tree
                        .request_drag
                        .as_ref()
                        .is_some_and(|drag| drag.request_id == id);
                    let moving_this = state.tree.moving_request_id.as_deref() == Some(id.as_str());
                    let handle_text = if moving_this { "…" } else { "⠿" };
                    let handle_color = if dragging_this {
                        ds.palette.accent
                    } else {
                        ds.palette.text_secondary
                    };
                    let handle_surface = container(
                        text(handle_text)
                            .size(body.size)
                            .font(body_font)
                            .color(handle_color),
                    )
                    .width(Length::Fixed(TREE_ACTION_SIZE))
                    .height(Length::Fixed(TREE_ACTION_SIZE))
                    .align_x(iced::alignment::Horizontal::Center)
                    .align_y(iced::alignment::Vertical::Center);
                    let drag_handle: Element<'_, Message> = if moving_this {
                        handle_surface.into()
                    } else {
                        mouse_area(handle_surface)
                            .on_press(Message::Tree(TreeMessage::RequestDragStarted(id.clone())))
                            .interaction(if dragging_this {
                                iced::mouse::Interaction::Grabbing
                            } else {
                                iced::mouse::Interaction::Grab
                            })
                            .into()
                    };
                    let request_hovered = state.tree.hovered_item.as_ref()
                        == Some(&TreeHoveredItem::Request(id.clone()));
                    let request_actions: Element<'_, Message> = if moving_this {
                        row![
                            action_tooltip(drag_handle, "Moviendo request…", ds),
                            space()
                                .width(Length::Fixed(TWO_ACTIONS_WIDTH))
                                .height(Length::Fixed(TREE_ACTION_SIZE)),
                        ]
                        .spacing(TREE_ACTION_GAP)
                        .into()
                    } else if request_hovered || dragging_this {
                        row![
                            action_tooltip(drag_handle, "Mover request", ds),
                            compact_action(
                                "✎",
                                "Renombrar o mover",
                                Message::Tree(TreeMessage::RequestEditRequested(id.clone())),
                                ds,
                            ),
                            compact_action(
                                "×",
                                "Eliminar request",
                                Message::WorkspaceCrud(
                                    WorkspaceCrudMessage::DeleteRequestRequested(id.clone()),
                                ),
                                ds,
                            ),
                        ]
                        .spacing(TREE_ACTION_GAP)
                        .into()
                    } else {
                        space()
                            .width(Length::Fixed(THREE_ACTIONS_WIDTH))
                            .height(Length::Fixed(TREE_ACTION_SIZE))
                            .into()
                    };
                    let request_row = row![request_button, request_actions]
                        .spacing(ds.spacing.xs)
                        .width(Length::Fill)
                        .align_y(iced::alignment::Vertical::Center);

                    let request_hover_area = mouse_area(container(request_row).width(Length::Fill))
                        .on_enter(Message::Tree(TreeMessage::ItemHovered(
                            TreeHoveredItem::Request(id.clone()),
                        )))
                        .on_exit(Message::Tree(TreeMessage::ItemUnhovered(
                            TreeHoveredItem::Request(id),
                        )));
                    tree_column =
                        tree_column.push(container(request_hover_area).padding(iced::Padding {
                            top: 0.0,
                            right: 0.0,
                            bottom: 0.0,
                            left: indent,
                        }));
                }
            }
        }
        scrollable(tree_column.width(Length::Fill).padding(ds.spacing.sm))
            .height(Length::Fill)
            .into()
    };

    // Error message if present (Req 5.7)
    let error_element: Option<Element<'_, Message>> = state.tree.error.as_ref().map(|err| {
        text(err.as_str())
            .size(secondary.size)
            .font(font_for(&secondary))
            .color(ds.palette.status_error)
            .into()
    });

    let collection_id = active_collection
        .map(|collection| collection.collection.id.clone())
        .unwrap_or_default();
    let collection_name = active_collection
        .map(|collection| collection.collection.name.as_str())
        .unwrap_or("Colección");
    let collection_actions: Element<'_, Message> = if state.tree.hovered_item
        == Some(TreeHoveredItem::Collection)
        && state.tree.moving_request_id.is_none()
    {
        row![
            compact_action(
                "✎",
                "Renombrar colección",
                Message::WorkspaceCrud(WorkspaceCrudMessage::RenameCollectionRequested(
                    collection_id.clone(),
                )),
                ds,
            ),
            compact_action(
                "×",
                "Eliminar colección",
                Message::WorkspaceCrud(WorkspaceCrudMessage::DeleteCollectionRequested(
                    collection_id,
                )),
                ds,
            ),
        ]
        .spacing(TREE_ACTION_GAP)
        .into()
    } else {
        space()
            .width(Length::Fixed(TWO_ACTIONS_WIDTH))
            .height(Length::Fixed(TREE_ACTION_SIZE))
            .into()
    };
    let new_request_button = button(text("+ Request").size(secondary.size))
        .on_press(Message::Tree(TreeMessage::CreateFirstRequestPressed));
    let new_folder_button = button(text("+ Carpeta").size(secondary.size)).on_press(
        Message::WorkspaceCrud(WorkspaceCrudMessage::CreateFolderRequested {
            parent_folder_id: None,
        }),
    );
    let header = row![
        text(collection_name)
            .size(ds.typography.subtitle.size)
            .font(body_font)
            .color(ds.palette.text_primary)
            .width(Length::Fill),
        collection_actions,
    ]
    .align_y(iced::alignment::Vertical::Center);
    let header = mouse_area(header)
        .on_enter(Message::Tree(TreeMessage::ItemHovered(
            TreeHoveredItem::Collection,
        )))
        .on_exit(Message::Tree(TreeMessage::ItemUnhovered(
            TreeHoveredItem::Collection,
        )));
    let create_actions: Element<'_, Message> = if state.tree.request_drag.is_some() {
        let root_target = RequestDropTarget::Root;
        let is_root_target = state
            .tree
            .request_drag
            .as_ref()
            .is_some_and(|drag| drag.target.as_ref() == Some(&root_target));
        let background = if is_root_target {
            Some(iced::Color {
                a: 0.18,
                ..ds.palette.accent
            })
        } else {
            None
        };
        let root_border = if is_root_target {
            ds.palette.accent
        } else {
            ds.palette.border
        };
        let root_radius = ds.radius.control;
        let root_zone = container(
            text("Soltar aquí para mover a la raíz")
                .size(secondary.size)
                .font(secondary_font)
                .color(ds.palette.text_secondary),
        )
        .width(Length::Fill)
        .padding([ds.spacing.xs, ds.spacing.sm])
        .style(move |_theme| container::Style {
            background: background.map(Into::into),
            border: Border {
                color: root_border,
                width: 1.0,
                radius: root_radius.into(),
            },
            ..container::Style::default()
        });
        mouse_area(root_zone)
            .on_enter(Message::Tree(TreeMessage::RequestDropTargetEntered(
                RequestDropTarget::Root,
            )))
            .on_exit(Message::Tree(TreeMessage::RequestDropTargetLeft(
                RequestDropTarget::Root,
            )))
            .interaction(iced::mouse::Interaction::Move)
            .into()
    } else {
        row![new_request_button, new_folder_button]
            .spacing(ds.spacing.xs)
            .into()
    };
    let mut main_column = column![
        container(header).padding([ds.spacing.sm, ds.spacing.md]),
        container(create_actions).padding([0.0, ds.spacing.md]),
        filter_input,
    ]
    .spacing(ds.spacing.sm);

    if let Some(error_el) = error_element {
        main_column = main_column.push(container(error_el).padding(ds.spacing.sm));
    }

    main_column = main_column.push(content);

    pane_shell(main_column.into(), ds)
}

// ─── Render flattening ──────────────────────────────────────────────────────

use midway_core::domain::http::HttpMethod;

fn request_name_without_method_prefix(name: &str, method: HttpMethod) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "Sin título".to_string();
    }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let prefix = parts.next().unwrap_or_default();
    let remainder = parts.next().unwrap_or_default().trim_start();
    if !remainder.is_empty() && prefix.eq_ignore_ascii_case(&method.to_string()) {
        remainder.to_string()
    } else {
        trimmed.to_string()
    }
}

/// A flattened render item with all the owned data needed to build UI elements.
enum RenderItem {
    Folder {
        id: String,
        name: String,
        collapsed: bool,
        depth: usize,
    },
    Request {
        id: String,
        name: String,
        method: HttpMethod,
        depth: usize,
    },
}

/// Flattens the tree into a list of render items (depth-first) so that the
/// view function can iterate over owned data without borrowing the tree.
fn flatten_tree_for_render(
    node: &TreeNode,
    tree_state: &TreeViewState,
    depth: usize,
    filter_active: bool,
) -> Vec<RenderItem> {
    let mut items = Vec::new();
    flatten_node(&mut items, node, tree_state, depth, filter_active);
    items
}

fn flatten_node(
    items: &mut Vec<RenderItem>,
    node: &TreeNode,
    tree_state: &TreeViewState,
    depth: usize,
    filter_active: bool,
) {
    // Render child folders
    for child in &node.child_folders {
        if let Some(ref folder) = child.folder {
            let is_collapsed = if filter_active {
                // When filtering, force-expand all ancestor folders (Req 5.2)
                false
            } else {
                tree_state.collapsed.contains(&folder.id)
            };

            items.push(RenderItem::Folder {
                id: folder.id.clone(),
                name: folder.name.clone(),
                collapsed: is_collapsed,
                depth,
            });

            if !is_collapsed {
                flatten_node(items, child, tree_state, depth + 1, filter_active);
            }
        }
    }

    // Render requests
    for request in &node.requests {
        items.push(RenderItem::Request {
            id: request.id.clone(),
            name: request.name.clone(),
            method: request.draft.method,
            depth,
        });
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use midway_core::domain::http::{
        AuthConfig, BodyMode, HttpMethod, RequestBodyDraft, RequestDraft,
    };

    fn make_folder(id: &str, name: &str, parent: Option<&str>, collection_id: &str) -> Folder {
        Folder {
            id: id.to_string(),
            collection_id: collection_id.to_string(),
            parent_folder_id: parent.map(|s| s.to_string()),
            name: name.to_string(),
        }
    }

    fn make_request(
        id: &str,
        name: &str,
        folder_id: Option<&str>,
        method: HttpMethod,
    ) -> SavedRequestRecord {
        SavedRequestRecord {
            id: id.to_string(),
            collection_id: "col1".to_string(),
            folder_id: folder_id.map(|s| s.to_string()),
            name: name.to_string(),
            draft: RequestDraft {
                id: None,
                name: name.to_string(),
                method,
                url: String::new(),
                query: Vec::new(),
                headers: Vec::new(),
                auth: AuthConfig::None,
                body: RequestBodyDraft {
                    mode: BodyMode::None,
                    value: String::new(),
                    form_data: Vec::new(),
                },
                timeout_ms: 0,
                environment_id: None,
                response_tests: Vec::new(),
            },
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn build_tree_sorts_folders_before_requests_alphabetically() {
        let folders = vec![
            make_folder("f2", "Zebra", None, "col1"),
            make_folder("f1", "Alpha", None, "col1"),
        ];
        let requests = vec![
            make_request("r2", "Zulu", None, HttpMethod::POST),
            make_request("r1", "Able", None, HttpMethod::GET),
        ];

        let tree = build_tree(&folders, &requests);

        // Folders sorted alphabetically: Alpha, Zebra
        assert_eq!(tree.child_folders.len(), 2);
        assert_eq!(tree.child_folders[0].folder.as_ref().unwrap().name, "Alpha");
        assert_eq!(tree.child_folders[1].folder.as_ref().unwrap().name, "Zebra");

        // Requests sorted alphabetically: Able, Zulu
        assert_eq!(tree.requests.len(), 2);
        assert_eq!(tree.requests[0].name, "Able");
        assert_eq!(tree.requests[1].name, "Zulu");
    }

    #[test]
    fn build_tree_case_insensitive_sort() {
        let folders = vec![
            make_folder("f1", "beta", None, "col1"),
            make_folder("f2", "Alpha", None, "col1"),
        ];
        let requests = vec![];

        let tree = build_tree(&folders, &requests);

        assert_eq!(tree.child_folders[0].folder.as_ref().unwrap().name, "Alpha");
        assert_eq!(tree.child_folders[1].folder.as_ref().unwrap().name, "beta");
    }

    #[test]
    fn build_tree_nested_folders() {
        let folders = vec![
            make_folder("f1", "Parent", None, "col1"),
            make_folder("f2", "Child", Some("f1"), "col1"),
        ];
        let requests = vec![make_request(
            "r1",
            "Request in Child",
            Some("f2"),
            HttpMethod::GET,
        )];

        let tree = build_tree(&folders, &requests);

        assert_eq!(tree.child_folders.len(), 1);
        let parent = &tree.child_folders[0];
        assert_eq!(parent.folder.as_ref().unwrap().name, "Parent");
        assert_eq!(parent.child_folders.len(), 1);
        let child = &parent.child_folders[0];
        assert_eq!(child.folder.as_ref().unwrap().name, "Child");
        assert_eq!(child.requests.len(), 1);
        assert_eq!(child.requests[0].name, "Request in Child");
    }

    #[test]
    fn filter_tree_empty_query_returns_full_tree() {
        let folders = vec![make_folder("f1", "Auth", None, "col1")];
        let requests = vec![make_request("r1", "Login", Some("f1"), HttpMethod::POST)];

        let tree = build_tree(&folders, &requests);
        let filtered = filter_tree(&tree, "");

        assert!(filtered.is_some());
        let filtered = filtered.unwrap();
        assert_eq!(filtered.child_folders.len(), 1);
    }

    #[test]
    fn filter_tree_matches_requests_case_insensitive() {
        let folders = vec![make_folder("f1", "Auth", None, "col1")];
        let requests = vec![
            make_request("r1", "Login", Some("f1"), HttpMethod::POST),
            make_request("r2", "Signup", Some("f1"), HttpMethod::POST),
        ];

        let tree = build_tree(&folders, &requests);
        let filtered = filter_tree(&tree, "log");

        assert!(filtered.is_some());
        let filtered = filtered.unwrap();
        // Auth folder is kept as ancestor of matching request
        assert_eq!(filtered.child_folders.len(), 1);
        let auth_folder = &filtered.child_folders[0];
        assert_eq!(auth_folder.requests.len(), 1);
        assert_eq!(auth_folder.requests[0].name, "Login");
    }

    #[test]
    fn filter_tree_no_matches_returns_none() {
        let folders = vec![make_folder("f1", "Auth", None, "col1")];
        let requests = vec![make_request("r1", "Login", Some("f1"), HttpMethod::POST)];

        let tree = build_tree(&folders, &requests);
        let filtered = filter_tree(&tree, "nonexistent");

        assert!(filtered.is_none());
    }

    #[test]
    fn filter_tree_preserves_ancestor_folders() {
        let folders = vec![
            make_folder("f1", "API", None, "col1"),
            make_folder("f2", "Auth", Some("f1"), "col1"),
        ];
        let requests = vec![
            make_request("r1", "Login", Some("f2"), HttpMethod::POST),
            make_request("r2", "List Users", Some("f1"), HttpMethod::GET),
        ];

        let tree = build_tree(&folders, &requests);
        let filtered = filter_tree(&tree, "login");

        assert!(filtered.is_some());
        let filtered = filtered.unwrap();
        // API folder kept (ancestor)
        assert_eq!(filtered.child_folders.len(), 1);
        let api = &filtered.child_folders[0];
        assert_eq!(api.folder.as_ref().unwrap().name, "API");
        // Auth subfolder kept (ancestor of matching "Login")
        assert_eq!(api.child_folders.len(), 1);
        // "List Users" should be excluded (doesn't match)
        assert_eq!(api.requests.len(), 0);
    }
}

// Feature: ux-flow-redesign, Property 6: Tree pane state derivation
#[cfg(test)]
mod tree_pane_state_property_tests {
    //! Feature: ux-flow-redesign, Property 6: Tree pane state derivation
    //! Validates: Requirements 5.1, 5.3, 5.4

    use super::*;
    use crate::app::{
        MainContentFocus, Midway, PaletteState, RequestTabState, SessionStoreState, TopBarMode,
        TreeViewState, UpdaterState, WorkspacePanelState,
    };
    use crate::state::AppState;
    use crate::ui::design_system::ThemeMode;
    use midway_core::domain::cookies::CookieJarHandle;
    use midway_core::domain::http::{
        AuthConfig, BodyMode, HttpMethod, RequestBodyDraft, RequestDraft,
    };
    use midway_core::domain::workspace::{
        CollectionSummary, CollectionWithRequests, Folder, SavedRequestRecord, WorkspaceSnapshot,
    };
    use midway_core::infra::sqlite_repository::SqliteRepository;
    use midway_core::runtime::request_executor::RequestExecutorHandle;
    use midway_core::runtime::secret_executor::SecretExecutorHandle;
    use proptest::prelude::*;
    use std::sync::Arc;

    /// Builds a minimal `Midway` state suitable for testing `determine_tree_state`.
    /// Only the fields accessed by that function are varied; the rest use defaults.
    ///
    /// Uses a shared `AppState` to avoid the overhead of creating a SQLite
    /// database for each proptest iteration.
    fn build_midway_for_tree_state(
        collections: Vec<CollectionWithRequests>,
        active_collection_id: Option<String>,
        filter: String,
    ) -> Midway {
        use std::sync::LazyLock;

        static SHARED_APP_STATE: LazyLock<Arc<AppState>> = LazyLock::new(|| {
            let temp_file = tempfile::NamedTempFile::new()
                .expect("could not create temp file for test AppState");
            let db_path = temp_file.path().to_path_buf();

            let runtime = tokio::runtime::Runtime::new()
                .expect("could not create tokio runtime for test AppState");

            let app_state = runtime.block_on(async {
                let repository = SqliteRepository::open(&db_path)
                    .await
                    .expect("could not open test SqliteRepository");

                let client = reqwest::Client::builder()
                    .build()
                    .expect("could not build test reqwest client");

                AppState {
                    repository,
                    request_executor: RequestExecutorHandle::spawn(client),
                    secret_executor: SecretExecutorHandle::spawn("midway-test".to_string()),
                    cookie_jar: CookieJarHandle::new(),
                }
            });

            // Leak the temp file intentionally — the LazyLock lives for the
            // entire test process and the DB is only used for structural
            // requirements (never read/written by determine_tree_state).
            std::mem::forget(temp_file);

            Arc::new(app_state)
        });

        Midway {
            app_state: Arc::clone(&SHARED_APP_STATE),
            workspace: WorkspaceSnapshot {
                collections,
                environments: Vec::new(),
                history: Vec::new(),
                secrets: Vec::new(),
            },
            tabs: vec![RequestTabState::blank()],
            active_tab: Some(0),
            closed_tabs: std::collections::VecDeque::new(),
            workspace_panel: WorkspacePanelState::default(),
            palette: PaletteState::default(),
            runner: None,
            session: SessionStoreState::default(),
            theme: crate::ui::theme_settings::ThemeSettingsState::default(),
            main_content_focus: MainContentFocus::default(),
            updater: UpdaterState::default(),
            crash_log: Vec::new(),
            unsaved_changes_prompt: None,
            active_collection_id,
            top_bar_mode: TopBarMode::default(),
            tree: TreeViewState {
                filter,
                ..TreeViewState::default()
            },
            create_collection_prompt: None,
            save_request_prompt: None,
            workspace_crud_dialog: None,
            panel_dragging: None,
            panel_hovered: None,
        }
    }

    /// Strategy to generate a collection id.
    fn arb_collection_id() -> impl Strategy<Value = String> {
        "[a-z0-9]{1,12}".prop_map(|s| s.to_string())
    }

    /// Strategy to generate a folder for a given collection id.
    fn arb_folder(collection_id: String) -> impl Strategy<Value = Folder> {
        "[a-z0-9]{1,10}".prop_map(move |id| Folder {
            id,
            collection_id: collection_id.clone(),
            parent_folder_id: None,
            name: "folder".to_string(),
        })
    }

    /// Strategy to generate a request for a given collection id.
    fn arb_request(collection_id: String) -> impl Strategy<Value = SavedRequestRecord> {
        "[a-z0-9]{1,10}".prop_map(move |id| SavedRequestRecord {
            id,
            collection_id: collection_id.clone(),
            folder_id: None,
            name: "request".to_string(),
            draft: RequestDraft {
                id: None,
                name: "request".to_string(),
                method: HttpMethod::GET,
                url: String::new(),
                query: Vec::new(),
                headers: Vec::new(),
                auth: AuthConfig::None,
                body: RequestBodyDraft {
                    mode: BodyMode::None,
                    value: String::new(),
                    form_data: Vec::new(),
                },
                timeout_ms: 0,
                environment_id: None,
                response_tests: Vec::new(),
            },
            created_at: String::new(),
            updated_at: String::new(),
        })
    }

    /// Strategy to generate a collection with a specific id, possibly with content.
    fn arb_collection_with_id(id: String) -> impl Strategy<Value = CollectionWithRequests> {
        let col_id = id.clone();
        let col_id2 = id.clone();
        (
            proptest::collection::vec(arb_folder(col_id.clone()), 0..=3),
            proptest::collection::vec(arb_request(col_id2.clone()), 0..=3),
        )
            .prop_map(move |(folders, requests)| CollectionWithRequests {
                collection: CollectionSummary {
                    id: id.clone(),
                    name: format!("col-{}", id),
                    request_count: requests.len() as u64,
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    updated_at: "2024-01-01T00:00:00Z".to_string(),
                },
                folders,
                requests,
            })
    }

    /// Strategy to generate filter text: either empty or some non-empty string.
    fn arb_filter() -> impl Strategy<Value = String> {
        prop_oneof![
            3 => Just(String::new()),
            1 => "[a-z]{1,10}".prop_map(|s| s.to_string()),
        ]
    }

    /// Describes the test scenario shape for proptest generation.
    #[derive(Debug, Clone)]
    enum ScenarioShape {
        /// No collections in workspace.
        NoCollections,
        /// Collections exist but no active collection (None or non-matching id).
        NoActiveCollection {
            collections: Vec<CollectionWithRequests>,
            active_id: Option<String>,
        },
        /// Active collection is empty and filter is empty.
        EmptyCollection {
            collections: Vec<CollectionWithRequests>,
            active_id: String,
        },
        /// Active collection has content OR filter is non-empty.
        Normal {
            collections: Vec<CollectionWithRequests>,
            active_id: String,
            filter: String,
        },
    }

    /// Master strategy that generates arbitrary combinations and checks the
    /// property against the expected `TreePaneState`.
    fn arb_tree_state_scenario() -> impl Strategy<
        Value = (
            Vec<CollectionWithRequests>,
            Option<String>,
            String,
            TreePaneState,
        ),
    > {
        prop_oneof![
            // Case 1: No collections → NoCollections
            2 => arb_filter().prop_map(|filter| {
                (Vec::new(), None, filter, TreePaneState::NoCollections)
            }),
            // Case 1b: No collections with some active_id → still NoCollections
            1 => (arb_collection_id(), arb_filter()).prop_map(|(id, filter)| {
                (Vec::new(), Some(id), filter, TreePaneState::NoCollections)
            }),
            // Case 2: Collections exist, active_collection_id is None → NoActiveCollection
            2 => arb_collection_id().prop_flat_map(|id| {
                arb_collection_with_id(id).prop_flat_map(|col| {
                    arb_filter().prop_map(move |filter| {
                        (vec![col.clone()], None, filter, TreePaneState::NoActiveCollection)
                    })
                })
            }),
            // Case 2b: Collections exist, active_collection_id is Some but doesn't match → NoActiveCollection
            2 => arb_collection_id().prop_flat_map(|id| {
                arb_collection_with_id(id).prop_flat_map(|col| {
                    arb_filter().prop_map(move |filter| {
                        let non_matching_id = format!("{}-nomatch", col.collection.id);
                        (vec![col.clone()], Some(non_matching_id), filter, TreePaneState::NoActiveCollection)
                    })
                })
            }),
            // Case 3: Active collection exists but is empty + filter empty → EmptyCollection
            2 => arb_collection_id().prop_map(|id| {
                let col = CollectionWithRequests {
                    collection: CollectionSummary {
                        id: id.clone(),
                        name: format!("col-{}", id),
                        request_count: 0,
                        created_at: "2024-01-01T00:00:00Z".to_string(),
                        updated_at: "2024-01-01T00:00:00Z".to_string(),
                    },
                    folders: Vec::new(),
                    requests: Vec::new(),
                };
                (vec![col], Some(id), String::new(), TreePaneState::EmptyCollection)
            }),
            // Case 4a: Active collection has content → Normal
            3 => arb_collection_id().prop_flat_map(|id| {
                let col_id = id.clone();
                (
                    proptest::collection::vec(arb_folder(col_id.clone()), 1..=3),
                    proptest::collection::vec(arb_request(col_id.clone()), 0..=3),
                    arb_filter(),
                ).prop_map(move |(folders, requests, filter)| {
                    let col = CollectionWithRequests {
                        collection: CollectionSummary {
                            id: id.clone(),
                            name: format!("col-{}", id),
                            request_count: requests.len() as u64,
                            created_at: "2024-01-01T00:00:00Z".to_string(),
                            updated_at: "2024-01-01T00:00:00Z".to_string(),
                        },
                        folders,
                        requests,
                    };
                    (vec![col], Some(id.clone()), filter, TreePaneState::Normal)
                })
            }),
            // Case 4b: Active collection has only requests → Normal
            2 => arb_collection_id().prop_flat_map(|id| {
                let col_id = id.clone();
                (
                    proptest::collection::vec(arb_request(col_id.clone()), 1..=3),
                    arb_filter(),
                ).prop_map(move |(requests, filter)| {
                    let col = CollectionWithRequests {
                        collection: CollectionSummary {
                            id: id.clone(),
                            name: format!("col-{}", id),
                            request_count: requests.len() as u64,
                            created_at: "2024-01-01T00:00:00Z".to_string(),
                            updated_at: "2024-01-01T00:00:00Z".to_string(),
                        },
                        folders: Vec::new(),
                        requests,
                    };
                    (vec![col], Some(id.clone()), filter, TreePaneState::Normal)
                })
            }),
            // Case 4c: Active collection is empty BUT filter is non-empty → Normal
            2 => (arb_collection_id(), "[a-z]{1,10}".prop_map(|s| s.to_string())).prop_map(|(id, filter)| {
                let col = CollectionWithRequests {
                    collection: CollectionSummary {
                        id: id.clone(),
                        name: format!("col-{}", id),
                        request_count: 0,
                        created_at: "2024-01-01T00:00:00Z".to_string(),
                        updated_at: "2024-01-01T00:00:00Z".to_string(),
                    },
                    folders: Vec::new(),
                    requests: Vec::new(),
                };
                (vec![col], Some(id), filter, TreePaneState::Normal)
            }),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// Property 6: Tree pane state derivation returns the correct variant
        /// for each condition exclusively.
        ///
        /// **Validates: Requirements 5.1, 5.3, 5.4**
        #[test]
        fn property_6_tree_pane_state_derivation(
            (collections, active_id, filter, expected) in arb_tree_state_scenario(),
        ) {
            let midway = build_midway_for_tree_state(collections, active_id, filter);
            let result = determine_tree_state(&midway);
            prop_assert_eq!(
                result,
                expected,
                "determine_tree_state returned {:?} but expected {:?}",
                result,
                expected,
            );
        }
    }

    #[test]
    fn request_tree_does_not_repeat_matching_method_prefix() {
        assert_eq!(
            request_name_without_method_prefix("POST posts 1", HttpMethod::POST),
            "posts 1"
        );
        assert_eq!(
            request_name_without_method_prefix("GET posts 1", HttpMethod::POST),
            "GET posts 1"
        );
    }

    #[test]
    fn opening_the_same_saved_request_twice_reuses_its_tab() {
        let draft = RequestDraft {
            id: Some("request-1".to_string()),
            name: "Request uno".to_string(),
            method: HttpMethod::GET,
            url: "https://example.com".to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            auth: AuthConfig::None,
            body: RequestBodyDraft {
                mode: BodyMode::None,
                value: String::new(),
                form_data: Vec::new(),
            },
            timeout_ms: 30_000,
            environment_id: None,
            response_tests: Vec::new(),
        };
        let record = SavedRequestRecord {
            id: "request-1".to_string(),
            collection_id: "collection-1".to_string(),
            folder_id: None,
            name: draft.name.clone(),
            draft: draft.clone(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let collection = CollectionWithRequests {
            collection: CollectionSummary {
                id: "collection-1".to_string(),
                name: "Colección".to_string(),
                request_count: 1,
                created_at: String::new(),
                updated_at: String::new(),
            },
            folders: Vec::new(),
            requests: vec![record],
        };
        let mut state = build_midway_for_tree_state(
            vec![collection],
            Some("collection-1".to_string()),
            String::new(),
        );

        let _ = update_tree(
            &mut state,
            TreeMessage::RequestOpened("request-1".to_string()),
        );
        let first_tab_id = state.tabs[0].id.clone();
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.tabs[0].draft.id.as_deref(), Some("request-1"));

        // Opening it again focuses the logical request instead of adding
        // another tab.
        let _ = update_tree(
            &mut state,
            TreeMessage::RequestOpened("request-1".to_string()),
        );

        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.tabs[0].id, first_tab_id);
        assert_eq!(state.active_tab, Some(0));
    }

    fn dnd_state(request_folder_id: Option<&str>) -> Midway {
        let draft = RequestDraft {
            id: Some("request-1".to_string()),
            name: "Request uno".to_string(),
            method: HttpMethod::GET,
            url: "https://example.com".to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            auth: AuthConfig::None,
            body: RequestBodyDraft {
                mode: BodyMode::None,
                value: String::new(),
                form_data: Vec::new(),
            },
            timeout_ms: 30_000,
            environment_id: None,
            response_tests: Vec::new(),
        };
        let request = SavedRequestRecord {
            id: "request-1".to_string(),
            collection_id: "collection-1".to_string(),
            folder_id: request_folder_id.map(str::to_string),
            name: draft.name.clone(),
            draft,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let folders = vec![
            Folder {
                id: "folder-1".to_string(),
                collection_id: "collection-1".to_string(),
                parent_folder_id: None,
                name: "Primera".to_string(),
            },
            Folder {
                id: "folder-2".to_string(),
                collection_id: "collection-1".to_string(),
                parent_folder_id: None,
                name: "Segunda".to_string(),
            },
        ];
        let collection = CollectionWithRequests {
            collection: CollectionSummary {
                id: "collection-1".to_string(),
                name: "Colección".to_string(),
                request_count: 1,
                created_at: String::new(),
                updated_at: String::new(),
            },
            folders,
            requests: vec![request],
        };
        build_midway_for_tree_state(
            vec![collection],
            Some("collection-1".to_string()),
            String::new(),
        )
    }

    #[test]
    fn request_drag_to_folder_starts_one_persistence_task() {
        let mut state = dnd_state(None);

        let _ = update_tree(
            &mut state,
            TreeMessage::RequestDragStarted("request-1".to_string()),
        );
        let _ = update_tree(
            &mut state,
            TreeMessage::RequestDropTargetEntered(RequestDropTarget::Folder(
                "folder-2".to_string(),
            )),
        );
        let task = update_tree(&mut state, TreeMessage::RequestDragReleased);

        assert_eq!(task.units(), 1);
        assert_eq!(state.tree.moving_request_id.as_deref(), Some("request-1"));
        assert!(state.tree.request_drag.is_none());
    }

    #[test]
    fn request_drop_on_same_folder_is_a_no_op() {
        let mut state = dnd_state(Some("folder-1"));

        let _ = update_tree(
            &mut state,
            TreeMessage::RequestDragStarted("request-1".to_string()),
        );
        let _ = update_tree(
            &mut state,
            TreeMessage::RequestDropTargetEntered(RequestDropTarget::Folder(
                "folder-1".to_string(),
            )),
        );
        let task = update_tree(&mut state, TreeMessage::RequestDragReleased);

        assert_eq!(task.units(), 0);
        assert!(state.tree.moving_request_id.is_none());
        assert!(!state.session.dirty);
    }

    #[test]
    fn invalid_drop_target_is_rejected_without_persistence() {
        let mut state = dnd_state(None);
        let _ = update_tree(
            &mut state,
            TreeMessage::RequestDragStarted("request-1".to_string()),
        );
        let _ = update_tree(
            &mut state,
            TreeMessage::RequestDropTargetEntered(RequestDropTarget::Folder(
                "missing-folder".to_string(),
            )),
        );

        assert!(state
            .tree
            .request_drag
            .as_ref()
            .is_some_and(|drag| drag.target.is_none()));
        assert_eq!(
            state.tree.error.as_deref(),
            Some("Ese destino ya no está disponible.")
        );
        let task = update_tree(&mut state, TreeMessage::RequestDragReleased);
        assert_eq!(task.units(), 0);
        assert!(state.tree.moving_request_id.is_none());
    }

    #[test]
    fn a_second_drag_is_ignored_while_a_move_is_in_flight() {
        let mut state = dnd_state(None);
        state.tree.moving_request_id = Some("request-1".to_string());

        let task = update_tree(
            &mut state,
            TreeMessage::RequestDragStarted("request-1".to_string()),
        );

        assert_eq!(task.units(), 0);
        assert!(state.tree.request_drag.is_none());
    }

    #[test]
    fn tree_view_builds_with_hover_actions_and_active_drop_target() {
        let mut state = dnd_state(None);
        state.tree.hovered_item = Some(TreeHoveredItem::Request("request-1".to_string()));
        state.tree.request_drag = Some(RequestDragState {
            request_id: "request-1".to_string(),
            target: Some(RequestDropTarget::Folder("folder-2".to_string())),
        });
        let ds = DesignSystem::for_mode(ThemeMode::Dark);

        let _element = view(&state, &ds);
    }

    #[test]
    fn request_can_move_from_folder_back_to_root() {
        let mut state = dnd_state(Some("folder-1"));

        let _ = update_tree(
            &mut state,
            TreeMessage::RequestDragStarted("request-1".to_string()),
        );
        let _ = update_tree(
            &mut state,
            TreeMessage::RequestDropTargetEntered(RequestDropTarget::Root),
        );
        let task = update_tree(&mut state, TreeMessage::RequestDragReleased);

        assert_eq!(task.units(), 1);
        assert_eq!(state.tree.moving_request_id.as_deref(), Some("request-1"));
    }

    #[test]
    fn request_move_completion_updates_snapshot_or_exposes_error() {
        let mut state = dnd_state(None);
        state.tree.moving_request_id = Some("request-1".to_string());

        let _ = update_tree(
            &mut state,
            TreeMessage::RequestMoveCompleted {
                request_id: "request-1".to_string(),
                destination_folder_id: Some("folder-2".to_string()),
                result: Err("falló el movimiento".to_string()),
            },
        );
        assert_eq!(state.workspace.collections[0].requests[0].folder_id, None);
        assert_eq!(state.tree.error.as_deref(), Some("falló el movimiento"));
        assert!(state.tree.moving_request_id.is_none());

        state.tree.moving_request_id = Some("request-1".to_string());
        state.tree.collapsed.insert("folder-2".to_string());
        let _ = update_tree(
            &mut state,
            TreeMessage::RequestMoveCompleted {
                request_id: "request-1".to_string(),
                destination_folder_id: Some("folder-2".to_string()),
                result: Ok(()),
            },
        );

        assert_eq!(
            state.workspace.collections[0].requests[0]
                .folder_id
                .as_deref(),
            Some("folder-2")
        );
        assert!(!state.tree.collapsed.contains("folder-2"));
        assert!(state.session.dirty);
        assert!(state.tree.error.is_none());
    }
}

const TREE_ACTION_SIZE: f32 = 24.0;
const TREE_ACTION_GAP: f32 = 2.0;
const TWO_ACTIONS_WIDTH: f32 = TREE_ACTION_SIZE * 2.0 + TREE_ACTION_GAP;
const THREE_ACTIONS_WIDTH: f32 = TREE_ACTION_SIZE * 3.0 + TREE_ACTION_GAP * 2.0;

fn action_tooltip<'a>(
    content: Element<'a, Message>,
    label: &'static str,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let background = ds.palette.surface_elevated;
    let border = ds.palette.border;
    let radius = ds.radius.control;
    tooltip(
        content,
        text(label).size(ds.typography.secondary.size),
        tooltip::Position::Top,
    )
    .gap(6)
    .padding(6)
    .delay(iced::time::milliseconds(300))
    .snap_within_viewport(true)
    .style(move |_theme| container::Style {
        background: Some(background.into()),
        border: Border {
            color: border,
            width: 1.0,
            radius: radius.into(),
        },
        ..container::Style::default()
    })
    .into()
}

fn compact_action<'a>(
    symbol: &'static str,
    label: &'static str,
    message: Message,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let control = button(
        container(text(symbol).size(ds.typography.secondary.size))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fixed(TREE_ACTION_SIZE))
    .height(Length::Fixed(TREE_ACTION_SIZE))
    .padding(0)
    .on_press(message);
    action_tooltip(control.into(), label, ds)
}

fn finish_request_drag(state: &mut Midway) -> iced::Task<Message> {
    let Some(drag) = state.tree.request_drag.take() else {
        return iced::Task::none();
    };
    let Some(target) = drag.target else {
        return iced::Task::none();
    };
    if state.tree.moving_request_id.is_some() {
        return iced::Task::none();
    }

    let Some(collection_id) = state.active_collection_id.as_deref() else {
        state.tree.error = Some("No hay una colección activa.".to_string());
        return iced::Task::none();
    };
    let Some(collection) = state
        .workspace
        .collections
        .iter()
        .find(|collection| collection.collection.id == collection_id)
    else {
        state.tree.error = Some("No se encontró la colección activa.".to_string());
        return iced::Task::none();
    };
    let Some(request) = collection
        .requests
        .iter()
        .find(|request| request.id == drag.request_id)
    else {
        state.tree.error = Some("No se encontró el request arrastrado.".to_string());
        return iced::Task::none();
    };

    let destination_folder_id = match target {
        RequestDropTarget::Root => None,
        RequestDropTarget::Folder(folder_id) => {
            if !collection
                .folders
                .iter()
                .any(|folder| folder.id == folder_id)
            {
                state.tree.error = Some("La carpeta de destino ya no existe.".to_string());
                return iced::Task::none();
            }
            Some(folder_id)
        }
    };
    if request.folder_id == destination_folder_id {
        return iced::Task::none();
    }

    let request_id = drag.request_id;
    state.tree.moving_request_id = Some(request_id.clone());
    state.tree.error = None;
    let app_state = std::sync::Arc::clone(&state.app_state);
    let result_request_id = request_id.clone();
    let result_destination = destination_folder_id.clone();

    iced::Task::perform(
        async move {
            app_state
                .repository
                .update_request_folder(&request_id, destination_folder_id.as_deref())
                .await
                .map_err(|error| error.to_string())
        },
        move |result| {
            Message::Tree(TreeMessage::RequestMoveCompleted {
                request_id: result_request_id.clone(),
                destination_folder_id: result_destination.clone(),
                result,
            })
        },
    )
}
