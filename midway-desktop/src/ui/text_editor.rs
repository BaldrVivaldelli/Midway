//! `Text_Editor_Component` (Tarea 3.11): wrapper reusable sobre
//! `iced::widget::text_editor::Content` + `iced_highlighter::Highlighter`
//! con resaltado de sintaxis JSON.
//!
//! Se usa para el body de request, el body de response y el preview
//! (todos ellos texto plano/JSON editable o solo-lectura), por lo que este
//! componente se mantiene deliberadamente genérico: no conoce nada sobre
//! requests, responses ni el dominio de Midway.
//!
//! El formateo JSON (Tarea 3.12) y el lint de JSON inválido (Tarea 3.14) ya
//! están implementados en este wrapper. La búsqueda de texto (Tarea 3.16,
//! Requirement 2.15) también está implementada aquí: cálculo manual de
//! coincidencias case-insensitive sobre el texto plano del `Content` y
//! navegación secuencial siguiente/anterior.
//!
//! Ver diseño: "Components and Interfaces > Text_Editor_Component".

use iced::widget::text_editor::{Action, Content, Cursor, Position};
use iced::widget::TextEditor;

/// Token de sintaxis usado para el resaltado JSON (ver
/// `iced_highlighter::Settings::token`).
const JSON_SYNTAX_TOKEN: &str = "json";

/// Tema por defecto del resaltado de sintaxis. `InspiredGitHub` es un tema
/// claro, adecuado como valor por defecto neutro; los llamadores pueden
/// cambiarlo más adelante si se añade soporte de tema oscuro/claro a nivel
/// de aplicación.
const DEFAULT_HIGHLIGHT_THEME: iced_highlighter::Theme = iced_highlighter::Theme::InspiredGitHub;

/// Estado de la búsqueda de texto (Tarea 3.16, Requirement 2.15).
///
/// Mantiene la consulta actual, todas las coincidencias encontradas (como
/// rangos de byte-offset `(start, end)` dentro del texto plano devuelto por
/// `Content::text()`) y el índice de la coincidencia actualmente
/// seleccionada, navegable con `next_match`/`previous_match`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchState {
    pub query: String,
    /// Rangos de byte-offset `(start, end)` de cada coincidencia, en el
    /// mismo orden en que aparecen en el texto (ascendente por `start`).
    pub matches: Vec<(usize, usize)>,
    pub current_match_index: Option<usize>,
}

/// Encuentra todas las posiciones (byte-offset, incluyendo solapadas) donde
/// `needle` ocurre dentro de `haystack`, comparando de forma
/// case-insensitive.
///
/// Se compara carácter a carácter (vía `char_indices`) en lugar de
/// lowercasear ambas cadenas y buscar sobre el resultado, para evitar el
/// problema de que `str::to_lowercase` no garantiza preservar byte-offsets
/// equivalentes a los del texto original para todo Unicode (sí lo hace
/// para ASCII, pero no en general). De esta forma los rangos devueltos son
/// siempre byte-offsets válidos dentro del `haystack` original.
///
/// Por diseño (Property 7: "no omitir ni duplicar"), se reporta CADA
/// posición de inicio posible donde ocurre la subcadena, incluyendo
/// coincidencias solapadas (p. ej. buscar "aa" en "aaa" encuentra
/// coincidencias en la posición 0 y en la posición 1).
fn find_all_case_insensitive(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }

    let needle_chars: Vec<char> = needle.chars().flat_map(char::to_lowercase).collect();
    if needle_chars.is_empty() {
        return Vec::new();
    }

    // Cada entrada es (byte_offset, char) del haystack original, en orden.
    let haystack_chars: Vec<(usize, char)> = haystack.char_indices().collect();

    let mut matches = Vec::new();

    if haystack_chars.len() < needle_chars.len() {
        return matches;
    }

    for start in 0..=(haystack_chars.len() - needle_chars.len()) {
        let window = &haystack_chars[start..start + needle_chars.len()];

        let is_match = window.iter().zip(needle_chars.iter()).all(|((_, hc), nc)| {
            // Comparamos usando `to_lowercase()` por carácter: para la gran
            // mayoría de casos produce como máximo un char, pero algunos
            // caracteres Unicode se expanden a más de uno (p. ej. 'İ'); en
            // ese caso comparamos solo el primer char resultante, lo cual
            // es una aproximación razonable y documentada para este scope.
            hc.to_lowercase().next() == Some(*nc)
        });

        if is_match {
            let match_start = window[0].0;
            let match_end = match window.last() {
                Some((offset, ch)) => offset + ch.len_utf8(),
                None => match_start,
            };
            matches.push((match_start, match_end));
        }
    }

    matches
}

/// Error de lint de JSON (Tarea 3.14, Requirement 2.14): indica la línea y
/// columna (1-indexadas, según la convención de `serde_json`) donde ocurre
/// el error de sintaxis/formato, junto con una descripción del motivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonLintError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Estado de un editor de texto con resaltado de sintaxis JSON.
///
/// Envuelve `iced::widget::text_editor::Content` (el buffer editable) junto
/// con la configuración de `iced_highlighter::Highlighter` usada para
/// pintar el resaltado en `view`.
///
/// Nota: aún no está conectado a `Midway`/`app.rs`; las tareas de la Fase 2
/// (Body de request/response, preview) lo instancian y lo usan como campo
/// de estado. Se permite código no usado hasta entonces.
#[allow(dead_code)]
pub struct TextEditorState {
    pub content: Content,
    highlighter_settings: iced_highlighter::Settings,
    /// Estado de la búsqueda de texto (Tarea 3.16, Requirement 2.15).
    pub search: SearchState,
}

#[allow(dead_code)]
impl TextEditorState {
    /// Crea un nuevo estado de editor a partir de un texto inicial,
    /// configurado con resaltado de sintaxis JSON.
    pub fn new(initial_text: &str) -> Self {
        Self {
            content: Content::with_text(initial_text),
            highlighter_settings: iced_highlighter::Settings {
                theme: DEFAULT_HIGHLIGHT_THEME,
                token: JSON_SYNTAX_TOKEN.to_string(),
            },
            search: SearchState::default(),
        }
    }

    /// Aplica una acción del widget (edición, movimiento del cursor,
    /// selección, etc.) al `Content` subyacente.
    pub fn update(&mut self, action: Action) {
        self.content.perform(action);
    }

    /// Construye el widget `iced::widget::text_editor` para este estado,
    /// con el resaltado de sintaxis JSON ya configurado.
    ///
    /// El closure de mapeo de mensajes (`on_action`) se aplica externamente
    /// por el llamador, ya que este componente no conoce el `Message` de
    /// nivel superior de la aplicación.
    pub fn view<Message: Clone>(&self) -> TextEditor<'_, iced_highlighter::Highlighter, Message> {
        iced::widget::text_editor(&self.content)
            .highlight(&self.highlighter_settings.token, self.highlighter_settings.theme)
    }

    /// Reformatea el contenido actual como JSON con indentación estándar
    /// (Tarea 3.12, Requirement 2.13).
    ///
    /// Si el contenido actual es JSON válido, lo reemplaza por su versión
    /// formateada con `serde_json::to_string_pretty`. Si no es JSON válido,
    /// el contenido no se modifica y se devuelve un `Err` describiendo el
    /// motivo (el lint/reporte de error en la UI se implementa en la Tarea
    /// 3.14; esta función solo se encarga de no mutar el contenido cuando
    /// el JSON es inválido).
    pub fn format_json(&mut self) -> Result<(), String> {
        let current_text = self.content.text();

        let parsed = serde_json::from_str::<serde_json::Value>(&current_text)
            .map_err(|err| format!("JSON inválido: {err}"))?;

        let formatted = serde_json::to_string_pretty(&parsed)
            .map_err(|err| format!("No se pudo formatear el JSON: {err}"))?;

        self.content = Content::with_text(&formatted);

        Ok(())
    }

    /// Verifica si el contenido actual es JSON válido (Tarea 3.14,
    /// Requirement 2.14), sin modificar el contenido bajo ninguna
    /// circunstancia (lectura pura).
    ///
    /// Devuelve `None` si el contenido es JSON válido. Si es inválido,
    /// devuelve `Some(JsonLintError)` con la línea, columna y descripción
    /// del error tal como los reporta `serde_json`, para que el llamador
    /// pueda señalarlo inline (p. ej. como una etiqueta bajo el editor) sin
    /// descartar el contenido existente.
    pub fn lint_json(&self) -> Option<JsonLintError> {
        let current_text = self.content.text();

        match serde_json::from_str::<serde_json::Value>(&current_text) {
            Ok(_) => None,
            Err(error) => Some(JsonLintError {
                line: error.line(),
                column: error.column(),
                message: error.to_string(),
            }),
        }
    }

    /// Actualiza la consulta de búsqueda (Tarea 3.16, Requirement 2.15) y
    /// recalcula todas las coincidencias case-insensitive contra el texto
    /// plano actual del `Content`.
    ///
    /// Si `query` no está vacía, `search.matches` se llena con TODAS las
    /// posiciones (byte-offset, incluyendo solapadas) donde ocurre la
    /// subcadena en el texto (ver `find_all_case_insensitive`), y
    /// `current_match_index` se fija en `Some(0)` si hay al menos una
    /// coincidencia, o en `None` si no hay ninguna.
    ///
    /// Si `query` está vacía, se limpian `matches` y `current_match_index`
    /// (no hay búsqueda activa).
    pub fn set_search_query(&mut self, query: &str) {
        self.search.query = query.to_string();

        if query.is_empty() {
            self.search.matches.clear();
            self.search.current_match_index = None;
            return;
        }

        let text = self.content.text();
        self.search.matches = find_all_case_insensitive(&text, query);

        self.search.current_match_index = if self.search.matches.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Avanza a la siguiente coincidencia, ciclando: tras la última
    /// coincidencia, `next_match` vuelve a la primera. No hace nada si no
    /// hay coincidencias.
    pub fn next_match(&mut self) {
        let count = self.search.matches.len();
        if count == 0 {
            return;
        }

        self.search.current_match_index = Some(match self.search.current_match_index {
            Some(index) => (index + 1) % count,
            None => 0,
        });
    }

    /// Retrocede a la coincidencia anterior, ciclando: antes de la primera
    /// coincidencia, `previous_match` va a la última. No hace nada si no
    /// hay coincidencias.
    pub fn previous_match(&mut self) {
        let count = self.search.matches.len();
        if count == 0 {
            return;
        }

        self.search.current_match_index = Some(match self.search.current_match_index {
            Some(0) => count - 1,
            Some(index) => index - 1,
            None => count - 1,
        });
    }

    /// Devuelve el rango de byte-offset `(start, end)` de la coincidencia
    /// actualmente seleccionada, si hay alguna.
    pub fn current_match(&self) -> Option<(usize, usize)> {
        self.search
            .current_match_index
            .and_then(|index| self.search.matches.get(index).copied())
    }

    /// Mueve la selección del `Content` al rango de la coincidencia actual
    /// (si hay alguna), para que el llamador pueda mostrarla resaltada y
    /// hacer scroll hasta ella.
    ///
    /// Traduce el rango de byte-offset (contra el texto plano) a una
    /// posición `(line, column)` de `iced::widget::text_editor` iterando
    /// las líneas del `Content` hasta ubicar el offset de inicio/fin,
    /// y usa `Content::move_to` con un `Cursor { position, selection }`
    /// para seleccionar exactamente ese rango.
    pub fn select_current_match(&mut self) {
        let Some((start, end)) = self.current_match() else {
            return;
        };

        let Some(start_position) = self.byte_offset_to_position(start) else {
            return;
        };
        let Some(end_position) = self.byte_offset_to_position(end) else {
            return;
        };

        self.content.move_to(Cursor {
            position: end_position,
            selection: Some(start_position),
        });
    }

    /// Traduce un byte-offset dentro del texto plano (`Content::text()`) a
    /// una `Position { line, column }`, donde `column` es también un
    /// byte-offset dentro de esa línea (según la convención de
    /// `iced::widget::text_editor::Position`/`cosmic_text::Cursor`).
    fn byte_offset_to_position(&self, target_offset: usize) -> Option<Position> {
        let mut consumed = 0usize;

        for (line_index, line) in self.content.lines().enumerate() {
            let line_len = line.text.len();
            let ending_len = line.ending.as_str().len();
            let line_span = line_len + ending_len;

            if target_offset <= consumed + line_len {
                return Some(Position {
                    line: line_index,
                    column: target_offset - consumed,
                });
            }

            consumed += line_span;
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::Value;

    /// Genera valores JSON arbitrarios (números, strings, bools, null,
    /// arrays y objects) con una profundidad y tamaño acotados, para que
    /// la generación sea rápida en el property test (Tarea 3.13,
    /// Property 5).
    fn arb_json_value() -> impl Strategy<Value = Value> {
        arb_json_value_with_depth(3)
    }

    fn arb_json_value_with_depth(depth: u32) -> proptest::strategy::BoxedStrategy<Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(|n| Value::Number(n.into())),
            ".*".prop_map(Value::String),
        ];

        if depth == 0 {
            leaf.boxed()
        } else {
            let inner = arb_json_value_with_depth(depth - 1);
            leaf.prop_recursive(depth, 20, 4, move |_inner_self| {
                prop_oneof![
                    proptest::collection::vec(arb_json_value_with_depth(depth - 1), 0..=4)
                        .prop_map(Value::Array),
                    proptest::collection::hash_map(
                        "[a-zA-Z][a-zA-Z0-9_]{0,5}",
                        arb_json_value_with_depth(depth - 1),
                        0..=4
                    )
                    .prop_map(|map| Value::Object(map.into_iter().collect())),
                ]
            })
            .boxed()
            .prop_union(inner.boxed())
            .boxed()
        }
    }

    proptest! {
        /// Property 5 (design.md): formatear JSON con
        /// `Text_Editor_Component` es idempotente y preserva el valor.
        ///
        /// Para cualquier valor JSON arbitrario: formatearlo produce
        /// `Ok(())`; volver a formatear el resultado también produce
        /// `Ok(())` y produce EXACTAMENTE el mismo texto (idempotencia);
        /// y el valor deserializado del texto formateado es
        /// estructuralmente igual al valor original (preservación del
        /// valor).
        ///
        /// **Validates: Requirements 2.13**
        #[test]
        fn format_json_is_idempotent_and_preserves_value(value in arb_json_value()) {
            let serialized = serde_json::to_string(&value)
                .expect("serde_json::to_string no debería fallar para un Value construido");

            let mut state = TextEditorState::new(&serialized);

            let first_format_result = state.format_json();
            prop_assert!(first_format_result.is_ok());
            let text_after_first_format = state.content.text();

            let second_format_result = state.format_json();
            prop_assert!(second_format_result.is_ok());
            let text_after_second_format = state.content.text();

            prop_assert_eq!(text_after_first_format.clone(), text_after_second_format);

            let round_tripped: Value = serde_json::from_str(&text_after_first_format)
                .expect("el texto formateado debería seguir siendo JSON válido");
            prop_assert_eq!(round_tripped, value);
        }
    }

    /// Genera cadenas arbitrarias a partir de un alfabeto reducido que
    /// incluye caracteres típicos de JSON (delimitadores, comillas, dos
    /// puntos, comas, dígitos, letras, espacios) y también `\n`, para que
    /// las cadenas generadas puedan abarcar varias líneas. Esto es
    /// necesario para poder ejercitar de forma no trivial el reporte de
    /// línea/columna del lint (Tarea 3.15, Property 6): un generador que
    /// nunca produjera `\n` solo cubriría casos de columna en la línea 1.
    fn arb_maybe_invalid_json_string() -> impl Strategy<Value = String> {
        let alphabet = prop_oneof![
            Just('{'),
            Just('}'),
            Just('['),
            Just(']'),
            Just('"'),
            Just(':'),
            Just(','),
            Just(' '),
            Just('\n'),
            Just('\t'),
            proptest::char::range('0', '9'),
            proptest::char::range('a', 'z'),
            proptest::char::range('A', 'Z'),
        ];

        proptest::collection::vec(alphabet, 0..=200).prop_map(|chars| chars.into_iter().collect())
    }

    proptest! {
        /// Property 6 (design.md): el lint JSON reporta la posición
        /// exacta del error.
        ///
        /// Para cualquier cadena que no sea JSON válido, `lint_json()`
        /// SHALL reportar una línea y columna que coincidan EXACTAMENTE
        /// con `serde_json::Error::line()`/`column()` obtenidos al
        /// parsear esa misma cadena directamente (es decir, el wrapper no
        /// debe transformar ni desplazar esos valores de ninguna forma).
        ///
        /// **Validates: Requirements 2.14**
        #[test]
        fn lint_json_reports_exact_error_position(text in arb_maybe_invalid_json_string()) {
            // La property solo aplica a cadenas que NO son JSON válido;
            // descartamos las (pocas) cadenas generadas que sí lo son.
            let raw_parse_result = serde_json::from_str::<Value>(&text);
            prop_assume!(raw_parse_result.is_err());
            let raw_error = raw_parse_result.unwrap_err();

            let state = TextEditorState::new(&text);
            let lint_result = state.lint_json();

            prop_assert!(lint_result.is_some());
            let lint_error = lint_result.unwrap();

            prop_assert_eq!(lint_error.line, raw_error.line());
            prop_assert_eq!(lint_error.column, raw_error.column());
        }
    }

    /// Genera cadenas a partir de un alfabeto muy reducido (`a`, `b`, `c`)
    /// para el haystack y el needle de la búsqueda de texto (Tarea 3.17,
    /// Property 7). Un alfabeto tan pequeño hace mucho más probable que
    /// aparezcan coincidencias múltiples y/o solapadas en cada caso
    /// generado (p. ej. needle "aa" contra haystack "aaaa"), lo cual es
    /// justo el escenario que esta property necesita ejercitar. Además,
    /// al estar restringido a ASCII simple, `to_lowercase()` no puede
    /// desplazar byte-offsets (cada char ASCII se lowercasea a sí mismo o
    /// a otro char ASCII de igual longitud en UTF-8), por lo que el
    /// oráculo de referencia puede usar `to_lowercase()` + comparación
    /// directa sin caer en el problema documentado en
    /// `find_all_case_insensitive` para Unicode general.
    fn arb_search_alphabet_string(size_range: std::ops::Range<usize>) -> impl Strategy<Value = String> {
        let alphabet = prop_oneof![Just('a'), Just('b'), Just('c'), Just('A'), Just('B'), Just('C')];
        proptest::collection::vec(alphabet, size_range).prop_map(|chars| chars.into_iter().collect())
    }

    /// Oráculo de referencia, deliberadamente simple e independiente de
    /// `find_all_case_insensitive`: escaneo naive O(n*m) por posición de
    /// byte, comparando `haystack.to_lowercase()` contra
    /// `needle.to_lowercase()`. Como el alfabeto de entrada está
    /// restringido a `[a-cA-C]`, lowercasear no cambia ninguna longitud en
    /// bytes, así que los offsets de `to_lowercase()` coinciden
    /// exactamente con los del `haystack` original.
    fn reference_find_all_case_insensitive(haystack: &str, needle: &str) -> Vec<(usize, usize)> {
        let haystack_lower = haystack.to_lowercase();
        let needle_lower = needle.to_lowercase();

        let haystack_bytes = haystack_lower.as_bytes();
        let needle_bytes = needle_lower.as_bytes();

        let mut matches = Vec::new();

        if needle_bytes.is_empty() || haystack_bytes.len() < needle_bytes.len() {
            return matches;
        }

        for start in 0..=(haystack_bytes.len() - needle_bytes.len()) {
            if &haystack_bytes[start..start + needle_bytes.len()] == needle_bytes {
                matches.push((start, start + needle_bytes.len()));
            }
        }

        matches
    }

    proptest! {
        /// Property 7 (design.md): la búsqueda de texto encuentra todas
        /// las coincidencias sin omitir ni duplicar, y la navegación
        /// secuencial visita cada coincidencia exactamente una vez por
        /// ciclo completo, sin omitir ni repetir ninguna fuera de orden.
        ///
        /// **Validates: Requirements 2.15**
        #[test]
        fn text_search_finds_all_matches_without_omission_or_duplication(
            haystack in arb_search_alphabet_string(0..30),
            needle in arb_search_alphabet_string(1..3),
        ) {
            let expected_matches = reference_find_all_case_insensitive(&haystack, &needle);

            let mut state = TextEditorState::new(&haystack);
            state.set_search_query(&needle);

            // Parte a) Completitud/no-omisión/no-duplicación del conjunto
            // de coincidencias: el conjunto (y orden) calculado por el
            // componente debe coincidir EXACTAMENTE con el oráculo de
            // referencia, calculado de forma independiente.
            prop_assert_eq!(state.search.matches.clone(), expected_matches.clone());

            if expected_matches.is_empty() {
                // Caso borde: sin coincidencias, no debe haber índice
                // actual, y next_match/previous_match deben ser no-ops
                // seguros (no deben entrar en pánico).
                prop_assert_eq!(state.search.current_match_index, None);
                prop_assert_eq!(state.current_match(), None);

                state.next_match();
                prop_assert_eq!(state.search.current_match_index, None);
                prop_assert_eq!(state.current_match(), None);

                state.previous_match();
                prop_assert_eq!(state.search.current_match_index, None);
                prop_assert_eq!(state.current_match(), None);
            } else {
                // set_search_query debe posicionar en la primera
                // coincidencia.
                prop_assert_eq!(state.search.current_match_index, Some(0));

                let match_count = expected_matches.len();

                // Parte b) Navegación secuencial hacia adelante: capturar
                // la posición inicial y luego cada una de las
                // `match_count` llamadas a next_match(). La secuencia
                // completa (inicial + match_count avances) debe visitar
                // cada coincidencia en orden 0, 1, ..., len-1 y volver a
                // la posición inicial (wraparound), sin saltos ni
                // repeticiones dentro de la pasada no-cíclica de longitud
                // `match_count`.
                let mut forward_sequence = vec![state.current_match()];
                for _ in 0..match_count {
                    state.next_match();
                    forward_sequence.push(state.current_match());
                }

                // Los primeros `match_count` elementos (posiciones 0..len)
                // deben coincidir exactamente, en orden, con
                // `expected_matches`.
                for (index, expected_match) in expected_matches.iter().enumerate() {
                    prop_assert_eq!(forward_sequence[index], Some(*expected_match));
                }
                // Tras exactamente `match_count` llamadas a next_match(),
                // se debe volver a la coincidencia inicial (wraparound de
                // un ciclo completo).
                prop_assert_eq!(forward_sequence[match_count], forward_sequence[0]);

                // Reposicionar en la primera coincidencia antes de probar
                // la navegación hacia atrás, para partir del mismo punto
                // de referencia.
                state.set_search_query(&needle);
                prop_assert_eq!(state.search.current_match_index, Some(0));

                // Parte b, en reversa: previous_match() desde la primera
                // coincidencia debe recorrer las coincidencias en orden
                // inverso (len-1, len-2, ..., 0) y, tras `match_count`
                // llamadas, volver a la posición inicial. El elemento
                // `backward_sequence[0]` es la posición INICIAL (antes de
                // cualquier llamada a previous_match()); los resultados
                // de las llamadas a previous_match() ocupan
                // `backward_sequence[1..=match_count]`.
                let mut backward_sequence = vec![state.current_match()];
                for _ in 0..match_count {
                    state.previous_match();
                    backward_sequence.push(state.current_match());
                }

                for (step, expected_match) in expected_matches.iter().rev().enumerate() {
                    prop_assert_eq!(backward_sequence[step + 1], Some(*expected_match));
                }
                prop_assert_eq!(backward_sequence[match_count], backward_sequence[0]);
            }
        }
    }
}
