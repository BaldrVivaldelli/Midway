# Verificación empírica de las cuatro dependencias de sistema (tarea 14.1)

> Archivo de trabajo. Contiene el hallazgo por dependencia y la fuerza de la evidencia.
> La tarea 14.2 transcribe este resultado a `docs/product-audit.md` (Req 10.4, 10.5).
>
> `.github/workflows/ci.yml` **no fue modificado**: la tarea 14.1 mantiene las cuatro
> dependencias en el workflow mientras la verificación no esté completa, y la eliminación
> no está autorizada acá. Lo que sigue es una **propuesta**, no un cambio aplicado.

Sonda: `docs/baseline-evidence/system_dep_probe.sh` (auxiliar:
`system_dep_links.py`). Salida cruda: `09-system-dep-probe.txt`.
Fecha de la corrida: 2026-08-21.

## El experimento que se pudo ejecutar no es el que describe la tarea

La tarea 14.1 pide quitar una dependencia a la vez de `ci.yml` y observar `cargo check` y
`cargo test`. Ese experimento **no se ejecutó y no es ejecutable en este entorno**:

- Requiere una corrida de CI, y por lo tanto un commit y un push. El Req 2.1-2.3 los
  prohíbe, así que ninguna edición de `ci.yml` puede llegar a un runner.
- El runner es `ubuntu-22.04`. Esta máquina es NixOS 26.05, sin `dpkg` ni `apt`, por lo
  que no puede reproducir ni el estado "con las cuatro instaladas por apt" ni el estado
  "con tres instaladas por apt".

Lo que sí se ejecutó es el experimento complementario, que subsume los cuatro casos de
"falta una" en esta plataforma: **`cargo check --workspace` y `cargo test --workspace`
con las cuatro dependencias ausentes a la vez**. Si el workspace compila y testea sin
ninguna de las cuatro, no puede necesitar ninguna de ellas por separado.

La ausencia se verificó antes de correr los comandos; sin ese paso un exit 0 no probaría
nada (paso 1 de la sonda):

| Dependencia | Comprobación de ausencia | Resultado |
| --- | --- | --- |
| `libwebkit2gtk-4.1-dev` | `pkg-config` sobre `webkit2gtk-4.1`, `webkit2gtk-4.0`, `javascriptcoregtk-4.1`, `libsoup-3.0`; `ldconfig -p`; `/usr/lib/x86_64-linux-gnu`; `~/.nix-profile/lib` | ausente en todas |
| `libappindicator3-dev` | `pkg-config` sobre `appindicator3-0.1` y `ayatana-appindicator3-0.1`; mismas rutas de bibliotecas | ausente en todas |
| `librsvg2-dev` | `pkg-config` sobre `librsvg-2.0`; mismas rutas de bibliotecas | ausente en todas |
| `patchelf` | `command -v patchelf` | ausente del `PATH` |

`dpkg` no existe en la máquina, así que ninguno de los cuatro **paquetes Debian** puede
estar instalado como tal. Hay derivaciones de `webkitgtk`, `librsvg` y `patchelf` en
`/nix/store`, pero no están en el perfil del usuario ni en `PKG_CONFIG_PATH` ni en
`PATH`, por lo que el build no las alcanza. La única `PKG_CONFIG_PATH` exportada es la de
dbus + sqlite ya registrada en `docs/product-audit.md` §3.8.

Resultado con las cuatro ausentes:

| Comando | Exit code | Resultado |
| --- | --- | --- |
| `cargo check --workspace` | 0 | pasó, sin advertencias |
| `cargo test --workspace` | 0 | 363 tests, 0 fallos, 0 ignorados |

Desglose de los 363: 54 (`midway-core` lib) + 1 (`no_tauri_dependency`) +
4 (`response_body_limit`) + 2 (`workspace_snapshot_folders`) +
299 (`midway-desktop`) + 3 (`vertical_import_guard`) + 0 doc-tests. Coincide con la
línea base vigente de 363 tests, así que la ausencia de las cuatro dependencias no
cambió el conteo ni el resultado de ningún test.

## Evidencia estructural del grafo de dependencias

Un crate solo puede depender de una biblioteca de sistema declarando `links` o emitiendo
flags desde su build script. Sobre el grafo resuelto con `--all-features` (543 paquetes),
solo **5** declaran `links`:

| Paquete | `links` |
| --- | --- |
| `libdbus-sys` | `dbus` |
| `libsqlite3-sys` | `sqlite3` |
| `objc-sys` | `objc_0_3` (solo macOS) |
| `ring` | `ring_core_0_17_14_` (código propio, vendorizado) |
| `wasm-bindgen-shared` | `wasm_bindgen` (solo wasm) |

Ninguna de las cuatro dependencias del workflow aparece. Las dos bibliotecas de sistema
que el workspace realmente necesita en Linux son `dbus-1` y `sqlite3`, y **ninguna de las
dos figura en el paso de instalación de `ci.yml`**: llegan por la imagen base de
`ubuntu-22.04`.

La búsqueda de crates de la pila WebKit/GTK/Tauri en `Cargo.lock`
(`webkit`, `soup`, `javascriptcore`, `appindicator`, `rsvg`, `gtk`, `gdk`, `glib`,
`gobject`, `cairo`, `pango`, `atk`, `tauri`, `wry`, `tao`, `tray`) no arroja **ninguna**
coincidencia. Es consistente con el test `no_tauri_dependency` del baseline: la migración
de Tauri a iced dejó las cuatro dependencias del workflow sin ningún consumidor en el
grafo. iced 0.14 renderiza con wgpu/tiny-skia sobre winit, no con WebKit ni GTK.

## Hallazgo por dependencia

| Dependencia | Necesaria para `cargo check` / `cargo test` | Fuerza de la evidencia | Propuesta |
| --- | --- | --- | --- |
| `libwebkit2gtk-4.1-dev` | **No** | Alta. Ausencia verificada por `pkg-config` en cuatro módulos y por las rutas de bibliotecas; check y test en exit 0; sin ningún crate de la pila WebKit en el grafo; era una dependencia de Tauri, y Tauri ya no está | Proponer eliminación |
| `libappindicator3-dev` | **No** | Alta. Misma verificación de ausencia; sin crate de bandeja/indicador en el grafo. El producto no tiene icono de bandeja: ningún módulo del workspace lo implementa | Proponer eliminación |
| `librsvg2-dev` | **No** | Alta para check/test. Ausencia verificada; sin crate que la enlace. Los íconos de empaquetado son PNG/ICNS/ICO (`[package.metadata.packager] icons`), no SVG | Proponer eliminación, con la reserva de empaquetado de abajo |
| `patchelf` | **No** | Alta para check/test, y además por naturaleza de la herramienta: `patchelf` es un binario que reescribe `RPATH`/intérprete de un ELF **ya compilado**. `cargo check` no produce ELF y `cargo test` no lo post-procesa. Ningún build script del grafo lo invoca (check y test pasan sin él en `PATH`) | Conservar por empaquetado, no por compilación (ver abajo) |

## Lo que queda sin verificar y por qué

1. **No hubo corrida de `ubuntu-22.04`.** La evidencia de arriba es de NixOS. El grafo de
   Cargo es idéntico en las dos máquinas, y es el grafo el que determina qué bibliotecas
   de sistema se necesitan, pero una diferencia propia de la imagen del runner no queda
   descartada por esta sonda. Cerrarlo requiere exactamente lo que la tarea 14.1 describe
   y este entorno no puede hacer: cuatro corridas de CI, una por dependencia.
2. **No se ejecutó `cargo packager`.** El Req 10.5 conserva una dependencia si es
   necesaria para compilar, testear **o empaquetar**. El empaquetado no se midió, y es la
   razón por la que `patchelf` no se propone para eliminación: el job `build` de
   `release.yml` genera AppImage, y las herramientas de AppImage usan `patchelf` para
   corregir el `RPATH` del binario desplegado
   ([linuxdeployqt/BUILDING.md](https://github.com/probonopd/linuxdeploy), [linuxdeploy #149](https://github.com/linuxdeploy/linuxdeploy/issues/149)).
   No se verificó acá que `cargo-packager 0.11.8` lo invoque; se registra como
   **presunción no verificada** que basta para conservarlo. Lo mismo, con menos fuerza,
   para `librsvg2-dev`: la ruta de `deb`/AppImage podría convertir íconos.
   *Contenido reformulado para cumplir con las restricciones de licencia de las fuentes.*
3. **`cargo check` y `cargo test` son headless.** No abren ventana y no inicializan el
   backend gráfico, así que no dicen nada sobre bibliotecas necesarias en **tiempo de
   ejecución**. Para esta pregunta igual son irrelevantes las cuatro: winit/wgpu no
   consumen WebKit ni appindicator.

## Estado de la verificación

Incompleta según la letra de la tarea 14.1: falta el experimento por dependencia en CI y
falta la prueba de empaquetado. Por lo tanto **las cuatro dependencias permanecen en
`.github/workflows/ci.yml`** (y en `release.yml`, que repite el mismo paso). La propuesta
de eliminar tres de las cuatro queda registrada, sin aplicar.
