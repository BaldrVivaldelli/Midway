# Changelog

Todos los cambios notables de este proyecto se documentan en este archivo.

El formato está basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto adhiere a [Semantic Versioning](https://semver.org/lang/es/).

## [Unreleased]

### Auditoría verificable del baseline y primera vertical de UI

#### Added

- Auditoría reproducible del producto y la arquitectura, con matriz de
  funcionalidades, limitaciones conocidas, principios de producto, ADRs y
  evidencias crudas de los quality gates.
- Primera vertical extraída de `app.rs`: ajustes de tema con estado, mensajes,
  actualización pura y vista propios, protegida por un test de dependencias.
- Divisor redimensionable del panel de respuesta, persistencia de su altura y
  nuevos estados vacíos con acciones contextuales.
- Tests de caracterización, integración y propiedades que elevan la suite de
  301 a 363 casos headless.

#### Changed

- El quality gate de CI y `make verify` ahora incluyen `rustfmt` y Clippy con
  warnings tratados como errores, además de compilación y tests del workspace.
- Se aplicó `rustfmt` a todo el workspace y se atendieron los lints de Clippy
  detectados por la nueva compuerta estática, sin cambios funcionales.
- Textos de las superficies modificadas unificados en español con voseo.

### Migración de Tauri + React/TypeScript a 100% Rust sobre `iced`

Reescritura de Midway como una aplicación de escritorio **100% Rust** sobre
[`iced`](https://iced.rs), eliminando por completo el runtime web, el frontend
React/TypeScript y el empaquetado con Tauri. La lógica de dominio, ejecución,
persistencia y secretos se preserva sin reescritura, extraída a un crate
reutilizable. La migración se realizó por fases (0 a 8), con un checkpoint de
`cargo check` + `cargo test` sobre el workspace completo al final de cada fase.

### Added

- **Cargo workspace** con dos crates: `midway-core` (dominio, infraestructura y
  runtime) y `midway-desktop` (binario GUI en `iced`).
- **Request composer** en `iced`: método HTTP, URL, Send, selector de
  environment, settings con preview del request y comando cURL equivalente.
- **Import de cURL** pegando el comando (port de `curl.ts`), con enrutamiento
  según el estado de la tab activa.
- **Response inspector**: status, tiempo, tamaño y tabs Body/Headers/Tests.
- **Editor de texto nativo** (`iced::widget::text_editor` + `iced_highlighter`)
  con resaltado JSON, formateo, lint con posición de error y búsqueda.
- **Tabs de configuración** Params/Headers/Auth/Body/Tests con selección por
  defecto según método HTTP y override manual.
- **Workspace panel** lateral: Environments (CRUD con validaciones), Data
  (import/export nativo v1, Postman v2.1, OpenAPI v3), History (límite 500),
  Diagnostics (límite 200) y App updates.
- **Collection runner** con ejecución secuencial, progreso incremental vía
  `mpsc` + subscription, reporte consolidado y cancelación.
- **Command palette** (Ctrl/Cmd+K), persistencia y restauración de sesión con
  escritura atómica y autosave, stack de tabs cerradas (LIFO), aviso de cambios
  sin guardar, shortcuts de teclado globales y error boundary por componente.
- **Updater in-app**: descarga de manifiesto, comparación semver, descarga con
  progreso, verificación de checksum SHA256 como compuerta y relanzamiento.
- **Empaquetado con `cargo-packager`** (Windows NSIS/MSI, Linux AppImage/deb) en
  reemplazo del CLI de Tauri, con configuración base en
  `[package.metadata.packager]` y overrides por canal (stable/beta).
- Suite de tests unitarios, de integración y **property-based** (`proptest`)
  con 28 correctness properties sobre el workspace.
- `CHANGELOG.md` (este archivo).

### Changed

- **CI/Release workflows**: eliminados los pasos de Node/npm/Vite del build de
  frontend; el quality gate corre `cargo check --workspace` y
  `cargo test --workspace`. El empaquetado usa `cargo-packager` pinneado a
  `0.11.8`.
- Scripts de release (`generate-updater-json.mjs`, `generate-checksums.mjs`)
  adaptados a los nombres de artefacto de `cargo-packager`;
  `render-tauri-config.mjs` reemplazado por `render-packager-config.mjs`.
- `README.md` y `docs/distribution.md` actualizados al stack 100% Rust.
- Tests `.mjs` de release ajustados para validar la config base de empaquetado
  en `midway-desktop/Cargo.toml` en lugar de los `tauri.*.conf.json` eliminados.
- `.gitignore`: reemplazado `src-tauri/target/` por `/target/` del workspace.

### Removed

- Crate `midway` (Tauri) completo: `src-tauri/` (`Cargo.toml`, `build.rs`,
  `tauri.conf.json` y variantes, `capabilities/`, `gen/`, `icons/`, fuentes).
- Frontend TypeScript/React: `src/`, `index.html`, `package.json`,
  `package-lock.json`, `tsconfig.json`, `vite.config.ts`, `vitest.config.ts`,
  `tests/ui/`.
- Tests `.mjs` obsoletos que verificaban el stack anterior (`tests/qa.test.mjs`).

## [0.1.0] - Initial

- Versión inicial de Midway sobre Tauri + React/TypeScript.

[Unreleased]: https://github.com/BaldrVivaldelli/Midway/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/BaldrVivaldelli/Midway/releases/tag/v0.1.0
