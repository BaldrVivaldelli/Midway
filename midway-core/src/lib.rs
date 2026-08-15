//! `midway-core`: dominio, infraestructura y runtime de Midway, independientes de Tauri.
//!
//! Movido desde `src-tauri/src/` (Fase 0, Tarea 1.2) sin reescritura de lógica interna.
//! `midway-desktop` y `midway` (Tauri) consumen estos módulos directamente.

pub mod app;
pub mod domain;
pub mod infra;
pub mod runtime;
