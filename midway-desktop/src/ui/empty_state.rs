//! Componente compartido de estado vacío accionable (Tarea 11.1 —
//! Requisitos 9.5, 14.2).
//!
//! Un estado vacío nombra qué falta ([`EmptyState::title`]), explica la acción
//! siguiente disponible ([`EmptyState::hint`]) y, cuando esa acción es posible
//! en el contexto, ofrece un botón que la dispara
//! ([`EmptyState::action`]). Si la acción no aplica, el componente se renderiza
//! sin botón: nunca un botón permanentemente deshabilitado (Req 14.2).
//!
//! El mismo componente cubre los periodos breves de carga (Req 9.5): se lo usa
//! con un `hint` de progreso en vez de dejar el área en blanco.
//!
//! Restricción estructural, igual que en `ui/theme_settings.rs`: las únicas
//! dependencias son `iced` y `crate::ui::design_system`. El componente no
//! conoce el `Message` raíz (es genérico en `M`), no lee el estado agregado de
//! la aplicación, y no toca sesión, disco, SQL ni red. Todo el color, tamaño y
//! espaciado sale de los tokens de [`DesignSystem`]; no introduce colores de
//! marca nuevos (Req 9.8).
//!
//! Las cadenas de los estados vacíos reemplazados por la tarea 11.2 viven en
//! este módulo, en la sección "Especificaciones concretas": son datos planos y
//! genéricos en `M`, así que el texto queda en un solo lugar (la tabla del
//! diseño §8.3) y los tests de la tarea 11.4 pueden fijarlos sin construir un
//! `Midway` ni abrir una ventana.
//!
//! El `#![allow(dead_code)]` de módulo que este archivo llevaba mientras no
//! tenía consumidores se eliminó al aterrizar la tarea 11.2: [`view`],
//! [`EmptyState::new`], [`EmptyState::with_action`] y las cinco
//! especificaciones concretas tienen llamador de producción. El único `allow`
//! que queda es el acotado a [`EmptyState::has_action`], con su justificación
//! escrita en el propio método.

use iced::widget::{button, column, container, text};
use iced::{Alignment, Border, Element, Length};

use crate::ui::design_system::{contrast_text_color, DesignSystem};

/// Especificación declarativa de un estado vacío.
///
/// Es un dato plano: no tiene lógica ni estado propio, así que un test puede
/// fijar título, hint y presencia o ausencia de acción sin abrir una ventana
/// (tarea 11.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyState<'a, M> {
    /// Nombra qué falta, en positivo y en español con voseo (Req 9.6).
    pub title: &'a str,
    /// Explica la acción siguiente disponible, o el progreso en curso durante
    /// un periodo breve de carga.
    pub hint: &'a str,
    /// Etiqueta y mensaje de la acción, cuando la acción es posible en el
    /// contexto. `None` significa "sin botón", no "botón deshabilitado".
    pub action: Option<(&'a str, M)>,
}

impl<'a, M> EmptyState<'a, M> {
    /// Estado vacío sin acción: solo nombra qué falta y explica el contexto.
    pub fn new(title: &'a str, hint: &'a str) -> Self {
        Self {
            title,
            hint,
            action: None,
        }
    }

    /// Agrega la acción siguiente al estado vacío.
    pub fn with_action(mut self, label: &'a str, message: M) -> Self {
        self.action = Some((label, message));
        self
    }

    /// `true` si el estado vacío ofrece un botón de acción.
    ///
    /// `allow(dead_code)` acotado a este método: la decisión de renderizar con
    /// o sin botón la toma [`view`] destructurando `action`, así que producción
    /// no necesita el predicado. Existe para que los tests de especificación
    /// (tarea 11.4) expresen "esta especificación ofrece acción" sin repetir
    /// `action.is_some()`. Se elimina si un consumidor de producción lo vuelve
    /// innecesario.
    #[allow(dead_code)]
    pub fn has_action(&self) -> bool {
        self.action.is_some()
    }
}

// ---------------------------------------------------------------------------
// Especificaciones concretas (tarea 11.2 — diseño §8.3).
//
// Cada una de las cuatro ubicaciones que el baseline resolvía con un `text()`
// plano tiene acá su especificación nombrada, más la variante de progreso que
// cubre el periodo breve de carga exigido por el Req 9.5. Son genéricas en `M`
// cuando no llevan acción, de forma que sirven para el `Message` raíz y para
// los mensajes de una vertical sin duplicar cadenas.
// ---------------------------------------------------------------------------

/// `Response_Inspector` sin respuesta para la tab activa.
///
/// Reemplaza el "Sin respuesta aún" del baseline. No lleva acción: el disparo
/// del request vive en el botón Send del `Request_Composer`, que está visible
/// en la misma pantalla, y duplicarlo acá agregaría un segundo punto de envío
/// en vez de nombrar el siguiente paso.
pub fn no_response<'a, M>() -> EmptyState<'a, M> {
    EmptyState::new(
        "Sin respuesta todavía",
        "Ejecutá el request para ver la respuesta",
    )
}

/// `Response_Inspector` mientras el request está en vuelo (Req 9.5: durante
/// periodos breves de carga se muestra el mismo componente con `hint` de
/// progreso, en vez de dejar el área en blanco).
pub fn response_in_flight<'a, M>() -> EmptyState<'a, M> {
    EmptyState::new(
        "Enviando el request",
        "Esperando la respuesta del servidor",
    )
}

/// Tab Cookies sin cookies para la `final_url` de la respuesta activa.
///
/// Reemplaza el "No hay cookies almacenadas" del baseline. Sin acción: el
/// usuario no puede crear cookies desde acá, las establece el servidor.
pub fn no_cookies<'a, M>() -> EmptyState<'a, M> {
    EmptyState::new(
        "Sin cookies almacenadas",
        "Las cookies aparecen acá después de una respuesta que las establezca",
    )
}

/// Tab Tests sin assertions configuradas para el request.
///
/// La acción despacha el mensaje que el editor de assertions del
/// `Request_Composer` ya usa para agregar una assertion nueva al draft: es la
/// misma capacidad real, no una simulación (Req 14.2). La assertion agregada se
/// configura en la tab Tests del `Request_Composer`, que es donde vive su
/// editor; el `hint` lo dice explícitamente para que el efecto de la acción no
/// quede sin explicar cuando esa tab no es la seleccionada.
pub fn no_assertions<'a, M>(add_assertion: M) -> EmptyState<'a, M> {
    EmptyState::new(
        "Sin assertions configuradas",
        "Agregá una assertion y configurala en la tab Tests del request",
    )
    .with_action("Agregar assertion", add_assertion)
}

/// Área Debug sin ninguna tab de request abierta.
///
/// Reemplaza el "Midway Desktop — no hay tabs abiertas." del baseline. La
/// acción despacha el mismo mensaje que el atajo Ctrl+Shift+N y el botón
/// "+ Request" del explorador, así que abre una tab en blanco de verdad.
pub fn no_open_requests<'a, M>(new_request: M) -> EmptyState<'a, M> {
    EmptyState::new(
        "Sin requests abiertos",
        "Creá un request nuevo para empezar a trabajar",
    )
    .with_action("Nuevo request", new_request)
}

/// Renderiza el estado vacío centrado en el área disponible.
///
/// Devuelve `Element<'a, M>` con `M` genérico: el llamador decide qué mensaje
/// emite la acción, de modo que el componente sirve tanto para el `Message`
/// raíz como para los mensajes de una vertical.
///
/// Cuando `spec.action` es `None` la columna se arma sin el botón; no se emite
/// un `button` sin `on_press` (que iced renderizaría como control muerto).
pub fn view<'a, M: Clone + 'a>(spec: EmptyState<'a, M>, ds: &DesignSystem) -> Element<'a, M> {
    let EmptyState {
        title,
        hint,
        action,
    } = spec;

    let mut content = column![
        text(title)
            .size(ds.typography.subtitle.size)
            .color(ds.palette.text_primary),
        text(hint)
            .size(ds.typography.body.size)
            .color(ds.palette.text_secondary),
    ]
    .spacing(ds.spacing.sm)
    .align_x(Alignment::Center);

    if let Some((label, message)) = action {
        content = content.push(action_button(label, message, ds));
    }

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(ds.spacing.lg)
        .into()
}

/// Botón de acción del estado vacío, con el mismo tratamiento visual que la
/// acción guiada del onboarding (`ui/onboarding.rs`): fondo `accent`, radio de
/// control y color de texto derivado de
/// [`contrast_text_color`](crate::ui::design_system::contrast_text_color) para
/// no depender de un blanco fijo.
fn action_button<'a, M: Clone + 'a>(
    label: &'a str,
    message: M,
    ds: &DesignSystem,
) -> Element<'a, M> {
    let accent = ds.palette.accent;
    let radius = ds.radius.control;
    let label_color = contrast_text_color(accent);

    button(text(label).size(ds.typography.body.size).color(label_color))
        .padding([ds.spacing.sm, ds.spacing.md])
        .style(move |_theme, _status| button::Style {
            background: Some(accent.into()),
            text_color: label_color,
            border: Border {
                radius: radius.into(),
                ..Border::default()
            },
            ..button::Style::default()
        })
        .on_press(message)
        .into()
}
#[cfg(test)]
mod tests {
    //! Tests de ejemplo de las especificaciones de `EmptyState`
    //! (tarea 11.4 — Requisitos 9.5, 9.6, 14.2).
    //!
    //! **Validates: Requirements 9.5, 9.6, 14.2**
    //!
    //! Son tests de ejemplo, no de propiedad: las cinco especificaciones son
    //! datos fijos del repositorio (la tabla del diseño §8.3), así que lo que
    //! hay que fijar es exactamente ese texto, no una invariante sobre
    //! entradas generadas.
    //!
    //! Corren sin ventana, sin disco y sin red: cada test construye la
    //! especificación y lee sus campos. No se llama a [`view`], que necesitaría
    //! un `DesignSystem` y un renderer.
    //!
    //! Las especificaciones se instancian con un `TestMessage` local en vez de
    //! con el `Message` raíz de la aplicación, por dos razones: mantiene la
    //! restricción estructural del módulo (sus únicas dependencias son `iced` y
    //! `crate::ui::design_system`, ver la doc de módulo), y permite comparar el
    //! mensaje de la acción por igualdad, cosa que `Message` no soporta porque
    //! no deriva `PartialEq`. Lo que estos tests fijan, entonces, es que el
    //! constructor cuelga en `action` el mensaje que recibió, sin sustituirlo;
    //! qué variante concreta le pasa producción se decide en cada sitio de
    //! llamada (`ui/response_inspector.rs` y `app.rs`, tarea 11.2).

    use super::*;

    /// Mensaje de prueba: representa las dos acciones que las
    /// especificaciones con botón reciben de producción, sin depender del
    /// `Message` raíz.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestMessage {
        AddAssertion,
        NewRequest,
    }

    /// Las cinco especificaciones concretas del módulo, para los tests que
    /// verifican algo sobre todas ellas.
    ///
    /// Si la tarea 11.2 agrega una especificación nueva y no se la agrega
    /// acá, los tests por estado vacío siguen pasando pero los transversales
    /// dejan de cubrirla; el comentario queda como recordatorio porque el
    /// compilador no lo puede avisar.
    fn all_specs() -> Vec<EmptyState<'static, TestMessage>> {
        vec![
            no_response(),
            response_in_flight(),
            no_cookies(),
            no_assertions(TestMessage::AddAssertion),
            no_open_requests(TestMessage::NewRequest),
        ]
    }

    // Req 9.5: el estado vacío nombra qué falta y explica la acción siguiente.
    // Req 14.2: sin botón cuando la acción no aplica en el contexto, nunca un
    // botón permanentemente deshabilitado.
    #[test]
    fn no_response_spec_has_title_hint_and_no_action() {
        let spec: EmptyState<'_, TestMessage> = no_response();

        assert_eq!(spec.title, "Sin respuesta todavía");
        assert_eq!(spec.hint, "Ejecutá el request para ver la respuesta");
        assert_eq!(
            spec.action, None,
            "el disparo del request vive en el botón Send del Request_Composer, \
             visible en la misma pantalla: este estado vacío no duplica ese punto de envío"
        );
        assert!(!spec.has_action());
    }

    // Req 9.5: durante un periodo breve de carga se muestra el mismo
    // componente con hint de progreso, en vez de dejar el área en blanco.
    #[test]
    fn response_in_flight_spec_has_progress_hint_and_no_action() {
        let spec: EmptyState<'_, TestMessage> = response_in_flight();

        assert_eq!(spec.title, "Enviando el request");
        assert_eq!(spec.hint, "Esperando la respuesta del servidor");
        assert_eq!(
            spec.action, None,
            "mientras el request está en vuelo no hay acción siguiente que ofrecer"
        );
        assert!(!spec.has_action());
    }

    #[test]
    fn no_cookies_spec_has_title_hint_and_no_action() {
        let spec: EmptyState<'_, TestMessage> = no_cookies();

        assert_eq!(spec.title, "Sin cookies almacenadas");
        assert_eq!(
            spec.hint,
            "Las cookies aparecen acá después de una respuesta que las establezca"
        );
        assert_eq!(
            spec.action, None,
            "el usuario no crea cookies desde acá: las establece el servidor"
        );
        assert!(!spec.has_action());
    }

    // Req 14.2: la acción propuesta es una capacidad real (el mismo mensaje
    // que el editor de assertions ya usa), no una funcionalidad simulada.
    #[test]
    fn no_assertions_spec_offers_add_assertion_action() {
        let spec = no_assertions(TestMessage::AddAssertion);

        assert_eq!(spec.title, "Sin assertions configuradas");
        assert_eq!(
            spec.hint,
            "Agregá una assertion y configurala en la tab Tests del request"
        );
        assert_eq!(
            spec.action,
            Some(("Agregar assertion", TestMessage::AddAssertion)),
            "la etiqueta y el mensaje recibido deben quedar tal cual en la acción"
        );
        assert!(spec.has_action());
    }

    #[test]
    fn no_open_requests_spec_offers_new_request_action() {
        let spec = no_open_requests(TestMessage::NewRequest);

        assert_eq!(spec.title, "Sin requests abiertos");
        assert_eq!(spec.hint, "Creá un request nuevo para empezar a trabajar");
        assert_eq!(
            spec.action,
            Some(("Nuevo request", TestMessage::NewRequest)),
            "la etiqueta y el mensaje recibido deben quedar tal cual en la acción"
        );
        assert!(spec.has_action());
    }

    // Req 9.5: ningún estado vacío puede quedar sin nombrar qué falta ni sin
    // explicar el contexto. Un `title` o un `hint` vacío renderizaría un
    // `text("")`, que es justamente el área en blanco que el requisito
    // prohíbe.
    #[test]
    fn every_spec_has_non_empty_title_and_hint() {
        for spec in all_specs() {
            assert!(
                !spec.title.trim().is_empty(),
                "una especificación quedó sin título: {:?}",
                spec
            );
            assert!(
                !spec.hint.trim().is_empty(),
                "la especificación {:?} quedó sin hint",
                spec.title
            );
        }
    }

    // Req 9.5 / 14.2: cuando hay acción, su etiqueta tiene que ser legible.
    // Un botón con etiqueta vacía es un control sin significado, tan inútil
    // como uno deshabilitado.
    #[test]
    fn every_action_label_is_non_empty() {
        for spec in all_specs() {
            if let Some((label, _)) = &spec.action {
                assert!(
                    !label.trim().is_empty(),
                    "la especificación {:?} ofrece acción con etiqueta vacía",
                    spec.title
                );
            }
        }
    }

    // Req 9.6: idioma único, con voseo consistente.
    //
    // Esto es un tripwire acotado, no un detector de idioma: verifica que las
    // cadenas de este módulo no contengan las formas de tuteo de los cuatro
    // imperativos que sí aparecen en ellas ("Ejecutá", "Agregá", "Creá",
    // "configurala"). Si alguien reescribe un hint en tuteo, el test falla.
    // No puede afirmar que el texto esté en español: eso se verifica leyendo
    // la tabla del diseño §8.3, no en un test.
    #[test]
    fn spec_strings_use_voseo_not_tuteo() {
        const TUTEO_FORMS: &[&str] = &[
            "Ejecuta ",
            "ejecuta ",
            "Agrega ",
            "agrega ",
            "Crea ",
            "crea ",
            "configúrala",
            "Configúrala",
        ];

        for spec in all_specs() {
            let action_label = spec.action.as_ref().map(|(label, _)| *label).unwrap_or("");
            for text in [spec.title, spec.hint, action_label] {
                for tuteo in TUTEO_FORMS {
                    assert!(
                        !text.contains(tuteo),
                        "la cadena {:?} usa la forma de tuteo {:?}; el criterio del \
                         Req 9.6 es voseo consistente",
                        text,
                        tuteo
                    );
                }
            }
        }
    }
}
