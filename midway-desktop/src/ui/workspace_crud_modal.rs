//! Modal compartido para altas, modificaciones y bajas del workspace.
//!
//! El componente es deliberadamente presentacional: `app.rs` conserva el
//! estado y ejecuta las mutaciones, mientras esta vista sólo traduce la
//! intención del usuario a [`WorkspaceCrudMessage`].

use iced::widget::{button, column, container, mouse_area, opaque, row, stack, text, text_input};
use iced::{Border, Color, Element, Length};

use crate::app::{Message, WorkspaceCrudDialogState, WorkspaceCrudKind, WorkspaceCrudMessage};
use crate::ui::design_system::{contrast_text_color, DesignSystem};

/// Superpone el diálogo CRUD cuando `dialog` está presente.
///
/// Recibir el diálogo por separado mantiene este componente independiente del
/// nombre concreto del campo que lo aloja dentro de `Midway`.
pub fn with_overlay<'a>(
    dialog: Option<&'a WorkspaceCrudDialogState>,
    main_content: Element<'a, Message>,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let Some(dialog) = dialog else {
        return main_content;
    };

    stack![main_content, opaque(workspace_crud_overlay(dialog, ds))].into()
}

fn workspace_crud_overlay<'a>(
    dialog: &'a WorkspaceCrudDialogState,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let overlay_background = Color {
        a: 0.5,
        ..ds.palette.background_primary
    };

    // No se cierra al hacer click fuera: tanto una edición en curso como una
    // baja destructiva requieren una decisión explícita.
    mouse_area(
        container(opaque(workspace_crud_box(dialog, ds)))
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

fn workspace_crud_box<'a>(
    dialog: &'a WorkspaceCrudDialogState,
    ds: &DesignSystem,
) -> Element<'a, Message> {
    let copy = dialog_copy(&dialog.kind, &dialog.entity_name);

    let mut content = column![
        text(copy.title)
            .size(ds.typography.subtitle.size)
            .color(ds.palette.text_primary),
        text(copy.description)
            .size(ds.typography.secondary.size)
            .color(ds.palette.text_secondary),
    ]
    .spacing(ds.spacing.sm);

    if kind_uses_name_input(&dialog.kind) {
        let mut name_input = text_input(copy.placeholder, &dialog.name_input)
            .width(Length::Fill)
            .size(ds.typography.body.size);

        if !dialog.busy {
            name_input = name_input
                .on_input(|name| Message::WorkspaceCrud(WorkspaceCrudMessage::NameChanged(name)))
                .on_submit(Message::WorkspaceCrud(WorkspaceCrudMessage::Confirmed));
        }

        content = content.push(text("Nombre").size(ds.typography.secondary.size));
        content = content.push(name_input);
    }

    if let Some(error) = dialog.error.as_ref() {
        content = content.push(
            text(error)
                .size(ds.typography.secondary.size)
                .color(ds.palette.status_error),
        );
    }

    let mut confirm_button = button(text(if dialog.busy {
        copy.busy_label
    } else {
        copy.confirm_label
    }))
    .padding([ds.spacing.sm, ds.spacing.md]);
    let mut cancel_button = button(text("Cancelar")).padding([ds.spacing.sm, ds.spacing.md]);

    let can_confirm = !dialog.busy
        && (!kind_uses_name_input(&dialog.kind) || !dialog.name_input.trim().is_empty());

    if can_confirm {
        confirm_button =
            confirm_button.on_press(Message::WorkspaceCrud(WorkspaceCrudMessage::Confirmed));
    }
    if !dialog.busy {
        cancel_button =
            cancel_button.on_press(Message::WorkspaceCrud(WorkspaceCrudMessage::Cancelled));
    }

    if kind_is_destructive(&dialog.kind) {
        let danger = ds.palette.status_error;
        let danger_text = contrast_text_color(danger);
        let radius = ds.radius.control;
        confirm_button = confirm_button.style(move |_theme, _status| button::Style {
            background: Some(danger.into()),
            text_color: danger_text,
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..button::Style::default()
        });
    }

    content = content.push(row![cancel_button, confirm_button].spacing(ds.spacing.sm));

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

#[derive(Debug, Clone)]
struct DialogCopy {
    title: String,
    description: String,
    placeholder: &'static str,
    confirm_label: &'static str,
    busy_label: &'static str,
}

fn dialog_copy(kind: &WorkspaceCrudKind, entity_name: &str) -> DialogCopy {
    match kind {
        WorkspaceCrudKind::CreateFolder { .. } => DialogCopy {
            title: "Nueva carpeta".to_string(),
            description: "Creá una carpeta para organizar los requests de la colección."
                .to_string(),
            placeholder: "Nombre de la carpeta",
            confirm_label: "Crear",
            busy_label: "Creando…",
        },
        WorkspaceCrudKind::RenameCollection { .. } => DialogCopy {
            title: "Renombrar colección".to_string(),
            description: format!("Cambiá el nombre de «{entity_name}»."),
            placeholder: "Nombre de la colección",
            confirm_label: "Guardar",
            busy_label: "Guardando…",
        },
        WorkspaceCrudKind::RenameFolder { .. } => DialogCopy {
            title: "Renombrar carpeta".to_string(),
            description: format!("Cambiá el nombre de «{entity_name}»."),
            placeholder: "Nombre de la carpeta",
            confirm_label: "Guardar",
            busy_label: "Guardando…",
        },
        WorkspaceCrudKind::DeleteCollection { .. } => DialogCopy {
            title: "Eliminar colección".to_string(),
            description: format!(
                "¿Eliminar «{entity_name}»? También se eliminarán sus carpetas, requests y tabs asociadas; se perderán sus cambios no guardados. Esta acción no se puede deshacer."
            ),
            placeholder: "",
            confirm_label: "Eliminar",
            busy_label: "Eliminando…",
        },
        WorkspaceCrudKind::DeleteFolder { .. } => DialogCopy {
            title: "Eliminar carpeta".to_string(),
            description: format!(
                "¿Eliminar «{entity_name}»? También se eliminarán sus subcarpetas, requests y tabs asociadas; se perderán sus cambios no guardados. Esta acción no se puede deshacer."
            ),
            placeholder: "",
            confirm_label: "Eliminar",
            busy_label: "Eliminando…",
        },
        WorkspaceCrudKind::DeleteRequest { .. } => DialogCopy {
            title: "Eliminar request".to_string(),
            description: format!(
                "¿Eliminar «{entity_name}»? Si está abierto, su tab se cerrará y se perderán los cambios no guardados. Esta acción no se puede deshacer."
            ),
            placeholder: "",
            confirm_label: "Eliminar",
            busy_label: "Eliminando…",
        },
    }
}

fn kind_uses_name_input(kind: &WorkspaceCrudKind) -> bool {
    matches!(
        kind,
        WorkspaceCrudKind::CreateFolder { .. }
            | WorkspaceCrudKind::RenameCollection { .. }
            | WorkspaceCrudKind::RenameFolder { .. }
    )
}

fn kind_is_destructive(kind: &WorkspaceCrudKind) -> bool {
    matches!(
        kind,
        WorkspaceCrudKind::DeleteCollection { .. }
            | WorkspaceCrudKind::DeleteFolder { .. }
            | WorkspaceCrudKind::DeleteRequest { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_rename_modes_use_the_name_input() {
        let create = WorkspaceCrudKind::CreateFolder {
            collection_id: "collection".to_string(),
            parent_folder_id: None,
        };
        let rename_collection = WorkspaceCrudKind::RenameCollection {
            collection_id: "collection".to_string(),
        };
        let rename_folder = WorkspaceCrudKind::RenameFolder {
            folder_id: "folder".to_string(),
        };

        assert!(kind_uses_name_input(&create));
        assert!(kind_uses_name_input(&rename_collection));
        assert!(kind_uses_name_input(&rename_folder));
        assert!(!kind_is_destructive(&create));
    }

    #[test]
    fn every_delete_mode_is_destructive_and_has_irreversible_copy() {
        let delete_modes = [
            WorkspaceCrudKind::DeleteCollection {
                collection_id: "collection".to_string(),
            },
            WorkspaceCrudKind::DeleteFolder {
                folder_id: "folder".to_string(),
            },
            WorkspaceCrudKind::DeleteRequest {
                request_id: "request".to_string(),
            },
        ];

        for mode in delete_modes {
            assert!(kind_is_destructive(&mode));
            assert!(!kind_uses_name_input(&mode));
            let copy = dialog_copy(&mode, "Elemento");
            assert_eq!(copy.confirm_label, "Eliminar");
            assert!(copy.description.contains("no se puede deshacer"));
        }
    }

    #[test]
    fn cascade_warning_is_explicit_for_collection_and_folder() {
        let collection_copy = dialog_copy(
            &WorkspaceCrudKind::DeleteCollection {
                collection_id: "collection".to_string(),
            },
            "API",
        );
        let folder_copy = dialog_copy(
            &WorkspaceCrudKind::DeleteFolder {
                folder_id: "folder".to_string(),
            },
            "Auth",
        );

        assert!(collection_copy.description.contains("carpetas"));
        assert!(collection_copy.description.contains("requests"));
        assert!(folder_copy.description.contains("subcarpetas"));
        assert!(folder_copy.description.contains("requests"));
        assert!(collection_copy.description.contains("cambios no guardados"));
        assert!(folder_copy.description.contains("cambios no guardados"));
    }

    #[test]
    fn create_and_rename_copy_uses_the_expected_labels() {
        let create_copy = dialog_copy(
            &WorkspaceCrudKind::CreateFolder {
                collection_id: "collection".to_string(),
                parent_folder_id: None,
            },
            "",
        );
        let rename_collection_copy = dialog_copy(
            &WorkspaceCrudKind::RenameCollection {
                collection_id: "collection".to_string(),
            },
            "API",
        );
        let rename_folder_copy = dialog_copy(
            &WorkspaceCrudKind::RenameFolder {
                folder_id: "folder".to_string(),
            },
            "Auth",
        );

        assert_eq!(create_copy.title, "Nueva carpeta");
        assert_eq!(create_copy.confirm_label, "Crear");
        assert_eq!(create_copy.busy_label, "Creando…");
        assert_eq!(create_copy.placeholder, "Nombre de la carpeta");

        for copy in [rename_collection_copy, rename_folder_copy] {
            assert_eq!(copy.confirm_label, "Guardar");
            assert_eq!(copy.busy_label, "Guardando…");
            assert!(copy.description.contains("Cambiá el nombre"));
        }
    }
}
