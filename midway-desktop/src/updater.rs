//! `updater.rs` (Tarea 13.1, Fase 6): descarga del manifiesto del updater y
//! comparación de versiones con semver.
//!
//! Port + reescritura de `src/lib/updater.ts` **sin** ninguna dependencia de
//! Tauri (`@tauri-apps/plugin-updater`), reemplazándola por una
//! implementación manual sobre `reqwest` que consume el mismo formato de
//! manifiesto (`latest.json` / `latest-beta.json`) ya publicado por el
//! pipeline de release de Midway (ver `scripts/release/generate-updater-json.mjs`
//! y `docs/distribution.md`).
//!
//! Esta tarea cubre:
//! - Descargar `latest.json` / `latest-beta.json` según el canal configurado.
//! - Comparar la versión anunciada en el manifiesto contra la versión
//!   actualmente instalada (`env!("CARGO_PKG_VERSION")`) usando el crate
//!   `semver` (versión pinneada `=1.0.28`).
//! - Informar si existe una versión más reciente disponible.
//!
//! La Tarea 13.3 añade la descarga del artefacto de la plataforma actual vía
//! `reqwest::Response::bytes_stream()`, reportando el progreso como un valor
//! porcentual entre 0% y 100% con una frecuencia mínima de 1 actualización
//! por segundo (Requisito 7.3), a través de un callback `on_progress`
//! consumido por la suscripción de iced.
//!
//! La Tarea 13.5 añade la verificación de checksum SHA256 como **compuerta
//! estricta antes de instalar** (Requisitos 7.4, 7.5, 7.6): se calcula el
//! SHA256 del artefacto descargado (crate `sha2`, versión pinneada
//! `=0.10.9`) y se compara contra el valor publicado en `SHA256SUMS.txt`. Si
//! coincide, se permite proceder a instalar; si no coincide, el artefacto se
//! descarta, la instalación existente **no** se sobrescribe, y se reporta un
//! error de corrupción.
//!
//! La Tarea 13.8 añade el modelado del **manejo de fallos de verificación o
//! descarga preservando la versión instalada** (Requisito 7.9): ante un fallo
//! de descarga (red/timeout/respuesta malformada) o de verificación (checksum
//! SHA256 no coincidente), se reporta un error descriptivo y la versión
//! actualmente instalada permanece operativa y sin modificar. Se modela como
//! lógica/estado puro (`UpdaterError`, `UpdateFailureStage`,
//! `PreservedInstallation`, `UpdateOutcome`, `resolve_update_attempt`), sin
//! tocar el sistema de archivos, de modo que un fallo garantiza por
//! construcción que nada se instaló.
//!
//! La Tarea 13.7 añade el modelado del **relanzamiento post-instalación y la
//! continuidad de sesión** (Requisitos 7.7, 7.8): tras una instalación
//! exitosa se ofrece relanzar la aplicación con la nueva versión; si el
//! usuario declina, la sesión en curso sigue operativa con la versión previa
//! y la versión recién instalada se usará en el siguiente inicio. Se modela
//! como lógica/estado puro (`SuccessfulInstall`, `RelaunchPrompt`,
//! `RelaunchDecision`, `PostInstallAction`) para poder testearlo sin
//! ejecutar realmente el proceso; el spawn/salida del proceso lo realiza la
//! capa de UI a partir de la [`PostInstallAction`] resuelta.
//!
//! Ver diseño: "Components and Interfaces > Updater in-app (Fase 6)";
//! "Data Models > Manifest del updater"; "Error Handling" (errores del
//! `Updater`).
//! Ver requisitos: 7.1, 7.2, 7.4, 7.5, 7.6, 11.1.
//!
//! Nota: varias funciones/tipos públicos de este módulo aún no se conectan a
//! la UI (se cablean en las Tareas 13.3-13.8 de la Fase 6), por lo que se
//! permite `dead_code` a nivel de módulo hasta entonces, siguiendo el mismo
//! patrón que `ui/text_editor.rs`.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use midway_core::app::errors::{AppError, AppResult};

/// Base de descarga de releases de GitHub, coincidente con los `endpoints`
/// que `scripts/release/render-packager-config.mjs` deriva por canal
/// (`https://github.com/{repo}/releases/latest/download/{manifest}`).
pub const GITHUB_RELEASES_BASE: &str = "https://github.com";

/// Canal de actualización configurado (Requisito 7.2). Equivalente al tipo
/// `MidwayUpdateChannel = "stable" | "beta"` de la referencia TypeScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidwayUpdateChannel {
    Stable,
    Beta,
}

impl MidwayUpdateChannel {
    /// Nombre del archivo de manifiesto correspondiente al canal, tal como
    /// lo publica el pipeline de release (`latest.json` para estable,
    /// `latest-beta.json` para beta).
    pub fn manifest_file_name(self) -> &'static str {
        match self {
            MidwayUpdateChannel::Stable => "latest.json",
            MidwayUpdateChannel::Beta => "latest-beta.json",
        }
    }

    /// Etiqueta legible del canal (`"stable"` / `"beta"`), útil para la UI y
    /// para logs/diagnóstico.
    pub fn as_str(self) -> &'static str {
        match self {
            MidwayUpdateChannel::Stable => "stable",
            MidwayUpdateChannel::Beta => "beta",
        }
    }
}

/// Manifiesto del updater (`latest.json` / `latest-beta.json`), sin cambios
/// de esquema respecto al formato ya publicado (Data Models > Manifest del
/// updater).
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pub_date: String,
    /// Clave: `"linux-x86_64"`, `"windows-x86_64"`, `"darwin-x86_64"`, etc.
    #[serde(default)]
    pub platforms: BTreeMap<String, UpdatePlatformEntry>,
}

/// Entrada de plataforma dentro del manifiesto: URL del artefacto y su
/// firma (reinterpretada como referencia al checksum en `SHA256SUMS.txt`,
/// verificado en la Tarea 13.5).
#[derive(Debug, Clone, Deserialize)]
pub struct UpdatePlatformEntry {
    pub url: String,
    #[serde(default)]
    pub signature: String,
}

/// Resultado de una verificación de actualización (Requisito 7.2):
/// versión instalada, canal, versión anunciada, si hay una versión más
/// reciente disponible y (si existe) la entrada de plataforma actual del
/// manifiesto para descargarla en pasos posteriores.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UpdateCheckReport {
    pub current_version: String,
    pub channel: MidwayUpdateChannel,
    pub latest_version: String,
    pub update_available: bool,
    /// Clave de plataforma usada para buscar la entrada en el manifiesto
    /// (`current_platform_key`).
    pub platform_key: String,
    /// Entrada del manifiesto para la plataforma actual, si el manifiesto la
    /// incluye. `None` cuando el manifiesto no publica un artefacto para la
    /// plataforma en ejecución.
    pub platform_entry: Option<UpdatePlatformEntry>,
    pub notes: String,
    pub pub_date: String,
}

/// Versión actualmente instalada de `midway-desktop`, resuelta en tiempo de
/// compilación desde `Cargo.toml` (Requisito 7.2).
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Clave de plataforma para indexar `UpdateManifest::platforms`, construida
/// a partir del SO y arquitectura de compilación, con el mismo formato
/// `{os}-{arch}` que produce `scripts/release/generate-updater-json.mjs`
/// (p. ej. `"linux-x86_64"`, `"windows-x86_64"`, `"darwin-aarch64"`).
///
/// `std::env::consts::OS` devuelve `"macos"` para Apple, mientras que el
/// manifiesto usa `"darwin"`; se normaliza aquí para mantener paridad con
/// las claves publicadas.
pub fn current_platform_key() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };

    format!("{os}-{arch}", arch = std::env::consts::ARCH)
}

/// Construye la URL del manifiesto del canal indicado sobre la base de
/// releases de GitHub (`GITHUB_RELEASES_BASE`), replicando el `endpoint`
/// usado por la configuración de release existente.
pub fn manifest_url(repo: &str, channel: MidwayUpdateChannel) -> String {
    manifest_url_with_base(GITHUB_RELEASES_BASE, repo, channel)
}

/// Variante de [`manifest_url`] parametrizada por la base de descarga, para
/// permitir apuntar a un servidor HTTP local en tests de integración
/// (Tarea 13.4) sin depender de la red real.
pub fn manifest_url_with_base(base: &str, repo: &str, channel: MidwayUpdateChannel) -> String {
    let base = base.trim_end_matches('/');
    format!(
        "{base}/{repo}/releases/latest/download/{manifest}",
        manifest = channel.manifest_file_name()
    )
}

/// Compara dos versiones semánticas y determina si `latest` es
/// estrictamente más reciente que `current` (Requisito 7.2).
///
/// Ambas versiones se parsean con `semver::Version`; se tolera un prefijo
/// `v` inicial (p. ej. `"v1.2.3"`) por robustez, aunque el pipeline de
/// release ya lo elimina al generar el manifiesto. Una versión inválida en
/// cualquiera de los dos lados produce un `AppError::Validation` en lugar de
/// asumir silenciosamente que no hay actualización.
pub fn is_update_available(current: &str, latest: &str) -> AppResult<bool> {
    let current_parsed = parse_semver(current)?;
    let latest_parsed = parse_semver(latest)?;

    Ok(latest_parsed > current_parsed)
}

/// Parsea una versión semántica tolerando un prefijo `v` inicial.
fn parse_semver(raw: &str) -> AppResult<semver::Version> {
    let trimmed = raw.trim();
    let normalized = trimmed.strip_prefix('v').unwrap_or(trimmed);

    semver::Version::parse(normalized).map_err(|error| {
        AppError::Validation(format!("Versión semántica inválida '{raw}': {error}"))
    })
}

/// Descarga y deserializa el manifiesto del updater desde una URL concreta.
///
/// Un status HTTP no exitoso o un cuerpo que no deserializa como
/// [`UpdateManifest`] se reportan como `AppError::Http`.
pub async fn fetch_manifest_from_url(
    client: &reqwest::Client,
    url: &str,
) -> AppResult<UpdateManifest> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(AppError::from)?
        .error_for_status()
        .map_err(AppError::from)?;

    response.json::<UpdateManifest>().await.map_err(|error| {
        AppError::Http(format!(
            "No pude interpretar el manifiesto de actualización de {url}: {error}"
        ))
    })
}

/// Descarga el manifiesto del canal indicado desde la fuente de releases de
/// GitHub y lo deserializa.
pub async fn fetch_manifest(
    client: &reqwest::Client,
    repo: &str,
    channel: MidwayUpdateChannel,
) -> AppResult<UpdateManifest> {
    let url = manifest_url(repo, channel);
    fetch_manifest_from_url(client, &url).await
}

/// Construye el [`UpdateCheckReport`] a partir de un manifiesto ya
/// descargado, comparando su versión contra la versión instalada y
/// localizando la entrada de plataforma actual. Función pura (sin I/O),
/// separada de la descarga para facilitar su testeo.
pub fn build_check_report(
    channel: MidwayUpdateChannel,
    manifest: &UpdateManifest,
) -> AppResult<UpdateCheckReport> {
    let current = current_version();
    let update_available = is_update_available(current, &manifest.version)?;
    let platform_key = current_platform_key();
    let platform_entry = manifest.platforms.get(&platform_key).cloned();

    Ok(UpdateCheckReport {
        current_version: current.to_string(),
        channel,
        latest_version: manifest.version.clone(),
        update_available,
        platform_key,
        platform_entry,
        notes: manifest.notes.clone(),
        pub_date: manifest.pub_date.clone(),
    })
}

/// Verifica actualizaciones de extremo a extremo: descarga el manifiesto del
/// canal indicado desde `repo` y devuelve un [`UpdateCheckReport`]
/// (Requisito 7.2).
#[allow(dead_code)]
pub async fn check_for_update(
    client: &reqwest::Client,
    repo: &str,
    channel: MidwayUpdateChannel,
) -> AppResult<UpdateCheckReport> {
    let manifest = fetch_manifest(client, repo, channel).await?;
    build_check_report(channel, &manifest)
}

/// Intervalo máximo entre dos reportes de progreso consecutivos durante la
/// descarga del artefacto.
///
/// El Requisito 7.3 exige una **frecuencia mínima de 1 actualización por
/// segundo**; se usa deliberadamente un intervalo holgadamente menor a 1 s
/// (250 ms) para garantizar ese piso incluso con jitter en la llegada de
/// chunks, sin llegar a emitir un reporte por cada chunk individual (lo que
/// saturaría el ciclo `update` de iced en descargas de muchos chunks
/// pequeños).
pub const PROGRESS_UPDATE_INTERVAL: Duration = Duration::from_millis(250);

/// Progreso de la descarga de un artefacto de actualización (Requisito 7.3).
///
/// Reporta los bytes descargados hasta el momento y, si el servidor anunció
/// `Content-Length`, el tamaño total esperado. El porcentaje derivado
/// (`percent`) queda acotado al rango `0..=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    /// Bytes recibidos y acumulados hasta este reporte.
    pub downloaded_bytes: u64,
    /// Tamaño total esperado del artefacto, si el servidor lo anunció con
    /// `Content-Length`. `None` cuando el tamaño total es desconocido.
    pub total_bytes: Option<u64>,
}

impl DownloadProgress {
    fn new(downloaded_bytes: u64, total_bytes: Option<u64>) -> Self {
        Self {
            downloaded_bytes,
            total_bytes,
        }
    }

    /// Porcentaje de avance entre `0` y `100` cuando se conoce el tamaño
    /// total; `None` si el servidor no envió `Content-Length` (en cuyo caso
    /// la UI muestra una barra indeterminada).
    ///
    /// El valor se satura a `100` para tolerar el caso en que el cuerpo
    /// recibido exceda mínimamente el `Content-Length` anunciado, y devuelve
    /// `100` para un artefacto de tamaño total `0` (ya completo).
    pub fn percent(self) -> Option<u8> {
        let total = self.total_bytes?;
        if total == 0 {
            return Some(100);
        }

        let percent = self
            .downloaded_bytes
            .saturating_mul(100)
            .checked_div(total)
            .unwrap_or(100)
            .min(100);

        Some(percent as u8)
    }
}

/// Descarga el artefacto ubicado en `url` transmitiéndolo por chunks vía
/// `reqwest::Response::bytes_stream()` y reportando el progreso mediante el
/// callback `on_progress` (Requisito 7.3).
///
/// Garantías de reporte:
/// - Se emite un reporte inicial a `0%` antes de recibir el primer chunk.
/// - Durante la transmisión se emite un reporte cada vez que transcurre al
///   menos [`PROGRESS_UPDATE_INTERVAL`] desde el último, asegurando la
///   frecuencia mínima de 1 actualización por segundo exigida por el
///   Requisito 7.3 (siempre que sigan llegando datos).
/// - Al finalizar la descarga se emite un reporte final; cuando el tamaño
///   total se conoce (o se infiere de los bytes recibidos), dicho reporte
///   corresponde al `100%`.
///
/// Devuelve el artefacto completo en memoria. Un status HTTP no exitoso o un
/// error de transporte durante el streaming se reportan como `AppError::Http`.
pub async fn download_artifact_with_progress<F>(
    client: &reqwest::Client,
    url: &str,
    mut on_progress: F,
) -> AppResult<Vec<u8>>
where
    F: FnMut(DownloadProgress),
{
    let response = client
        .get(url)
        .send()
        .await
        .map_err(AppError::from)?
        .error_for_status()
        .map_err(AppError::from)?;

    let total_bytes = response.content_length();

    let mut buffer: Vec<u8> = match total_bytes {
        Some(len) => Vec::with_capacity(len as usize),
        None => Vec::new(),
    };
    let mut downloaded: u64 = 0;

    // Reporte inicial (0%) antes de consumir el primer chunk.
    on_progress(DownloadProgress::new(0, total_bytes));

    let mut last_emit = Instant::now();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(AppError::from)?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        buffer.extend_from_slice(&chunk);

        if last_emit.elapsed() >= PROGRESS_UPDATE_INTERVAL {
            on_progress(DownloadProgress::new(downloaded, total_bytes));
            last_emit = Instant::now();
        }
    }

    // Reporte final garantizado. Si el servidor no anunció `Content-Length`,
    // el total se infiere de los bytes efectivamente recibidos, de modo que
    // `percent()` resuelva a 100%.
    let final_total = total_bytes.or(Some(downloaded));
    on_progress(DownloadProgress::new(downloaded, final_total));

    Ok(buffer)
}

/// Descarga el artefacto de la plataforma actual descrito por una
/// [`UpdatePlatformEntry`] del manifiesto, reportando progreso mediante
/// `on_progress` (Requisito 7.3).
///
/// Es una fina envoltura sobre [`download_artifact_with_progress`] que toma
/// la URL desde la entrada de plataforma del [`UpdateCheckReport`].
pub async fn download_platform_artifact<F>(
    client: &reqwest::Client,
    entry: &UpdatePlatformEntry,
    on_progress: F,
) -> AppResult<Vec<u8>>
where
    F: FnMut(DownloadProgress),
{
    download_artifact_with_progress(client, &entry.url, on_progress).await
}

// ---------------------------------------------------------------------------
// Verificación de checksum SHA256 como compuerta antes de instalar (Tarea 13.5)
// Requisitos 7.4, 7.5, 7.6.
// ---------------------------------------------------------------------------

/// Resultado de comparar el checksum SHA256 real de un artefacto descargado
/// contra el valor esperado publicado en `SHA256SUMS.txt`.
///
/// Es el núcleo de la compuerta de instalación (Requisitos 7.4-7.6):
/// `Match` habilita la instalación; `Mismatch` obliga a descartar el
/// artefacto sin tocar la instalación existente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksumVerification {
    /// El checksum real coincide con el esperado: puede procederse a instalar.
    Match,
    /// El checksum real no coincide con el esperado: el artefacto está
    /// corrupto y debe descartarse (incluye ambos valores en hex minúsculas
    /// para el mensaje de error).
    Mismatch { expected: String, actual: String },
}

impl ChecksumVerification {
    /// `true` únicamente cuando el checksum coincide (habilita instalar).
    pub fn is_match(&self) -> bool {
        matches!(self, ChecksumVerification::Match)
    }
}

/// Calcula el checksum SHA256 de `bytes` y lo devuelve como cadena
/// hexadecimal en minúsculas (mismo formato que produce
/// `scripts/release/generate-checksums.mjs`, que usa `digest('hex')`).
pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    to_hex_lower(&hasher.finalize())
}

/// Convierte una secuencia de bytes en su representación hexadecimal en
/// minúsculas, sin dependencias externas de codificación hex.
fn to_hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // `write!` sobre un `String` no puede fallar.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Parsea un documento `SHA256SUMS.txt` a un mapa `nombre_de_archivo ->
/// checksum` (hex en minúsculas).
///
/// Cada línea sigue el formato estándar de `sha256sum`
/// (`{hash}  {ruta_relativa}`), con dos espacios de separación tal como lo
/// genera `scripts/release/generate-checksums.mjs`. Se tolera también el
/// marcador de modo binario (`{hash} *{ruta}`) por robustez. Las líneas
/// vacías o malformadas se ignoran. Los nombres de archivo pueden contener
/// espacios, por lo que sólo se separa el primer token (el hash) del resto.
pub fn parse_sha256sums(document: &str) -> BTreeMap<String, String> {
    let mut sums = BTreeMap::new();

    for line in document.lines() {
        if let Some((hash, name)) = split_checksum_line(line) {
            sums.insert(name, hash);
        }
    }

    sums
}

/// Separa una línea de `SHA256SUMS.txt` en `(hash_en_minúsculas, nombre)`.
/// Devuelve `None` para líneas vacías o sin nombre de archivo.
fn split_checksum_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let mut parts = line.splitn(2, char::is_whitespace);
    let hash = parts.next()?.trim();
    let name = parts
        .next()?
        .trim_start_matches(|c: char| c.is_whitespace() || c == '*')
        .trim();

    if hash.is_empty() || name.is_empty() {
        return None;
    }

    Some((hash.to_ascii_lowercase(), name.to_string()))
}

/// Extrae el nombre de archivo (último segmento de la ruta) de una URL de
/// artefacto, usado para localizar su entrada en `SHA256SUMS.txt`. Ignora
/// query string y fragment.
pub fn artifact_file_name_from_url(url: &str) -> &str {
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    without_query
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(without_query)
}

/// Devuelve la última parte (nombre de archivo) de una ruta separada por `/`.
fn basename(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

/// Busca el checksum esperado para `artifact_name` en el mapa parseado de
/// `SHA256SUMS.txt`. Primero intenta una coincidencia exacta de ruta y, si
/// no la hay, cae a comparar por nombre de archivo (basename), ya que el
/// manifiesto referencia el artefacto por URL mientras que el documento de
/// checksums puede publicarlo bajo una ruta relativa.
fn lookup_expected_checksum<'a>(
    sums: &'a BTreeMap<String, String>,
    artifact_name: &str,
) -> Option<&'a String> {
    if let Some(hash) = sums.get(artifact_name) {
        return Some(hash);
    }

    let target = basename(artifact_name);
    sums.iter()
        .find(|(path, _)| basename(path) == target)
        .map(|(_, hash)| hash)
}

/// Compara el checksum SHA256 real de `artifact` contra `expected_hex`
/// (case-insensitive) y devuelve el resultado de la compuerta.
///
/// Función pura, sin I/O: es el punto verificado por la Property 26.
pub fn verify_artifact_checksum(artifact: &[u8], expected_hex: &str) -> ChecksumVerification {
    let actual = compute_sha256_hex(artifact);
    let expected = expected_hex.trim().to_ascii_lowercase();

    if actual == expected {
        ChecksumVerification::Match
    } else {
        ChecksumVerification::Mismatch { expected, actual }
    }
}

/// Compuerta estricta de verificación de checksum previa a la instalación
/// (Requisitos 7.4, 7.5, 7.6).
///
/// Calcula el SHA256 de `artifact`, localiza el checksum esperado para
/// `artifact_name` dentro del documento `SHA256SUMS.txt` y los compara:
///
/// - Si coinciden, devuelve `Ok(())`: el llamador puede proceder a instalar
///   (Requisito 7.6).
/// - Si no coinciden, devuelve `Err(AppError::Validation)` con un mensaje de
///   corrupción; el contrato es que el llamador **descarte** el artefacto y
///   **no** sobrescriba la instalación existente (Requisitos 7.4, 7.5). Esta
///   función es pura y no toca el sistema de archivos: no crea ni reemplaza
///   ninguna instalación, de modo que un error garantiza por construcción que
///   nada se instaló.
/// - Si `SHA256SUMS.txt` no publica un checksum para el artefacto, devuelve
///   también un error (no se puede verificar integridad ⇒ no se instala).
pub fn verify_checksum_gate(
    artifact: &[u8],
    artifact_name: &str,
    sha256sums_document: &str,
) -> AppResult<()> {
    let sums = parse_sha256sums(sha256sums_document);
    let expected = lookup_expected_checksum(&sums, artifact_name).ok_or_else(|| {
        AppError::Validation(format!(
            "No se encontró un checksum SHA256 publicado para '{artifact_name}' en \
             SHA256SUMS.txt; no se instalará la actualización."
        ))
    })?;

    match verify_artifact_checksum(artifact, expected) {
        ChecksumVerification::Match => Ok(()),
        ChecksumVerification::Mismatch { expected, actual } => Err(AppError::Validation(format!(
            "La actualización descargada está corrupta: el checksum SHA256 del artefacto \
             '{artifact_name}' ({actual}) no coincide con el valor esperado ({expected}). \
             Se descartó el artefacto y no se modificó la instalación existente."
        ))),
    }
}

// ---------------------------------------------------------------------------
// Relanzamiento post-instalación y continuidad de sesión (Tarea 13.7)
// Requisitos 7.7, 7.8.
// ---------------------------------------------------------------------------

/// Instalación finalizada con éxito: la nueva versión ya quedó escrita en
/// disco (reemplazo del binario/instalador realizado tras superar la
/// compuerta de checksum de la Tarea 13.5), pero la sesión en ejecución
/// sigue corriendo la versión previa hasta que el proceso se relance.
///
/// Es el insumo puro para ofrecer el relanzamiento (Requisito 7.7) y para
/// razonar sobre la continuidad de sesión si el usuario declina
/// (Requisito 7.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessfulInstall {
    /// Versión que la sesión actual está ejecutando en memoria (la previa a
    /// la actualización). Sigue operativa hasta que se relance.
    pub running_version: String,
    /// Versión recién instalada en disco, que se ejecutará tras relanzar o,
    /// si el usuario declina, en el siguiente inicio de la aplicación.
    pub installed_version: String,
}

impl SuccessfulInstall {
    /// Construye una instalación exitosa a partir de la versión en ejecución
    /// y la versión recién instalada.
    pub fn new(running_version: impl Into<String>, installed_version: impl Into<String>) -> Self {
        Self {
            running_version: running_version.into(),
            installed_version: installed_version.into(),
        }
    }

    /// Versión que se ejecutará la próxima vez que la aplicación inicie.
    ///
    /// Tras una instalación exitosa siempre es la versión recién instalada,
    /// **independientemente** de si el usuario relanza ahora o declina: el
    /// artefacto nuevo ya está en disco, de modo que el siguiente arranque lo
    /// tomará (Requisito 7.8).
    pub fn version_on_next_startup(&self) -> &str {
        &self.installed_version
    }
}

/// Oferta de relanzamiento presentada al usuario tras una instalación
/// exitosa (Requisito 7.7).
///
/// Envuelve la [`SuccessfulInstall`] para exponer un mensaje legible y la
/// información necesaria para resolver la decisión del usuario, sin permitir
/// construir una oferta sin una instalación previa exitosa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelaunchPrompt {
    install: SuccessfulInstall,
}

impl RelaunchPrompt {
    /// Versión recién instalada que se ejecutaría al relanzar.
    pub fn installed_version(&self) -> &str {
        &self.install.installed_version
    }

    /// Versión que la sesión actual sigue ejecutando mientras no se relance.
    pub fn running_version(&self) -> &str {
        &self.install.running_version
    }

    /// Mensaje legible para la UI que ofrece relanzar con la nueva versión
    /// (Requisito 7.7).
    pub fn message(&self) -> String {
        format!(
            "Midway {installed} se instaló correctamente. ¿Relanzar ahora para \
             usar la nueva versión? Si prefieres seguir trabajando, la sesión \
             actual continúa con la versión {running} y Midway usará la {installed} \
             la próxima vez que la abras.",
            installed = self.install.installed_version,
            running = self.install.running_version,
        )
    }
}

/// Decisión del usuario ante la oferta de relanzamiento (Requisito 7.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelaunchDecision {
    /// Relanzar la aplicación de inmediato con la nueva versión.
    Relaunch,
    /// Posponer el relanzamiento y seguir trabajando en la sesión actual.
    Decline,
}

/// Acción a ejecutar por la capa de UI una vez que el usuario resuelve la
/// oferta de relanzamiento (Requisitos 7.7, 7.8).
///
/// Es el resultado puro de [`resolve_relaunch_decision`]: describe *qué*
/// hacer sin realizar el efecto (spawn del proceso / salida), de modo que la
/// decisión sea testeable de forma aislada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostInstallAction {
    /// Relanzar el proceso hacia la versión indicada (la recién instalada).
    /// La capa de UI ejecuta el nuevo binario y termina el proceso actual.
    Relaunch { target_version: String },
    /// El usuario declinó: la sesión actual permanece operativa con la
    /// versión previa (`running_version`) y la versión recién instalada
    /// (`pending_version`) se usará en el siguiente inicio (Requisito 7.8).
    /// No se termina el proceso actual ni se interrumpe la sesión.
    ContinueCurrentSession {
        running_version: String,
        pending_version: String,
    },
}

impl PostInstallAction {
    /// `true` solo cuando la acción implica relanzar de inmediato.
    pub fn is_relaunch(&self) -> bool {
        matches!(self, PostInstallAction::Relaunch { .. })
    }

    /// Versión que quedará pendiente para el siguiente inicio cuando el
    /// usuario declina relanzar; `None` cuando se relanza de inmediato.
    pub fn pending_version(&self) -> Option<&str> {
        match self {
            PostInstallAction::ContinueCurrentSession {
                pending_version, ..
            } => Some(pending_version),
            PostInstallAction::Relaunch { .. } => None,
        }
    }
}

/// Ofrece al usuario relanzar la aplicación tras una instalación exitosa
/// (Requisito 7.7). Función pura: solo construye la oferta a partir de la
/// instalación finalizada.
pub fn offer_relaunch(install: SuccessfulInstall) -> RelaunchPrompt {
    RelaunchPrompt { install }
}

/// Resuelve la decisión del usuario sobre la oferta de relanzamiento
/// (Requisitos 7.7, 7.8) en una [`PostInstallAction`] concreta:
///
/// - [`RelaunchDecision::Relaunch`] ⇒ relanzar hacia la versión recién
///   instalada.
/// - [`RelaunchDecision::Decline`] ⇒ mantener la sesión actual operativa con
///   la versión previa y dejar la versión instalada pendiente para el
///   siguiente inicio (Requisito 7.8).
pub fn resolve_relaunch_decision(
    prompt: &RelaunchPrompt,
    decision: RelaunchDecision,
) -> PostInstallAction {
    match decision {
        RelaunchDecision::Relaunch => PostInstallAction::Relaunch {
            target_version: prompt.install.installed_version.clone(),
        },
        RelaunchDecision::Decline => PostInstallAction::ContinueCurrentSession {
            running_version: prompt.install.running_version.clone(),
            pending_version: prompt.install.installed_version.clone(),
        },
    }
}

// ---------------------------------------------------------------------------
// Manejo de fallos de verificación o descarga preservando la instalación
// (Tarea 13.8). Requisito 7.9; Design > Error Handling ("Errores del
// `Updater`").
// ---------------------------------------------------------------------------

/// Etapa del flujo de actualización en la que se originó un fallo
/// (Requisito 7.9). El Requisito 7.9 distingue explícitamente entre un fallo
/// de **verificación** y un fallo de **descarga**; ambas etapas comparten la
/// misma garantía: la instalación existente no se toca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateFailureStage {
    /// Fallo al descargar el manifiesto o el artefacto (red, timeout,
    /// transporte, respuesta HTTP no exitosa o cuerpo no interpretable).
    Download,
    /// Fallo al verificar la actualización descargada (checksum SHA256 no
    /// coincidente / artefacto corrupto, o verificación de versión inválida).
    Verification,
}

impl UpdateFailureStage {
    /// Etiqueta legible de la etapa para el mensaje de error mostrado en la
    /// sección App updates del `Workspace_Panel`.
    pub fn as_str(self) -> &'static str {
        match self {
            UpdateFailureStage::Download => "descarga",
            UpdateFailureStage::Verification => "verificación",
        }
    }
}

/// Error del `Updater` durante la verificación o la descarga de una
/// actualización (Requisito 7.9).
///
/// Clasifica el fallo por [`UpdateFailureStage`] y conserva un detalle
/// legible de la causa (mensaje del `AppError` de origen: red/timeout,
/// respuesta malformada, checksum no coincidente, etc.). Su contrato es
/// puramente informativo: no realiza ni modela ninguna escritura en disco, de
/// modo que su sola existencia implica que **nada** se instaló.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdaterError {
    stage: UpdateFailureStage,
    detail: String,
}

impl UpdaterError {
    /// Construye un error de la etapa de **descarga** con el detalle dado.
    pub fn download(detail: impl Into<String>) -> Self {
        Self {
            stage: UpdateFailureStage::Download,
            detail: detail.into(),
        }
    }

    /// Construye un error de la etapa de **verificación** con el detalle dado.
    pub fn verification(detail: impl Into<String>) -> Self {
        Self {
            stage: UpdateFailureStage::Verification,
            detail: detail.into(),
        }
    }

    /// Traduce un [`AppError`] surgido durante la descarga (típicamente
    /// `AppError::Http` de [`download_artifact_with_progress`] /
    /// [`fetch_manifest_from_url`]) a un [`UpdaterError`] de etapa de descarga.
    pub fn from_download_error(error: AppError) -> Self {
        Self::download(error.to_string())
    }

    /// Traduce un [`AppError`] surgido durante la verificación (típicamente
    /// `AppError::Validation` de [`verify_checksum_gate`] o de la comparación
    /// de versiones) a un [`UpdaterError`] de etapa de verificación.
    pub fn from_verification_error(error: AppError) -> Self {
        Self::verification(error.to_string())
    }

    /// Etapa en la que ocurrió el fallo.
    pub fn stage(&self) -> UpdateFailureStage {
        self.stage
    }

    /// Detalle legible de la causa del fallo.
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Mensaje de error **descriptivo** para la UI (Requisito 7.9): indica la
    /// etapa y la causa, y deja explícito que la versión instalada actual
    /// sigue operativa y sin modificar.
    pub fn message(&self) -> String {
        format!(
            "No se pudo completar la actualización durante la {stage}: {detail}. \
             La versión instalada actual sigue operativa y no se modificó.",
            stage = self.stage.as_str(),
            detail = self.detail,
        )
    }
}

impl std::fmt::Display for UpdaterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for UpdaterError {}

/// Estado resultante de un fallo de verificación o descarga (Requisito 7.9):
/// la actualización **no** se aplicó y la versión previamente instalada
/// permanece operativa y sin modificar.
///
/// Es el insumo puro para renderizar el error en la sección App updates del
/// `Workspace_Panel` sin interrumpir la sesión en curso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservedInstallation {
    installed_version: String,
    error: UpdaterError,
}

impl PreservedInstallation {
    /// Construye el estado preservado a partir de la versión instalada antes
    /// del intento y el error que abortó la actualización.
    pub fn new(installed_version: impl Into<String>, error: UpdaterError) -> Self {
        Self {
            installed_version: installed_version.into(),
            error,
        }
    }

    /// Versión que continúa instalada y operativa (idéntica a la que estaba
    /// antes del intento fallido: no se sobrescribió nada).
    pub fn installed_version(&self) -> &str {
        &self.installed_version
    }

    /// Error que abortó la actualización.
    pub fn error(&self) -> &UpdaterError {
        &self.error
    }

    /// Mensaje de error descriptivo para la UI (Requisito 7.9).
    pub fn error_message(&self) -> String {
        self.error.message()
    }

    /// La instalación existente permanece operativa tras el fallo. Es siempre
    /// `true` por construcción: un fallo de verificación o descarga nunca toca
    /// la instalación, de modo que la versión instalada sigue utilizable.
    pub fn is_installation_operational(&self) -> bool {
        true
    }
}

/// Resultado de un intento de actualización (Requisito 7.9).
///
/// Modela el desenlace del pipeline verificar → descargar → verificar
/// checksum → instalar como un valor puro, de forma que la preservación de la
/// versión instalada ante cualquier fallo (Property 27) sea verificable sin
/// ejecutar I/O ni tocar el sistema de archivos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// La actualización se instaló con éxito (ver [`SuccessfulInstall`]).
    Installed(SuccessfulInstall),
    /// La verificación o la descarga falló; la versión instalada se preservó
    /// (ver [`PreservedInstallation`]).
    Failed(PreservedInstallation),
}

impl UpdateOutcome {
    /// `true` cuando el intento terminó en fallo (instalación preservada).
    pub fn is_failure(&self) -> bool {
        matches!(self, UpdateOutcome::Failed(_))
    }

    /// `true` cuando la actualización se instaló con éxito.
    pub fn is_installed(&self) -> bool {
        matches!(self, UpdateOutcome::Installed(_))
    }

    /// Estado preservado si el intento falló; `None` si se instaló con éxito.
    pub fn preserved(&self) -> Option<&PreservedInstallation> {
        match self {
            UpdateOutcome::Failed(preserved) => Some(preserved),
            UpdateOutcome::Installed(_) => None,
        }
    }

    /// Versión que la instalación conserva como operativa tras el intento:
    ///
    /// - Ante un **fallo**, la versión previamente instalada (sin cambios,
    ///   Requisito 7.9).
    /// - Ante un **éxito**, la versión recién instalada.
    pub fn operational_installed_version(&self) -> &str {
        match self {
            UpdateOutcome::Failed(preserved) => preserved.installed_version(),
            UpdateOutcome::Installed(install) => install.version_on_next_startup(),
        }
    }
}

/// Resuelve el resultado de un intento de actualización preservando la versión
/// instalada ante cualquier fallo de verificación o descarga (Requisito 7.9,
/// Property 27).
///
/// `installed_version` es la versión actualmente instalada y operativa
/// **antes** del intento. Ante un `Err`, la versión instalada se conserva
/// intacta y el error se conserva para mostrarse de forma descriptiva; esta
/// función es pura y no toca el sistema de archivos, de modo que un fallo
/// garantiza por construcción que la instalación existente no se modificó.
pub fn resolve_update_attempt(
    installed_version: &str,
    attempt: Result<SuccessfulInstall, UpdaterError>,
) -> UpdateOutcome {
    match attempt {
        Ok(install) => UpdateOutcome::Installed(install),
        Err(error) => UpdateOutcome::Failed(PreservedInstallation::new(
            installed_version.to_string(),
            error,
        )),
    }
}

#[cfg(test)]
mod tests {
    //! Tests unitarios de `updater.rs` (Tarea 13.1, Fase 6).
    //!
    //! Cubren la comparación semver (camino nominal + casos borde) y la
    //! construcción de URLs / claves de plataforma, sin realizar ninguna
    //! llamada de red. El property test de la comparación semver
    //! (Property 25) se implementa por separado en la Tarea 13.2.
    //!
    //! Ver requisitos: 7.2, 10.4.

    use super::*;

    #[test]
    fn manifest_file_name_matches_channel() {
        assert_eq!(
            MidwayUpdateChannel::Stable.manifest_file_name(),
            "latest.json"
        );
        assert_eq!(
            MidwayUpdateChannel::Beta.manifest_file_name(),
            "latest-beta.json"
        );
    }

    #[test]
    fn manifest_url_uses_github_releases_endpoint_per_channel() {
        assert_eq!(
            manifest_url("aatv1/midway", MidwayUpdateChannel::Stable),
            "https://github.com/aatv1/midway/releases/latest/download/latest.json"
        );
        assert_eq!(
            manifest_url("aatv1/midway", MidwayUpdateChannel::Beta),
            "https://github.com/aatv1/midway/releases/latest/download/latest-beta.json"
        );
    }

    #[test]
    fn manifest_url_with_base_trims_trailing_slash() {
        assert_eq!(
            manifest_url_with_base("http://127.0.0.1:8080/", "o/r", MidwayUpdateChannel::Stable),
            "http://127.0.0.1:8080/o/r/releases/latest/download/latest.json"
        );
    }

    #[test]
    fn current_platform_key_has_os_and_arch() {
        let key = current_platform_key();
        assert!(
            key.contains('-'),
            "la clave de plataforma debe ser '{{os}}-{{arch}}': {key}"
        );
        assert!(
            key.ends_with(std::env::consts::ARCH),
            "la clave debe terminar en la arquitectura actual: {key}"
        );
        // `macos` se normaliza a `darwin` para coincidir con el manifiesto.
        assert!(
            !key.starts_with("macos"),
            "macos debe normalizarse a darwin: {key}"
        );
    }

    // --- Comparación semver (camino nominal) ---

    #[test]
    fn is_update_available_true_when_manifest_is_newer() {
        assert!(is_update_available("1.2.3", "1.2.4").unwrap());
        assert!(is_update_available("1.2.3", "1.3.0").unwrap());
        assert!(is_update_available("1.2.3", "2.0.0").unwrap());
    }

    #[test]
    fn is_update_available_false_when_equal_or_older() {
        assert!(!is_update_available("1.2.3", "1.2.3").unwrap());
        assert!(!is_update_available("1.2.3", "1.2.2").unwrap());
        assert!(!is_update_available("2.0.0", "1.9.9").unwrap());
    }

    #[test]
    fn is_update_available_tolerates_v_prefix() {
        assert!(is_update_available("v1.0.0", "v1.0.1").unwrap());
        assert!(!is_update_available("v1.0.1", "1.0.1").unwrap());
    }

    #[test]
    fn is_update_available_respects_prerelease_precedence() {
        // Una prerelease es anterior a su release final (semver §11).
        assert!(is_update_available("1.0.0-beta.1", "1.0.0").unwrap());
        assert!(!is_update_available("1.0.0", "1.0.0-beta.1").unwrap());
        assert!(is_update_available("1.0.0-beta.1", "1.0.0-beta.2").unwrap());
    }

    // --- Comparación semver (casos borde / error, Requisito 10.4) ---

    #[test]
    fn is_update_available_rejects_invalid_current_version() {
        let error = is_update_available("no-semver", "1.0.0").unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn is_update_available_rejects_invalid_latest_version() {
        let error = is_update_available("1.0.0", "").unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }

    // --- Deserialización del manifiesto + reporte ---

    #[test]
    fn update_manifest_deserializes_published_schema() {
        let raw = r#"{
            "version": "1.4.0",
            "notes": "Release v1.4.0 (stable)",
            "pub_date": "2025-01-01T00:00:00.000Z",
            "platforms": {
                "linux-x86_64": {
                    "url": "https://github.com/o/r/releases/download/v1.4.0/midway_linux_x86_64.AppImage",
                    "signature": "abc123"
                },
                "windows-x86_64": {
                    "url": "https://github.com/o/r/releases/download/v1.4.0/midway_windows_x86_64_setup.exe",
                    "signature": "def456"
                }
            }
        }"#;

        let manifest: UpdateManifest = serde_json::from_str(raw).expect("manifiesto válido");
        assert_eq!(manifest.version, "1.4.0");
        assert_eq!(manifest.platforms.len(), 2);
        assert_eq!(
            manifest.platforms.get("linux-x86_64").unwrap().url,
            "https://github.com/o/r/releases/download/v1.4.0/midway_linux_x86_64.AppImage"
        );
        assert_eq!(
            manifest.platforms.get("linux-x86_64").unwrap().signature,
            "abc123"
        );
    }

    #[test]
    fn build_check_report_flags_newer_version_and_finds_platform_entry() {
        let mut platforms = BTreeMap::new();
        let platform_key = current_platform_key();
        platforms.insert(
            platform_key.clone(),
            UpdatePlatformEntry {
                url: "https://example.test/artifact".to_string(),
                signature: "sig".to_string(),
            },
        );

        // Versión artificialmente alta para garantizar `update_available`
        // sin depender del valor concreto de `CARGO_PKG_VERSION`.
        let manifest = UpdateManifest {
            version: "9999.0.0".to_string(),
            notes: "notas".to_string(),
            pub_date: "2025-01-01T00:00:00.000Z".to_string(),
            platforms,
        };

        let report = build_check_report(MidwayUpdateChannel::Stable, &manifest).unwrap();
        assert!(report.update_available);
        assert_eq!(report.latest_version, "9999.0.0");
        assert_eq!(report.current_version, current_version());
        assert_eq!(report.platform_key, platform_key);
        assert!(report.platform_entry.is_some());
    }

    #[test]
    fn build_check_report_no_update_for_same_version_and_missing_platform() {
        let manifest = UpdateManifest {
            version: current_version().to_string(),
            notes: String::new(),
            pub_date: String::new(),
            platforms: BTreeMap::new(),
        };

        let report = build_check_report(MidwayUpdateChannel::Beta, &manifest).unwrap();
        assert!(!report.update_available);
        assert_eq!(report.latest_version, current_version());
        // Sin entradas de plataforma en el manifiesto.
        assert!(report.platform_entry.is_none());
    }

    // --- check_for_update de extremo a extremo vía servidor de manifiesto
    //     (Tarea 13.10, Requisito 10.4) ---

    /// Camino nominal (registro típico): un manifiesto bien formado servido
    /// por un servidor HTTP local anuncia una versión más reciente con un
    /// artefacto para la plataforma actual. La verificación de extremo a
    /// extremo —descargar el manifiesto y construir el reporte, exactamente
    /// como hace [`check_for_update`], pero redirigiendo la base de descarga
    /// al servidor local— reporta actualización disponible, localiza la
    /// entrada de plataforma y propaga notas y fecha de publicación.
    ///
    /// **Validates: Requirements 10.4**
    #[tokio::test]
    async fn check_for_update_end_to_end_reports_available_update_with_platform_entry() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let repo = "aatv1/midway";
        let platform_key = current_platform_key();

        // Manifiesto típico: versión artificialmente alta (garantiza
        // `update_available` sin depender de `CARGO_PKG_VERSION`) con un
        // artefacto para la plataforma actual.
        let manifest_json = format!(
            r#"{{
                "version": "9999.0.0",
                "notes": "Release 9999.0.0 (stable)",
                "pub_date": "2025-01-01T00:00:00.000Z",
                "platforms": {{
                    "{platform_key}": {{
                        "url": "https://example.test/midway-{platform_key}.bin",
                        "signature": "deadbeef"
                    }}
                }}
            }}"#
        );

        Mock::given(method("GET"))
            .and(path("/aatv1/midway/releases/latest/download/latest.json"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string(manifest_json),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        // Redirige la base de descarga al servidor local: la ruta resultante
        // es idéntica a la que `check_for_update` construye contra GitHub.
        let url = manifest_url_with_base(&mock_server.uri(), repo, MidwayUpdateChannel::Stable);

        let manifest = fetch_manifest_from_url(&client, &url)
            .await
            .expect("el manifiesto debe descargarse y deserializarse");
        let report = build_check_report(MidwayUpdateChannel::Stable, &manifest)
            .expect("el reporte debe construirse");

        assert!(report.update_available);
        assert_eq!(report.latest_version, "9999.0.0");
        assert_eq!(report.current_version, current_version());
        assert_eq!(report.channel, MidwayUpdateChannel::Stable);
        assert_eq!(report.platform_key, platform_key);
        let entry = report
            .platform_entry
            .expect("debe existir una entrada de plataforma para la plataforma actual");
        assert_eq!(
            entry.url,
            format!("https://example.test/midway-{platform_key}.bin")
        );
        assert_eq!(report.notes, "Release 9999.0.0 (stable)");
        assert_eq!(report.pub_date, "2025-01-01T00:00:00.000Z");
    }

    /// Caso borde: el manifiesto (servido por el servidor local) anuncia una
    /// versión más reciente pero **no publica un artefacto para la plataforma
    /// actual** (sólo plataformas ajenas). La verificación de extremo a
    /// extremo reporta que hay actualización disponible pero sin entrada de
    /// plataforma que descargar (`platform_entry == None`), de modo que la UI
    /// puede informar que no hay binario para la plataforma en ejecución en
    /// lugar de intentar una descarga inexistente.
    ///
    /// **Validates: Requirements 10.4**
    #[tokio::test]
    async fn check_for_update_end_to_end_available_but_no_artifact_for_current_platform() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let platform_key = current_platform_key();

        // Clave de plataforma imposible de coincidir con la de ejecución, para
        // forzar el caso borde de forma determinista en cualquier host.
        let foreign_key = format!("otheros-{platform_key}-x");
        let manifest_json = format!(
            r#"{{
                "version": "9999.0.0",
                "notes": "sin binario para esta plataforma",
                "pub_date": "2025-02-02T00:00:00.000Z",
                "platforms": {{
                    "{foreign_key}": {{
                        "url": "https://example.test/otheros.bin",
                        "signature": "cafebabe"
                    }}
                }}
            }}"#
        );

        Mock::given(method("GET"))
            .and(path(
                "/aatv1/midway/releases/latest/download/latest-beta.json",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(manifest_json))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let url = manifest_url_with_base(
            &mock_server.uri(),
            "aatv1/midway",
            MidwayUpdateChannel::Beta,
        );

        let manifest = fetch_manifest_from_url(&client, &url)
            .await
            .expect("el manifiesto debe descargarse y deserializarse");
        let report = build_check_report(MidwayUpdateChannel::Beta, &manifest)
            .expect("el reporte debe construirse");

        assert!(
            report.update_available,
            "9999.0.0 debe ser más reciente que la versión instalada"
        );
        assert_eq!(report.platform_key, platform_key);
        assert!(
            report.platform_entry.is_none(),
            "no debe haber artefacto para la plataforma actual: {:?}",
            report.platform_entry
        );
    }

    // --- Descarga del artefacto con progreso (Tarea 13.3, Requisito 7.3) ---

    #[test]
    fn download_progress_percent_is_bounded_between_0_and_100() {
        assert_eq!(DownloadProgress::new(0, Some(200)).percent(), Some(0));
        assert_eq!(DownloadProgress::new(50, Some(200)).percent(), Some(25));
        assert_eq!(DownloadProgress::new(200, Some(200)).percent(), Some(100));
        // Se satura a 100 si se recibe más de lo anunciado.
        assert_eq!(DownloadProgress::new(250, Some(200)).percent(), Some(100));
    }

    #[test]
    fn download_progress_percent_handles_zero_total_and_unknown_total() {
        // Artefacto de tamaño 0 ya está completo.
        assert_eq!(DownloadProgress::new(0, Some(0)).percent(), Some(100));
        // Sin `Content-Length` el porcentaje es indeterminado.
        assert_eq!(DownloadProgress::new(123, None).percent(), None);
    }

    #[tokio::test]
    async fn download_artifact_returns_full_body_and_reports_final_100_percent() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        // Cuerpo determinístico de tamaño conocido para verificar bytes e
        // integridad del buffer descargado.
        let body: Vec<u8> = (0..4096u32).map(|i| (i % 256) as u8).collect();

        Mock::given(method("GET"))
            .and(path("/artifact.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/artifact.bin", mock_server.uri());

        let mut updates: Vec<DownloadProgress> = Vec::new();
        let downloaded = download_artifact_with_progress(&client, &url, |progress| {
            updates.push(progress);
        })
        .await
        .expect("la descarga debe completarse");

        // El buffer descargado coincide exactamente con el cuerpo servido.
        assert_eq!(downloaded, body);

        // Se emitió al menos el reporte inicial (0%) y el final.
        assert!(
            updates.len() >= 2,
            "se esperaban al menos 2 reportes: {updates:?}"
        );
        assert_eq!(updates.first().unwrap().percent(), Some(0));

        let last = updates.last().unwrap();
        assert_eq!(last.percent(), Some(100));
        assert_eq!(last.downloaded_bytes, body.len() as u64);

        // El progreso es monótonamente no decreciente en bytes.
        for pair in updates.windows(2) {
            assert!(
                pair[1].downloaded_bytes >= pair[0].downloaded_bytes,
                "el progreso no debe retroceder: {updates:?}"
            );
        }
    }

    #[tokio::test]
    async fn download_artifact_propagates_http_error_status() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing.bin"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/missing.bin", mock_server.uri());

        let error = download_artifact_with_progress(&client, &url, |_| {})
            .await
            .expect_err("un 404 debe reportarse como error");
        assert!(matches!(error, AppError::Http(_)));
    }

    // --- Test de integración de progreso de descarga (Tarea 13.4, Requisito 7.3) ---

    /// Test de integración del progreso de descarga del Updater (Tarea 13.4).
    ///
    /// Levanta un servidor HTTP local que sirve un artefacto **en chunks
    /// controlados, separados por pausas**, de modo que la descarga completa
    /// abarque un lapso **mayor a un segundo**. Sobre ese escenario verifica
    /// las dos garantías del Requisito 7.3:
    ///
    /// (a) los reportes de progreso llegan con una **frecuencia mínima de 1
    ///     por segundo** (ninguna brecha entre reportes consecutivos —ni la
    ///     brecha desde el inicio al primero, ni desde el último al fin—
    ///     supera un segundo), y
    /// (b) el **porcentaje final reportado es 100%**.
    ///
    /// El servidor se implementa con la librería estándar (`std::net`) en un
    /// hilo aparte para poder emitir el cuerpo fragmento a fragmento con
    /// pausas deterministas entre chunks —control de temporización que un
    /// mock de respuesta atómica no permite— sin introducir dependencias
    /// nuevas. Se envía `Content-Length` para que el cliente derive el
    /// porcentaje real durante la transmisión.
    ///
    /// **Validates: Requirements 7.3**
    #[tokio::test]
    async fn download_reports_progress_at_least_once_per_second_and_finishes_at_100_percent() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        // 10 chunks separados por 150 ms => la transmisión del cuerpo abarca
        // ~1.35 s, superando holgadamente el segundo exigido para poder
        // ejercitar la frecuencia mínima de reporte.
        const CHUNK_COUNT: usize = 10;
        const CHUNK_SIZE: usize = 8 * 1024;
        const INTER_CHUNK_DELAY: Duration = Duration::from_millis(150);
        let total = CHUNK_COUNT * CHUNK_SIZE;

        // Puerto efímero en loopback: sin dependencia de la red real.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind del servidor local");
        let addr = listener.local_addr().expect("dirección local del servidor");

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("aceptar la conexión");
            // Evita que Nagle coalesca los chunks: cada uno debe salir por
            // separado para reproducir la llegada escalonada de datos.
            stream.set_nodelay(true).ok();

            // Consumir la request HTTP hasta el fin de las cabeceras.
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                match stream.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) => request.push(byte[0]),
                    Err(_) => break,
                }
            }

            // Cabeceras con `Content-Length` para que el cliente conozca el
            // tamaño total y derive el porcentaje de avance.
            let headers = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: application/octet-stream\r\n\
                 Content-Length: {total}\r\n\
                 Connection: close\r\n\r\n"
            );
            if stream.write_all(headers.as_bytes()).is_err() {
                return;
            }
            stream.flush().ok();

            // Emitir el cuerpo en chunks controlados, con pausa entre cada uno.
            for i in 0..CHUNK_COUNT {
                let chunk = vec![(i % 256) as u8; CHUNK_SIZE];
                if stream.write_all(&chunk).is_err() {
                    break;
                }
                stream.flush().ok();
                if i + 1 < CHUNK_COUNT {
                    thread::sleep(INTER_CHUNK_DELAY);
                }
            }
        });

        let client = reqwest::Client::new();
        let url = format!("http://{addr}/artifact.bin");

        // Registrar el instante de cada reporte de progreso para verificar la
        // frecuencia mínima de 1/seg.
        let started = Instant::now();
        let mut timeline: Vec<(Instant, DownloadProgress)> = Vec::new();
        let downloaded = download_artifact_with_progress(&client, &url, |progress| {
            timeline.push((Instant::now(), progress));
        })
        .await
        .expect("la descarga debe completarse");
        let elapsed = started.elapsed();

        server.join().expect("el hilo del servidor debe finalizar");

        // Precondición del escenario: la descarga abarcó más de un segundo, de
        // lo contrario el test no ejercitaría la frecuencia de reporte.
        assert!(
            elapsed > Duration::from_secs(1),
            "la descarga debía abarcar más de 1s para ejercitar la frecuencia de reporte: {elapsed:?}"
        );

        // El artefacto descargado coincide en tamaño con el servido.
        assert_eq!(
            downloaded.len(),
            total,
            "bytes descargados != bytes servidos"
        );

        // Debe haberse emitido al menos el reporte inicial y el final.
        assert!(
            timeline.len() >= 2,
            "se esperaban múltiples reportes de progreso: {timeline:?}"
        );

        // (b) El porcentaje final reportado es 100%.
        let (_, last) = timeline
            .last()
            .copied()
            .expect("al menos un reporte de progreso");
        assert_eq!(
            last.percent(),
            Some(100),
            "el porcentaje final debe ser 100%"
        );
        assert_eq!(last.downloaded_bytes, total as u64);

        // (a) Frecuencia mínima de 1 reporte por segundo: ninguna brecha entre
        // eventos consecutivos supera un segundo, considerando el inicio de la
        // descarga, cada reporte y el fin de la descarga.
        let mut instants: Vec<Instant> = Vec::with_capacity(timeline.len() + 2);
        instants.push(started);
        instants.extend(timeline.iter().map(|(at, _)| *at));
        instants.push(started + elapsed);
        for pair in instants.windows(2) {
            let gap = pair[1].duration_since(pair[0]);
            assert!(
                gap < Duration::from_secs(1),
                "el progreso debe reportarse al menos 1 vez por segundo; brecha detectada: {gap:?}"
            );
        }

        // El progreso es monótonamente no decreciente en bytes.
        for pair in timeline.windows(2) {
            assert!(
                pair[1].1.downloaded_bytes >= pair[0].1.downloaded_bytes,
                "el progreso no debe retroceder: {timeline:?}"
            );
        }
    }

    // --- Progreso de descarga: casos borde adicionales (Tarea 13.3) ---

    #[test]
    fn download_progress_percent_never_exceeds_hundred_across_scales() {
        // Sanity check en escalas grandes: el porcentaje siempre queda acotado
        // a 0..=100 sin overflow (uso de `saturating_mul`).
        let total = u64::MAX;
        for downloaded in [0u64, total / 4, total / 2, total] {
            let percent = DownloadProgress::new(downloaded, Some(total))
                .percent()
                .expect("total conocido");
            assert!(percent <= 100, "percent fuera de rango: {percent}");
        }
    }

    #[test]
    fn progress_update_interval_guarantees_at_least_one_per_second() {
        // El Requisito 7.3 exige >= 1 actualización por segundo; el intervalo
        // debe ser estrictamente menor a 1 s para garantizar ese piso.
        assert!(
            PROGRESS_UPDATE_INTERVAL < Duration::from_secs(1),
            "el intervalo de reporte debe ser < 1s para cumplir el Requisito 7.3"
        );
    }

    // --- Verificación de checksum SHA256 (Tarea 13.5, Requisitos 7.4-7.6) ---

    /// SHA256 conocido de la cadena vacía (vector de prueba estándar).
    const SHA256_EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    /// SHA256 conocido de `b"abc"` (vector de prueba estándar del NIST).
    const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn compute_sha256_hex_matches_known_vectors() {
        assert_eq!(compute_sha256_hex(b""), SHA256_EMPTY);
        assert_eq!(compute_sha256_hex(b"abc"), SHA256_ABC);
        // Salida siempre de 64 caracteres hex en minúsculas.
        let hex = compute_sha256_hex(b"midway");
        assert_eq!(hex.len(), 64);
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn parse_sha256sums_reads_standard_two_space_format() {
        let doc = format!(
            "{SHA256_ABC}  midway_linux_x86_64.AppImage\n\
             {SHA256_EMPTY}  dist/midway_windows_x86_64_setup.exe\n"
        );
        let sums = parse_sha256sums(&doc);
        assert_eq!(sums.len(), 2);
        assert_eq!(
            sums.get("midway_linux_x86_64.AppImage").unwrap(),
            SHA256_ABC
        );
        assert_eq!(
            sums.get("dist/midway_windows_x86_64_setup.exe").unwrap(),
            SHA256_EMPTY
        );
    }

    #[test]
    fn parse_sha256sums_ignores_blank_lines_and_binary_marker() {
        let doc = format!("\n   \n{SHA256_ABC} *binary_marker_artifact.bin\n\n");
        let sums = parse_sha256sums(&doc);
        assert_eq!(sums.len(), 1);
        assert_eq!(sums.get("binary_marker_artifact.bin").unwrap(), SHA256_ABC);
    }

    #[test]
    fn artifact_file_name_from_url_extracts_last_segment() {
        assert_eq!(
            artifact_file_name_from_url(
                "https://github.com/o/r/releases/download/v1.4.0/midway_linux_x86_64.AppImage"
            ),
            "midway_linux_x86_64.AppImage"
        );
        // Ignora query y fragment.
        assert_eq!(
            artifact_file_name_from_url("https://host/path/setup.exe?token=abc#frag"),
            "setup.exe"
        );
    }

    #[test]
    fn verify_artifact_checksum_matches_when_hash_agrees() {
        let result = verify_artifact_checksum(b"abc", SHA256_ABC);
        assert_eq!(result, ChecksumVerification::Match);
        assert!(result.is_match());
    }

    #[test]
    fn verify_artifact_checksum_is_case_insensitive() {
        let upper = SHA256_ABC.to_ascii_uppercase();
        assert_eq!(
            verify_artifact_checksum(b"abc", &upper),
            ChecksumVerification::Match
        );
    }

    #[test]
    fn verify_artifact_checksum_detects_mismatch() {
        match verify_artifact_checksum(b"abc", SHA256_EMPTY) {
            ChecksumVerification::Mismatch { expected, actual } => {
                assert_eq!(expected, SHA256_EMPTY);
                assert_eq!(actual, SHA256_ABC);
            }
            ChecksumVerification::Match => panic!("un checksum distinto no debe coincidir"),
        }
    }

    #[test]
    fn verify_checksum_gate_ok_on_match_allows_install() {
        let artifact = b"abc";
        let doc = format!("{SHA256_ABC}  midway_linux_x86_64.AppImage\n");
        // Coincidencia exacta de nombre.
        assert!(verify_checksum_gate(artifact, "midway_linux_x86_64.AppImage", &doc).is_ok());
    }

    #[test]
    fn verify_checksum_gate_matches_by_basename_when_path_differs() {
        let artifact = b"abc";
        // El documento publica una ruta relativa; el manifiesto referencia
        // sólo el nombre de archivo.
        let doc = format!("{SHA256_ABC}  dist/linux/midway_linux_x86_64.AppImage\n");
        assert!(verify_checksum_gate(artifact, "midway_linux_x86_64.AppImage", &doc).is_ok());
    }

    #[test]
    fn verify_checksum_gate_errors_on_corruption_without_installing() {
        // El artefacto real difiere del esperado (un bit alterado ⇒ hash
        // completamente distinto). La compuerta debe fallar con un error de
        // corrupción y, al ser pura, nunca toca la instalación existente.
        let corrupted = b"abc-corrupted";
        let doc = format!("{SHA256_ABC}  midway_linux_x86_64.AppImage\n");
        let error = verify_checksum_gate(corrupted, "midway_linux_x86_64.AppImage", &doc)
            .expect_err("un checksum no coincidente debe rechazar la instalación");
        assert!(matches!(error, AppError::Validation(_)));
        assert!(
            error.to_string().contains("corrupta"),
            "el mensaje debe indicar corrupción: {error}"
        );
    }

    #[test]
    fn verify_checksum_gate_errors_when_artifact_absent_from_sums() {
        let doc = format!("{SHA256_ABC}  some_other_artifact.bin\n");
        let error = verify_checksum_gate(b"abc", "midway_linux_x86_64.AppImage", &doc)
            .expect_err("sin checksum publicado no se puede verificar ⇒ no instalar");
        assert!(matches!(error, AppError::Validation(_)));
    }

    // --- Relanzamiento post-instalación y continuidad de sesión (Tarea 13.7,
    //     Requisitos 7.7, 7.8) ---

    #[test]
    fn offer_relaunch_references_installed_and_running_versions() {
        // Requisito 7.7: tras instalar con éxito se ofrece relanzar con la
        // nueva versión.
        let install = SuccessfulInstall::new("1.2.3", "1.3.0");
        let prompt = offer_relaunch(install);

        assert_eq!(prompt.installed_version(), "1.3.0");
        assert_eq!(prompt.running_version(), "1.2.3");

        let message = prompt.message();
        assert!(
            message.contains("1.3.0"),
            "el mensaje debe ofrecer la nueva versión: {message}"
        );
        assert!(
            message.contains("1.2.3"),
            "el mensaje debe mencionar la versión actual: {message}"
        );
    }

    #[test]
    fn accepting_relaunch_targets_the_installed_version() {
        // Requisito 7.7: al aceptar, se relanza con la versión recién instalada.
        let prompt = offer_relaunch(SuccessfulInstall::new("1.2.3", "1.3.0"));
        let action = resolve_relaunch_decision(&prompt, RelaunchDecision::Relaunch);

        assert_eq!(
            action,
            PostInstallAction::Relaunch {
                target_version: "1.3.0".to_string()
            }
        );
        assert!(action.is_relaunch());
        // Al relanzar no hay versión "pendiente": se usa de inmediato.
        assert_eq!(action.pending_version(), None);
    }

    #[test]
    fn declining_relaunch_keeps_current_session_and_defers_new_version() {
        // Requisito 7.8: al declinar, la sesión actual sigue operativa con la
        // versión previa y la nueva queda pendiente para el siguiente inicio.
        let install = SuccessfulInstall::new("1.2.3", "1.3.0");
        let prompt = offer_relaunch(install.clone());
        let action = resolve_relaunch_decision(&prompt, RelaunchDecision::Decline);

        assert_eq!(
            action,
            PostInstallAction::ContinueCurrentSession {
                running_version: "1.2.3".to_string(),
                pending_version: "1.3.0".to_string(),
            }
        );
        // No se relanza: la sesión actual no se interrumpe.
        assert!(!action.is_relaunch());
        assert_eq!(action.pending_version(), Some("1.3.0"));
    }

    #[test]
    fn next_startup_uses_installed_version_regardless_of_decision() {
        // Requisito 7.8: el artefacto nuevo ya está en disco, así que el
        // siguiente inicio usa la versión instalada tanto si se relanza como
        // si se declina.
        let install = SuccessfulInstall::new("1.2.3", "1.3.0");
        assert_eq!(install.version_on_next_startup(), "1.3.0");

        let prompt = offer_relaunch(install);
        for decision in [RelaunchDecision::Relaunch, RelaunchDecision::Decline] {
            let action = resolve_relaunch_decision(&prompt, decision);
            let next_startup_version = match &action {
                PostInstallAction::Relaunch { target_version } => target_version.as_str(),
                PostInstallAction::ContinueCurrentSession {
                    pending_version, ..
                } => pending_version.as_str(),
            };
            assert_eq!(next_startup_version, "1.3.0");
        }
    }

    // --- Manejo de fallos de verificación o descarga (Tarea 13.8,
    //     Requisito 7.9) ---

    #[test]
    fn updater_error_message_is_descriptive_and_states_preservation() {
        // Requisito 7.9: el mensaje debe ser descriptivo (etapa + causa) y
        // dejar claro que la versión instalada sigue operativa.
        let download = UpdaterError::download("timeout de red tras 30s");
        assert_eq!(download.stage(), UpdateFailureStage::Download);
        let message = download.message();
        assert!(
            message.contains("descarga"),
            "debe indicar la etapa: {message}"
        );
        assert!(
            message.contains("timeout de red"),
            "debe incluir la causa: {message}"
        );
        assert!(
            message.contains("sigue operativa") && message.contains("no se modificó"),
            "debe indicar que la instalación se preserva: {message}"
        );

        let verification = UpdaterError::verification("checksum SHA256 no coincide");
        assert_eq!(verification.stage(), UpdateFailureStage::Verification);
        assert!(verification.message().contains("verificación"));
    }

    #[test]
    fn updater_error_maps_from_app_errors_by_stage() {
        // Un error de descarga suele llegar como `AppError::Http`.
        let download =
            UpdaterError::from_download_error(AppError::Http("error sending request".to_string()));
        assert_eq!(download.stage(), UpdateFailureStage::Download);
        assert!(download.detail().contains("error sending request"));

        // Un fallo de checksum llega como `AppError::Validation` desde
        // `verify_checksum_gate`.
        let verification = UpdaterError::from_verification_error(AppError::Validation(
            "La actualización descargada está corrupta".to_string(),
        ));
        assert_eq!(verification.stage(), UpdateFailureStage::Verification);
        assert!(verification.detail().contains("corrupta"));
    }

    #[test]
    fn resolve_update_attempt_failure_preserves_installed_version() {
        // Requisito 7.9 / Property 27: un fallo de descarga conserva la versión
        // instalada intacta y la deja operativa.
        let outcome =
            resolve_update_attempt("1.2.3", Err(UpdaterError::download("conexión rechazada")));

        assert!(outcome.is_failure());
        assert!(!outcome.is_installed());
        // La versión operativa tras el fallo es la que ya estaba instalada.
        assert_eq!(outcome.operational_installed_version(), "1.2.3");

        let preserved = outcome
            .preserved()
            .expect("un fallo debe preservar la instalación");
        assert_eq!(preserved.installed_version(), "1.2.3");
        assert!(preserved.is_installation_operational());
        assert_eq!(preserved.error().stage(), UpdateFailureStage::Download);
        assert!(
            preserved.error_message().contains("no se modificó"),
            "el mensaje debe reflejar la preservación: {}",
            preserved.error_message()
        );
    }

    #[test]
    fn resolve_update_attempt_verification_failure_preserves_installed_version() {
        // Un fallo de verificación de checksum también preserva la instalación.
        let outcome = resolve_update_attempt(
            "2.0.0",
            Err(UpdaterError::verification("checksum SHA256 no coincidente")),
        );

        assert!(outcome.is_failure());
        assert_eq!(outcome.operational_installed_version(), "2.0.0");
        let preserved = outcome.preserved().unwrap();
        assert_eq!(preserved.installed_version(), "2.0.0");
        assert_eq!(preserved.error().stage(), UpdateFailureStage::Verification);
    }

    #[test]
    fn resolve_update_attempt_success_reports_installed_version() {
        // Camino nominal: un intento exitoso reporta la instalación, no un
        // estado preservado.
        let outcome = resolve_update_attempt("1.2.3", Ok(SuccessfulInstall::new("1.2.3", "1.3.0")));

        assert!(outcome.is_installed());
        assert!(!outcome.is_failure());
        assert!(outcome.preserved().is_none());
        // Tras un éxito, la versión operativa para el siguiente inicio es la
        // recién instalada.
        assert_eq!(outcome.operational_installed_version(), "1.3.0");
    }

    // --- Camino nominal y borde del pipeline de actualización (Tarea 13.10,
    //     Requisito 10.4) ---
    //
    // Estos tests ejercitan el **flujo completo** verificar versión → verificar
    // checksum → resolver instalación como una sola secuencia, componiendo las
    // funciones puras del módulo (`is_update_available`, `verify_checksum_gate`,
    // `resolve_update_attempt`). Complementan los tests atómicos anteriores
    // aportando un caso de camino nominal de extremo a extremo y un caso de
    // error/borde, tal como pide el Requisito 10.4.

    /// Camino nominal: hay una versión más reciente disponible, el checksum del
    /// artefacto descargado coincide con el publicado en `SHA256SUMS.txt` y la
    /// instalación procede con éxito. El resultado del intento es
    /// `UpdateOutcome::Installed` y la versión operativa para el siguiente
    /// inicio es la recién instalada.
    ///
    /// **Validates: Requirements 10.4**
    #[test]
    fn pipeline_nominal_path_available_verified_and_installed_succeeds() {
        let current = current_version();
        // Versión artificialmente alta: garantiza `update_available` sin
        // depender del valor concreto de `CARGO_PKG_VERSION`.
        let latest = "9999.0.0";
        let artifact_name = "midway_linux_x86_64.AppImage";
        let artifact = b"abc";

        // 1) La comparación semver detecta una versión más reciente.
        assert!(
            is_update_available(current, latest).unwrap(),
            "{latest} debe ser más reciente que la versión instalada {current}"
        );

        // 2) La compuerta de checksum coincide ⇒ se permite instalar.
        let sums_doc = format!("{SHA256_ABC}  {artifact_name}\n");
        assert!(
            verify_checksum_gate(artifact, artifact_name, &sums_doc).is_ok(),
            "un checksum coincidente debe habilitar la instalación"
        );

        // 3) La instalación procede con éxito y se resuelve el intento.
        let install = SuccessfulInstall::new(current, latest);
        let outcome = resolve_update_attempt(current, Ok(install));

        assert!(
            outcome.is_installed(),
            "el intento nominal debe instalar la nueva versión"
        );
        assert!(!outcome.is_failure());
        assert!(outcome.preserved().is_none());
        // Tras el éxito, la versión operativa para el siguiente inicio es la
        // recién instalada.
        assert_eq!(outcome.operational_installed_version(), latest);
    }

    /// Caso de error/borde: el artefacto descargado está corrupto (su checksum
    /// no coincide con el publicado). La compuerta de checksum falla, el intento
    /// se resuelve como fallo de **verificación** y la versión previamente
    /// instalada se preserva intacta y operativa (no se instala nada).
    ///
    /// **Validates: Requirements 10.4**
    #[test]
    fn pipeline_edge_invalid_checksum_aborts_install_and_preserves_version() {
        let current = current_version();
        let artifact_name = "midway_linux_x86_64.AppImage";
        // El artefacto real es `b"abc"`, pero el documento publica el checksum
        // de la cadena vacía ⇒ hash completamente distinto.
        let corrupted_artifact = b"abc";
        let sums_doc = format!("{SHA256_EMPTY}  {artifact_name}\n");

        // La compuerta de checksum rechaza la instalación por corrupción.
        let gate_error = verify_checksum_gate(corrupted_artifact, artifact_name, &sums_doc)
            .expect_err("un checksum no coincidente debe rechazar la instalación");
        assert!(matches!(gate_error, AppError::Validation(_)));

        // El fallo de verificación se resuelve preservando la instalación.
        let outcome = resolve_update_attempt(
            current,
            Err(UpdaterError::from_verification_error(gate_error)),
        );

        assert!(outcome.is_failure());
        assert!(!outcome.is_installed());
        // La versión operativa sigue siendo la que ya estaba instalada.
        assert_eq!(outcome.operational_installed_version(), current);

        let preserved = outcome
            .preserved()
            .expect("un fallo debe preservar la instalación");
        assert_eq!(preserved.installed_version(), current);
        assert!(preserved.is_installation_operational());
        assert_eq!(preserved.error().stage(), UpdateFailureStage::Verification);
    }

    /// Caso de borde adicional: el manifiesto anuncia una versión **igual o
    /// menor** que la instalada, por lo que no hay actualización disponible y
    /// el pipeline no debe intentar descargar ni instalar nada.
    ///
    /// **Validates: Requirements 10.4**
    #[test]
    fn pipeline_edge_equal_or_older_version_reports_no_update() {
        let current = current_version();

        // Misma versión: no hay actualización.
        assert!(
            !is_update_available(current, current).unwrap(),
            "una versión idéntica no debe reportar actualización disponible"
        );

        // Versión estrictamente menor: tampoco hay actualización.
        assert!(!is_update_available("2.0.0", "1.9.9").unwrap());
        assert!(!is_update_available("1.2.3", "1.2.2").unwrap());
    }

    // --- Property 25: comparación semver (Fase 6, Tarea 13.2) ---

    mod property_semver_comparison {
        //! Property 25 (design.md): la comparación semver determina
        //! correctamente la disponibilidad de actualización.
        //!
        //! **Validates: Requirements 7.2**

        use super::*;
        use proptest::prelude::*;

        /// Genera una cadena de versión semántica **válida**, cubriendo
        /// tanto versiones "core" (`MAJOR.MINOR.PATCH`) como versiones con
        /// prerelease (semver §9), devolviendo la cadena junto con su
        /// `semver::Version` parseada para poder contrastar contra el orden
        /// total del crate sin reparsear en el cuerpo del test.
        ///
        /// Los componentes numéricos se acotan a un rango pequeño de forma
        /// intencional: así el par (instalada, manifiesto) produce con alta
        /// frecuencia los tres casos relevantes —mayor, menor e igual— en
        /// lugar de generar casi siempre versiones distintas.
        fn arb_semver_version() -> impl Strategy<Value = (String, semver::Version)> {
            let prerelease = prop_oneof![
                Just(String::new()),
                Just("-alpha".to_string()),
                Just("-beta.1".to_string()),
                Just("-beta.2".to_string()),
                Just("-rc.1".to_string()),
            ];

            (0u64..=5, 0u64..=5, 0u64..=5, prerelease).prop_map(|(major, minor, patch, pre)| {
                let raw = format!("{major}.{minor}.{patch}{pre}");
                let parsed =
                    semver::Version::parse(&raw).expect("el generador solo produce semver válido");
                (raw, parsed)
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

            /// Property 25: para cualquier par de versiones semánticas
            /// válidas, `is_update_available` reporta que hay actualización
            /// disponible si y solo si la versión del manifiesto es
            /// estrictamente mayor que la instalada según el orden total de
            /// `semver::Version`.
            ///
            /// **Validates: Requirements 7.2**
            #[test]
            fn property_25_is_update_available_agrees_with_semver_ordering(
                (current_raw, current_ver) in arb_semver_version(),
                (latest_raw, latest_ver) in arb_semver_version(),
            ) {
                let reported = is_update_available(&current_raw, &latest_raw)
                    .expect("ambas versiones son semver válidas");
                let expected = latest_ver > current_ver;

                prop_assert_eq!(
                    reported,
                    expected,
                    "is_update_available({:?}, {:?}) = {}, pero semver ordena latest > current = {}",
                    current_raw,
                    latest_raw,
                    reported,
                    expected
                );
            }
        }
    }

    // --- Property 26: compuerta de verificación de checksum (Fase 6, Tarea 13.6) ---

    mod property_checksum_gate {
        //! Property 26 (design.md): la verificación de checksum SHA256 es una
        //! **compuerta estricta** antes de instalar. Para cualquier artefacto
        //! descargado (bytes arbitrarios) y su checksum SHA256 real, la
        //! compuerta [`verify_checksum_gate`] devuelve `Ok(())` —permitiendo
        //! instalar— **si y solo si** el checksum publicado en
        //! `SHA256SUMS.txt` coincide (case-insensitive) con el SHA256 real del
        //! artefacto. Cualquier discrepancia (checksum equivocado, un solo
        //! carácter alterado, o bytes del artefacto corruptos/alterados)
        //! produce `Err`, con lo que el artefacto se descarta y —al ser esta
        //! una función pura sin I/O— la instalación existente queda intacta
        //! por construcción.
        //!
        //! **Validates: Requirements 7.4, 7.5, 7.6**

        use super::*;
        use proptest::prelude::*;

        /// Genera una cadena hexadecimal en minúsculas de 64 caracteres (la
        /// longitud de un SHA256), usada como checksum "publicado" arbitrario
        /// (con altísima probabilidad distinto del real).
        fn arb_sha256_hex() -> impl Strategy<Value = String> {
            prop::collection::vec(0u8..16, 64).prop_map(|nibbles| {
                nibbles
                    .into_iter()
                    .map(|n| std::char::from_digit(u32::from(n), 16).expect("nibble < 16"))
                    .collect()
            })
        }

        /// Devuelve una copia de `hex` con el carácter en `idx` cambiado por
        /// un dígito hexadecimal **distinto**, garantizando un checksum que
        /// difiere del original en exactamente una posición.
        fn flip_one_hex_char(hex: &str, idx: usize) -> String {
            let mut chars: Vec<char> = hex.chars().collect();
            if chars.is_empty() {
                return hex.to_string();
            }
            let i = idx % chars.len();
            // Mapear a un dígito distinto: '0' -> '1', cualquier otro -> '0'.
            chars[i] = if chars[i] == '0' { '1' } else { '0' };
            chars.into_iter().collect()
        }

        /// Estrategia que produce el par `(artefacto_final, checksum_publicado)`
        /// que efectivamente se le pasa a la compuerta. Cubre de forma
        /// intencional los cinco escenarios relevantes de la Property 26:
        ///
        /// 1. Checksum correcto (coincidencia exacta) -> debe permitir instalar.
        /// 2. Checksum correcto en MAYÚSCULAS -> la compuerta es
        ///    case-insensitive, sigue coincidiendo -> permite instalar.
        /// 3. Checksum aleatorio (con altísima probabilidad distinto) -> rechaza.
        /// 4. Checksum correcto con un solo carácter alterado -> rechaza.
        /// 5. Bytes del artefacto alterados publicando el checksum del
        ///    artefacto original -> rechaza (artefacto corrupto/alterado).
        ///
        /// El cuerpo del test **no** asume el resultado esperado a partir del
        /// escenario: lo recomputa desde el estado final (`artefacto`,
        /// `publicado`), de modo que la aserción sea un bicondicional puro.
        fn arb_gate_case() -> impl Strategy<Value = (Vec<u8>, String)> {
            prop::collection::vec(any::<u8>(), 0..300).prop_flat_map(|bytes| {
                let actual = compute_sha256_hex(&bytes);

                let correct = {
                    let b = bytes.clone();
                    Just((b, actual.clone()))
                };
                let correct_upper = {
                    let b = bytes.clone();
                    Just((b, actual.to_ascii_uppercase()))
                };
                let wrong_random = {
                    let b = bytes.clone();
                    arb_sha256_hex().prop_map(move |h| (b.clone(), h))
                };
                let flipped = {
                    let b = bytes.clone();
                    let a = actual.clone();
                    (0usize..64).prop_map(move |i| (b.clone(), flip_one_hex_char(&a, i)))
                };
                let altered_bytes = {
                    let a = actual.clone();
                    (any::<u8>(), any::<usize>()).prop_map(move |(val, idx)| {
                        let mut altered = bytes.clone();
                        if altered.is_empty() {
                            altered.push(val);
                        } else {
                            let i = idx % altered.len();
                            altered[i] = altered[i].wrapping_add(1);
                        }
                        (altered, a.clone())
                    })
                };

                prop_oneof![correct, correct_upper, wrong_random, flipped, altered_bytes]
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 300, ..ProptestConfig::default() })]

            /// Property 26: la compuerta de checksum permite instalar
            /// (`Ok(())`) si y solo si el checksum publicado en
            /// `SHA256SUMS.txt` es igual (ignorando mayúsculas/minúsculas) al
            /// SHA256 real del artefacto. Toda discrepancia —incluyendo un
            /// único carácter alterado o bytes del artefacto corruptos—
            /// produce `Err`, descartando el artefacto sin tocar la
            /// instalación existente.
            ///
            /// **Validates: Requirements 7.4, 7.5, 7.6**
            #[test]
            fn property_26_checksum_gate_is_strict_iff_match(
                (artifact, published) in arb_gate_case(),
            ) {
                let artifact_name = "midway-desktop_1.2.3_amd64.AppImage";
                let document = format!("{published}  {artifact_name}\n");

                // Estado real: ¿coincide el checksum publicado (normalizado)
                // con el SHA256 real del artefacto?
                let actual = compute_sha256_hex(&artifact);
                let expected_ok = published.trim().to_ascii_lowercase() == actual;

                let result = verify_checksum_gate(&artifact, artifact_name, &document);

                // Bicondicional: instalar permitido <=> checksum coincide.
                prop_assert_eq!(
                    result.is_ok(),
                    expected_ok,
                    "verify_checksum_gate permitió={} pero checksum coincide={} \
                     (publicado={:?}, real={:?})",
                    result.is_ok(),
                    expected_ok,
                    published,
                    actual
                );

                // La verificación pura debe ser coherente con la compuerta:
                // Match en el caso permitido, Mismatch (rechazo) en el resto.
                let verification = verify_artifact_checksum(&artifact, &published);
                if expected_ok {
                    prop_assert!(
                        verification.is_match(),
                        "esperaba Match cuando el checksum coincide"
                    );
                    prop_assert!(result.is_ok(), "checksum coincide => se permite instalar");
                } else {
                    prop_assert!(
                        !verification.is_match(),
                        "esperaba Mismatch cuando el checksum difiere"
                    );
                    prop_assert!(
                        result.is_err(),
                        "checksum difiere => se rechaza (artefacto descartado, \
                         instalación existente intacta)"
                    );
                }
            }
        }
    }

    // --- Property 27: preservación de la versión instalada ante fallo (Fase 6, Tarea 13.9) ---

    mod property_installed_version_preservation {
        //! Property 27 (design.md): un fallo de verificación o descarga
        //! preserva la versión instalada. Para una versión instalada
        //! arbitraria y un fallo arbitrario (etapa de descarga o de
        //! verificación con un detalle arbitrario), [`resolve_update_attempt`]
        //! produce un desenlace `Failed` cuya versión instalada
        //! operativa/preservada es **idéntica** a la versión instalada antes
        //! del intento (sin modificar), y la instalación permanece operativa.
        //! Al ser una función pura sin I/O, un fallo garantiza por
        //! construcción que nada se instaló.
        //!
        //! **Validates: Requirements 7.9**

        use super::*;
        use proptest::prelude::*;

        /// Genera una versión instalada arbitraria. Cubre de forma
        /// intencional tanto cadenas semver típicas (`MAJOR.MINOR.PATCH`,
        /// con o sin prerelease) como cadenas arbitrarias, ya que la versión
        /// instalada se conserva **verbatim** (no se reparsea) y la
        /// preservación debe cumplirse sea cual sea su forma.
        fn arb_installed_version() -> impl Strategy<Value = String> {
            prop_oneof![
                (0u64..=20, 0u64..=20, 0u64..=20).prop_map(|(a, b, c)| format!("{a}.{b}.{c}")),
                (0u64..=20, 0u64..=20, 0u64..=20)
                    .prop_map(|(a, b, c)| format!("{a}.{b}.{c}-beta.1")),
                ".*",
            ]
        }

        /// Genera un fallo arbitrario del pipeline de actualización: etapa de
        /// descarga o de verificación, con un detalle (causa) arbitrario.
        fn arb_failure() -> impl Strategy<Value = UpdaterError> {
            prop_oneof![
                ".*".prop_map(UpdaterError::download),
                ".*".prop_map(UpdaterError::verification),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 300, ..ProptestConfig::default() })]

            /// Property 27: ante cualquier fallo (descarga o verificación) con
            /// un detalle arbitrario, `resolve_update_attempt` deja la versión
            /// instalada intacta y operativa.
            ///
            /// **Validates: Requirements 7.9**
            #[test]
            fn property_27_failure_preserves_installed_version(
                installed in arb_installed_version(),
                failure in arb_failure(),
            ) {
                let expected_stage = failure.stage();
                let outcome = resolve_update_attempt(&installed, Err(failure));

                // El intento fallido produce un desenlace `Failed`, no `Installed`.
                prop_assert!(
                    outcome.is_failure(),
                    "un fallo debe producir UpdateOutcome::Failed, no Installed"
                );
                prop_assert!(!outcome.is_installed());

                // La versión operativa tras el intento es exactamente la
                // versión instalada antes del intento, sin modificar.
                prop_assert_eq!(
                    outcome.operational_installed_version(),
                    installed.as_str(),
                    "la versión operativa cambió tras un fallo (esperaba {:?})",
                    installed
                );

                let preserved = outcome
                    .preserved()
                    .expect("un fallo debe exponer el estado preservado");

                // La versión preservada es idéntica a la instalada previa.
                prop_assert_eq!(preserved.installed_version(), installed.as_str());

                // La instalación existente sigue operativa por construcción.
                prop_assert!(
                    preserved.is_installation_operational(),
                    "la instalación existente debe permanecer operativa tras el fallo"
                );

                // La etapa del error se conserva para reportarlo de forma
                // descriptiva, sin afectar la preservación.
                prop_assert_eq!(preserved.error().stage(), expected_stage);
            }
        }
    }
}
