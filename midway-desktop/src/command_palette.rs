//! `command_palette.rs` (Tarea 11.1, Fase 5): port directo del algoritmo de
//! scoring y búsqueda de `src/lib/commandPalette.ts`.
//!
//! Preserva exactamente la normalización, la escalera de pesos (100/80/72/
//! 60/48/36/28/24/-1) y el orden de evaluación de la referencia
//! TypeScript (una cadena if/else-if: la primera condición que aplica
//! determina el score, no la de mayor peso posible entre todas las que
//! calcen). También preserva:
//! - Query vacía (tras `trim`): devuelve los primeros `limit` ítems en su
//!   orden original, sin puntuar ni reordenar (`items.slice(0, limit)`).
//! - Orden de resultados: score descendente y, en caso de empate, título
//!   ascendente (equivalente a `localeCompare` para el rango de entradas
//!   típico de esta app).
//!
//! Este módulo implementa solo el algoritmo (Tarea 11.1); la apertura/
//! cierre del `Command_Palette` y la ejecución de ítems seleccionados se
//! implementan en la Tarea 11.2 sobre `app.rs`.
//!
//! Ver diseño: "Components and Interfaces > Command Palette, Session_Store
//! y shortcuts (Fase 5)".
//! Ver requisitos: 6.1.

use serde::{Deserialize, Serialize};

/// Ítem indexable por el Command Palette: acciones fijas, colecciones o
/// requests guardados (Data Models de la referencia TypeScript
/// `CommandPaletteItem`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPaletteItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub keywords: Vec<String>,
    pub section: String,
}

/// Normaliza texto para comparación: minúsculas + `trim`, igual que
/// `normalize` en la referencia TypeScript (`text.toLowerCase().trim()`).
fn normalize(text: &str) -> String {
    text.to_lowercase().trim().to_string()
}

/// Calcula el score de `item` para una `query` ya normalizada
/// (`normalize`d), replicando exactamente la cadena if/else-if de
/// `scoreItem` en la referencia TypeScript:
///
/// 1. Título coincide exactamente con la query -> 100
/// 2. Título empieza con la query -> 80
/// 3. Alguna keyword coincide exactamente -> 72
/// 4. Título contiene la query -> 60
/// 5. Alguna keyword empieza con la query -> 48
/// 6. Subtítulo contiene la query -> 36
/// 7. Alguna keyword contiene la query -> 28
/// 8. Algún token del título (separado por espacios) empieza con la query
///    -> 24
/// 9. Ninguna condición anterior aplica -> -1 (excluido de los resultados)
///
/// El orden importa: es la primera condición que aplica la que determina
/// el score, no la de mayor peso entre todas las que calcen (idéntico al
/// `if`/`else if` original, no un `max` sobre todas las reglas).
///
/// Nota de fidelidad con la referencia: la regla 8 es, en la práctica,
/// inalcanzable tanto en `scoreItem` (TypeScript) como en este port. Si
/// algún token del título empieza con la query, entonces la query aparece
/// como substring del título en la posición donde empieza ese token, por
/// lo que la regla 4 (`title.contains(q)`, peso 60) siempre se evalúa
/// antes y siempre calza primero. Se conserva la regla 8 igualmente para
/// preservar la estructura exacta de la referencia ("port directo").
fn score_item(normalized_query: &str, item: &CommandPaletteItem) -> i32 {
    if normalized_query.is_empty() {
        return 0;
    }

    let title = normalize(&item.title);
    let subtitle = normalize(item.subtitle.as_deref().unwrap_or(""));
    let keywords: Vec<String> = item.keywords.iter().map(|keyword| normalize(keyword)).collect();

    if title == normalized_query {
        return 100;
    }
    if title.starts_with(normalized_query) {
        return 80;
    }
    if keywords.iter().any(|keyword| keyword == normalized_query) {
        return 72;
    }
    if title.contains(normalized_query) {
        return 60;
    }
    if keywords.iter().any(|keyword| keyword.starts_with(normalized_query)) {
        return 48;
    }
    if subtitle.contains(normalized_query) {
        return 36;
    }
    if keywords.iter().any(|keyword| keyword.contains(normalized_query)) {
        return 28;
    }

    let title_tokens = title.split_whitespace();
    if title_tokens.into_iter().any(|token| token.starts_with(normalized_query)) {
        return 24;
    }

    -1
}

/// Busca y ordena `items` según su relevancia respecto a `query`, igual que
/// `searchPaletteItems` en la referencia TypeScript.
///
/// - Si `query` (normalizada) es vacía, devuelve los primeros `limit`
///   ítems en su orden original, sin puntuar ni reordenar.
/// - En otro caso, puntúa cada ítem con [`score_item`], descarta los que
///   obtienen score negativo, ordena por score descendente (y por título
///   ascendente en caso de empate) y trunca a `limit`.
pub fn search_palette_items(items: &[CommandPaletteItem], query: &str, limit: usize) -> Vec<CommandPaletteItem> {
    let normalized_query = normalize(query);

    if normalized_query.is_empty() {
        return items.iter().take(limit).cloned().collect();
    }

    let mut scored: Vec<(i32, &CommandPaletteItem)> = items
        .iter()
        .map(|item| (score_item(&normalized_query, item), item))
        .filter(|(score, _)| *score >= 0)
        .collect();

    scored.sort_by(|(left_score, left_item), (right_score, right_item)| {
        right_score.cmp(left_score).then_with(|| left_item.title.cmp(&right_item.title))
    });

    scored.into_iter().take(limit).map(|(_, item)| item.clone()).collect()
}

#[cfg(test)]
mod tests {
    //! Tests unitarios de `command_palette.rs` (Tarea 11.1).
    //!
    //! Cubren la escalera de pesos (una prueba por nivel, verificando que
    //! gana el primero que aplica en la cadena if/else-if, no el de mayor
    //! peso entre todos los que calzarían), el passthrough de query vacía,
    //! la truncación por `limit` y el tie-break alfabético por título.
    //!
    //! No se asigna una Property numerada del diseño a la Tarea 11.1; se
    //! usan tests unitarios simples, suficientes para fijar la precedencia
    //! exacta del if/else-if.

    use super::*;

    fn item(id: &str, title: &str, subtitle: Option<&str>, keywords: &[&str]) -> CommandPaletteItem {
        CommandPaletteItem {
            id: id.to_string(),
            title: title.to_string(),
            subtitle: subtitle.map(|s| s.to_string()),
            keywords: keywords.iter().map(|k| k.to_string()).collect(),
            section: "test".to_string(),
        }
    }

    #[test]
    fn score_exact_title_match_wins_over_prefix() {
        // El título "new request" coincide exactamente con la query, y
        // también sería prefijo/substring de sí mismo: debe ganar la regla
        // de coincidencia exacta (100), no la de prefijo (80).
        let candidate = item("1", "new request", None, &[]);
        assert_eq!(score_item("new request", &candidate), 100);
    }

    #[test]
    fn score_title_prefix_wins_over_keyword_exact() {
        // El título empieza con la query ("new" -> "new request") y además
        // hay una keyword que coincide exactamente ("new"): debe ganar el
        // prefijo de título (80), evaluado antes que la keyword exacta (72).
        let candidate = item("1", "new request", None, &["new"]);
        assert_eq!(score_item("new", &candidate), 80);
    }

    #[test]
    fn score_keyword_exact_wins_over_title_substring() {
        // El título no empieza con la query pero la contiene
        // ("http request" contiene "request"), y una keyword coincide
        // exactamente con la query: debe ganar la keyword exacta (72),
        // evaluada antes que el substring de título (60).
        let candidate = item("1", "http request", None, &["request"]);
        assert_eq!(score_item("request", &candidate), 72);
    }

    #[test]
    fn score_title_substring_wins_over_keyword_prefix() {
        // El título contiene la query ("http request" contiene "reques"),
        // y una keyword empieza con la query: debe ganar el substring de
        // título (60), evaluado antes que el prefijo de keyword (48).
        let candidate = item("1", "http request", None, &["requesting"]);
        assert_eq!(score_item("reques", &candidate), 60);
    }

    #[test]
    fn score_keyword_prefix_wins_over_subtitle_substring() {
        // El título no contiene la query, una keyword empieza con la
        // query, y el subtítulo también la contiene: debe ganar el
        // prefijo de keyword (48), evaluado antes que el substring de
        // subtítulo (36).
        let candidate = item("1", "collection", Some("saved requests"), &["requesting"]);
        assert_eq!(score_item("reques", &candidate), 48);
    }

    #[test]
    fn score_subtitle_substring_wins_over_keyword_substring() {
        // Ni el título ni ninguna keyword empiezan con la query, pero el
        // subtítulo la contiene y también una keyword la contiene: debe
        // ganar el substring de subtítulo (36), evaluado antes que el
        // substring de keyword (28).
        let candidate = item("1", "collection", Some("saved requests"), &["outrequest"]);
        assert_eq!(score_item("reques", &candidate), 36);
    }

    #[test]
    fn score_keyword_substring_wins_over_title_token_prefix() {
        // Ni título, ni prefijo de keyword, ni subtítulo calzan; una
        // keyword contiene la query como substring y, a la vez, ningún
        // token del título empieza con la query: debe ganar el substring
        // de keyword (28).
        let candidate = item("1", "collection panel", None, &["outrequest"]);
        assert_eq!(score_item("reques", &candidate), 28);
    }

    #[test]
    fn score_title_token_prefix_rule_is_unreachable_since_title_substring_always_wins() {
        // La regla de "token de título empieza con la query" (24) es
        // inalcanzable en la práctica: si un token del título empieza con
        // la query, el título necesariamente CONTIENE la query como
        // substring en ese punto, por lo que la regla de substring de
        // título (60) siempre se evalúa antes y siempre calza primero.
        // Este test fija ese comportamiento (idéntico a la referencia
        // TypeScript), en lugar de asumir que la regla 8 es alcanzable.
        let candidate = item("1", "collection requester", None, &[]);
        assert_eq!(score_item("reques", &candidate), 60);
    }

    #[test]
    fn score_no_match_returns_negative_one() {
        let candidate = item("1", "collection panel", Some("workspace"), &["environments"]);
        assert_eq!(score_item("zzz", &candidate), -1);
    }

    #[test]
    fn score_is_case_and_whitespace_insensitive_via_normalization() {
        let candidate = item("1", "New Request", None, &[]);
        assert_eq!(score_item(&normalize("  NEW REQUEST  "), &candidate), 100);
    }

    #[test]
    fn empty_query_returns_items_unscored_in_original_order_truncated_to_limit() {
        let items = vec![
            item("1", "zebra", None, &[]),
            item("2", "alpha", None, &[]),
            item("3", "mango", None, &[]),
        ];

        let result = search_palette_items(&items, "", 2);

        // Sin puntuar ni reordenar: se conservan los primeros `limit` en
        // el orden original (no alfabético).
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "1");
        assert_eq!(result[1].id, "2");
    }

    #[test]
    fn whitespace_only_query_is_treated_as_empty() {
        let items = vec![item("1", "zebra", None, &[]), item("2", "alpha", None, &[])];

        let result = search_palette_items(&items, "   ", 10);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "1");
        assert_eq!(result[1].id, "2");
    }

    #[test]
    fn search_truncates_to_limit_after_sorting() {
        let items = vec![
            item("1", "request one", None, &[]),
            item("2", "request two", None, &[]),
            item("3", "request three", None, &[]),
        ];

        let result = search_palette_items(&items, "request", 2);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn search_sorts_by_score_descending_then_title_ascending_on_tie() {
        // "request b" y "request a" empatan en score (prefijo == 80 para
        // ambos), por lo que el tie-break alfabético por título debe
        // decidir el orden ("request a" antes de "request b").
        let items = vec![
            item("1", "request b", None, &[]),
            item("2", "request a", None, &[]),
            item("3", "unrelated", None, &[]),
        ];

        let result = search_palette_items(&items, "request", 10);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title, "request a");
        assert_eq!(result[1].title, "request b");
    }

    #[test]
    fn search_excludes_items_with_negative_score() {
        let items = vec![item("1", "matching request", None, &[]), item("2", "no relation", None, &[])];

        let result = search_palette_items(&items, "request", 10);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "1");
    }
}
