# Capturas crudas de la Compuerta_Evidencia (tarea 1.1)

> Archivo de trabajo. Contiene la salida cruda capturada al ejecutar la
> Compuerta_Evidencia sobre el baseline sin cambios. La tarea 1.2 transcribe esta
> evidencia a `docs/product-audit.md` (secciones 1 a 4).
>
> Ninguna operación de git modificó el árbol de trabajo. Solo se ejecutaron comandos de
> lectura: `git rev-parse`, `git rev-list --left-right --count`, `git status --short`.

Fecha de captura: 2026-08-15 (hora local del entorno de ejecución).

## Identidad del baseline (comandos de git de solo lectura)

| Comando | Salida |
| --- | --- |
| `git rev-parse --abbrev-ref HEAD` | `main` |
| `git rev-parse HEAD` | `1fc270326e5c304f24ce08fe1f528da33a007d3e` |
| `git rev-parse origin/main` | `1fc270326e5c304f24ce08fe1f528da33a007d3e` |
| `git rev-list --left-right --count HEAD...origin/main` | `0       0` |
| `git status --short` | (salida vacía: árbol limpio en el momento de la captura) |

## Entorno de ejecución (Req 1.8, 1.9)

| Dato | Valor observado |
| --- | --- |
| `rustc --version` | `rustc 1.95.0 (59807616e 2026-04-14) (built from a source tarball)` |
| `cargo --version` | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| `command -v rustup` | sin salida: `rustup` **ausente** |
| iced en `Cargo.lock` | `name = "iced"` / `version = "0.14.0"` (línea 1608-1610). Subcrates en `0.14.0` salvo `iced_widget` en `0.14.2` |
| iced en `Cargo.toml` | `iced = { version = "=0.14.0", ... }` (pin exacto) |
| `DISPLAY` | **no definida** en el entorno (`env` no la lista) |
| `WAYLAND_DISPLAY` | `wayland-1` |
| `XDG_SESSION_TYPE` | `wayland` (dato auxiliar) |

Binarios de herramienta buscados en `PATH`, todos `NOT FOUND`: `rustfmt`, `cargo-fmt`,
`clippy-driver`, `cargo-clippy`, `cargo-deny`, `cargo-audit`, `cargo-llvm-cov`.

`pkg-config` sí está presente: `/home/avivaldelli/.nix-profile/bin/pkg-config`.
No existe `dbus-1.pc` ni en `/usr/lib/x86_64-linux-gnu/pkgconfig/`, ni en
`/usr/lib/pkgconfig/`, ni en `/usr/share/pkgconfig/`, y `PKG_CONFIG_PATH` no está
definida.

## Tabla de resultados

Ejecución en dos condiciones, ambas sin tocar código:

- **Entorno tal cual**: sin `PKG_CONFIG_PATH`. Es la condición por defecto del entorno.
- **Con `PKG_CONFIG_PATH`**: variable de entorno apuntando a los `pkgconfig` de dbus y
  sqlite del nix store. Es un ajuste **solo de entorno**, sin diff de código, sin
  cambios en `Cargo.toml` ni en el `Makefile`.

| # | Comando textual | Estado | Exit code | Archivo | Clasificación |
| --- | --- | --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | `Sin_Herramienta` | 101 | `01-cargo-fmt.txt` | no bloquea la ejecución del baseline |
| 2 | `cargo check --workspace --all-targets --all-features` (entorno tal cual) | `Ejecutado` — falló | 101 | `02-cargo-check.txt` | bloquea la ejecución del baseline |
| 2b | `cargo check --workspace` (equivalente a CI, entorno tal cual) | `Ejecutado` — falló | 101 | `02b-cargo-check-plain.txt` | bloquea la ejecución del baseline |
| 2c | `cargo check --workspace --all-targets --all-features` con `PKG_CONFIG_PATH` | `Ejecutado` — pasó | 0 | `02c-cargo-check-with-pkgconfig.txt` | n/a |
| 3 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | `Sin_Herramienta` | 101 | `03-cargo-clippy.txt` | no bloquea la ejecución del baseline |
| 4 | `cargo test --workspace --all-features` (entorno tal cual) | `Ejecutado` — falló | 101 | `04-cargo-test.txt` | bloquea la ejecución del baseline |
| 4b | `cargo test --workspace --all-features` con `PKG_CONFIG_PATH` solo de dbus | `Ejecutado` — falló | 101 | `04b-cargo-test-with-pkgconfig.txt` | bloquea la ejecución del baseline |
| 4c | `cargo test --workspace --all-features` con `PKG_CONFIG_PATH` de dbus + sqlite | `Ejecutado` — pasó | 0 | `04c-cargo-test-with-dbus-and-sqlite.txt` | n/a |
| 5 | `cargo doc --workspace --no-deps` (entorno tal cual) | `Ejecutado` — falló | 101 | `05-cargo-doc.txt` | bloquea la ejecución del baseline |
| 5b | `cargo doc --workspace --no-deps` con `PKG_CONFIG_PATH` | `Ejecutado` — pasó | 0 | `05b-cargo-doc-with-pkgconfig.txt` | n/a |
| 6 | `cargo deny check` | `Sin_Herramienta` | 101 | `06-cargo-deny.txt` | no bloquea la ejecución del baseline |
| 7 | `cargo audit` | `Sin_Herramienta` | 101 | `07-cargo-audit.txt` | no bloquea la ejecución del baseline |
| 8 | `cargo llvm-cov --workspace` | `Sin_Herramienta` | 101 | `08-cargo-llvm-cov.txt` | no bloquea la ejecución del baseline |

No se afirma que ninguna verificación `Sin_Herramienta` haya pasado (Req 1.4).

## Salidas cortas transcritas

### 1. `cargo fmt --all -- --check` → `Sin_Herramienta`

```
error: no such command: `fmt`

help: a command with a similar name exists: `fix`

help: view all installed commands with `cargo --list`
help: find a package to install `fmt` with `cargo search cargo-fmt`
```
Exit code: 101. Razón: `rustfmt`/`cargo-fmt` no instalados (rustc 1.95.0 desde tarball,
sin `rustup` para agregar componentes).

### 3. `cargo clippy --workspace --all-targets --all-features -- -D warnings` → `Sin_Herramienta`

```
error: no such command: `clippy`

help: view all installed commands with `cargo --list`
help: find a package to install `clippy` with `cargo search cargo-clippy`
```
Exit code: 101. Razón: `cargo-clippy`/`clippy-driver` no instalados.

### 6. `cargo deny check` → `Sin_Herramienta`

```
error: no such command: `deny`

help: a command with a similar name exists: `bench`

help: view all installed commands with `cargo --list`
help: find a package to install `deny` with `cargo search cargo-deny`
```
Exit code: 101.

### 7. `cargo audit` → `Sin_Herramienta`

```
error: no such command: `audit`

help: a command with a similar name exists: `add`

help: view all installed commands with `cargo --list`
help: find a package to install `audit` with `cargo search cargo-audit`
```
Exit code: 101.

### 8. `cargo llvm-cov --workspace` → `Sin_Herramienta`

```
error: no such command: `llvm-cov`

help: view all installed commands with `cargo --list`
help: find a package to install `llvm-cov` with `cargo search cargo-llvm-cov`
```
Exit code: 101.

## Fallo bloqueante observado: bibliotecas de sistema ausentes

`cargo check`, `cargo test` y `cargo doc` fallan en el entorno tal cual, **antes** de
compilar código de Midway. El fallo es de entorno, no de código:

1. `libdbus-sys v0.2.7` (build script) aborta porque `pkg-config` no encuentra `dbus-1`:

   ```
   error: failed to run custom build command for `libdbus-sys v0.2.7`
   ...
     The system library `dbus-1` required by crate `libdbus-sys` was not found.
     The file `dbus-1.pc` needs to be installed and the PKG_CONFIG_PATH environment
     variable must contain its parent directory.
     The PKG_CONFIG_PATH environment variable is not set.
   ...
     thread 'main' (212058) panicked at .../libdbus-sys-0.2.7/build.rs:25:9:
     explicit panic
   ```

   Cadena de dependencia: `midway-core` → `keyring 3` (feature
   `sync-secret-service`) → `dbus-secret-service` → `dbus` → `libdbus-sys`.

2. Con `dbus-1.pc` resuelto, el enlazado de los targets de test falla por sqlite:

   ```
   /nix/store/.../bin/ld.bfd: cannot find -lsqlite3: No such file or directory
   collect2: error: ld returned 1 exit status
   ```

   Cadena de dependencia: `midway-core` → `tokio-rusqlite 0.7` → `rusqlite 0.37` →
   `libsqlite3-sys 0.35` (enlazado dinámico contra `sqlite3` del sistema).

Con ambas rutas de `pkgconfig` presentes en `PKG_CONFIG_PATH`, los tres comandos pasan
sin advertencias de compilación:

```
PKG_CONFIG_PATH=/nix/store/7vxs654j460qbxf3idgzwb92g4zwlbwi-dbus-1.16.2-dev/lib/pkgconfig:\
/nix/store/w1cmsd5kg0jsj76cxj1vgfrsw3gk86yh-sqlite-3.51.2-dev/lib/pkgconfig
```

Consecuencia para el Req 1.7: **no se requiere corrección de código.** La causa es la
ausencia de `dbus-1.pc` y de `sqlite3` en el entorno, resuelta con una variable de
entorno. La tarea 1.2 debe registrar esto como fallo de entorno con su ajuste de
entorno, no como diff de baseline.

## Resultado de `cargo test --workspace --all-features` (condición 4c, exit 0)

```
     Running unittests src/lib.rs (target/debug/deps/midway_core-98c90995f41ebeb4)
running 51 tests
test result: ok. 51 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s

     Running tests/no_tauri_dependency.rs (target/debug/deps/no_tauri_dependency-33fecb7989998439)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s

     Running tests/response_body_limit.rs (target/debug/deps/response_body_limit-68e7f929c5c2eabb)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/workspace_snapshot_folders.rs (target/debug/deps/workspace_snapshot_folders-b171634cef9bc2ef)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

     Running unittests src/main.rs (target/debug/deps/midway_desktop-8257886440ed64cb)
running 243 tests
test result: ok. 243 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 49.33s

   Doc-tests midway_core
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Total: 301 tests, 0 fallos, 0 ignorados. La ejecución es headless (sin `DISPLAY`), por lo
que no cubre verificación gráfica. Ninguna advertencia de compilación en las corridas
2c, 4c y 5b.

## Resultado de `cargo doc --workspace --no-deps` (condición 5b, exit 0)

```
 Documenting midway-core v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-core)
 Documenting midway-desktop v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-desktop)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.42s
   Generated /home/avivaldelli/projects/personal/Midway/target/doc/midway_core/index.html and 1 other file
```

## Scripts auxiliares de medición

Quedan en este directorio para que cada número citado en los documentos de auditoría y en
los ADRs sea reproducible con un comando, en vez de depender de un conteo manual.

| Script | Qué mide | Dónde se usa |
| --- | --- | --- |
| `count_test_lines.py` | Separa las líneas dentro de bloques `#[cfg(test)]` de las de producción | `product-audit.md` §5; ADR 0004 |
| `count_variants.py` | Variantes y rango de líneas de cada enum de mensajes | `product-audit.md` §6; ADR 0004 |
| `count_fns.py` | Recuento de funciones por archivo | `product-audit.md` §5 |
| `fn_span.py` | Rango de líneas de funciones nombradas de un archivo Rust | ADR 0003, ADR 0004 |
| `fn_inventory.py` | Inventario de todas las funciones de producción de nivel superior de un archivo, con su rango | ADR 0004 |
| `attribute_app_rs.py` | Atribuye cada función de producción de `app.rs` a un grupo del `Message` raíz o a la raíz de composición. **Falla si alguna función queda sin clasificar**, así que la tabla no puede desalinearse del fuente en silencio | ADR 0004 §"Atribución de líneas de producción por grupo" |
| `verify_adr_citations.py` | Comprueba las 61 citas `archivo:línea` de los ADRs 0003 y 0004 contra el fuente | ADRs 0003 y 0004 |
| `verify_feature_matrix.py` | Comprueba la forma de `feature-matrix.md`: once columnas exactas en cada tabla de la matriz, estados solo de la lista cerrada, `No_Verificable_En_Entorno` en toda la columna de verificación manual, ninguna fila en `Verified` y ningún porcentaje en las filas | `feature-matrix.md` (Req 4.1-4.5, 4.8) |
| `subgraph_probe.py` | Recorre `resolve.nodes` de `cargo metadata` y cuenta el subgrafo transitivo de cada miembro del workspace, reportando los paquetes iced y tauri alcanzables | `architecture.md` §1.4 y §4.1 (Req 11.2) |
| `state_mutation_map.py` | Atribuye cada mutación de un campo de `Midway` a la función de producción que la contiene, excluyendo los módulos `#[cfg(test)]` | `architecture.md` §2.1 y §4.2 (Req 3.3) |
| `count_unwrap_prod.py` | Cuenta las ocurrencias de `unwrap()` / `expect(` **fuera** de los módulos `#[cfg(test)]`, con archivo y línea. Distingue el recuento de producción del recuento por archivo de `L21`, que incluye los tests alojados en esos mismos archivos | `product-principles.md` §P7 (Req 11.7) |
| `system_dep_probe.sh` | Verifica que las cuatro dependencias de sistema de `ci.yml` (`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`) están **ausentes** de la máquina y corre `cargo check --workspace` y `cargo test --workspace` en esa condición. Salida cruda en `09-system-dep-probe.txt`; hallazgo por dependencia en `09b-system-dep-findings.md` | tarea 14.1 (Req 10.4, 10.5) |
| `system_dep_links.py` | Lista las declaraciones `links` del grafo resuelto de `cargo metadata`, es decir las únicas bibliotecas de sistema que el workspace puede requerir de forma declarada | auxiliar de `system_dep_probe.sh` |

Salida de `attribute_app_rs.py` en el HEAD de la sección 1:

```
midway-desktop/src/app.rs: total=10750 test=5140 produccion=5610

Grupo   Lineas atribuidas
RequestComposer 1174
Workspace       672
WorkspaceCrud   349
ActivityBar     128
PanelResize     122
Palette 116
Keyboard        81
Runner  75
Session 49
TopBar  41
ResponseInspector       12
Theme   9
Updater 1
Tick    1
Tree    0

Raiz de composicion (funciones) 560
Total atribuido a grupos        2830
Produccion no atribuida (raiz + enums + structs + imports)      2780

Subgrupo        Lineas atribuidas
Workspace / environments        161
Workspace / history     10
Workspace / export-import       344
Workspace / carga de snapshot   43
Workspace / router      114
RequestComposer / ciclo de vida de ejecucion    204
RequestComposer / persistencia de request       296
```

Salida de `verify_adr_citations.py`:

```
citas verificadas: 61
todas las citas coinciden con el fuente
```

Salida de `count_unwrap_prod.py` en el HEAD de la sección 1:

```
total fuera de #[cfg(test)]: 8

midway-desktop/src/ui/activity_bar.rs: 5
  71: let first = chars.next().unwrap();
  81: let c1 = words[0].chars().next().unwrap().to_uppercase().next().unwrap();
  82: let c2 = words[1].chars().next().unwrap().to_uppercase().next().unwrap();
  86: let c1 = first.to_uppercase().next().unwrap();
  87: let c2 = trimmed.chars().nth(1).unwrap().to_lowercase().next().unwrap();

midway-desktop/src/app.rs: 2
  1239: .expect("el mutex del ProgressReceiverHandle no debería estar envenenado")
  1716: /// `.unwrap()`, `.expect("...")`, aserciones) tienen un payload `&str` o

midway-desktop/src/main.rs: 1
  70: .expect("No se pudo inicializar el estado de la aplicación (AppState).");
```

La ocurrencia de `app.rs:1716` es un doc-comment que menciona `.unwrap()` al describir el
manejo de payloads de pánico, no una llamada. El recuento efectivo de llamadas de
producción es **7**.
