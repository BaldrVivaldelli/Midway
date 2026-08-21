//! Guardia de importaciones de la vertical Tema/Ajustes.
//!
//! Tarea 6.3 — Requisitos: 8.5, 8.6.
//!
//! Este test es determinístico (de ejemplo, no de generación aleatoria): el
//! texto fuente de `src/ui/theme_settings.rs` es un dato fijo del repositorio,
//! así que basta con una única aserción que escanee ese archivo y falle si
//! aparece alguna de las referencias prohibidas.
//!
//! Qué protege:
//!
//! - Req 8.5: la vertical declara sus dependencias en su firma pública y no lee
//!   estado global, por lo que no puede nombrar el estado agregado de la
//!   aplicación ni el módulo de sesión.
//! - Req 8.6: la función de vista de la vertical no accede a disco, no consulta
//!   SQL y no llama al cliente HTTP. La forma más fuerte de garantizarlo es
//!   estructural: si el módulo no puede nombrar esas capas, no puede usarlas.
//!
//! Por qué se escanea el texto y no el grafo de dependencias (a diferencia de
//! `midway-core/tests/no_tauri_dependency.rs`, que usa `cargo metadata`): acá la
//! restricción es intra-crate. `midway-desktop` sí depende legítimamente del
//! cliente HTTP y de la capa de infraestructura; lo que debe seguir siendo
//! cierto es que **este módulo** no las alcanza. `cargo metadata` trabaja a
//! nivel de paquete y no puede expresar eso.
//!
//! El escaneo descarta comentarios antes de buscar. Motivo: la documentación del
//! módulo describe justamente estas restricciones, y una guardia puramente
//! textual sobre el archivo completo obligaría a la prosa a esquivar sus propios
//! términos para no autodelatarse (hoy los esquiva a propósito). Al eliminar
//! comentarios, la guardia mira solo código real y la documentación queda libre
//! de nombrar lo que documenta. Los literales de cadena **sí** se conservan en el
//! texto escaneado: una referencia prohibida dentro de una cadena también es
//! señal de que la vertical está tocando algo que no le corresponde.

use std::fs;
use std::path::PathBuf;

/// Ruta del archivo de la vertical, relativa al manifiesto de `midway-desktop`.
const VERTICAL_PATH: &str = "src/ui/theme_settings.rs";

/// Referencias prohibidas: `(patrón buscado, razón)`.
///
/// Los patrones de ruta se escriben sin espacios alrededor de `::` porque el
/// texto escaneado se normaliza con [`normalize_path_separators`] antes de la
/// búsqueda.
const FORBIDDEN_REFERENCES: &[(&str, &str)] = &[
    (
        "AppState",
        "la vertical no debe leer el estado agregado de la aplicación (Req 8.5)",
    ),
    (
        "crate::session",
        "la vertical no debe tocar el módulo de sesión ni su persistencia (Req 8.5, 8.6)",
    ),
    (
        "reqwest",
        "la vertical no debe llamar al cliente HTTP (Req 8.6)",
    ),
    (
        "midway_core::infra",
        "la vertical no debe alcanzar la capa de infraestructura: disco, SQL, keyring (Req 8.6)",
    ),
];

fn vertical_source_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(VERTICAL_PATH)
}

/// Elimina comentarios de línea y de bloque de un fuente Rust, preservando el
/// contenido de los literales (cadenas normales, cadenas crudas y caracteres)
/// para que un `//` o un `/*` dentro de una cadena no corte el escaneo.
///
/// No es un lexer completo de Rust y no pretende serlo: solo distingue los
/// contextos necesarios para no confundir comentarios con literales. Los
/// literales de carácter se reconocen con una heurística (`'x'` o `'\x'`) que
/// deja pasar los tiempos de vida (`'a`, `'static`) como texto normal, que es lo
/// correcto acá. Los tests `strip_comments_*` fijan este comportamiento.
fn strip_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();

        // Comentario de línea (incluye `///` y `//!`): se descarta hasta el fin
        // de línea, conservando el salto de línea para no pegar tokens.
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Comentario de bloque, con anidamiento (Rust lo permite).
        if c == '/' && next == Some('*') {
            let mut depth = 1usize;
            i += 2;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            // Un espacio en lugar del comentario, para no fusionar tokens.
            out.push(' ');
            continue;
        }

        // Cadena cruda: `r"..."`, `r#"..."#`, `r##"..."##`, ...
        if c == 'r' {
            let mut hashes = 0usize;
            while chars.get(i + 1 + hashes) == Some(&'#') {
                hashes += 1;
            }
            if chars.get(i + 1 + hashes) == Some(&'"') {
                let closing: String =
                    std::iter::once('"').chain(std::iter::repeat_n('#', hashes)).collect();
                out.push('r');
                for _ in 0..hashes {
                    out.push('#');
                }
                out.push('"');
                i += hashes + 2;
                let mut body = String::new();
                loop {
                    if i >= chars.len() {
                        break;
                    }
                    let rest: String = chars[i..].iter().collect();
                    if rest.starts_with(&closing) {
                        i += closing.chars().count();
                        break;
                    }
                    body.push(chars[i]);
                    i += 1;
                }
                out.push_str(&body);
                out.push_str(&closing);
                continue;
            }
        }

        // Cadena normal, con escapes.
        if c == '"' {
            out.push('"');
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    out.push(chars[i]);
                    if let Some(escaped) = chars.get(i + 1) {
                        out.push(*escaped);
                    }
                    i += 2;
                    continue;
                }
                out.push(chars[i]);
                let closed = chars[i] == '"';
                i += 1;
                if closed {
                    break;
                }
            }
            continue;
        }

        // Literal de carácter: `'x'` o `'\n'`. Un tiempo de vida como `'a` no
        // cumple la condición y cae al camino de texto normal.
        if c == '\'' && (next == Some('\\') || chars.get(i + 2) == Some(&'\'')) {
            out.push('\'');
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    out.push(chars[i]);
                    if let Some(escaped) = chars.get(i + 1) {
                        out.push(*escaped);
                    }
                    i += 2;
                    continue;
                }
                out.push(chars[i]);
                let closed = chars[i] == '\'';
                i += 1;
                if closed {
                    break;
                }
            }
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

/// Quita los espacios en blanco alrededor de `::` para que
/// `midway_core :: infra` (formato válido en Rust) no evada la guardia.
fn normalize_path_separators(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == ':' && chars.get(i + 1) == Some(&':') {
            while out.ends_with(char::is_whitespace) {
                out.pop();
            }
            out.push_str("::");
            i += 2;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    out
}

/// Devuelve el número de línea (1-indexado) de la primera aparición de
/// `needle`, para que el mensaje de fallo sea accionable.
fn first_line_containing(source: &str, needle: &str) -> Option<usize> {
    source
        .lines()
        .position(|line| normalize_path_separators(line).contains(needle))
        .map(|index| index + 1)
}

#[test]
fn la_vertical_no_referencia_dependencias_prohibidas() {
    let path = vertical_source_path();
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("no se pudo leer {}: {error}", path.display()));

    let scanned = normalize_path_separators(&strip_comments(&source));

    // Control positivo: si el escaneo se rompiera y devolviera algo vacío o
    // irreconocible, la ausencia de referencias prohibidas sería un falso
    // verde. Estas dos aserciones confirman que sí estamos mirando el código de
    // la vertical.
    assert!(
        scanned.contains("ThemeSettingsState"),
        "el texto escaneado de {} no contiene `ThemeSettingsState`; revisar el escaneo antes de confiar en la guardia",
        path.display()
    );
    assert!(
        scanned.contains("pub fn update"),
        "el texto escaneado de {} no contiene `pub fn update`; revisar el escaneo antes de confiar en la guardia",
        path.display()
    );

    let offending: Vec<String> = FORBIDDEN_REFERENCES
        .iter()
        .filter(|(pattern, _)| scanned.contains(*pattern))
        .map(|(pattern, reason)| match first_line_containing(&source, pattern) {
            Some(line) => format!("`{pattern}` (línea {line} aprox.): {reason}"),
            None => format!("`{pattern}`: {reason}"),
        })
        .collect();

    assert!(
        offending.is_empty(),
        "{} debe permanecer autocontenido, pero referencia: {offending:#?}",
        VERTICAL_PATH
    );
}

#[test]
fn strip_comments_elimina_comentarios_y_preserva_cadenas() {
    let sample = concat!(
        "//! doc de módulo con AppState\n",
        "/// doc de item con reqwest\n",
        "/* bloque /* anidado */ con crate::session */\n",
        "let marcador = \"cadena // sin comentario\";\n",
        "let crudo = r#\"crudo /* sin comentario */\"#;\n",
        "let barra = '/';\n",
        "fn control<'a>() {} // cola con midway_core::infra\n",
    );

    let stripped = strip_comments(sample);

    assert!(!stripped.contains("AppState"));
    assert!(!stripped.contains("reqwest"));
    assert!(!stripped.contains("crate::session"));
    assert!(!stripped.contains("midway_core::infra"));

    assert!(stripped.contains("cadena // sin comentario"));
    assert!(stripped.contains("crudo /* sin comentario */"));
    assert!(stripped.contains("let barra = '/';"));
    assert!(stripped.contains("fn control<'a>() {}"));
}

#[test]
fn normalize_path_separators_colapsa_espacios_alrededor_de_dos_puntos() {
    assert_eq!(
        normalize_path_separators("use midway_core :: infra :: secret_store;"),
        "use midway_core::infra::secret_store;"
    );
    assert_eq!(
        normalize_path_separators("crate\n    ::session"),
        "crate::session"
    );
    assert_eq!(normalize_path_separators("a: b"), "a: b");
}
