//! `Curl_Importer`: port directo de `src/lib/curl.ts` (tokenizador de comandos
//! cURL + extracción de campos de un `RequestDraft`).
//!
//! Ver Design > Components and Interfaces > Curl_Importer y Requisitos 2.6 / 2.16.
//!
//! Este módulo expone únicamente el tokenizador y el parser (`RequestDraft` a
//! partir de un string de cURL). La lógica de "a dónde pegar" (sobrescribir la
//! tab activa vs. crear una nueva) se implementa en otra tarea (3.5) y no vive
//! aquí.

use midway_core::domain::http::{
    AuthConfig, BodyMode, FormDataFieldKind, FormDataRow, HttpMethod, KeyValueRow,
    RequestBodyDraft, RequestDraft,
};
use uuid::Uuid;

const METHODS: [HttpMethod; 7] = [
    HttpMethod::GET,
    HttpMethod::POST,
    HttpMethod::PUT,
    HttpMethod::PATCH,
    HttpMethod::DELETE,
    HttpMethod::HEAD,
    HttpMethod::OPTIONS,
];

fn create_row(key: &str, value: &str, enabled: bool) -> KeyValueRow {
    KeyValueRow {
        id: Uuid::new_v4().to_string(),
        key: key.to_string(),
        value: value.to_string(),
        enabled,
    }
}

fn create_form_data_row(
    key: &str,
    value: &str,
    kind: FormDataFieldKind,
    enabled: bool,
    file_name: Option<String>,
) -> FormDataRow {
    FormDataRow {
        id: Uuid::new_v4().to_string(),
        key: key.to_string(),
        value: value.to_string(),
        enabled,
        kind,
        file_name,
    }
}

/// Equivalente a `createBlankDraft()` en `src/lib/curl.ts`.
///
/// `pub(crate)` porque también se usa desde `app.rs` (Tarea 3.2) para crear
/// la primera tab de request en blanco al arrancar la aplicación.
pub(crate) fn create_blank_draft() -> RequestDraft {
    RequestDraft {
        id: None,
        name: "Nueva petición".to_string(),
        method: HttpMethod::GET,
        url: String::new(),
        query: vec![],
        headers: vec![],
        auth: AuthConfig::None,
        body: RequestBodyDraft {
            mode: BodyMode::None,
            value: String::new(),
            form_data: vec![],
        },
        timeout_ms: 30_000,
        environment_id: None,
        response_tests: vec![],
    }
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Decodificación de porcentaje equivalente a `decodeURIComponent`: decodifica
/// secuencias `%XX` y valida que el resultado sea UTF-8 válido. Devuelve
/// `None` si alguna secuencia es inválida (equivalente a que
/// `decodeURIComponent` lance una excepción).
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = hex_val(bytes[index + 1])?;
            let low = hex_val(bytes[index + 2])?;
            out.push((high << 4) | low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Port de `decodeQueryValue`: reemplaza `+` por espacio y decodifica con
/// `decodeURIComponent`; si la decodificación falla, devuelve el valor
/// original SIN el reemplazo de `+` (igual que el `catch { return value; }`
/// de la referencia TypeScript).
fn decode_query_value(value: &str) -> String {
    let replaced: String = value.chars().map(|c| if c == '+' { ' ' } else { c }).collect();
    percent_decode(&replaced).unwrap_or_else(|| value.to_string())
}

/// Port de `parseQueryStringToRows`.
fn parse_query_string_to_rows(query_string: &str) -> Vec<KeyValueRow> {
    query_string
        .split('&')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| match part.find('=') {
            None => create_row(&decode_query_value(part), "", true),
            Some(separator_index) => create_row(
                &decode_query_value(&part[..separator_index]),
                &decode_query_value(&part[separator_index + 1..]),
                true,
            ),
        })
        .collect()
}

struct SplitUrl {
    url: String,
    query_rows: Vec<KeyValueRow>,
}

/// Port de `splitUrlAndQuery`.
fn split_url_and_query(raw_url: &str) -> SplitUrl {
    let question_mark_index = match raw_url.find('?') {
        None => {
            return SplitUrl {
                url: raw_url.to_string(),
                query_rows: vec![],
            };
        }
        Some(idx) => idx,
    };
    let base = &raw_url[..question_mark_index];
    let query_and_hash = &raw_url[question_mark_index + 1..];
    match query_and_hash.find('#') {
        None => SplitUrl {
            url: base.to_string(),
            query_rows: parse_query_string_to_rows(query_and_hash),
        },
        Some(hash_index) => {
            let query = &query_and_hash[..hash_index];
            let hash = &query_and_hash[hash_index..];
            SplitUrl {
                url: format!("{}{}", base, hash),
                query_rows: parse_query_string_to_rows(query),
            }
        }
    }
}

fn method_to_str(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::GET => "GET",
        HttpMethod::POST => "POST",
        HttpMethod::PUT => "PUT",
        HttpMethod::PATCH => "PATCH",
        HttpMethod::DELETE => "DELETE",
        HttpMethod::HEAD => "HEAD",
        HttpMethod::OPTIONS => "OPTIONS",
    }
}

/// Port de `buildRequestNameFromUrl`.
fn build_request_name_from_url(method: HttpMethod, raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        return "Nueva petición".to_string();
    }
    let without_query = trimmed.split('?').next().unwrap_or(trimmed);
    let segments: Vec<&str> = without_query.split('/').filter(|segment| !segment.is_empty()).collect();
    let last_segment = segments.last().copied().unwrap_or("petición");
    format!("{} {}", method_to_str(method), last_segment)
}

/// Port de `looksLikeJsonText`.
fn looks_like_json_text(value: &str) -> bool {
    let trimmed = value.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

/// Port de `prettifyJsonText`: en caso de error de parseo, devuelve el valor
/// original sin modificar (igual que el `catch` de la referencia).
fn prettify_json_text(value: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(parsed) => serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| value.to_string()),
        Err(_) => value.to_string(),
    }
}

fn is_ascii_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Port de `looksLikeCurlCommand` (equivalente a `/^curl(?:\.exe)?\b/i`).
pub fn looks_like_curl_command(value: &str) -> bool {
    let trimmed = value.trim();
    let lower = trimmed.to_lowercase();
    if !lower.starts_with("curl") {
        return false;
    }
    let after_curl = &lower[4..];
    // Intento 1 (con backtracking del regex): consumir ".exe" y verificar el
    // límite de palabra después de eso.
    if let Some(after_exe) = after_curl.strip_prefix(".exe") {
        let boundary_ok = match after_exe.chars().next() {
            None => true,
            Some(c) => !is_ascii_word_char(c),
        };
        if boundary_ok {
            return true;
        }
    }
    // Intento 2: no consumir ".exe", verificar el límite de palabra
    // inmediatamente después de "curl".
    match after_curl.chars().next() {
        None => true,
        Some(c) => !is_ascii_word_char(c),
    }
}

/// Equivalente a `/^curl(?:\.exe)?$/i` (coincidencia exacta del primer token).
fn is_curl_token(token: &str) -> bool {
    let lower = token.to_lowercase();
    lower == "curl" || lower == "curl.exe"
}

/// Port de `tokenizeCurlCommand`.
pub fn tokenize_curl_command(command: &str) -> Vec<String> {
    let normalized = normalize_line_continuations(command);

    #[derive(PartialEq, Clone, Copy)]
    enum Quote {
        None,
        Single,
        Double,
    }

    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote = Quote::None;
    let mut escaping = false;

    for character in normalized.chars() {
        if escaping {
            current.push(character);
            escaping = false;
            continue;
        }
        match quote {
            Quote::Single => {
                if character == '\'' {
                    quote = Quote::None;
                } else {
                    current.push(character);
                }
                continue;
            }
            Quote::Double => {
                if character == '"' {
                    quote = Quote::None;
                } else if character == '\\' {
                    escaping = true;
                } else {
                    current.push(character);
                }
                continue;
            }
            Quote::None => {}
        }
        if character == '\\' {
            escaping = true;
            continue;
        }
        if character == '\'' {
            quote = Quote::Single;
            continue;
        }
        if character == '"' {
            quote = Quote::Double;
            continue;
        }
        if character.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            continue;
        }
        current.push(character);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Normaliza continuaciones de línea (`\` + `\r`? + `\n`) a un espacio, y
/// cualquier `\r` restante también a espacio, luego recorta los extremos.
/// Equivalente a:
/// `command.replace(/\\\r?\n/g, " ").replace(/\r/g, " ").trim()`.
fn normalize_line_continuations(command: &str) -> String {
    let chars: Vec<char> = command.chars().collect();
    let mut result = String::with_capacity(chars.len());
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '\\' {
            let mut lookahead = index + 1;
            if lookahead < chars.len() && chars[lookahead] == '\r' {
                lookahead += 1;
            }
            if lookahead < chars.len() && chars[lookahead] == '\n' {
                result.push(' ');
                index = lookahead + 1;
                continue;
            }
        }
        if chars[index] == '\r' {
            result.push(' ');
            index += 1;
            continue;
        }
        result.push(chars[index]);
        index += 1;
    }
    result.trim().to_string()
}

/// Port de `normalizeHttpMethodFromCurl`.
fn normalize_http_method_from_curl(value: &str, warnings: &mut Vec<String>) -> HttpMethod {
    let normalized = value.trim().to_uppercase();
    for candidate in METHODS {
        if method_to_str(candidate) == normalized {
            return candidate;
        }
    }
    warnings.push(format!("Método no soportado: {}. Se usó GET.", value));
    HttpMethod::GET
}

/// Port de `parseCurlHeader`.
fn parse_curl_header(header_line: &str) -> Option<(String, String)> {
    header_line.find(':').map(|separator_index| {
        (
            header_line[..separator_index].trim().to_string(),
            header_line[separator_index + 1..].trim().to_string(),
        )
    })
}

/// Port de `parseCurlBasicAuth`.
fn parse_curl_basic_auth(raw_value: &str) -> AuthConfig {
    match raw_value.find(':') {
        None => AuthConfig::Basic {
            username: raw_value.to_string(),
            password: String::new(),
        },
        Some(separator_index) => AuthConfig::Basic {
            username: raw_value[..separator_index].to_string(),
            password: raw_value[separator_index + 1..].to_string(),
        },
    }
}

/// Port de `parseCurlFormToken`.
fn parse_curl_form_token(token: &str, as_string: bool) -> Option<FormDataRow> {
    let separator_index = token.find('=')?;
    let key = token[..separator_index].trim().to_string();
    let raw_value = &token[separator_index + 1..];
    if key.is_empty() {
        return None;
    }
    if !as_string {
        if let Some(file_path) = raw_value.strip_prefix('@') {
            return Some(create_form_data_row(&key, file_path, FormDataFieldKind::File, true, None));
        }
    }
    Some(create_form_data_row(&key, raw_value, FormDataFieldKind::Text, true, None))
}

/// Equivalente a `value.match(/^Bearer\s+(.+)$/i)?.[1]`.
fn extract_bearer_token(value: &str) -> Option<String> {
    if value.len() < 6 {
        return None;
    }
    let prefix = &value[..6];
    if !prefix.eq_ignore_ascii_case("Bearer") {
        return None;
    }
    let rest: Vec<char> = value[6..].chars().collect();
    let mut whitespace_count = 0;
    while whitespace_count < rest.len() && rest[whitespace_count].is_whitespace() {
        whitespace_count += 1;
    }
    if whitespace_count == 0 || whitespace_count == rest.len() {
        return None;
    }
    Some(rest[whitespace_count..].iter().collect())
}

fn take_value(tokens: &[String], index: usize, label: &str) -> Result<String, String> {
    tokens
        .get(index + 1)
        .cloned()
        .ok_or_else(|| format!("Falta un valor para {}.", label))
}

/// Port de `parseCurlCommandToDraft`.
///
/// Devuelve el `RequestDraft` resultante junto con la lista de warnings
/// (métodos no soportados, headers ignorados, flags ignoradas), o un `Err`
/// con el mensaje de error cuando el comando no puede parsearse (falta la
/// URL, falta un valor requerido por un flag, o el comando no empieza con
/// `curl`).
pub fn parse_curl_command_to_draft(command: &str) -> Result<(RequestDraft, Vec<String>), String> {
    let tokens = tokenize_curl_command(command);
    if tokens.is_empty() || !is_curl_token(&tokens[0]) {
        return Err("Pegá un comando completo que empiece con \"curl\".".to_string());
    }

    let mut warnings: Vec<String> = Vec::new();
    let mut method: Option<HttpMethod> = None;
    let mut url = String::new();
    let mut force_query_string = false;
    let mut infer_json_body = false;
    let mut header_tokens: Vec<String> = Vec::new();
    let mut data_tokens: Vec<String> = Vec::new();
    let mut form_tokens: Vec<FormDataRow> = Vec::new();
    let mut auth = AuthConfig::None;

    let mut index = 1;
    while index < tokens.len() {
        let token = tokens[index].clone();

        if token == "-X" || token == "--request" {
            let value = take_value(&tokens, index, &token)?;
            method = Some(normalize_http_method_from_curl(&value, &mut warnings));
            index += 2;
            continue;
        }
        if let Some(rest) = token.strip_prefix("--request=") {
            method = Some(normalize_http_method_from_curl(rest, &mut warnings));
            index += 1;
            continue;
        }
        if token.starts_with("-X") && token.len() > 2 {
            method = Some(normalize_http_method_from_curl(&token[2..], &mut warnings));
            index += 1;
            continue;
        }
        if token == "--url" {
            url = take_value(&tokens, index, &token)?;
            index += 2;
            continue;
        }
        if let Some(rest) = token.strip_prefix("--url=") {
            url = rest.to_string();
            index += 1;
            continue;
        }
        if token == "-H" || token == "--header" {
            header_tokens.push(take_value(&tokens, index, &token)?);
            index += 2;
            continue;
        }
        if let Some(rest) = token.strip_prefix("--header=") {
            header_tokens.push(rest.to_string());
            index += 1;
            continue;
        }
        if token.starts_with("-H") && token.len() > 2 {
            header_tokens.push(token[2..].to_string());
            index += 1;
            continue;
        }
        if token == "-u" || token == "--user" {
            auth = parse_curl_basic_auth(&take_value(&tokens, index, &token)?);
            index += 2;
            continue;
        }
        if let Some(rest) = token.strip_prefix("--user=") {
            auth = parse_curl_basic_auth(rest);
            index += 1;
            continue;
        }
        if token.starts_with("-u") && token.len() > 2 {
            auth = parse_curl_basic_auth(&token[2..]);
            index += 1;
            continue;
        }
        if token == "-I" || token == "--head" {
            method = Some(HttpMethod::HEAD);
            index += 1;
            continue;
        }
        if token == "-G" || token == "--get" {
            method = Some(HttpMethod::GET);
            force_query_string = true;
            index += 1;
            continue;
        }
        if token == "--json" {
            let value = take_value(&tokens, index, &token)?;
            data_tokens.push(value);
            infer_json_body = true;
            if method.is_none() {
                method = Some(HttpMethod::POST);
            }
            index += 2;
            continue;
        }

        let inline_data_prefix = ["--data=", "--data-raw=", "--data-binary=", "--data-ascii=", "--data-urlencode="]
            .into_iter()
            .find(|prefix| token.starts_with(prefix));
        if let Some(prefix) = inline_data_prefix {
            data_tokens.push(token[prefix.len()..].to_string());
            if method.is_none() && !force_query_string {
                method = Some(HttpMethod::POST);
            }
            index += 1;
            continue;
        }

        let inline_form_prefix = ["--form=", "--form-string="]
            .into_iter()
            .find(|prefix| token.starts_with(prefix));
        if let Some(prefix) = inline_form_prefix {
            let as_string = prefix == "--form-string=";
            if let Some(form_token) = parse_curl_form_token(&token[prefix.len()..], as_string) {
                form_tokens.push(form_token);
            }
            if method.is_none() && !force_query_string {
                method = Some(HttpMethod::POST);
            }
            index += 1;
            continue;
        }

        if token == "-d"
            || token == "--data"
            || token == "--data-raw"
            || token == "--data-binary"
            || token == "--data-ascii"
            || token == "--data-urlencode"
        {
            let value = take_value(&tokens, index, &token)?;
            data_tokens.push(value);
            if method.is_none() && !force_query_string {
                method = Some(HttpMethod::POST);
            }
            index += 2;
            continue;
        }
        if token == "-F" || token == "--form" || token == "--form-string" {
            let raw_value = take_value(&tokens, index, &token)?;
            if let Some(form_token) = parse_curl_form_token(&raw_value, token == "--form-string") {
                form_tokens.push(form_token);
            }
            if method.is_none() && !force_query_string {
                method = Some(HttpMethod::POST);
            }
            index += 2;
            continue;
        }
        if matches!(
            token.as_str(),
            "--location"
                | "-L"
                | "--silent"
                | "-s"
                | "--compressed"
                | "--fail"
                | "--include"
                | "-i"
                | "--verbose"
                | "-v"
                | "--insecure"
                | "-k"
        ) {
            index += 1;
            continue;
        }
        if !token.starts_with('-') && url.is_empty() {
            url = token.clone();
            index += 1;
            continue;
        }
        if token.starts_with('-') {
            warnings.push(format!("Flag ignorada: {}", token));
        }
        index += 1;
    }

    if url.trim().is_empty() {
        return Err("No pude encontrar la URL dentro del cURL.".to_string());
    }

    let mut headers: Vec<KeyValueRow> = Vec::new();
    let mut has_json_content_type = false;
    for header_token in &header_tokens {
        match parse_curl_header(header_token) {
            None => {
                warnings.push(format!("Header ignorado: {}", header_token));
            }
            Some((key, value)) => {
                let lower_key = key.to_lowercase();
                if lower_key == "authorization" && matches!(auth, AuthConfig::None) {
                    if let Some(token_value) = extract_bearer_token(&value) {
                        auth = AuthConfig::Bearer { token: token_value };
                        continue;
                    }
                }
                if lower_key == "content-type" && value.to_lowercase().contains("application/json") {
                    has_json_content_type = true;
                }
                headers.push(create_row(&key, &value, true));
            }
        }
    }

    if infer_json_body {
        let has_content_type_header = headers.iter().any(|row| row.key.to_lowercase() == "content-type");
        let has_accept_header = headers.iter().any(|row| row.key.to_lowercase() == "accept");
        if !has_content_type_header {
            headers.push(create_row("content-type", "application/json", true));
            has_json_content_type = true;
        }
        if !has_accept_header {
            headers.push(create_row("accept", "application/json", true));
        }
    }

    let split_url = split_url_and_query(url.trim());
    let mut query_rows = split_url.query_rows;
    if force_query_string && !data_tokens.is_empty() {
        query_rows.extend(parse_query_string_to_rows(&data_tokens.join("&")));
    }

    let mut body_mode = BodyMode::None;
    let mut body_value = String::new();
    let mut form_data: Vec<FormDataRow> = Vec::new();

    if !force_query_string && !form_tokens.is_empty() {
        body_mode = BodyMode::FormData;
        form_data = form_tokens;
    } else if !force_query_string && !data_tokens.is_empty() {
        body_value = data_tokens.join("&");
        body_mode = if has_json_content_type || infer_json_body || looks_like_json_text(&body_value) {
            BodyMode::Json
        } else {
            BodyMode::Text
        };
        if matches!(body_mode, BodyMode::Json) {
            body_value = prettify_json_text(&body_value);
        }
    }

    let inferred_method = method.unwrap_or(if matches!(body_mode, BodyMode::None) {
        HttpMethod::GET
    } else {
        HttpMethod::POST
    });

    let mut draft = create_blank_draft();
    draft.name = build_request_name_from_url(inferred_method, &split_url.url);
    draft.method = inferred_method;
    draft.url = split_url.url;
    draft.query = query_rows;
    draft.headers = headers;
    draft.auth = auth;
    draft.body = RequestBodyDraft {
        mode: body_mode,
        value: body_value,
        form_data,
    };
    draft.response_tests = vec![];

    Ok((draft, warnings))
}

#[cfg(test)]
mod tests {
    //! Feature: tauri-to-iced-migration, Property 1: Equivalencia de parseo
    //! cURL con la referencia TypeScript.
    //!
    //! No hay runtime de TypeScript disponible en este workspace (solo Rust),
    //! por lo que el oráculo de la propiedad se computa analíticamente a
    //! partir de las mismas reglas documentadas en `src/lib/curl.ts` (que
    //! `parse_curl_command_to_draft` porta 1:1, ver Tarea 3.3): dado que los
    //! comandos de entrada se construyen programáticamente a partir de
    //! componentes generados (método, URL, query params, headers, body
    //! JSON), el valor esperado de cada campo del `RequestDraft` es
    //! derivable directamente de esos componentes sin ambigüedad.

    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    /// Alfabeto seguro para keys/values de query params y headers: evita
    /// caracteres que tokenize_curl_command o el propio formato de query
    /// string interpretarían de forma especial (espacios, `&`, `=`, `?`,
    /// `#`, comillas, `\`, `%`, `+`).
    fn simple_token() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9]{0,7}".prop_map(|s| s.to_string())
    }

    fn http_method_strategy() -> impl Strategy<Value = HttpMethod> {
        prop_oneof![
            Just(HttpMethod::GET),
            Just(HttpMethod::POST),
            Just(HttpMethod::PUT),
            Just(HttpMethod::PATCH),
            Just(HttpMethod::DELETE),
        ]
    }

    fn base_url_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("https://example.com/foo".to_string()),
            Just("https://api.example.org/v1/items".to_string()),
            Just("https://example.com/a/b/c".to_string()),
        ]
    }

    /// Genera entre 0 y 3 pares clave/valor únicos por clave (para que el
    /// oráculo de comparación de conjuntos sea directo, sin colisiones que
    /// dupliquen/sobreescriban una clave).
    fn kv_pairs_strategy(max: usize) -> impl Strategy<Value = Vec<(String, String)>> {
        prop::collection::vec((simple_token(), simple_token()), 0..=max).prop_map(|pairs| {
            let mut seen = std::collections::HashSet::new();
            pairs
                .into_iter()
                .filter(|(k, _)| seen.insert(k.clone()))
                .collect::<Vec<_>>()
        })
    }

    /// Un header generado: o bien un header "normal" (clave/valor simple), o
    /// bien un `Authorization: Bearer <token>` (para ejercer la regla de
    /// extracción de auth de la referencia TS).
    #[derive(Debug, Clone)]
    enum GeneratedHeader {
        Plain(String, String),
        AuthorizationBearer(String),
    }

    fn header_strategy() -> impl Strategy<Value = GeneratedHeader> {
        prop_oneof![
            (simple_token(), simple_token())
                .prop_map(|(k, v)| GeneratedHeader::Plain(k, v)),
            simple_token().prop_map(GeneratedHeader::AuthorizationBearer),
        ]
    }

    fn headers_strategy(max: usize) -> impl Strategy<Value = Vec<GeneratedHeader>> {
        prop::collection::vec(header_strategy(), 0..=max).prop_map(|headers| {
            // A lo sumo un Authorization header (el segundo, de haberlo, se
            // trata como header "normal" para no complicar el oráculo, ya
            // que curl real tampoco tiene sentido repitiendo -H Authorization
            // con Bearer dos veces).
            let mut seen_auth = false;
            let mut seen_keys = std::collections::HashSet::new();
            headers
                .into_iter()
                .filter_map(|header| match header {
                    GeneratedHeader::AuthorizationBearer(token) => {
                        if seen_auth {
                            None
                        } else {
                            seen_auth = true;
                            Some(GeneratedHeader::AuthorizationBearer(token))
                        }
                    }
                    GeneratedHeader::Plain(k, v) => {
                        if !seen_keys.insert(k.clone()) {
                            None
                        } else {
                            Some(GeneratedHeader::Plain(k, v))
                        }
                    }
                })
                .collect()
        })
    }

    /// Genera un cuerpo JSON simple (objeto plano de 0 a 3 pares clave/valor
    /// string) junto con su representación serializada.
    fn json_body_strategy() -> impl Strategy<Value = BTreeMap<String, String>> {
        prop::collection::vec((simple_token(), simple_token()), 0..=3).prop_map(|pairs| {
            pairs.into_iter().collect::<BTreeMap<_, _>>()
        })
    }

    fn kv_rows_to_map(rows: &[KeyValueRow]) -> BTreeMap<String, String> {
        rows.iter()
            .map(|row| (row.key.clone(), row.value.clone()))
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, ..ProptestConfig::default() })]

        #[test]
        fn property_1_curl_parse_equivalence_with_ts_reference(
            method in http_method_strategy(),
            base_url in base_url_strategy(),
            query_pairs in kv_pairs_strategy(3),
            headers in headers_strategy(3),
            json_body in proptest::option::of(json_body_strategy()),
        ) {
            // --- Construir el comando cURL a partir de los componentes ---
            let mut command = format!("curl -X {}", method_to_str(method));

            let mut url_with_query = base_url.clone();
            if !query_pairs.is_empty() {
                let query_string = query_pairs
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                url_with_query = format!("{}?{}", base_url, query_string);
            }
            command.push_str(&format!(" '{}'", url_with_query));

            let mut expected_bearer_token: Option<String> = None;
            let mut expected_headers: BTreeMap<String, String> = BTreeMap::new();

            for header in &headers {
                match header {
                    GeneratedHeader::Plain(k, v) => {
                        command.push_str(&format!(" -H '{}: {}'", k, v));
                        expected_headers.insert(k.clone(), v.clone());
                    }
                    GeneratedHeader::AuthorizationBearer(token) => {
                        command.push_str(&format!(" -H 'Authorization: Bearer {}'", token));
                        expected_bearer_token = Some(token.clone());
                    }
                }
            }

            let expected_json_value: Option<serde_json::Value> = json_body.as_ref().map(|map| {
                serde_json::to_value(map).expect("map de strings siempre serializa a JSON")
            });

            if let Some(map) = &json_body {
                let body_str = serde_json::to_string(map).expect("serialización de body JSON");
                // Comillas simples internas escapadas para no cerrar el
                // quoting de shell simulado por el tokenizer (el body no
                // contiene comillas porque las keys/values son alfanuméricos).
                command.push_str(&format!(" -d '{}'", body_str));
            }

            // --- Parsear con el port de Rust ---
            let (draft, _warnings) = parse_curl_command_to_draft(&command)
                .expect("comando construido a partir de componentes válidos debe parsear");

            // --- Método ---
            // Regla de la referencia TS: si hay -X explícito, se respeta
            // siempre (incluso con body presente, "-X" gana). Como este test
            // siempre genera -X explícitamente, el método esperado es
            // siempre el generado.
            prop_assert_eq!(draft.method, method);

            // --- URL (sin query string) ---
            prop_assert_eq!(&draft.url, &base_url);

            // --- Query params ---
            let expected_query: BTreeMap<String, String> = query_pairs.into_iter().collect();
            prop_assert_eq!(kv_rows_to_map(&draft.query), expected_query);
            prop_assert!(draft.query.iter().all(|row| row.enabled));

            // --- Headers y auth ---
            // Ningún header Authorization: Bearer debe haber quedado
            // duplicado en draft.headers.
            prop_assert!(!draft
                .headers
                .iter()
                .any(|row| row.key.to_lowercase() == "authorization"));
            prop_assert_eq!(kv_rows_to_map(&draft.headers), expected_headers);
            prop_assert!(draft.headers.iter().all(|row| row.enabled));

            match expected_bearer_token {
                Some(token) => {
                    prop_assert_eq!(draft.auth, AuthConfig::Bearer { token });
                }
                None => {
                    prop_assert_eq!(draft.auth, AuthConfig::None);
                }
            }

            // --- Body ---
            match expected_json_value {
                Some(expected_value) => {
                    prop_assert_eq!(draft.body.mode, BodyMode::Json);
                    let parsed_body: serde_json::Value = serde_json::from_str(&draft.body.value)
                        .expect("draft.body.value con mode Json debe ser JSON válido");
                    prop_assert_eq!(parsed_body, expected_value);
                }
                None => {
                    prop_assert_eq!(draft.body.mode, BodyMode::None);
                    prop_assert_eq!(&draft.body.value, "");
                }
            }
        }
    }
}
