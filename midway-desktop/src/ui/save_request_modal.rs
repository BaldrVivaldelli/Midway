//! Diálogo para guardar el request activo dentro de una colección.

use std::collections::HashSet;

use iced::widget::{
    button, column, container, mouse_area, opaque, pick_list, row, stack, text, text_input,
};
use iced::{Border, Color, Element, Length};

use midway_core::domain::workspace::Folder;

use crate::app::{Message, Midway, RequestComposerMessage, SaveRequestPromptState};
use crate::ui::design_system::DesignSystem;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CollectionOption {
    id: String,
    label: String,
}

impl std::fmt::Display for CollectionOption {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FolderOption {
    id: Option<String>,
    label: String,
}

impl std::fmt::Display for FolderOption {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub fn with_overlay<'a>(
    state: &'a Midway,
    main_content: Element<'a, Message>,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let Some(prompt) = state.save_request_prompt.as_ref() else {
        return main_content;
    };

    stack![
        main_content,
        opaque(save_request_overlay(state, prompt, ds))
    ]
    .into()
}

fn save_request_overlay<'a>(
    state: &'a Midway,
    prompt: &'a SaveRequestPromptState,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let overlay_background = Color {
        a: 0.5,
        ..ds.palette.background_primary
    };

    mouse_area(
        container(opaque(save_request_box(state, prompt, ds)))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(ds.spacing.md)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_theme| container::Style {
                background: Some(overlay_background.into()),
                ..container::Style::default()
            }),
    )
    .into()
}

fn save_request_box<'a>(
    state: &'a Midway,
    prompt: &'a SaveRequestPromptState,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let collection_options: Vec<CollectionOption> = state
        .workspace
        .collections
        .iter()
        .map(|collection| CollectionOption {
            id: collection.collection.id.clone(),
            label: collection.collection.name.clone(),
        })
        .collect();
    let selected_collection = collection_options
        .iter()
        .find(|option| Some(option.id.as_str()) == prompt.collection_id.as_deref())
        .cloned();

    let folder_options = prompt
        .collection_id
        .as_deref()
        .and_then(|collection_id| {
            state
                .workspace
                .collections
                .iter()
                .find(|collection| collection.collection.id == collection_id)
        })
        .map(|collection| folder_options(&collection.folders))
        .unwrap_or_else(|| {
            vec![FolderOption {
                id: None,
                label: "Raíz de la colección".to_string(),
            }]
        });
    let selected_folder = folder_options
        .iter()
        .find(|option| option.id == prompt.folder_id)
        .cloned()
        .or_else(|| folder_options.first().cloned());

    let name_input = text_input("Nombre del request", &prompt.name_input)
        .on_input(|name| Message::RequestComposer(RequestComposerMessage::SaveNameChanged(name)))
        .on_submit(Message::RequestComposer(
            RequestComposerMessage::SaveConfirmed,
        ))
        .width(Length::Fill);

    let collection_picker = pick_list(
        collection_options,
        selected_collection,
        |option: CollectionOption| {
            Message::RequestComposer(RequestComposerMessage::SaveCollectionChanged(option.id))
        },
    )
    .placeholder("Seleccioná una colección")
    .width(Length::Fill);

    let folder_picker = pick_list(folder_options, selected_folder, |option: FolderOption| {
        Message::RequestComposer(RequestComposerMessage::SaveFolderChanged(option.id))
    })
    .placeholder("Raíz de la colección")
    .width(Length::Fill);

    let mut content = column![
        text("Guardar request")
            .size(ds.typography.subtitle.size)
            .color(ds.palette.text_primary),
        text("Elegí el nombre y la ubicación que aparecerán en el árbol.")
            .size(ds.typography.secondary.size)
            .color(ds.palette.text_secondary),
        text("Nombre").size(ds.typography.secondary.size),
        name_input,
        text("Colección").size(ds.typography.secondary.size),
        collection_picker,
        text("Carpeta").size(ds.typography.secondary.size),
        folder_picker,
    ]
    .spacing(ds.spacing.sm);

    if state.workspace.collections.is_empty() {
        content = content.push(
            text("Primero creá una colección con el botón +.").color(ds.palette.status_error),
        );
    }
    if let Some(error) = prompt.error.as_ref() {
        content = content.push(text(error).color(ds.palette.status_error));
    }

    let save_label = if prompt.saving {
        "Guardando…"
    } else {
        "Guardar"
    };
    let mut save_button = button(text(save_label));
    let mut cancel_button = button(text("Cancelar"));
    if !prompt.saving && !state.workspace.collections.is_empty() {
        save_button = save_button.on_press(Message::RequestComposer(
            RequestComposerMessage::SaveConfirmed,
        ));
        cancel_button = cancel_button.on_press(Message::RequestComposer(
            RequestComposerMessage::SaveCancelled,
        ));
    }
    content = content.push(row![save_button, cancel_button].spacing(ds.spacing.sm));

    let background = ds.palette.surface_elevated;
    let border_color = ds.palette.border;
    let radius = ds.radius.panel;

    container(content)
        .width(Length::Fill)
        .max_width(440.0)
        .padding(ds.spacing.md)
        .style(move |_theme| container::Style {
            background: Some(background.into()),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: radius.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn folder_options(folders: &[Folder]) -> Vec<FolderOption> {
    let mut options = vec![FolderOption {
        id: None,
        label: "Raíz de la colección".to_string(),
    }];
    let mut folder_options: Vec<FolderOption> = folders
        .iter()
        .map(|folder| FolderOption {
            id: Some(folder.id.clone()),
            label: folder_path(folder, folders),
        })
        .collect();
    folder_options.sort_by(|left, right| left.label.cmp(&right.label));
    options.extend(folder_options);
    options
}

fn folder_path(folder: &Folder, folders: &[Folder]) -> String {
    let mut names = vec![folder.name.clone()];
    let mut parent_id = folder.parent_folder_id.as_deref();
    let mut visited = HashSet::from([folder.id.as_str()]);

    while let Some(id) = parent_id {
        if !visited.insert(id) {
            break;
        }
        let Some(parent) = folders.iter().find(|candidate| candidate.id == id) else {
            break;
        };
        names.push(parent.name.clone());
        parent_id = parent.parent_folder_id.as_deref();
    }

    names.reverse();
    names.join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_options_include_root_and_nested_paths() {
        let folders = vec![
            Folder {
                id: "child".to_string(),
                collection_id: "collection".to_string(),
                parent_folder_id: Some("parent".to_string()),
                name: "Child".to_string(),
            },
            Folder {
                id: "parent".to_string(),
                collection_id: "collection".to_string(),
                parent_folder_id: None,
                name: "Parent".to_string(),
            },
        ];

        let options = folder_options(&folders);

        assert_eq!(options[0].label, "Raíz de la colección");
        assert!(options
            .iter()
            .any(|option| option.label == "Parent / Child"));
    }
}
