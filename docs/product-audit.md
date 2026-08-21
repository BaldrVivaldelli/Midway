# Auditoría del producto — Midway

> Documento de auditoría del baseline. Registra el estado real del código, no el que
> declara el README.
>
> Alcance de este archivo al cierre de la tarea 2.1: secciones 1 a 4 (Compuerta de
> evidencia, tarea 1.2) más secciones 5 a 11 y 14 (deuda técnica, costuras de extracción,
> deuda de representación de respuesta, migración a `xtask`, deuda de proceso,
> discrepancias del README, riesgos y plataformas de primer nivel). La estructura completa
> tiene catorce secciones: la 12 (reducción de líneas de `app.rs`) la escribe la tarea 8.2
> y la 13 (afirmaciones no verificadas) la tarea 15.2. Las tareas 13.2 y 14.2 agregan el
> registro de `Sin_Herramienta` del gate local y la tabla de dependencias del sistema. Los
> encabezados de las secciones pendientes todavía no existen acá: no se declaran vacíos ni
> se anticipan conclusiones.
>
> **Con la sección 13, escrita por la tarea 15.2, la estructura de catorce secciones está
> completa.** La 13 no reescribe ninguna sección anterior: recoge en un solo lugar las ocho
> afirmaciones que el Req 14.5 prohíbe sin evidencia (§13.1), catorce afirmaciones sin
> evidencia reproducible registradas como no verificadas (§13.2), el alcance real de la
> comparación de advertencias nuevas del Req 13.3 (§13.3) y la referencia cruzada de las
> brechas medidas, que son otra categoría (§13.4). Dos resultados negativos quedan
> declarados ahí: la mitad de lint del Req 13.3 **no tiene comparación posible** porque el
> baseline nunca produjo una salida de clippy y el job `static-checks` todavía no corrió, y
> la advertencia de rustdoc de `ui/empty_state.rs:200` registrada en §3.17 es **nueva
> respecto de §3.10** y quedó sin corregir.
>
> La tarea 4.8 agregó dos subsecciones a la sección 3, sin reescribir ninguna existente:
> §3.14 con la reejecución de `cargo test --workspace --all-features` posterior a los
> Test_Caracterización de las tareas 4.1 a 4.7 (323 tests, 0 fallos, exit 0) y su delta
> traceable respecto de los 301 de §3.8, y §3.15 con la reejecución de `cargo fmt` y
> `cargo clippy`, que **siguen en `Sin_Herramienta`** en este entorno.
>
> La tarea 7.3 agregó §3.16 con la reejecución de `cargo test --workspace --all-features`
> posterior al cableado de la vertical Tema/Ajustes (327 tests, 0 fallos, exit 0) y su
> delta traceable respecto de los 323 de §3.14. No reescribe §3.14 ni §3.15.
>
> La tarea 16 (Checkpoint final) agregó §3.19 con la reejecución de la Compuerta_Evidencia al
> cierre del spec: `cargo check`, `cargo test` y `cargo doc` en exit 0, **363 tests, 0
> fallos**, delta cero respecto de §3.17 en los siete binarios, la misma advertencia de
> rustdoc sin corregir, el resultado de los dos validadores de documentación
> (`verify_feature_matrix.py` en verde, `verify_adr_citations.py` en rojo conocido con 55 de
> 61 citas caducas) y la referencia cruzada de lo que queda abierto. No reescribe ninguna
> subsección anterior y no modificó ningún `.rs`.
>
> La tarea 8.2 agregó la sección 12 con el recuento de líneas de `app.rs` antes y después
> de la extracción de la vertical, y el tamaño de `src/ui/theme_settings.rs`. **El
> resultado es que no hubo reducción**: §12 lo registra como brecha con hito propietario, no
> como cumplimiento del Req 8.9. La sección 5 no se reescribe; §12.2 anota la diferencia de
> una línea entre los dos métodos de split y por qué no afecta el delta.
>
> La tarea 14.2 agregó §14.1 con el resultado por dependencia del paso de instalación de
> dependencias del sistema de `ci.yml`. No reescribe la tabla de plataformas de §14 ni su
> Deuda_Registrada. **El experimento que pide el Req 10.4 (quitar una dependencia a la vez
> en CI) no se ejecutó**: requiere commit y push, prohibidos por el Req 2.1-2.3. §14.1
> registra el experimento complementario que sí se corrió, con las cuatro dependencias
> ausentes a la vez, y delimita qué establece y qué no. Ninguna dependencia fue eliminada.
>
> La evidencia cruda que sustenta las secciones 1 a 4 está en
> `docs/baseline-evidence/` (13 capturas `.txt` más `INDEX.md`). Esas capturas son el
> registro textual autoritativo; este documento las transcribe.
>
> Método de las secciones 5 a 11 y 14: cada número se midió sobre el árbol de trabajo en
> el HEAD de la sección 1 con comandos de solo lectura, y se registra el **valor
> observado**. Donde el valor observado difiere del que anticipaba el spec, la diferencia
> queda anotada de forma explícita en el mismo lugar donde aparece el dato. Los scripts
> auxiliares usados para medir (`count_test_lines.py`, `count_variants.py`,
> `count_fns.py`) quedan en `docs/baseline-evidence/` para que las mediciones sean
> reproducibles.
>
> Tres discrepancias con el spec quedaron registradas en su lugar y conviene anticiparlas
> acá porque contradicen afirmaciones repetidas en otros documentos: **`scripts/` sí
> existe** y está versionado con siete archivos (§8; el spec, `L7` de
> `docs/known-limitations.md` y `docs/architecture.md:36` afirman que no existe);
> **el pipeline de release depende de Node.js** para tres scripts `.mjs` (§8, §9, D1); y
> de los quince grupos del `Message` raíz **solo doce son costuras de extracción reales**
> (§6). Los siete recuentos de líneas del inventario de deuda, en cambio, coinciden
> exactamente con los del spec.

## 1. Identidad del baseline

Fecha de captura: 2026-08-15 (hora local del entorno de ejecución).

| Dato | Comando de solo lectura | Valor observado |
| --- | --- | --- |
| Rama | `git rev-parse --abbrev-ref HEAD` | `main` |
| HEAD | `git rev-parse HEAD` | `1fc270326e5c304f24ce08fe1f528da33a007d3e` |
| Remoto | `git rev-parse origin/main` | `1fc270326e5c304f24ce08fe1f528da33a007d3e` |
| Divergencia | `git rev-list --left-right --count HEAD...origin/main` | `0 0` (dos ceros separados por tabulación) |
| Árbol de trabajo | `git status --short` | salida vacía: limpio, sin cambios locales al momento de la captura |

`main` es la única fuente de verdad autorizada. `HEAD` y `origin/main` apuntan al mismo
commit, por lo que no hay commits locales por delante ni por detrás del remoto.

Ninguna operación de git modificó el árbol de trabajo durante la captura. Solo se
ejecutaron `git rev-parse`, `git rev-list --left-right --count` y `git status --short`.
No se crearon commits, no se hizo push, no se cambió de rama y no se ejecutó ninguna
operación destructiva.

## 2. Entorno de ejecución

| Dato | Valor observado |
| --- | --- |
| `rustc --version` | `rustc 1.95.0 (59807616e 2026-04-14) (built from a source tarball)` |
| `cargo --version` | `cargo 1.95.0 (f2d3ce0bd 2026-03-21)` |
| `command -v rustup` | sin salida: `rustup` **ausente** |
| iced en `Cargo.lock` | `name = "iced"` / `version = "0.14.0"`; subcrates en `0.14.0` salvo `iced_widget 0.14.2` |
| iced en `midway-desktop/Cargo.toml` | `iced = { version = "=0.14.0", features = ["tokio", "highlighter", "tiny-skia"] }` (pin exacto) más `iced_highlighter = "=0.14.0"` |
| `DISPLAY` | **no definida** en el entorno (`env` no la lista) |
| `WAYLAND_DISPLAY` | `wayland-1` |
| `XDG_SESSION_TYPE` | `wayland` (dato auxiliar, fuera del Req 1.9) |

Precisión sobre `DISPLAY`: la nota de contexto del spec la describía como "vacía". Lo
observado es más fuerte que eso: la variable **no está definida**. La consecuencia
práctica es la misma —no hay display X11 utilizable— pero se registra el valor observado
y no la expectativa.

Consecuencia de `rustup` ausente: rustc 1.95.0 vino de un tarball, así que no hay
mecanismo de componentes para agregar `rustfmt` ni `clippy`. Binarios buscados en `PATH`,
todos `NOT FOUND`: `rustfmt`, `cargo-fmt`, `clippy-driver`, `cargo-clippy`, `cargo-deny`,
`cargo-audit`, `cargo-llvm-cov`.

`pkg-config` sí está presente (`/home/avivaldelli/.nix-profile/bin/pkg-config`), pero no
existe `dbus-1.pc` en `/usr/lib/x86_64-linux-gnu/pkgconfig/`, `/usr/lib/pkgconfig/` ni
`/usr/share/pkgconfig/`, y `PKG_CONFIG_PATH` no está definida. Esto explica el fallo
bloqueante de la sección 3.

Consecuencia para toda verificación gráfica: sin `DISPLAY` y con la app siendo un binario
iced de escritorio, la verificación manual con interfaz gráfica no se realizó. Se registra
como `No_Verificable_En_Entorno` en `docs/feature-matrix.md`. No se afirma haber visto
ninguna pantalla.

## 3. Tabla de la Compuerta_Evidencia

La Compuerta_Evidencia se ejecutó sobre el baseline sin cambios, en dos condiciones, y
ninguna de las dos tocó código:

- **Entorno tal cual**: sin `PKG_CONFIG_PATH`. Es la condición por defecto del entorno y
  la que define el estado del baseline en esta máquina.
- **Con `PKG_CONFIG_PATH`**: variable de entorno apuntando a los directorios `pkgconfig`
  de dbus y sqlite del nix store. Es un ajuste **solo de entorno**: sin diff de código,
  sin cambios en `Cargo.toml`, sin cambios en el `Makefile`.

Estados admitidos: `Ejecutado`, `Sin_Herramienta`. Clasificación de fallos:
`bloquea la ejecución del baseline` / `no bloquea la ejecución del baseline`.

| # | Comando textual | Estado | Exit | Salida registrada | Clasificación |
| --- | --- | --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | `Sin_Herramienta` | 101 | 3.1 / `01-cargo-fmt.txt` | no bloquea la ejecución del baseline |
| 2 | `cargo check --workspace --all-targets --all-features` (entorno tal cual) | `Ejecutado` — falló | 101 | 3.2 / `02-cargo-check.txt` | bloquea la ejecución del baseline |
| 2b | `cargo check --workspace` (equivalente al de CI, entorno tal cual) | `Ejecutado` — falló | 101 | 3.3 / `02b-cargo-check-plain.txt` | bloquea la ejecución del baseline |
| 2c | `cargo check --workspace --all-targets --all-features` con `PKG_CONFIG_PATH` | `Ejecutado` — pasó | 0 | 3.4 / `02c-cargo-check-with-pkgconfig.txt` | n/a (no falló) |
| 3 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | `Sin_Herramienta` | 101 | 3.5 / `03-cargo-clippy.txt` | no bloquea la ejecución del baseline |
| 4 | `cargo test --workspace --all-features` (entorno tal cual) | `Ejecutado` — falló | 101 | 3.6 / `04-cargo-test.txt` | bloquea la ejecución del baseline |
| 4b | `cargo test --workspace --all-features` con `PKG_CONFIG_PATH` solo de dbus | `Ejecutado` — falló | 101 | 3.7 / `04b-cargo-test-with-pkgconfig.txt` | bloquea la ejecución del baseline |
| 4c | `cargo test --workspace --all-features` con `PKG_CONFIG_PATH` de dbus + sqlite | `Ejecutado` — pasó | 0 | 3.8 / `04c-cargo-test-with-dbus-and-sqlite.txt` | n/a (no falló) |
| 5 | `cargo doc --workspace --no-deps` (entorno tal cual) | `Ejecutado` — falló | 101 | 3.9 / `05-cargo-doc.txt` | bloquea la ejecución del baseline |
| 5b | `cargo doc --workspace --no-deps` con `PKG_CONFIG_PATH` | `Ejecutado` — pasó | 0 | 3.10 / `05b-cargo-doc-with-pkgconfig.txt` | n/a (no falló) |
| 6 | `cargo deny check` | `Sin_Herramienta` | 101 | 3.11 / `06-cargo-deny.txt` | no bloquea la ejecución del baseline |
| 7 | `cargo audit` | `Sin_Herramienta` | 101 | 3.12 / `07-cargo-audit.txt` | no bloquea la ejecución del baseline |
| 8 | `cargo llvm-cov --workspace` | `Sin_Herramienta` | 101 | 3.13 / `08-cargo-llvm-cov.txt` | no bloquea la ejecución del baseline |

Declaración explícita (Req 1.4): de los ocho comandos de la Compuerta_Evidencia, **cinco
no se pudieron ejecutar** por herramienta ausente. No se afirma que `cargo fmt`,
`cargo clippy`, `cargo deny`, `cargo audit` ni `cargo llvm-cov` hayan pasado. No hay
evidencia de formato conforme, de ausencia de lints, de licencias auditadas, de ausencia
de vulnerabilidades conocidas ni de cobertura. El estado de esas cinco verificaciones es
**desconocido**, no "bueno".

Por qué los cinco `Sin_Herramienta` se clasifican como
`no bloquea la ejecución del baseline`: ninguno participa en compilar, testear ni
documentar el workspace. Su ausencia impide *verificar* propiedades del código, no
*ejecutar* el baseline. Por eso el Req 10.1 y 10.2 los llevan a CI (tarea 13.1), donde el
toolchain sí tiene los componentes.

Por qué los fallos de 2, 2b, 4, 4b y 5 se clasifican como
`bloquea la ejecución del baseline`: en el entorno tal cual el workspace **no compila**.
Sin `cargo check` no hay build, sin build no hay tests y sin tests no hay línea base de
comportamiento. Es el fallo más severo de la compuerta, y su causa es de entorno, no de
código (sección 4).

### 3.1 `cargo fmt --all -- --check` → `Sin_Herramienta`

```
error: no such command: `fmt`

help: a command with a similar name exists: `fix`

help: view all installed commands with `cargo --list`
help: find a package to install `fmt` with `cargo search cargo-fmt`
```

Exit code: 101. Razón: `rustfmt` / `cargo-fmt` no instalados; rustc 1.95.0 vino de un
tarball y no hay `rustup` para agregar componentes.

### 3.2 `cargo check --workspace --all-targets --all-features` (entorno tal cual) → `Ejecutado`, falló

```
   Compiling ring v0.17.14
   Compiling libdbus-sys v0.2.7
   Compiling libsqlite3-sys v0.35.0
error: failed to run custom build command for `libdbus-sys v0.2.7`

Caused by:
  process didn't exit successfully: `/home/avivaldelli/projects/personal/Midway/target/debug/build/libdbus-sys-bd32125d34c37a1f/build-script-build` (exit status: 101)
  --- stdout
  cargo:rerun-if-changed=build.rs
  cargo:rerun-if-changed=build_vendored.rs
  cargo:rerun-if-env-changed=DBUS_1_NO_PKG_CONFIG
  cargo:rerun-if-env-changed=PKG_CONFIG_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG
  cargo:rerun-if-env-changed=PKG_CONFIG
  cargo:rerun-if-env-changed=DBUS_1_STATIC
  cargo:rerun-if-env-changed=DBUS_1_DYNAMIC
  cargo:rerun-if-env-changed=PKG_CONFIG_ALL_STATIC
  cargo:rerun-if-env-changed=PKG_CONFIG_ALL_DYNAMIC
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_PATH
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_LIBDIR
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_SYSROOT_DIR
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR

  --- stderr
  pkg_config failed: 
  pkg-config exited with status code 1
  > PKG_CONFIG_ALLOW_SYSTEM_LIBS=1 PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 pkg-config --libs --cflags dbus-1 'dbus-1 >= 1.6'

  pkg-config output:
    Package dbus-1 was not found in the pkg-config search path.
    Perhaps you should add the directory containing `dbus-1.pc'
    to the PKG_CONFIG_PATH environment variable
    No package 'dbus-1' found
    Package dbus-1 was not found in the pkg-config search path.
    Perhaps you should add the directory containing `dbus-1.pc'
    to the PKG_CONFIG_PATH environment variable
    No package 'dbus-1' found

  The system library `dbus-1` required by crate `libdbus-sys` was not found.
  The file `dbus-1.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain its parent directory.
  The PKG_CONFIG_PATH environment variable is not set.

  HINT: if you have installed the library, try setting PKG_CONFIG_PATH to the directory containing `dbus-1.pc`.

  One possible solution is to check whether packages
  'libdbus-1-dev' and 'pkg-config' are installed:
  On Ubuntu:
  sudo apt install libdbus-1-dev pkg-config
  On Fedora:
  sudo dnf install dbus-devel pkgconf-pkg-config


  thread 'main' (212058) panicked at /home/avivaldelli/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libdbus-sys-0.2.7/build.rs:25:9:
  explicit panic
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
warning: build failed, waiting for other jobs to finish...
```

Exit code: 101. El fallo ocurre en un build script de dependencia, **antes** de compilar
código de Midway.

### 3.3 `cargo check --workspace` (equivalente al de CI, entorno tal cual) → `Ejecutado`, falló

Mismo fallo, mismo build script, distinto PID y distinto orden de compilación. Se registra
esta corrida aparte porque es el comando exacto que hoy corre `.github/workflows/ci.yml`:
el fallo no es un artefacto de `--all-targets --all-features`.

```
    Checking futures v0.3.32
    Checking naga v27.0.3
   Compiling tiny-xlib v0.2.5
    Checking ring v0.17.14
   Compiling rustls v0.23.41
   Compiling getrandom v0.3.4
   Compiling libdbus-sys v0.2.7
    Checking fancy-regex v0.16.2
error: failed to run custom build command for `libdbus-sys v0.2.7`

Caused by:
  process didn't exit successfully: `/home/avivaldelli/projects/personal/Midway/target/debug/build/libdbus-sys-bd32125d34c37a1f/build-script-build` (exit status: 101)
  --- stdout
  cargo:rerun-if-changed=build.rs
  cargo:rerun-if-changed=build_vendored.rs
  cargo:rerun-if-env-changed=DBUS_1_NO_PKG_CONFIG
  cargo:rerun-if-env-changed=PKG_CONFIG_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG
  cargo:rerun-if-env-changed=PKG_CONFIG
  cargo:rerun-if-env-changed=DBUS_1_STATIC
  cargo:rerun-if-env-changed=DBUS_1_DYNAMIC
  cargo:rerun-if-env-changed=PKG_CONFIG_ALL_STATIC
  cargo:rerun-if-env-changed=PKG_CONFIG_ALL_DYNAMIC
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_PATH
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_LIBDIR
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_SYSROOT_DIR
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR

  --- stderr
  pkg_config failed: 
  pkg-config exited with status code 1
  > PKG_CONFIG_ALLOW_SYSTEM_LIBS=1 PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 pkg-config --libs --cflags dbus-1 'dbus-1 >= 1.6'

  pkg-config output:
    Package dbus-1 was not found in the pkg-config search path.
    Perhaps you should add the directory containing `dbus-1.pc'
    to the PKG_CONFIG_PATH environment variable
    No package 'dbus-1' found
    Package dbus-1 was not found in the pkg-config search path.
    Perhaps you should add the directory containing `dbus-1.pc'
    to the PKG_CONFIG_PATH environment variable
    No package 'dbus-1' found

  The system library `dbus-1` required by crate `libdbus-sys` was not found.
  The file `dbus-1.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain its parent directory.
  The PKG_CONFIG_PATH environment variable is not set.

  HINT: if you have installed the library, try setting PKG_CONFIG_PATH to the directory containing `dbus-1.pc`.

  One possible solution is to check whether packages
  'libdbus-1-dev' and 'pkg-config' are installed:
  On Ubuntu:
  sudo apt install libdbus-1-dev pkg-config
  On Fedora:
  sudo dnf install dbus-devel pkgconf-pkg-config


  thread 'main' (212578) panicked at /home/avivaldelli/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libdbus-sys-0.2.7/build.rs:25:9:
  explicit panic
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
warning: build failed, waiting for other jobs to finish...
```

Exit code: 101.

### 3.4 `cargo check --workspace --all-targets --all-features` con `PKG_CONFIG_PATH` → `Ejecutado`, pasó

```
   Compiling libdbus-sys v0.2.7
    Checking rustls v0.23.41
   Compiling libsqlite3-sys v0.35.0
   Compiling tiny-xlib v0.2.5
   Compiling x11-dl v2.21.0
    Checking dbus v0.9.12
    Checking rusqlite v0.37.0
    Checking softbuffer v0.4.8
    Checking iced_tiny_skia v0.14.0
    Checking iced_renderer v0.14.0
    Checking tokio-rusqlite v0.7.0
    Checking iced_widget v0.14.2
    Checking dbus-secret-service v4.1.0
    Checking keyring v3.6.3
    Checking winit v0.30.13
    Checking tokio-rustls v0.26.4
    Checking hyper-rustls v0.27.9
    Checking reqwest v0.12.28
    Checking midway-core v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-core)
    Checking iced_winit v0.14.0
    Checking iced v0.14.0
    Checking midway-desktop v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-desktop)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.68s
```

Exit code: 0. Cero advertencias de compilación en la salida.

### 3.5 `cargo clippy --workspace --all-targets --all-features -- -D warnings` → `Sin_Herramienta`

```
error: no such command: `clippy`

help: view all installed commands with `cargo --list`
help: find a package to install `clippy` with `cargo search cargo-clippy`
```

Exit code: 101. Razón: `cargo-clippy` / `clippy-driver` no instalados.

### 3.6 `cargo test --workspace --all-features` (entorno tal cual) → `Ejecutado`, falló

Mismo fallo del build script de `libdbus-sys`, con PID propio:

```
   Compiling ring v0.17.14
   Compiling libdbus-sys v0.2.7
   Compiling libsqlite3-sys v0.35.0
error: failed to run custom build command for `libdbus-sys v0.2.7`

Caused by:
  process didn't exit successfully: `/home/avivaldelli/projects/personal/Midway/target/debug/build/libdbus-sys-bd32125d34c37a1f/build-script-build` (exit status: 101)
  --- stdout
  cargo:rerun-if-changed=build.rs
  cargo:rerun-if-changed=build_vendored.rs
  cargo:rerun-if-env-changed=DBUS_1_NO_PKG_CONFIG
  cargo:rerun-if-env-changed=PKG_CONFIG_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG
  cargo:rerun-if-env-changed=PKG_CONFIG
  cargo:rerun-if-env-changed=DBUS_1_STATIC
  cargo:rerun-if-env-changed=DBUS_1_DYNAMIC
  cargo:rerun-if-env-changed=PKG_CONFIG_ALL_STATIC
  cargo:rerun-if-env-changed=PKG_CONFIG_ALL_DYNAMIC
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_PATH
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_LIBDIR
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_SYSROOT_DIR
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR

  --- stderr
  pkg_config failed: 
  pkg-config exited with status code 1
  > PKG_CONFIG_ALLOW_SYSTEM_LIBS=1 PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 pkg-config --libs --cflags dbus-1 'dbus-1 >= 1.6'

  pkg-config output:
    Package dbus-1 was not found in the pkg-config search path.
    Perhaps you should add the directory containing `dbus-1.pc'
    to the PKG_CONFIG_PATH environment variable
    No package 'dbus-1' found
    Package dbus-1 was not found in the pkg-config search path.
    Perhaps you should add the directory containing `dbus-1.pc'
    to the PKG_CONFIG_PATH environment variable
    No package 'dbus-1' found

  The system library `dbus-1` required by crate `libdbus-sys` was not found.
  The file `dbus-1.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain its parent directory.
  The PKG_CONFIG_PATH environment variable is not set.

  HINT: if you have installed the library, try setting PKG_CONFIG_PATH to the directory containing `dbus-1.pc`.

  One possible solution is to check whether packages
  'libdbus-1-dev' and 'pkg-config' are installed:
  On Ubuntu:
  sudo apt install libdbus-1-dev pkg-config
  On Fedora:
  sudo dnf install dbus-devel pkgconf-pkg-config


  thread 'main' (214239) panicked at /home/avivaldelli/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libdbus-sys-0.2.7/build.rs:25:9:
  explicit panic
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
warning: build failed, waiting for other jobs to finish...
```

Exit code: 101. **Cero tests ejecutados**: el fallo es previo a la compilación de Midway.

### 3.7 `cargo test --workspace --all-features` con `PKG_CONFIG_PATH` solo de dbus → `Ejecutado`, falló

Resuelto `dbus-1.pc`, aparece el segundo fallo de entorno: el enlazado dinámico contra
`sqlite3`. Se transcriben los diagnósticos de las cinco fallas de enlazado. Cada bloque
`error: linking with cc failed` de la captura incluye además una línea de invocación de
`cc` de varias decenas de miles de caracteres con la lista completa de `.rlib`; esa línea
se marca abajo con `[...]` y su texto íntegro está en
`docs/baseline-evidence/04b-cargo-test-with-pkgconfig.txt`. Es la única elisión de este
documento y se declara explícitamente.

```
error: linking with `cc` failed: exit status: 1
  |
  = note:  "cc" "-m64" [...]
  = note: some arguments are omitted. use `--verbose` to show all linker arguments
  = note: /nix/store/39lyf5g2iz4jd2r6q4g5whk9vw09zglj-binutils-2.46/bin/ld.bfd: cannot find -lsqlite3: No such file or directory
          collect2: error: ld returned 1 exit status
          

error: could not compile `midway-core` (example "mem_probe") due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: linking with `cc` failed: exit status: 1
  |
  = note:  "cc" "-m64" [...]
  = note: some arguments are omitted. use `--verbose` to show all linker arguments
  = note: /nix/store/39lyf5g2iz4jd2r6q4g5whk9vw09zglj-binutils-2.46/bin/ld.bfd: cannot find -lsqlite3: No such file or directory
          collect2: error: ld returned 1 exit status
          

error: could not compile `midway-core` (test "workspace_snapshot_folders") due to 1 previous error
error: linking with `cc` failed: exit status: 1
  |
  = note:  "cc" "-m64" [...]
  = note: some arguments are omitted. use `--verbose` to show all linker arguments
  = note: /nix/store/39lyf5g2iz4jd2r6q4g5whk9vw09zglj-binutils-2.46/bin/ld.bfd: cannot find -lsqlite3: No such file or directory
          collect2: error: ld returned 1 exit status
          

error: could not compile `midway-core` (test "response_body_limit") due to 1 previous error
error: linking with `cc` failed: exit status: 1
  |
  = note:  "cc" "-m64" [...]
  = note: some arguments are omitted. use `--verbose` to show all linker arguments
  = note: /nix/store/39lyf5g2iz4jd2r6q4g5whk9vw09zglj-binutils-2.46/bin/ld.bfd: cannot find -lsqlite3: No such file or directory
          collect2: error: ld returned 1 exit status
          

error: could not compile `midway-core` (lib test) due to 1 previous error
error: linking with `cc` failed: exit status: 1
  |
  = note:  "cc" "-m64" [...]
  = note: some arguments are omitted. use `--verbose` to show all linker arguments
  = note: /nix/store/39lyf5g2iz4jd2r6q4g5whk9vw09zglj-binutils-2.46/bin/ld.bfd: cannot find -lsqlite3: No such file or directory
          collect2: error: ld returned 1 exit status
          

error: could not compile `midway-desktop` (bin "midway-desktop" test) due to 1 previous error
```

Exit code: 101. Cinco targets fallaron al enlazar: el ejemplo `mem_probe`, los tests de
integración `workspace_snapshot_folders` y `response_body_limit`, el lib test de
`midway-core` y el bin test de `midway-desktop`. El target `no_tauri_dependency` no
aparece en esta salida: cargo cortó tras el primer fallo
(`warning: build failed, waiting for other jobs to finish...`), así que no hay evidencia de
su resultado en esta corrida. Causa: `libsqlite3-sys 0.35`
enlaza dinámicamente contra el `sqlite3` del sistema, que no está en la ruta de búsqueda
del enlazador.

### 3.8 `cargo test --workspace --all-features` con `PKG_CONFIG_PATH` de dbus + sqlite → `Ejecutado`, pasó

Salida estructural de la corrida. Las 301 líneas individuales `test <nombre> ... ok` están
en `docs/baseline-evidence/04c-cargo-test-with-dbus-and-sqlite.txt` (357 líneas); acá se
transcriben las líneas de compilación, de arranque de cada binario de test y de resultado.

```
   Compiling libdbus-sys v0.2.7
   Compiling libsqlite3-sys v0.35.0
   Compiling tiny-xlib v0.2.5
   Compiling x11-dl v2.21.0
   Compiling dbus v0.9.12
   Compiling rusqlite v0.37.0
   Compiling softbuffer v0.4.8
   Compiling iced_tiny_skia v0.14.0
   Compiling tokio-rusqlite v0.7.0
   Compiling iced_renderer v0.14.0
   Compiling iced_widget v0.14.2
   Compiling dbus-secret-service v4.1.0
   Compiling keyring v3.6.3
   Compiling winit v0.30.13
   Compiling midway-core v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-core)
   Compiling iced_winit v0.14.0
   Compiling iced v0.14.0
   Compiling midway-desktop v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-desktop)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 19.69s
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

Exit code: 0. Total: 51 + 1 + 4 + 2 + 243 + 0 = **301 tests, 0 fallos, 0 ignorados**, sin
advertencias de compilación.

Límite de esta evidencia: la corrida es headless, sin `DISPLAY`. Los 301 tests son lógica
pura y de dominio; ninguno abre una ventana ni valida render. No sustentan ninguna
afirmación de verificación gráfica (Req 4.4, 4.5).

### 3.9 `cargo doc --workspace --no-deps` (entorno tal cual) → `Ejecutado`, falló

Mismo fallo del build script de `libdbus-sys`, con PID propio:

```
    Checking iced_futures v0.14.0
    Checking wgpu-hal v27.0.4
    Checking rustls-webpki v0.103.13
   Compiling libdbus-sys v0.2.7
    Checking hyper v1.10.1
   Compiling x11-dl v2.21.0
error: failed to run custom build command for `libdbus-sys v0.2.7`

Caused by:
  process didn't exit successfully: `/home/avivaldelli/projects/personal/Midway/target/debug/build/libdbus-sys-bd32125d34c37a1f/build-script-build` (exit status: 101)
  --- stdout
  cargo:rerun-if-changed=build.rs
  cargo:rerun-if-changed=build_vendored.rs
  cargo:rerun-if-env-changed=DBUS_1_NO_PKG_CONFIG
  cargo:rerun-if-env-changed=PKG_CONFIG_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG
  cargo:rerun-if-env-changed=PKG_CONFIG
  cargo:rerun-if-env-changed=DBUS_1_STATIC
  cargo:rerun-if-env-changed=DBUS_1_DYNAMIC
  cargo:rerun-if-env-changed=PKG_CONFIG_ALL_STATIC
  cargo:rerun-if-env-changed=PKG_CONFIG_ALL_DYNAMIC
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_PATH
  cargo:rerun-if-env-changed=PKG_CONFIG_PATH
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_LIBDIR
  cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR_x86_64-unknown-linux-gnu
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR_x86_64_unknown_linux_gnu
  cargo:rerun-if-env-changed=HOST_PKG_CONFIG_SYSROOT_DIR
  cargo:rerun-if-env-changed=PKG_CONFIG_SYSROOT_DIR

  --- stderr
  pkg_config failed: 
  pkg-config exited with status code 1
  > PKG_CONFIG_ALLOW_SYSTEM_LIBS=1 PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 pkg-config --libs --cflags dbus-1 'dbus-1 >= 1.6'

  pkg-config output:
    Package dbus-1 was not found in the pkg-config search path.
    Perhaps you should add the directory containing `dbus-1.pc'
    to the PKG_CONFIG_PATH environment variable
    No package 'dbus-1' found
    Package dbus-1 was not found in the pkg-config search path.
    Perhaps you should add the directory containing `dbus-1.pc'
    to the PKG_CONFIG_PATH environment variable
    No package 'dbus-1' found

  The system library `dbus-1` required by crate `libdbus-sys` was not found.
  The file `dbus-1.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain its parent directory.
  The PKG_CONFIG_PATH environment variable is not set.

  HINT: if you have installed the library, try setting PKG_CONFIG_PATH to the directory containing `dbus-1.pc`.

  One possible solution is to check whether packages
  'libdbus-1-dev' and 'pkg-config' are installed:
  On Ubuntu:
  sudo apt install libdbus-1-dev pkg-config
  On Fedora:
  sudo dnf install dbus-devel pkgconf-pkg-config


  thread 'main' (214614) panicked at /home/avivaldelli/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libdbus-sys-0.2.7/build.rs:25:9:
  explicit panic
  note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
warning: build failed, waiting for other jobs to finish...
```

Exit code: 101.

### 3.10 `cargo doc --workspace --no-deps` con `PKG_CONFIG_PATH` → `Ejecutado`, pasó

```
    Checking wgpu-core-deps-windows-linux-android v27.0.0
    Checking iced_graphics v0.14.0
    Checking iced_debug v0.14.0
    Checking tiny-xlib v0.2.5
    Checking libdbus-sys v0.2.7
    Checking syntect v5.3.0
    Checking hyper-util v0.1.20
    Checking libsqlite3-sys v0.35.0
    Checking getrandom v0.3.4
    Checking x11-dl v2.21.0
    Checking iced_runtime v0.14.0
    Checking tokio-util v0.7.18
    Checking wgpu-core v27.0.3
    Checking dbus v0.9.12
    Checking ahash v0.8.12
    Checking rusqlite v0.37.0
    Checking softbuffer v0.4.8
    Checking iced_program v0.14.0
    Checking iced_tiny_skia v0.14.0
    Checking tokio-rusqlite v0.7.0
    Checking hyper-rustls v0.27.9
    Checking two-face v0.4.5
    Checking reqwest v0.12.28
    Checking iced_highlighter v0.14.0
    Checking dbus-secret-service v4.1.0
    Checking keyring v3.6.3
    Checking midway-core v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-core)
 Documenting midway-core v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-core)
    Checking winit v0.30.13
    Checking wgpu v27.0.1
    Checking iced_winit v0.14.0
    Checking cryoglyph v0.1.0
    Checking iced_wgpu v0.14.0
    Checking iced_renderer v0.14.0
    Checking iced_widget v0.14.2
    Checking iced v0.14.0
 Documenting midway-desktop v0.1.0 (/home/avivaldelli/projects/personal/Midway/midway-desktop)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.42s
   Generated /home/avivaldelli/projects/personal/Midway/target/doc/midway_core/index.html and 1 other file
```

Exit code: 0. Sin advertencias de rustdoc en la salida.

### 3.11 `cargo deny check` → `Sin_Herramienta`

```
error: no such command: `deny`

help: a command with a similar name exists: `bench`

help: view all installed commands with `cargo --list`
help: find a package to install `deny` with `cargo search cargo-deny`
```

Exit code: 101. El Req 1.3 solo exige ejecutarlo donde esté disponible; acá no lo está.

### 3.12 `cargo audit` → `Sin_Herramienta`

```
error: no such command: `audit`

help: a command with a similar name exists: `add`

help: view all installed commands with `cargo --list`
help: find a package to install `audit` with `cargo search cargo-audit`
```

Exit code: 101. No hay evidencia sobre vulnerabilidades conocidas de las dependencias.

### 3.13 `cargo llvm-cov --workspace` → `Sin_Herramienta`

```
error: no such command: `llvm-cov`

help: view all installed commands with `cargo --list`
help: find a package to install `llvm-cov` with `cargo search cargo-llvm-cov`
```

Exit code: 101. No hay número de cobertura para el baseline, y ninguno se estima.

### 3.14 Reejecución de la suite al cierre de la tarea 4.8

Esta subsección la agrega la tarea 4.8. No reemplaza a la 3.8: la 3.8 es la corrida del
baseline sin ningún test agregado (301 tests) y queda como está. Acá se registra la corrida
posterior a las tareas 4.1 a 4.7, que agregaron Test_Caracterización **solo dentro de
bloques `#[cfg(test)]`**, sin tocar producción.

Comando textual, con el mismo ajuste de entorno de la sección 4 (dbus + sqlite):

```
PKG_CONFIG_PATH=/nix/store/7vxs654j460qbxf3idgzwb92g4zwlbwi-dbus-1.16.2-dev/lib/pkgconfig:/nix/store/w1cmsd5kg0jsj76cxj1vgfrsw3gk86yh-sqlite-3.51.2-dev/lib/pkgconfig cargo test --workspace --all-features
```

Salida estructural:

```
     Running unittests src/lib.rs (target/debug/deps/midway_core-98c90995f41ebeb4)
running 54 tests
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.60s

     Running tests/no_tauri_dependency.rs (target/debug/deps/no_tauri_dependency-33fecb7989998439)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s

     Running tests/response_body_limit.rs (target/debug/deps/response_body_limit-68e7f929c5c2eabb)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/workspace_snapshot_folders.rs (target/debug/deps/workspace_snapshot_folders-b171634cef9bc2ef)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

     Running unittests src/main.rs (target/debug/deps/midway_desktop-8257886440ed64cb)
running 262 tests
test result: ok. 262 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.51s

   Doc-tests midway_core
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: **0**. Total: 54 + 1 + 4 + 2 + 262 + 0 = **323 tests, 0 fallos, 0 ignorados**,
sin advertencias de compilación: la salida cierra el perfil de test sin ninguna línea
`warning:` ni `error:`.

Delta respecto de la 3.8, traceable test por test:

| Binario de test | 3.8 (tarea 1.2) | 3.14 (tarea 4.8) | Delta |
| --- | --- | --- | --- |
| `midway_core` unittests (`src/lib.rs`) | 51 | 54 | +3 |
| `tests/no_tauri_dependency.rs` | 1 | 1 | 0 |
| `tests/response_body_limit.rs` | 4 | 4 | 0 |
| `tests/workspace_snapshot_folders.rs` | 2 | 2 | 0 |
| `midway_desktop` unittests (`src/main.rs`) | 243 | 262 | +19 |
| Doc-tests `midway_core` | 0 | 0 | 0 |
| **Total** | **301** | **323** | **+22** |

Los 22 tests nuevos, obtenidos comparando los nombres de test de
`docs/baseline-evidence/04c-cargo-test-with-dbus-and-sqlite.txt` con los de esta corrida
(`comm` sobre las dos listas ordenadas). **Ningún nombre de la corrida 3.8 desapareció ni
cambió de nombre**: el conjunto anterior está contenido en el nuevo, lo que sustenta el
Req 6.8 (los tests preexistentes se preservan sin reducir cobertura).

| Tarea | Tests agregados |
| --- | --- |
| 4.1 | `app::theme_toggle_characterization_tests::toggled_from_dark_yields_light_and_marks_session_dirty`, `…::toggled_from_light_yields_dark_and_marks_session_dirty`, `…::toggled_twice_returns_to_initial_mode_and_leaves_session_dirty` |
| 4.2 | `session::tests::session_snapshot_serde_round_trip_property_tests::property_2_session_snapshot_json_round_trip_preserves_theme_and_panel_sizes` |
| 4.3 | `session::tests::session_schema_characterization_tests::panel_sizes_json_key_is_exactly_response_panel_height`, `…::session_json_without_optional_fields_deserializes_with_defaults`, `…::session_schema_version_tripwire_is_one` |
| 4.4 | `app::session_restore_property_tests::property_3_session_restore_preserves_panel_sizes_and_theme` |
| 4.5 | `app::debug_layout_tests::property_4_debug_pane_layout_is_total_and_decided_by_breakpoint`, `app::debug_layout_tests::degenerate_and_non_finite_widths_have_a_defined_layout` |
| 4.6 | `app::panel_resize_tests::tree_pane_width_clamps_exactly_at_range_boundaries`, `…::tree_pane_width_handles_degenerate_cursor_coordinates`, `…::request_panel_width_from_cursor_passes_through_inside_range`, `…::request_panel_width_from_cursor_clamps_outside_range`, `…::request_panel_width_from_cursor_handles_degenerate_inputs`, `…::request_panel_width_from_cursor_ignores_available_width`, `…::request_panel_width_for_available_passes_through_inside_range`, `…::request_panel_width_for_available_collapses_to_minimum_when_space_is_tight`, `…::request_panel_width_for_available_handles_non_finite_inputs` |
| 4.7 | `domain::http::domain_surface_tests::http_method_all_has_exactly_seven_variants`, `…::body_mode_has_exactly_four_variants`, `…::auth_config_has_exactly_four_variants` |

Clasificación de fallos de Test_Caracterización (Req 6.6, 6.7): **ninguno falla sobre el
baseline sin cambios.** No hay fallos que clasificar como defecto del baseline, y se declara
de forma explícita en lugar de dejarlo implícito. Lo que la escritura de esos tests sí
produjo son cuatro comportamientos del baseline previamente no documentados, registrados
como `L35` a `L38` en `docs/known-limitations.md` §2 con hito propietario y criterio de
cierre. Ningún test se ajustó para que pase y ningún archivo de producción se modificó para
volverlo verde.

Estado del árbol de trabajo al momento de esta corrida, con `git status --short` (solo
lectura): tres archivos modificados —`midway-core/src/domain/http.rs`,
`midway-desktop/src/app.rs`, `midway-desktop/src/session.rs`— y los documentos de auditoría
sin versionar. Las tres modificaciones son **exclusivamente** bloques `#[cfg(test)]`
agregados por las tareas 4.1 a 4.7; el comportamiento de producción del baseline es el del
HEAD `1fc270326e5c304f24ce08fe1f528da33a007d3e` de la sección 1.

Límite de esta evidencia, igual que en la 3.8: la corrida es headless, sin `DISPLAY`. Los
323 tests son de lógica pura y de dominio. Ninguno abre una ventana ni valida render, y no
sustentan ninguna afirmación de verificación gráfica (Req 4.4, 4.5).

Confirmación del requisito de entorno, reejecutada en esta tarea: **sin** el
`PKG_CONFIG_PATH` documentado, el mismo comando falla con **exit code 101** en el build
script de `libdbus-sys 0.2.7` (`explicit panic`, `The PKG_CONFIG_PATH environment variable
is not set`), sin ejecutar ningún test. Es el fallo de entorno ya registrado en la 3.6 y en
la sección 4, no un fallo de código ni de test.

### 3.15 `cargo fmt` y `cargo clippy` al cierre de la tarea 4.8 → siguen en `Sin_Herramienta`

Ambos comandos se reejecutaron en esta tarea, en este mismo entorno, y el resultado no
cambió respecto de las secciones 3.1 y 3.5 (Req 1.4):

| Comando textual | Estado | Exit | Salida observada |
| --- | --- | --- | --- |
| `cargo fmt --all -- --check` | `Sin_Herramienta` | 101 | ``error: no such command: `fmt` `` |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | `Sin_Herramienta` | 101 | ``error: no such command: `clippy` `` |

Razón, sin cambios: rustc 1.95.0 vino de un tarball y `rustup` está ausente, así que no hay
mecanismo para agregar los componentes `rustfmt` y `clippy`. Ningún ajuste de
`PKG_CONFIG_PATH` incide acá: el bloqueo es de toolchain, no de bibliotecas de sistema.

Declaración explícita: **no se afirma que `cargo fmt` ni `cargo clippy` hayan pasado.** El
estado de formato y de lints del código agregado por las tareas 4.1 a 4.7 es **desconocido**,
igual que el del baseline. `cargo deny`, `cargo audit` y `cargo llvm-cov` siguen igualmente
ausentes (3.11 a 3.13) y tampoco se reejecutaron con otro resultado. La verificación real de
formato y lints llega con el job `static-checks` de CI (tarea 13.1), donde el toolchain sí
tiene los componentes; el gate local queda cubierto por la tarea 13.2. Registrado como `L4`
en `docs/known-limitations.md`.

### 3.16 Reejecución de la suite al cierre de la tarea 7.3 (vertical Tema/Ajustes cableada)

Esta subsección la agrega la tarea 7.3. No reemplaza a la 3.8 ni a la 3.14: la 3.8 es la
corrida del baseline sin tests agregados (301), la 3.14 es la corrida posterior a los
Test_Caracterización de las tareas 4.1 a 4.7 (323), y acá se registra la corrida posterior
al **primer cambio de producción** del spec: la creación de la vertical
`ui/theme_settings.rs` (tarea 6.1) y su cableado en la App_Raíz (tareas 7.1 y 7.2).

Comando textual, con el mismo ajuste de entorno de la sección 4 (dbus + sqlite):

```
PKG_CONFIG_PATH=/nix/store/7vxs654j460qbxf3idgzwb92g4zwlbwi-dbus-1.16.2-dev/lib/pkgconfig:/nix/store/w1cmsd5kg0jsj76cxj1vgfrsw3gk86yh-sqlite-3.51.2-dev/lib/pkgconfig cargo test --workspace --all-features
```

Salida estructural:

```
     Running unittests src/lib.rs (target/debug/deps/midway_core-98c90995f41ebeb4)
running 54 tests
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.61s

     Running tests/no_tauri_dependency.rs (target/debug/deps/no_tauri_dependency-33fecb7989998439)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s

     Running tests/response_body_limit.rs (target/debug/deps/response_body_limit-68e7f929c5c2eabb)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/workspace_snapshot_folders.rs (target/debug/deps/workspace_snapshot_folders-b171634cef9bc2ef)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

     Running unittests src/main.rs (target/debug/deps/midway_desktop-8257886440ed64cb)
running 263 tests
test result: ok. 263 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.94s

     Running tests/vertical_import_guard.rs (target/debug/deps/vertical_import_guard-137cdd3a7b1bb470)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midway_core
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Exit code: **0**. Total: 54 + 1 + 4 + 2 + 263 + 3 + 0 = **327 tests, 0 fallos, 0 ignorados**,
sin ninguna línea `warning:` ni `error:` en la salida.

Delta respecto de la 3.14, por binario:

| Binario de test | 3.14 (tarea 4.8) | 3.16 (tarea 7.3) | Delta |
| --- | --- | --- | --- |
| `midway_core` unittests (`src/lib.rs`) | 54 | 54 | 0 |
| `tests/no_tauri_dependency.rs` | 1 | 1 | 0 |
| `tests/response_body_limit.rs` | 4 | 4 | 0 |
| `tests/workspace_snapshot_folders.rs` | 2 | 2 | 0 |
| `midway_desktop` unittests (`src/main.rs`) | 262 | 263 | +1 |
| `tests/vertical_import_guard.rs` | — (no existía) | 3 | +3 |
| Doc-tests `midway_core` | 0 | 0 | 0 |
| **Total** | **323** | **327** | **+4** |

Los cuatro tests nuevos, obtenidos comparando los nombres de test de esta corrida
(`cargo test --workspace --all-features -- --list`) con la lista de la 3.8 y con el delta ya
registrado en la 3.14:

| Tarea | Tests agregados |
| --- | --- |
| 6.2 | `ui::theme_settings::tests::property_1_theme_toggle_changes_mode_emits_event_and_is_involutive` |
| 6.3 | en `tests/vertical_import_guard.rs` (test de integración, sin prefijo de módulo): `la_vertical_no_referencia_dependencias_prohibidas`, `strip_comments_elimina_comentarios_y_preserva_cadenas`, `normalize_path_separators_colapsa_espacios_alrededor_de_dos_puntos` |
| 7.1, 7.2, 7.3 | **Ninguno.** El cableado de la vertical no agregó ni quitó ningún nombre de test |

**Ningún nombre de test de la 3.8 ni de la 3.14 desapareció ni cambió de nombre**: el
conjunto anterior está contenido en el nuevo (`comm -23` sobre las dos listas ordenadas
devuelve 0 líneas), lo que sustenta el Req 6.8 y hace comparable esta corrida con la 3.14.

Comportamiento observable idéntico al baseline (Req 8.7): los tres
`app::theme_toggle_characterization_tests` de la tarea 4.1 pasan por la ruta nueva
(`Message::Theme` → `theme_settings::update`) **con los mismos valores esperados** que
fijaron sobre el baseline —`Dark` → `Light`, `Light` → `Dark`, dos toggles vuelven al modo
inicial y `session.dirty == true` en los tres casos—. La tarea 7.3 solo **agregó** la
aserción de paleta derivada (`state.theme.design_system() ==
DesignSystem::for_mode(mode)`, la misma expresión que evalúa `app::view`); no modificó
ningún valor esperado preexistente. Esa invariancia es la evidencia de que el cableado
reubicó comportamiento sin alterarlo.

Límite de esta evidencia, igual que en la 3.8 y la 3.14: la corrida es headless, sin
`DISPLAY`. Los 327 tests son de lógica pura y de dominio; ninguno abre una ventana ni valida
render. **No se realizó verificación gráfica del control de tema** y no se afirma ninguna:
`docs/feature-matrix.md` registra esa verificación como `No_Verificable_En_Entorno` (Req 4.5,
4.8) y la fila del tema no pasa a `Verified` (Req 4.4).

`cargo fmt` y `cargo clippy` siguen en `Sin_Herramienta` por la razón de la 3.15 (toolchain
de tarball sin `rustup`), así que el estado de formato y de lints del código de la vertical
es **desconocido** y no se declara aprobado.

### 3.17 Reejecución de la Compuerta_Evidencia al cierre de la tarea 12 (incremento de UX)

Esta subsección la agrega la tarea 12 (Checkpoint del incremento de UX). No reemplaza a la
3.8, la 3.14 ni la 3.16: la 3.8 es el baseline sin tests agregados (301), la 3.14 es la
corrida posterior a los Test_Caracterización de las tareas 4.1 a 4.7 (323), la 3.16 es la
corrida posterior al cableado de la vertical Tema/Ajustes (327), y acá se registra la
corrida posterior al **incremento de UX**: divisor horizontal del panel de respuesta
(tareas 10.1 a 10.5), estados vacíos accionables, jerarquía visual por elevación y
unificación de idioma (tareas 11.1 a 11.4).

Los tres comandos se ejecutaron con el mismo ajuste de entorno de la sección 4 (dbus +
sqlite). Antes de `cargo check` se ejecutó `cargo clean -p midway-core -p midway-desktop`
para invalidar solo los dos crates del workspace: sin eso, cargo reporta `Finished` desde
caché y una advertencia preexistente no volvería a imprimirse.

| Comando textual | Estado | Exit | Resultado |
| --- | --- | --- | --- |
| `cargo check --workspace --all-targets --all-features` | `Ejecutado` | **0** | Recompiló ambos crates. Cero líneas `warning:` y cero `error:` |
| `cargo test --workspace --all-features` | `Ejecutado` | **0** | **363 tests, 0 fallos, 0 ignorados** |
| `cargo doc --workspace --no-deps` | `Ejecutado` | **0** | Docs generadas, **con 1 advertencia de rustdoc** (ver abajo) |
| `cargo fmt --all -- --check` | `Sin_Herramienta` | 101 | ``error: no such command: `fmt` `` |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | `Sin_Herramienta` | 101 | ``error: no such command: `clippy` `` |

Salida estructural de `cargo test --workspace --all-features`:

```
     Running unittests src/lib.rs (target/debug/deps/midway_core-98c90995f41ebeb4)
running 54 tests
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s

     Running tests/no_tauri_dependency.rs (target/debug/deps/no_tauri_dependency-33fecb7989998439)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s

     Running tests/response_body_limit.rs (target/debug/deps/response_body_limit-68e7f929c5c2eabb)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/workspace_snapshot_folders.rs (target/debug/deps/workspace_snapshot_folders-b171634cef9bc2ef)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

     Running unittests src/main.rs (target/debug/deps/midway_desktop-8257886440ed64cb)
running 299 tests
test result: ok. 299 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 52.29s

     Running tests/vertical_import_guard.rs (target/debug/deps/vertical_import_guard-137cdd3a7b1bb470)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midway_core
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Total: 54 + 1 + 4 + 2 + 299 + 3 + 0 = **363 tests, 0 fallos, 0 ignorados**, sin ninguna
línea `warning:` ni `error:` en la salida de test. El recuento coincide con
`cargo test --workspace --all-features -- --list`, que enumera 363 nombres de test.

Delta respecto de la 3.16, por binario:

| Binario de test | 3.16 (tarea 7.3) | 3.17 (tarea 12) | Delta |
| --- | --- | --- | --- |
| `midway_core` unittests (`src/lib.rs`) | 54 | 54 | 0 |
| `tests/no_tauri_dependency.rs` | 1 | 1 | 0 |
| `tests/response_body_limit.rs` | 4 | 4 | 0 |
| `tests/workspace_snapshot_folders.rs` | 2 | 2 | 0 |
| `midway_desktop` unittests (`src/main.rs`) | 263 | 299 | +36 |
| `tests/vertical_import_guard.rs` | 3 | 3 | 0 |
| Doc-tests `midway_core` | 0 | 0 | 0 |
| **Total** | **327** | **363** | **+36** |

Los 36 nombres nuevos, obtenidos con `comm` entre la lista ordenada de esta corrida y la
lista del baseline de la 3.8, descontando los 22 nombres ya atribuidos en la 3.14 y los 4
de la 3.16. Todos caen en `midway_desktop`; ningún test de `midway-core` ni de integración
cambió.

| Tarea | Tests agregados | Cantidad |
| --- | --- | --- |
| 10.1 | `app::panel_resize_tests::response_panel_height_tracks_cursor_from_bottom_edge`, `…::response_panel_height_from_cursor_is_clamped_to_supported_range`, `…::response_panel_height_from_cursor_handles_non_finite_inputs`, `…::response_panel_height_for_available_preserves_the_baseline_range`, `…::response_panel_height_for_available_reserves_the_request_editor_minimum`, `…::response_panel_height_for_available_handles_non_finite_inputs` | 6 |
| 10.2 | `app::panel_resize_tests::response_height_resize_tracks_cursor_and_persists_on_release`, `…::response_height_drag_is_ignored_without_active_drag`, `…::response_height_drag_state_compares_by_divider_not_by_geometry` | 3 |
| 10.3 | `app::panel_resize_tests::response_height_divider_propagates_the_area_geometry_to_the_reducer`, `…::both_dividers_share_the_same_three_visual_states`, `…::stacked_divider_drag_resolves_to_the_height_the_layout_renders` | 3 |
| 10.4 | `app::panel_resize_tests::property_5_resize_bounds_clamp_and_are_idempotent` (Property 5) | 1 |
| 10.5 | `app::panel_resize_tests::property_6_response_divider_drag_bounds_height_and_keeps_state_consistent` (Property 6) | 1 |
| 11.1 | **Ninguno.** El componente `ui/empty_state.rs` llegó sin tests; sus tests son la tarea 11.4 | 0 |
| 11.2 | **Ninguno.** El reemplazo de los cuatro estados vacíos consume las especificaciones del módulo, cuyos tests son la tarea 11.4 | 0 |
| 11.3 | `ui::request_composer::language_tests::` ×8 (`api_key_placement_labels_are_in_spanish_except_the_protocol_term`, `assertion_operator_labels_are_fully_in_spanish`, `assertion_source_labels_translate_the_prose_and_keep_the_spec_names`, `auth_kind_labels_translate_only_the_absence_of_a_scheme`, `body_mode_labels_translate_the_mode_and_keep_the_format_names`, `form_data_field_kind_labels_are_in_spanish`, `no_environment_option_is_in_spanish`, `retired_english_labels_do_not_come_back`); `ui::response_inspector::tests::assertion_verdict_labels_are_in_spanish`, `…::retired_english_verdict_labels_do_not_come_back`; `app::run_progress_label_tests::` ×4 (`every_phase_has_a_spanish_label`, `finished_phase_reports_the_counters`, `per_request_phases_degrade_without_a_request_name`, `per_request_phases_name_the_request_and_its_position`) | 14 |
| 11.4 | `ui::empty_state::tests::` ×8 (`no_response_spec_has_title_hint_and_no_action`, `no_cookies_spec_has_title_hint_and_no_action`, `no_assertions_spec_offers_add_assertion_action`, `no_open_requests_spec_offers_new_request_action`, `response_in_flight_spec_has_progress_hint_and_no_action`, `every_spec_has_non_empty_title_and_hint`, `every_action_label_is_non_empty`, `spec_strings_use_voseo_not_tuteo`) | 8 |
| **Total** | | **36** |

Nota sobre la jerarquía visual (tarea 11.3): los cambios de elevación de `app.rs`,
`ui/request_tree_pane.rs`, `ui/request_composer.rs` y `ui/response_inspector.rs` **no
agregaron ningún test**. Son cambios de estilo de contenedor, no verificables sin render, y
por eso la evidencia de esa parte de la tarea 11.3 queda en `No_Verificable_En_Entorno`, no
en la suite. Los 14 tests atribuidos a la 11.3 cubren exclusivamente la mitad de
unificación de idioma, que sí es texto inspeccionable sin ventana.

**Ningún nombre de test de la 3.8, la 3.14 ni la 3.16 desapareció ni cambió de nombre**:
`comm -23` entre la lista del baseline (301 nombres) y la de esta corrida (363) devuelve 0
líneas. Eso sustenta el Req 6.8 y hace comparable esta corrida con las tres anteriores. En
particular, los tests de ejemplo de `panel_resize_tests` de la tarea 4.6 siguen presentes
con sus mismos nombres y valores esperados, después de que la tarea 10.2 cambiara
`DividerDragged(f32)` por `DividerDragged { x, y }`: esa invariancia es la guardia de
regresión del Req 9.7.

Advertencia de rustdoc, nueva respecto de la 3.10 (que registró `cargo doc` sin ninguna
advertencia):

```
warning: redundant explicit link target
   --> midway-desktop/src/ui/empty_state.rs:200:29
    |
200 | /// [`contrast_text_color`](crate::ui::design_system::contrast_text_color) para
    |      ---------------------  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ explicit target is redundant
    |      |
    |      because label contains path that resolves to same destination
    |
    = note: `#[warn(rustdoc::redundant_explicit_links)]` on by default
warning: `midway-desktop` (bin "midway-desktop" doc) generated 1 warning
```

Se registra sin corregirla: la tarea 12 es un checkpoint de verificación y no modifica
ningún `.rs`. Es una advertencia de documentación en el módulo creado por la tarea 11.1, no
un error; `cargo doc` cierra con exit 0 y genera las docs. `cargo doc` no corre hoy en CI
(Deuda_Registrada de la tarea 14.2), así que nada la bloquea automáticamente. Queda como
ítem menor de limpieza para la tarea 13 o para quien toque `ui/empty_state.rs`.

Estado del árbol de trabajo al momento de esta corrida, con `git status --short` (solo
lectura): diez archivos modificados —`midway-core/src/domain/http.rs`,
`midway-desktop/src/app.rs`, `midway-desktop/src/main.rs`,
`midway-desktop/src/session.rs`, `midway-desktop/src/ui/mod.rs`,
`midway-desktop/src/ui/onboarding.rs`, `midway-desktop/src/ui/request_composer.rs`,
`midway-desktop/src/ui/request_tree_pane.rs`,
`midway-desktop/src/ui/response_inspector.rs`, `midway-desktop/src/ui/top_bar.rs`— más
`midway-desktop/src/ui/empty_state.rs`, `midway-desktop/src/ui/theme_settings.rs`,
`midway-desktop/tests/` y los documentos de auditoría sin versionar. Ningún commit, push,
cambio de rama ni operación destructiva de git en esta tarea (Req 2.1-2.3).

Límites del entorno, sin cambios respecto de la 3.8, la 3.14 y la 3.16 (Req 1.4, 4.4, 4.5):

- `cargo fmt` y `cargo clippy` siguen en **`Sin_Herramienta`** (exit 101,
  ``no such command``) por la razón de la 3.15: rustc 1.95.0 desde tarball y `rustup`
  ausente, así que no hay forma de agregar los componentes `rustfmt` y `clippy`. **No se
  afirma que hayan pasado.** El estado de formato y de lints del código de las tareas 10 y
  11 es **desconocido**. `cargo deny`, `cargo audit` y `cargo llvm-cov` siguen igualmente
  ausentes (3.11 a 3.13). La verificación real llega con el job `static-checks` de CI
  (tarea 13.1) y el gate local de la tarea 13.2.
- La corrida es **headless**: `DISPLAY` vacío, solo `WAYLAND_DISPLAY=wayland-1`. Los 363
  tests son de lógica pura, de dominio y de cadenas de texto. Ninguno abre una ventana ni
  valida render. **No se realizó ninguna verificación gráfica** del divisor horizontal, de
  los estados vacíos ni de la jerarquía visual por elevación, y no se afirma ninguna: el
  cursor de redimensionado, el arrastre real con mouse, el contraste percibido y los tres
  niveles de elevación quedan en `No_Verificable_En_Entorno` en
  `docs/feature-matrix.md`. Ninguna fila pasa a `Verified` con solo tests unitarios como
  evidencia (Req 4.4, 4.8).
- El `PKG_CONFIG_PATH` de la sección 4 sigue siendo requisito para los tres comandos en
  esta máquina.

### 3.18 `make verify` extendido al cierre de la tarea 13.2 → falla por `Sin_Herramienta`

Esta subsección la agrega la tarea 13.2. No reemplaza a la 3.15 ni a la 3.17: la 3.15
registra el estado de `cargo fmt` y `cargo clippy` invocados directamente, la 3.17 los
reconfirma en el checkpoint del incremento de UX, y acá se registra qué pasa al **correr el
gate local extendido** en este entorno.

Cambio del `Makefile` aplicado por esta tarea, íntegro:

```
-verify: check test ## Quality gate local equivalente a CI (check + test)
+verify: fmt-check check clippy test ## Quality gate local equivalente a CI (fmt-check + check + clippy + test)
```

Los targets `fmt-check` y `clippy` ya existían en el `Makefile` desde el baseline; lo único
que cambió es que ahora forman parte del gate, de modo que `make verify` declara la misma
intención que el job `static-checks` de CI (tarea 13.1). Ningún target nuevo, ningún comando
nuevo, ningún `.rs` tocado.

Orden efectivo de los prerrequisitos, con `make -n verify` (solo imprime, no ejecuta):

```
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Salida observada de `make verify` en este entorno, transcrita sin resumir:

```
$ make verify
cargo fmt --all -- --check
error: no such command: `fmt`

help: a command with a similar name exists: `fix`

help: view all installed commands with `cargo --list`
help: find a package to install `fmt` with `cargo search cargo-fmt`
make: *** [Makefile:40: fmt-check] Error 101
```

Exit code de `make`: **2** (make propaga el fallo de la receta; el exit del comando
subyacente es 101).

Clasificación: **`Sin_Herramienta`, no un defecto de código.** El gate se detiene en su
primer prerrequisito, `fmt-check`, porque `rustfmt` no está instalado. Como make aborta al
primer error, `check`, `clippy` y `test` no llegan a ejecutarse en esa corrida. Para dejar
constancia de que el fallo es de toolchain y no de código, cada target del gate se ejecutó
por separado en este mismo árbol:

| Target del gate | Comando que corre | Estado | Exit | Salida observada |
| --- | --- | --- | --- | --- |
| `fmt-check` | `cargo fmt --all -- --check` | `Sin_Herramienta` | 101 | ``error: no such command: `fmt` `` |
| `check` | `cargo check --workspace` | `Ejecutado`, pasó | 0 | `Finished dev profile [unoptimized + debuginfo] target(s)` |
| `clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | `Sin_Herramienta` | 101 | ``error: no such command: `clippy` `` |
| `test` | `cargo test --workspace` | `Ejecutado`, pasó | 0 | **363 tests, 0 fallos, 0 ignorados** (54 + 1 + 4 + 2 + 299 + 3 + 0, idéntico al recuento de la 3.17) |

Razón de la ausencia, sin cambios respecto de la 3.15: rustc 1.95.0 vino de un tarball y
`rustup` está ausente, así que no hay mecanismo para agregar los componentes `rustfmt` y
`clippy`. Ningún ajuste de `PKG_CONFIG_PATH` incide en esos dos: el bloqueo es de toolchain.
Los dos targets que sí corren, `check` y `test`, se ejecutaron con el `PKG_CONFIG_PATH` de
la sección 4; el `Makefile` no lo define, así que en esta máquina sigue siendo requisito de
entorno para cualquier target que compile (limitación `L1`).

Declaraciones explícitas:

- **No se afirma que `make verify` pase en este entorno.** Falla, y falla por herramienta
  ausente. El estado de formato y de lints del workspace sigue siendo **desconocido**, no
  "bueno" (`L4`).
- **No se afirma que el gate extendido sea equivalente verificado a CI**, solo que declara
  la misma intención. La verificación real de `fmt` y `clippy` ocurre en el job
  `static-checks` de CI (tarea 13.1), donde el toolchain sí trae los componentes. En una
  máquina con `rustup` y los componentes instalados, `make verify` corre los cuatro pasos;
  eso no se verificó acá y no se declara.
- El gate extendido **no debilita** el gate anterior: `check` y `test` siguen dentro, con el
  mismo resultado que la 3.17.

Con este cambio, la limitación `L2` de `docs/known-limitations.md` (el gate local declaraba
menos de lo que exige el proyecto) queda cerrada en cuanto a la declaración del gate, y lo
que persiste es `L4`, que es del entorno y no del `Makefile`.

### 3.19 Reejecución de la Compuerta_Evidencia al cierre de la tarea 16 (checkpoint final del spec)

Esta subsección la agrega la tarea 16 (Checkpoint final). No reemplaza a ninguna anterior:
la 3.8 es el baseline sin tests agregados (301), la 3.14 la corrida posterior a los
Test_Caracterización (323), la 3.16 la posterior al cableado de la vertical (327), la 3.17
la posterior al incremento de UX (363) y la 3.18 el gate local extendido. Acá se registra la
**corrida de cierre del spec completo**, con las tareas 1 a 15 terminadas, incluidas las
tareas de solo documentación 15.1 a 15.3 y las tareas 13 y 14 que tocaron `ci.yml` y el
`Makefile` pero ningún `.rs`.

Esta tarea **no modificó ningún `.rs`** y no ajustó nada para volver verde ninguna
verificación. Es una corrida de contraste contra la 3.17.

Mismo procedimiento de la 3.17: `PKG_CONFIG_PATH` de la sección 4 (dbus + sqlite), y
`cargo clean -p midway-core -p midway-desktop` antes de `cargo check` para invalidar solo
los dos crates del workspace, de modo que las advertencias no queden escondidas detrás de
la caché. La limpieza removió 3 741 archivos (3,6 GiB).

| Comando textual | Estado | Exit | Resultado |
| --- | --- | --- | --- |
| `cargo check --workspace --all-targets --all-features` | `Ejecutado`, pasó | **0** | Recompiló ambos crates. Cero líneas `warning:` y cero `error:` |
| `cargo test --workspace --all-features` | `Ejecutado`, pasó | **0** | **363 tests, 0 fallos, 0 ignorados** |
| `cargo doc --workspace --no-deps` | `Ejecutado`, pasó | **0** | Docs generadas, **con la misma 1 advertencia de rustdoc** de la 3.17 |
| `cargo fmt --all -- --check` | `Sin_Herramienta` | 101 | ``error: no such command: `fmt` `` |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | `Sin_Herramienta` | 101 | ``error: no such command: `clippy` `` |

Salida estructural de `cargo test --workspace --all-features`:

```
     Running unittests src/lib.rs (target/debug/deps/midway_core-98c90995f41ebeb4)
running 54 tests
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.19s

     Running tests/no_tauri_dependency.rs (target/debug/deps/no_tauri_dependency-33fecb7989998439)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s

     Running tests/response_body_limit.rs (target/debug/deps/response_body_limit-68e7f929c5c2eabb)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/workspace_snapshot_folders.rs (target/debug/deps/workspace_snapshot_folders-b171634cef9bc2ef)
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s

     Running unittests src/main.rs (target/debug/deps/midway_desktop-8257886440ed64cb)
running 299 tests
test result: ok. 299 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 51.81s

     Running tests/vertical_import_guard.rs (target/debug/deps/vertical_import_guard-137cdd3a7b1bb470)
running 3 tests
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests midway_core
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Total: 54 + 1 + 4 + 2 + 299 + 3 + 0 = **363 tests, 0 fallos, 0 ignorados**, sin ninguna
línea `warning:` ni `error:` en la salida de test. El recuento se corroboró de forma
independiente con `cargo test --workspace --all-features -- --list`, que enumera **363**
nombres de test.

Contraste con la 3.17, por binario:

| Binario de test | 3.17 (tarea 12) | 3.19 (tarea 16) | Delta |
| --- | --- | --- | --- |
| `midway_core` unittests (`src/lib.rs`) | 54 | 54 | 0 |
| `tests/no_tauri_dependency.rs` | 1 | 1 | 0 |
| `tests/response_body_limit.rs` | 4 | 4 | 0 |
| `tests/workspace_snapshot_folders.rs` | 2 | 2 | 0 |
| `midway_desktop` unittests (`src/main.rs`) | 299 | 299 | 0 |
| `tests/vertical_import_guard.rs` | 3 | 3 | 0 |
| Doc-tests `midway_core` | 0 | 0 | 0 |
| **Total** | **363** | **363** | **0** |

**Delta cero en los siete binarios.** Es el resultado esperado: entre la 3.17 y esta corrida
las únicas tareas fueron 13 (`ci.yml` y `Makefile`), 14 (documentación de dependencias del
sistema) y 15 (documentación de cierre), y ninguna agrega, quita ni renombra tests. Ningún
nombre de test desapareció (Req 6.8).

Advertencia de rustdoc, **idéntica a la registrada en la 3.17** y sin corregir:

```
warning: redundant explicit link target
   --> midway-desktop/src/ui/empty_state.rs:200:29
    |
200 | /// [`contrast_text_color`](crate::ui::design_system::contrast_text_color) para
    |      ---------------------  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ explicit target is redundant
    |      |
    |      because label contains path that resolves to same destination
    |
    = note: `#[warn(rustdoc::redundant_explicit_links)]` on by default
warning: `midway-desktop` (bin "midway-desktop" doc) generated 1 warning
```

Sigue en pie al cierre del spec, y por lo tanto **el Req 13.3 queda parcialmente
incumplido**: la mitad de rustdoc del requisito pide que no se introduzcan advertencias
nuevas respecto del baseline, y esta advertencia es nueva respecto de la 3.10, que registró
`cargo doc` sin ninguna. La tarea 16 es un checkpoint de verificación y no modifica `.rs`,
así que no se corrige acá. La mitad de lint del mismo requisito sigue **sin comparación
posible**, por lo que ya declara §13.3. Queda como ítem de limpieza del Hito 1, en la
primera tarea que toque `ui/empty_state.rs`.

#### Validadores de documentación

Los dos validadores de `docs/baseline-evidence/` se ejecutaron en esta misma corrida:

| Validador | Exit | Resultado |
| --- | --- | --- |
| `python3 docs/baseline-evidence/verify_feature_matrix.py` | **0** | 7 tablas, 60 filas, **0 filas en `Verified`**, 0 porcentajes. `OK: once columnas exactas, estados cerrados, verificación manual uniforme, sin Verified, sin porcentajes` |
| `python3 docs/baseline-evidence/verify_adr_citations.py` | **1** | **61 citas verificadas, 55 fallos** |

El fallo de `verify_adr_citations.py` es **el resultado ya registrado y explicado**, no un
hallazgo nuevo: `docs/adr/0006-archivos-sobre-mil-lineas.md` §7 lo documenta con el mismo
número (55 de 61) y con la misma causa. El script codifica citas `archivo:línea` de los ADRs
0003 y 0004 fijadas contra los números de línea de la línea base; `app.rs` creció 1 928
líneas a lo largo del spec y todos los punteros se corrieron de lugar. Lo que caducó son las
coordenadas, no los hechos que esos ADRs afirman: por ejemplo, el script espera
`pub enum Message {` en `app.rs:58` y encuentra una línea de doc, y espera
`pub theme_mode: ThemeMode,` en `app.rs:1380`, campo que la tarea 7.1 reemplazó por
`theme: ThemeSettingsState` de forma deliberada.

**No se repara acá, por diseño.** Repararlo exigiría reescribir las 61 coordenadas de dos
ADRs ya aprobados, y el ADR 0006 dejó esa reparación asignada al **Hito 2**. Se confirma y se
registra; el validador queda en rojo conocido, no en rojo silencioso.

#### Límites del entorno al cierre (Req 1.4, 4.4, 4.5)

Sin cambios respecto de la 3.17 y la 3.18:

- `cargo fmt` y `cargo clippy` siguen en **`Sin_Herramienta`** (exit 101,
  ``no such command``): rustc 1.95.0 desde tarball y `rustup` ausente. **No se afirma que
  hayan pasado en ningún momento de este spec.** El estado de formato y de lints de todo el
  código que el spec agregó o modificó es **desconocido**. `cargo deny`, `cargo audit` y
  `cargo llvm-cov` siguen igualmente ausentes (3.11 a 3.13): el spec cierra **sin número de
  cobertura y sin auditoría de dependencias**.
- El job `static-checks` de `.github/workflows/ci.yml` (tarea 13.1) es el único mecanismo
  que va a ejecutar `fmt` y `clippy` de verdad, y **nunca corrió**: agregarlo a CI no lo
  ejecuta, hace falta un push, prohibido por el Req 2.1-2.3. Es previsible que falle en su
  primera ejecución, porque ningún archivo de este spec pasó por `rustfmt` ni por `clippy`.
  Eso no es un defecto oculto: es una consecuencia declarada de la ausencia de herramientas,
  y el trabajo de responder a ese primer rojo pertenece al Hito 1.
- La corrida es **headless**: `DISPLAY` vacío, solo `WAYLAND_DISPLAY=wayland-1`. Los 363
  tests son de lógica pura, de dominio y de cadenas de texto. **Ninguna funcionalidad
  gráfica de este spec fue verificada manualmente** y ninguna se afirma: el control de tema,
  el divisor horizontal, los estados vacíos y los tres niveles de elevación quedan en
  `No_Verificable_En_Entorno` en `docs/feature-matrix.md`, cuyo validador confirma **0 filas
  en `Verified`** (Req 4.4, 4.8).
- El `PKG_CONFIG_PATH` de la sección 4 sigue siendo requisito de esta máquina para los tres
  comandos que compilan (limitación `L1`).

Estado del árbol de trabajo, con `git status --short` (solo lectura): doce archivos
modificados —`.github/workflows/ci.yml`, `Makefile`, `midway-core/src/domain/http.rs`,
`midway-desktop/src/app.rs`, `midway-desktop/src/main.rs`,
`midway-desktop/src/session.rs`, `midway-desktop/src/ui/mod.rs`,
`midway-desktop/src/ui/onboarding.rs`, `midway-desktop/src/ui/request_composer.rs`,
`midway-desktop/src/ui/request_tree_pane.rs`,
`midway-desktop/src/ui/response_inspector.rs`, `midway-desktop/src/ui/top_bar.rs`— más los
archivos nuevos sin versionar: `midway-desktop/src/ui/empty_state.rs`,
`midway-desktop/src/ui/theme_settings.rs`, `midway-desktop/tests/`, `docs/adr/`,
`docs/baseline-evidence/`, `docs/architecture.md`, `docs/feature-matrix.md`,
`docs/known-limitations.md`, `docs/product-audit.md` y `docs/product-principles.md`. El
spec cierra **sin ningún commit, push, cambio de rama ni operación destructiva de git**
(Req 2.1-2.3): todo el trabajo queda en el árbol de trabajo para que alguien lo revise antes
de versionarlo.

#### Qué queda abierto al cierre

Referencia cruzada, sin repetir el detalle que ya vive en su lugar:

| Ítem | Estado | Dónde está registrado |
| --- | --- | --- |
| Req 8.9 — la App_Raíz debía reducir su recuento de líneas | **Incumplido.** `app.rs` pasó de 10 750 a 12 678 crudas y de 5 618 a 5 963 de producción | §12.8 y ADR 0006 §5-6 |
| Req 13.3 — sin advertencias nuevas | **Parcialmente incumplido** en rustdoc (1 advertencia nueva, sin corregir) y **sin comparación posible** en lint | §3.19 arriba y §13.3 |
| Req 10.4 — quitar una dependencia del sistema a la vez en CI | **No ejecutado**: requiere commit y push. Las cuatro dependencias permanecen en el workflow | §14.1 |
| `L35` a `L38` — cuatro comportamientos del baseline previamente desconocidos | **Fijados por tests, no corregidos.** `NaN` se propaga, el panel derecho baja de su mínimo, un ancho negativo persistido infla el editor y `debug_pane_layout(NaN)` decide por IEEE 754 | `docs/known-limitations.md` §2 |
| Job `static-checks` de CI | **Nunca ejecutado.** Probable rojo en su primera corrida | §3.19 arriba, tarea 13.1 |
| `verify_adr_citations.py` | **Rojo conocido**: 55 de 61 citas caducas por el crecimiento de `app.rs` | ADR 0006 §7 |
| Cobertura, `cargo deny`, `cargo audit` | **Sin dato.** Ninguno se estima | §3.11 a §3.13 |

## 4. Correcciones mínimas de baseline

**Sin correcciones de código.** No se modificó ningún `.rs`, ni `Cargo.toml`, ni
`Cargo.lock`, ni el `Makefile`, ni ningún workflow para resolver los fallos bloqueantes de
la sección 3. El diff de código del baseline en esta tarea es vacío.

Justificación (Req 1.7): el Req 1.7 habilita la corrección mínima de un fallo clasificado
como `bloquea la ejecución del baseline`. Acá la corrección mínima **no es un cambio de
código**, porque la causa raíz no está en el código:

| Fallo bloqueante | Causa raíz observada | Cadena de dependencia | Corrección mínima aplicada |
| --- | --- | --- | --- |
| Build script de `libdbus-sys 0.2.7` con `explicit panic` | Ausencia de `dbus-1.pc` en el sistema y `PKG_CONFIG_PATH` no definida | `midway-core` → `keyring 3` (feature `sync-secret-service`) → `dbus-secret-service` → `dbus` → `libdbus-sys` | Definir `PKG_CONFIG_PATH` apuntando al `pkgconfig` de dbus |
| `cannot find -lsqlite3` al enlazar los targets de test | Ausencia de `sqlite3` en la ruta de búsqueda del enlazador | `midway-core` → `tokio-rusqlite 0.7` → `rusqlite 0.37` → `libsqlite3-sys 0.35` (enlazado dinámico contra el `sqlite3` del sistema) | Agregar a `PKG_CONFIG_PATH` el `pkgconfig` de sqlite |

Ajuste de entorno aplicado, íntegro y reproducible:

```
PKG_CONFIG_PATH=/nix/store/7vxs654j460qbxf3idgzwb92g4zwlbwi-dbus-1.16.2-dev/lib/pkgconfig:/nix/store/w1cmsd5kg0jsj76cxj1vgfrsw3gk86yh-sqlite-3.51.2-dev/lib/pkgconfig
```

Las rutas del nix store son específicas de esta máquina. En otra máquina el equivalente es
instalar `libdbus-1-dev` y `libsqlite3-dev` (Debian/Ubuntu) o `dbus-devel` y
`sqlite-devel` (Fedora). En `ubuntu-22.04`, que es donde corre CI hoy, ambas bibliotecas
vienen en la imagen y por eso CI no reproduce este fallo.

Evidencia previa y posterior al ajuste, lado a lado:

| Comando | Antes del ajuste | Después del ajuste |
| --- | --- | --- |
| `cargo check --workspace --all-targets --all-features` | exit 101, `explicit panic` en `libdbus-sys` (3.2) | exit 0, sin advertencias (3.4) |
| `cargo test --workspace --all-features` | exit 101, cero tests ejecutados (3.6) | exit 0, 301 tests, 0 fallos, 0 ignorados (3.8) |
| `cargo doc --workspace --no-deps` | exit 101, `explicit panic` en `libdbus-sys` (3.9) | exit 0, docs generadas (3.10) |

Paso intermedio registrado como evidencia de que las dos bibliotecas hacen falta, no una
sola: con `PKG_CONFIG_PATH` solo de dbus, `cargo test` sigue fallando por `-lsqlite3`
(3.7, exit 101).

Consecuencias que quedan registradas y no resueltas acá:

- El baseline **no compila en un entorno Linux sin `libdbus-1` ni `libsqlite3` de
  desarrollo**. Es una dependencia de sistema no documentada en el README ni en el
  `Makefile`. Se recoge como Deuda_Registrada en la tarea 2.1 y como limitación conocida
  con hito propietario en `docs/known-limitations.md` (tarea 2.3).
- Todas las tareas siguientes de este spec que ejecuten `cargo check`, `cargo test` o
  `cargo doc` en esta máquina necesitan el mismo `PKG_CONFIG_PATH`. Es requisito de
  entorno, no un cambio del proyecto.
- El estado de `cargo fmt` y `cargo clippy` sigue siendo `Sin_Herramienta` y ningún ajuste
  de entorno lo resuelve: hacen falta los componentes del toolchain, que sin `rustup` no se
  pueden agregar. Su verificación real llega con el job `static-checks` de CI (tarea 13.1).

## 5. Inventario de deuda técnica

Método: `find midway-core/src midway-desktop/src -name '*.rs' -exec wc -l {} +`, más
`docs/baseline-evidence/count_test_lines.py` para separar las líneas alojadas en bloques
`#[cfg(test)]` de las de producción. Los recuentos son del árbol de trabajo en el HEAD de
la sección 1.

Total del workspace: **31 786 líneas** en 39 archivos `.rs` bajo `midway-core/src` y
`midway-desktop/src`. El valor coincide con el ≈31 786 que anticipaba el diseño.

Los siete archivos que exceden 1 000 líneas, con el recuento observado (Req 5.1, 5.2):

| Archivo | Líneas | Problema | Riesgo | Hito propietario |
| --- | --- | --- | --- | --- |
| `midway-desktop/src/app.rs` | **10 750** (5 610 producción / 5 140 en cuatro bloques `#[cfg(test)]`) | **Monolito principal.** Contiene el `Message` raíz (15 variantes, `app.rs:58-88`), las catorce enumeraciones de mensajes por área, el struct `Midway`, los catorce handlers de `update`, sus auxiliares, las funciones `async` de efecto, `view`, `subscription` y la aritmética de paneles | Cualquier cambio en cualquier área toca el mismo archivo: conflictos de merge, revisión imposible de acotar, y un `update` cuyo alcance nadie puede verificar de un vistazo. `guarded_update` acota el pánico en runtime, no el acoplamiento | Hito 2 (reducción parcial en el Hito 1, tarea 7) |
| `midway-core/src/infra/sqlite_repository.rs` | 2 322 (1 485 producción / 837 test) | Toda la persistencia en un archivo: `migrate`, workspace, colecciones, requests, environments, historial y metadata de secretos. `migrate()` es `CREATE TABLE IF NOT EXISTS` más `ALTER TABLE` condicional por `PRAGMA table_info`, sin `user_version` | Sin versión de esquema no hay migración verificable, ni respaldo, ni rollback. Concentra además 97 de las 358 ocurrencias de `unwrap()`/`expect()` del workspace | Hito 6 (esquema); Hito 2 (tamaño) |
| `midway-desktop/src/updater.rs` | 2 311 (1 003 producción / 1 308 test) | Flujo completo del updater in-app (check, descarga, verificación de checksum, instalación, relaunch) con `#![allow(dead_code)]` a nivel de módulo: **no tiene ningún llamador de producción** | 1 003 líneas de producción que nunca se ejecutan y que el README presenta como funcionalidad disponible. Es la mayor superficie de código no ejercitado del workspace | Hito 4 (cableado); ver `L10` |
| `midway-desktop/src/ui/request_tree_pane.rs` | 2 115 (1 336 producción / 779 test) | Único módulo de `src/ui/` con handler de `update` propio (`update_tree`), pero recibe `&mut Midway` completo y devuelve `Task<Message>`: es un archivo movido, no una frontera. `TreeMessage` sigue definido en `app.rs:869-900` | Da la apariencia de módulo extraído sin ninguna de sus garantías. El acoplamiento al estado global permanece intacto | Hito 2 |
| `midway-core/src/domain/interop.rs` | 2 068 (1 535 producción / 533 test) | Import y export de tres formatos (bundle nativo v1, Postman Collection v2.1, OpenAPI v3 JSON/YAML) en un solo módulo, sin separación por formato | Un formato nuevo agranda el mismo archivo. Un bug de parseo de un formato se revisa junto al código de los otros dos | Hito 2 |
| `midway-desktop/src/collection_runner.rs` | 1 497 (427 producción / 1 070 test) | 427 líneas de producción contra 1 070 de test. El runner funciona pero **no es disparable ni cancelable desde la UI**: `RunnerMessage::StartRequested` y `CancelRequested` llevan `#[allow(dead_code)]` | Funcionalidad completa e inalcanzable. La vista del modo Test dice "Presioná Run" sin que ese control exista | Hito 4 (cableado); ver `L12` |
| `midway-desktop/src/session.rs` | 1 054 (297 producción / 757 test) | 297 líneas de producción; el resto son tests. `SESSION_SCHEMA_VERSION = 1` sin ruta de migración: un `session.json` con otra versión se descarta entero | El usuario pierde tabs abiertas, tab activa, tamaños de panel y stack de tabs cerradas ante cualquier cambio de esquema | Hito 6; ver `L26` |

Precisión sobre el conteo de `app.rs`: las 10 750 líneas del spec se confirman, pero el
número por sí solo sobreestima el monolito de producción. Los cuatro bloques
`#[cfg(test)]` (líneas 2675-2809, 5179-5230, 5468-5487 y 5819-10751) suman 5 140 líneas,
así que el código de producción es **5 610 líneas**. Ambos números se registran: 10 750 es
el tamaño del archivo, 5 610 es la superficie a descomponer.

El ADR 0004 declara ~5 132 de test y ~5 619 de producción para el mismo archivo. La
diferencia de 8 líneas es de método de conteo, no de contenido: acá se cuenta desde la línea
de la anotación `#[cfg(test)]` hasta la llave de cierre del bloque, ambas inclusive, con
`docs/baseline-evidence/count_test_lines.py`. Se registran los dos valores en vez de
armonizarlos en silencio; ninguna conclusión de este documento depende de esas 8 líneas.

### 5.1 Deuda que no es un archivo

Estas entradas no se miden en líneas de un archivo, pero son Deuda_Registrada con hito
propietario igual que las anteriores (Req 5.7).

| Deuda | Evidencia observada | Riesgo | Hito propietario |
| --- | --- | --- | --- |
| **Dependencia de sistema no documentada: `libdbus-1` y `libsqlite3`.** El baseline no compila en Linux sin las bibliotecas de desarrollo de dbus y sqlite. El build script de `libdbus-sys 0.2.7` entra en `explicit panic` cuando `pkg-config` no encuentra `dbus-1.pc`; con `dbus-1` resuelto, cinco targets de test siguen fallando al enlazar con `cannot find -lsqlite3`. Ni el README (§"Cómo levantar el proyecto", que solo pide "un toolchain de Rust reciente") ni el `Makefile` enumeran requisitos de sistema | Secciones 3.2, 3.3, 3.6, 3.7 y 4 de este documento. Cadenas verificadas en `Cargo.lock`: `midway-core` → `keyring 3` (feature `sync-secret-service`) → `dbus-secret-service` → `dbus` → `libdbus-sys`; y `midway-core` → `tokio-rusqlite 0.7` → `rusqlite 0.37` → `libsqlite3-sys 0.35` | Un colaborador nuevo en Linux encuentra un `explicit panic` de un build script de tercero como primera experiencia, sin ninguna pista en la documentación del repo. CI no lo reproduce porque `ubuntu-22.04` trae ambas bibliotecas en la imagen, así que el problema es invisible desde el pipeline | **Hito 3** (documentar requisitos de sistema y hacerlos verificables en el build). Registrada también como `L1` en `docs/known-limitations.md` |
| **`unwrap()`/`expect()` sin justificación escrita**: 358 ocurrencias en `midway-core/src` y `midway-desktop/src` | `grep -rn -E '\.unwrap\(\)\|\.expect\(' midway-core/src midway-desktop/src --include='*.rs' \| wc -l` = 358. Concentración: `sqlite_repository.rs` 97, `app.rs` 82, `updater.rs` 48, `session.rs` 32, `collection_runner.rs` 31 | El conteo es por archivo e **incluye los módulos `#[cfg(test)]` alojados en esos mismos archivos**: no se afirma que las 358 sean de producción. El caso concreto verificado en producción es el `.expect(...)` de `main.rs::boot`, que hace entrar en pánico el arranque si `AppState::initialize` falla | Hito 2; ver `L21` y `L20` |
| **Sin módulo `platform`** y sin ninguna compilación condicional por sistema operativo: cero ocurrencias de `target_os`, `target_family`, `cfg(windows)` o `cfg(unix)` en todo el fuente | `grep -rn 'target_os\|target_family\|cfg(windows)\|cfg(unix)' midway-core/src midway-desktop/src` → sin resultados. `midway-desktop/src/platform` no existe | El Req 11.3 exige encapsular integraciones específicas de plataforma en un módulo `platform` **cuando alguna se introduzca**. Hoy no hay ninguna, así que no hay incumplimiento; el riesgo es que la primera se agregue en línea dentro de `app.rs` | Hito 3 (junto con la matriz multiplataforma) |
| **`dist/` con residuo del frontend eliminado**: el directorio existe en el árbol de trabajo con `index.html` y `assets/`, restos del build de Vite | `git ls-files dist` sin salida; `git check-ignore -v dist/index.html` → `.gitignore:4:dist/`. No está versionado, está ignorado | Riesgo bajo y acotado: no llega al repositorio ni a los artefactos. Se registra porque confunde a quien inspecciona el árbol y porque `.gitignore` conserva un bloque entero "Node / frontend" (`node_modules/`, `.vite/`, `*.tsbuildinfo`, logs de npm/yarn/pnpm) para un stack que ya no existe | Hito 3 |

## 6. Costuras de extracción

Los quince grupos de variantes del `Message` raíz, verificados leyendo `app.rs:58-88`, con
el recuento de variantes de cada enum de área
(`docs/baseline-evidence/count_variants.py`) y el módulo destino propuesto (Req 5.3). El
módulo destino y el orden de prioridad son los del ADR 0004; esta tabla es el registro de
la costura, no la decisión.

| # | Variante del `Message` raíz | Enum de área | Variantes | Módulo destino propuesto | Estado observado hoy |
| --- | --- | --- | --- | --- | --- |
| 1 | `RequestComposer` | `RequestComposerMessage` (`app.rs:128-394`) | 51 | `request_execution.rs` (ciclo de vida de ejecución) + `ui/request_composer.rs` (edición del draft) | Handler en `app.rs:2909`. **No es unidad de extracción**: hay que partirlo primero |
| 2 | `ResponseInspector` | `ResponseInspectorMessage` (`app.rs:397-401`) | 1 | `ui/response_inspector.rs` (ya existe, hoy solo vista) | Handler en `app.rs:4319` |
| 3 | `Workspace` | `WorkspaceMessage` (`app.rs:404-493`) | 19 | `ui/environment_settings.rs` + `ui/history_panel.rs` + `workspace_interop.rs` | Handler en `app.rs:4356`. **No es unidad de extracción**: mezcla environments, historial y export/import |
| 4 | `Runner` | `RunnerMessage` (`app.rs:502-532`) | 4 | `collection_runner.rs` (ya existe) | Handler en `app.rs:1840`. Dos de las cuatro variantes con `#[allow(dead_code)]` |
| 5 | `Palette` | `PaletteMessage` (`app.rs:540-562`) | 4 | `ui/command_palette.rs` (ya existe, hoy solo el overlay; el scoring vive en `src/command_palette.rs`) | Handler en `app.rs:2097` |
| 6 | `Theme` | `ThemeMessage` (`app.rs:565-569`) | 1 | **`ui/theme_settings.rs`** | Handler en `app.rs:2123` (`update_theme`, ~12 líneas). Vertical de este spec |
| 7 | `Session` | `SessionMessage` (`app.rs:572-588`) | 2 | `session.rs` (ya existe) | Handler en `app.rs:1930` |
| 8 | `Updater` | `UpdaterMessage` (`app.rs:591-596`) | 1 | `updater.rs` (ya existe, 2 311 líneas) | **Sin handler**: `Message::Updater(_) => Task::none()` (`app.rs:1815`). `#[allow(dead_code)]`. Cablear antes de extraer |
| 9 | `Keyboard` | `KeyboardMessage` (`app.rs:620-669`) | 5 | sin destino propuesto: es enrutamiento global, se queda en la raíz | Handler en `app.rs:4234` |
| 10 | `ActivityBar` | `ActivityBarMessage` (`app.rs:846-865`) | 8 | `ui/activity_bar.rs` (ya existe, hoy solo vista) | Handler en `app.rs:2135` |
| 11 | `Tree` | `TreeMessage` (`app.rs:869-900`) | 13 | `ui/request_tree_pane.rs` (ya existe) | Handler **ya fuera de `app.rs`** (`request_tree_pane::update_tree`), pero recibe `&mut Midway` y devuelve `Task<Message>`: archivo movido, no frontera. `#[allow(dead_code)]` en la variante raíz |
| 12 | `WorkspaceCrud` | `WorkspaceCrudMessage` (`app.rs:829-842`) | 10 | `ui/workspace_crud.rs` | Handler en `app.rs:2265`. Comparte con `Tree` la reconciliación posterior a cada mutación |
| 13 | `TopBar` | `TopBarMessage` (`app.rs:904-913`) | 3 | `ui/top_bar.rs` (ya existe, hoy solo vista) | Handler en `app.rs:2623` |
| 14 | `PanelResize` | `PanelResizeMessage` (`app.rs:103-116`) | 6 | `ui/panel_layout.rs` | Handler en `app.rs:2815`. Este spec lo **modifica** en la tarea 10: no se extrae y modifica a la vez |
| 15 | `Tick` | *(sin enum: variante sin payload)* | — | sin destino: no es grupo extraíble | `Message::Tick => Task::none()` (`app.rs:1802`), `#[allow(dead_code)]`. El autosave real viaja por `SessionMessage::AutosaveTick` desde `iced::time::every` |

Tres precisiones sobre "los quince grupos", que el spec enunciaba como quince costuras
equivalentes y el fuente contradice:

1. **`Tick` no es un grupo.** No tiene enum propio ni handler: es una variante sin payload
   enrutada a `Task::none()`. Es un punto de entrada de subscription sin cablear, no una
   costura de extracción.
2. **`Updater` tampoco tiene handler.** Su lógica ya vive fuera de `app.rs`, en
   `updater.rs`, sin conexión al `update` raíz. Extraerlo no removería código de `app.rs`.
3. **Dos grupos no son unidades de extracción**: `RequestComposer` (51 variantes) y
   `Workspace` (19 variantes, tres asuntos sin relación). Extraerlos como bloque produciría
   archivos de ~1 316 y ~824 líneas con múltiples responsabilidades, es decir, monolitos más
   chicos. Hay que partirlos antes, y ese trabajo de diseño no está hecho.

Costuras efectivamente disponibles hoy, entonces: **doce**, no quince. De esas doce, el
Hito 1 toma una (`Theme`) y deja `PanelResize` fuera por conflicto con el incremento de UX
de este mismo spec.

## 7. Deuda de representación de respuesta

Comportamiento actual del baseline, verificado leyendo
`midway-core/src/domain/http.rs` y `midway-core/src/infra/http_reqwest.rs` (Req 5.4, 5.5).
Esta sección **registra** el comportamiento; no propone el rediseño, que queda `Diferido`
en el Hito 5 con ADR propio (0005).

| Elemento | Ubicación exacta | Qué hace hoy |
| --- | --- | --- |
| `ResponseEnvelope.body_text: String` | `midway-core/src/domain/http.rs:198` | El cuerpo de la respuesta se almacena como `String`, no como bytes. **El texto es la representación canónica.** |
| `String::from_utf8_lossy` como fallback | `midway-core/src/infra/http_reqwest.rs:112-116` | `String::from_utf8(buffer)` en el camino feliz; en el error, `String::from_utf8_lossy(error.as_bytes()).into_owned()`. Los bytes inválidos se reemplazan por `U+FFFD` y **el payload original ya no es recuperable** |
| `DEFAULT_MAX_BODY_BYTES` | `midway-core/src/domain/http.rs:190` | `8 * 1024 * 1024` = 8 MiB. Es el tope por defecto que usa `execute_request` (`http_reqwest.rs:19`) |
| `truncated: bool` | `midway-core/src/domain/http.rs:209`; fijado en `http_reqwest.rs:94` y `:101` | `true` cuando el body superó el límite y `body_text` es un prefijo del payload real. La lectura es en streaming y corta al alcanzar el límite |
| `body_evicted: bool` | `midway-core/src/domain/http.rs:214`; siempre `false` al crear (`http_reqwest.rs:126`) | Lo fija la UI cuando libera el body de una tab inactiva para no retener payloads. La metadata sigue válida; `body_text` queda vacío |
| `total_size_bytes: Option<u64>` | `midway-core/src/domain/http.rs:219`; tomado de `response.content_length()` (`http_reqwest.rs:75`) | Tamaño total informado por el servidor vía `Content-Length`, cuando está disponible. Permite mostrar "8 MB de 240 MB" en una respuesta truncada |
| `size_bytes: u64` | `midway-core/src/domain/http.rs:200-204`; `body_text.len()` (`http_reqwest.rs:122`) | Mide el **texto retenido**, no el payload original. Con `truncated == true` es el tamaño del fragmento; con `from_utf8_lossy` es el tamaño después del reemplazo |

Consecuencias registradas como Deuda_Registrada:

| Deuda | Riesgo | Hito propietario |
| --- | --- | --- |
| El texto es la representación canónica de la respuesta, no los bytes | Pérdida irreversible en respuestas binarias (imágenes, protobuf, gzip sin decodificar). Un cliente API que no puede devolver los bytes exactos que recibió no es confiable para depurar un endpoint binario | **Hito 5**; ver `L22` y ADR 0005 |
| El body se recorta a 8 MiB y las tabs inactivas se vacían | El payload completo no queda disponible ni recuperable: no hay ruta de "descargar respuesta completa" ni relectura desde disco | **Hito 5**; ver `L23` |
| `size_bytes` mide el texto retenido | Un número que parece el tamaño de la respuesta y no lo es. Con truncado o con reemplazo lossy, difiere del payload real | **Hito 5** |
| Dos consumidores de dominio ya dependen de `body_text` como `String` | Las aserciones de test (`domain/testing.rs:215` sobre `AssertionSource::BodyText` y `:231` con `serde_json::from_str(&response.body_text)`) y el preview (`domain/preview.rs`) leen texto. El rediseño a bytes tiene que atravesarlos: no es un cambio de un solo campo | **Hito 5** |

Lo que este documento **no** afirma: no se afirma que 8 MiB sea el límite correcto, ni que
el reemplazo lossy sea aceptable, ni que el rediseño sea sencillo. Se registra el
comportamiento observado y su dueño.

## 8. Ausencia de `xtask`: pasos a migrar

Estado observado del workspace (Req 5.6, 10.6):

- `Cargo.toml`: `members = ["midway-core", "midway-desktop"]`. **No existe crate `xtask`.**
- **`scripts/` sí existe**, con siete archivos versionados en git y 805 líneas en total.

**Discrepancia con el spec, registrada de forma explícita.** El Req 5.6 y el diseño
enuncian "la ausencia del crate `xtask` y del directorio `scripts/`". Lo observado es
mixto: `xtask` efectivamente no existe, pero `scripts/` **sí existe y está versionado**.
`docs/architecture.md:36` (tarea 2.4, escrita en paralelo) y `L7` de
`docs/known-limitations.md` repiten la misma afirmación incorrecta; ambas quedan pendientes
de corrección con este dato. Lo que se registra acá es el valor observado:

| Archivo | Líneas | Versionado | Invocado desde |
| --- | --- | --- | --- |
| `scripts/release/render-packager-config.mjs` | 201 | sí | `.github/workflows/release.yml:156`, `docs/distribution.md` |
| `scripts/release/generate-updater-json.mjs` | 165 | sí | `.github/workflows/release.yml:329`, `docs/distribution.md`, `docs/functional-equivalence.md` |
| `scripts/release/generate-checksums.mjs` | 69 | sí | `.github/workflows/release.yml:340`, `docs/distribution.md` |
| `scripts/release/verify-cleanup-gate.mjs` | 187 | sí | **ningún llamador** en `Makefile`, `.github` ni `docs` |
| `scripts/release/run-generate-updater-json.mjs` | 33 | sí | **ningún llamador** |
| `scripts/check-budgets.mjs` | 61 | sí | **ningún llamador** |
| `scripts/verify-workspace.sh` | 89 | sí | **ningún llamador** |

Consecuencia que agrava la deuda, no que la reduce: **el pipeline de release depende de
Node.js.** `.github/workflows/release.yml` declara `NODE_VERSION: "22"` (línea 22) y usa
`actions/setup-node@v4` en dos jobs (`build` línea 135, `metadata` línea 296) para ejecutar
tres scripts `.mjs`. El comentario del propio workflow lo dice: "Node se conserva
únicamente para ejecutar los scripts de release". Cuatro de los siete scripts no tienen
llamador y son código muerto de build.

### Pasos a migrar al futuro crate `xtask`

| # | Paso | Ubicación actual | Qué hay que preservar |
| --- | --- | --- | --- |
| 1 | Instalación pinneada de `cargo-packager` | `.github/workflows/release.yml:158-159`; `Makefile` target `package` (comentario "requiere cargo-packager instalado"); `README.md`; `midway-desktop/Cargo.toml` (comentario de `[package.metadata.packager]`) | El pin literal exacto: `cargo install cargo-packager --version =0.11.8 --locked`. Sin rangos abiertos, y verificable de forma idéntica en el `Cargo.toml`, en el workflow, en el README y en `docs/distribution.md` |
| 2 | Invocación de `cargo packager --release` | `.github/workflows/release.yml:160-166` (con `--verbose`, `--config` y `--formats`); `Makefile` target `package: release` | La restricción de `--formats` por plataforma (`appimage deb` en Linux, `nsis wix` en Windows): no invocar formatos ajenos al SO actual. `before-packaging-command = "cargo build --release --bin midway-desktop"` sale de la metadata |
| 3 | Render de la configuración por canal | `scripts/release/render-packager-config.mjs` (201 líneas de Node), invocado en `release.yml:156` | Genera `midway-desktop/packager.{stable,beta}.generated.json` desde `[package.metadata.packager]` como fuente única de verdad, aplicando los overrides por canal: `identifier`, `product-name`, `homepage`, `publisher`. El canal se deriva de si el tag contiene `-` (`stable` vs `beta`). Ambos archivos generados están en `.gitignore` |
| 4 | Copia de íconos | `midway-desktop/Cargo.toml`, clave `icons` de `[package.metadata.packager]`; directorio `midway-desktop/icons/` | Las cinco rutas declaradas: `icons/32x32.png`, `icons/128x128.png`, `icons/128x128@2x.png`, `icons/icon.icns`, `icons/icon.ico`. El directorio contiene además `64x64.png` e `icon.png`, **no declarados** en la metadata: la relación entre archivos presentes y archivos usados no está registrada en ningún lado |
| 5 | Variables de entorno gráficas del `Makefile` | `Makefile:11-15` | La detección de WSL por `grep -qi microsoft /proc/sys/kernel/osrelease`, que fija `WGPU_BACKEND ?= vulkan` solo en WSL para evitar que Mesa pruebe Zink antes del fallback de wgpu. Preserva los defaults nativos en el resto de plataformas y respeta `make run WGPU_BACKEND=...` |
| 6 | Generación del manifiesto del updater | `scripts/release/generate-updater-json.mjs` (165 líneas de Node), invocado en `release.yml:329` | `latest.json` para el canal stable y `latest-beta.json` para beta, a partir de los assets del release |
| 7 | Generación de checksums | `scripts/release/generate-checksums.mjs` (69 líneas de Node), invocado en `release.yml:340` | `SHA256SUMS.txt` sobre los assets del release, incluyendo el propio manifiesto del updater |
| 8 | Recolección y verificación de instaladores | `.github/workflows/release.yml`, pasos "Collect installers" y "Verify installers were produced" (shell embebido en YAML) | La recolección por extensión (`.AppImage`, `.deb`, `*-setup.exe`, `.msi`) excluyendo el binario crudo, y el corte del pipeline si el conteo es cero |
| 9 | Los cuatro scripts sin llamador | `scripts/release/verify-cleanup-gate.mjs`, `scripts/release/run-generate-updater-json.mjs`, `scripts/check-budgets.mjs`, `scripts/verify-workspace.sh` | Decidir por cada uno: migrar a `xtask` o eliminar. Hoy son 370 líneas de build tooling que nada invoca |

Hito propietario de todos los pasos de esta sección: **Hito 3**. El beneficio declarado de
la migración es doble: un único lugar de verdad para build, empaquetado y release, y la
eliminación de Node.js del pipeline de release, que hoy es la última dependencia de
JavaScript del proyecto.

## 9. Deuda de proceso

Comentarios del código que referencian números de fase, tarea o requisito de specs ya
cerrados (Req 3.8). Medición sobre `midway-core/src` y `midway-desktop/src`:

```
grep -rn -E '(Fase|Phase) [0-9]|(Tarea|Task) [0-9]+\.[0-9]|Requisito [0-9]+\.[0-9]' \
  midway-core/src midway-desktop/src --include='*.rs' | sort -u | wc -l
```

**588 líneas** en **20 archivos**. Desglose por patrón (una línea puede coincidir con más
de uno, así que la suma excede 588):

| Patrón | Coincidencias |
| --- | --- |
| `Tarea N.N` | 402 |
| `Requisito N.N` | 281 |
| `Fase N` | 49 |
| `Task N.N` | 7 |
| `Phase N` | 0 |

Concentración por archivo (los diez primeros):

| Archivo | Líneas con referencia obsoleta |
| --- | --- |
| `midway-desktop/src/app.rs` | 352 |
| `midway-desktop/src/updater.rs` | 70 |
| `midway-desktop/src/collection_runner.rs` | 33 |
| `midway-desktop/src/session.rs` | 24 |
| `midway-desktop/src/ui/workspace_panel.rs` | 21 |
| `midway-desktop/src/ui/request_composer.rs` | 18 |
| `midway-desktop/src/ui/text_editor.rs` | 13 |
| `midway-desktop/src/ui/response_inspector.rs` | 13 |
| `midway-desktop/src/diagnostics.rs` | 9 |
| `midway-desktop/src/ui/command_palette.rs` | 8 |

Ejemplos textuales, para que la deuda sea reconocible y no un número abstracto:

- `app.rs:1797-1799` — "envuelta en el `Error_Boundary` de `guarded_update` (Tarea 11.14,
  Requisito 6.9)".
- `midway-core/src/lib.rs:3` — "Movido desde `src-tauri/src/` (Fase 0, Tarea 1.2) sin
  reescritura de lógica interna": referencia a un directorio que ya no existe.
- `ui/workspace_panel.rs:420` — "se implementa en la Fase 6 (Tareas 13.1-13.10)": promesa
  de trabajo futuro que ya se hizo (existe `updater.rs`) pero que nunca se cableó, así que
  el comentario describe un estado que nunca ocurrió.
- `ui/text_editor.rs:121` — "aún no está conectado a `Midway`/`app.rs`; las tareas de la
  Fase 2...": afirmación que el fuente actual contradice.
- `app.rs:83-85` — "el autosave se agenda en la Fase 5 pero no está cableado al runtime
  todavía": el autosave **sí** está cableado, por `SessionMessage::AutosaveTick` desde
  `iced::time::every` (`app.rs:5661`). El comentario aplica a `Message::Tick`, que es otra
  cosa, y la redacción induce a error.

### Comentario de cabecera de `.github/workflows/ci.yml`

Se registra aparte porque contiene dos afirmaciones sin evidencia, no solo una referencia
obsoleta. Texto vigente (`ci.yml:14-16`):

```
  # Tras la Fase 8, el frontend TypeScript/React (`src/`) y el crate `midway`
  # (`src-tauri`) fueron eliminados, por lo que ya no existe job de frontend ni
  # tooling de Node/npm en CI: la aplicación es 100% Rust sobre iced.
```

| Problema | Evidencia observada |
| --- | --- |
| Referencia a "Fase 8", número de un spec cerrado | Deuda de proceso, igual que las 588 líneas del fuente |
| "la aplicación es 100% Rust sobre iced" | Afirmación prohibida por el Req 14.5 sin evidencia reproducible registrada. Y el pipeline de release contradice el espíritu de la frase: `release.yml` declara `NODE_VERSION: "22"`, usa `actions/setup-node@v4` en dos jobs y ejecuta tres scripts `.mjs`. La afirmación es correcta para el **artefacto** (el binario no lleva JavaScript) e incorrecta para el **proyecto** (el release necesita Node) |
| "ya no existe ... tooling de Node/npm en CI" | Verdadera para `ci.yml`, falsa para `.github/workflows/`: `release.yml` sí tiene tooling de Node. El comentario dice "en CI" y podría defenderse por su alcance literal, pero se registra la ambigüedad |

La reescritura de este comentario la ejecuta la tarea 13.1 (Hito 1). Las 588 líneas del
fuente son **Hito 3**: son cambios de comentario en 20 archivos, incluidas 352 líneas de
`app.rs`, y hacerlos ahora crearía ruido de diff sobre el mismo archivo que el Hito 2 va a
descomponer.

Deuda de proceso relacionada, en documentación en vez de código: `README.md` y varios
comentarios de `src/ui/` citan `iced_aw` como dependencia para tabs. **`iced_aw` no está en
ningún `Cargo.toml` ni en `Cargo.lock`** (`grep -c iced_aw Cargo.lock` = 0):
`ui/tab_bar.rs` es una implementación propia. Registrado como `L33`, Hito 3.

## 10. Discrepancias del README

Cada fila es una afirmación del `README.md` vigente que el fuente o la
Matriz_Funcionalidades contradicen (Req 3.7). Se registra la afirmación textual, lo
observado y la limitación asociada de `docs/known-limitations.md`.

Criterio de inclusión: solo entra una fila cuando la afirmación del README es **verificable
y falsa o no verificada**. Las afirmaciones correctas no se listan, y "no listado" no
significa "verificado": la verificación por funcionalidad vive en `docs/feature-matrix.md`.

| # | Afirmación del README | Observado en el fuente | Limitación |
| --- | --- | --- | --- |
| D1 | "hecho **100% en Rust**" y "toda la app ... resuelta en **Rust**, sin runtime web ni JavaScript" | Correcta para el artefacto: no hay Node, npm ni JavaScript en `Cargo.toml`, en el `Makefile` ni en `ci.yml`. **Incorrecta para el proyecto**: `.github/workflows/release.yml` declara `NODE_VERSION: "22"`, usa `actions/setup-node@v4` en dos jobs y ejecuta tres scripts `.mjs` de `scripts/release/`. Sin Node no hay release | `L33`; sección 8 y 9 de este documento |
| D2 | "**updater in-app** con check, descarga, verificación de checksum, instalación y relaunch" enumerado en "Novedades de esta versión" | El flujo está implementado en `updater.rs` (2 311 líneas) y **nunca se ejecuta**: el módulo lleva `#![allow(dead_code)]`, `Message::Updater(_) => Task::none()` (`app.rs:1815`) y `grep 'crate::updater\|use crate::updater'` no devuelve ningún llamador fuera de `main.rs:15` (`mod updater;`). La sección "App updates" del panel es una lectura pura de estado: su propio doc-comment dice "sin `WorkspaceMessage` ni botones funcionales" | `L10` |
| D3 | "**cancelación manual** del request en curso" enumerada en "Novedades de esta versión" | `RequestExecutorHandle::cancel` existe y funciona en `midway-core/src/runtime/request_executor.rs:45`, y **no tiene ningún llamador en `midway-desktop`**. No hay variante de mensaje que la dispare ni control en la UI | `L17` |
| D4 | "**Runner por colección**" con "ejecución secuencial", "reporte consolidado", "progreso del runner vía eventos" y "override opcional de environment" | El motor existe (`collection_runner.rs`, 427 líneas de producción) y no es alcanzable: `RunnerMessage::StartRequested` y `CancelRequested` llevan `#[allow(dead_code)]` con el comentario explícito "sin botón Run en la UI que lo dispare todavía". La vista del modo Test es un placeholder que dice "Presioná Run" sin que ese control exista | `L12` |
| D5 | "**Workspace** separado para Environments, Data, History y Diagnostics" y "El panel lateral secundario concentra ... Environments, Data, History, Diagnostics, App updates" | Solo tres de las cinco secciones son alcanzables. `workspace_panel::view` es código muerto y es el único emisor de `WorkspaceMessage::SectionSelected`: se llega a `Environments` (default), `History` (icono del Activity_Bar) y `Data` (al elegir una colección en el palette). **`Diagnostics` y `App updates` no tienen camino de navegación** | `L14` |
| D6 | "**Secrets en keychain** del sistema operativo" enumerado en "Novedades de esta versión" | La lectura por alias al resolver `{{secret:...}}` funciona. **No hay gestión desde la UI**: `SecretExecutorHandle::set`/`delete` y `SqliteRepository::upsert_secret_metadata`/`delete_secret_metadata` no tienen llamador en `midway-desktop`. No se puede crear, editar ni borrar un secreto desde la app | `L16` |
| D7 | "`iced_aw` para tabs" en "Stack técnico" | **`iced_aw` no existe en el proyecto**: `grep -c iced_aw Cargo.lock` = 0, y no aparece en ningún `Cargo.toml`. `midway-desktop/src/ui/tab_bar.rs` (93 líneas) es una implementación propia. Varios comentarios de `src/ui/` repiten la misma cita incorrecta | `L33` |
| D8 | "**Import / Export**" presentado sin salvedad de usabilidad | Funciona, pero opera sobre una ruta escrita a mano en un `text_input` con placeholder "Ruta de destino (ej. /home/user/export.json)": **no hay selector de archivos del sistema** | `L18` |
| D9 | "Requisitos: un toolchain de **Rust** reciente (`rustup` + `cargo`). No hace falta Node ni ningún gestor de paquetes de JavaScript para desarrollar o correr la app" | Incompleto en el requisito de sistema: en Linux hacen falta además las bibliotecas de desarrollo `libdbus-1` y `libsqlite3`, sin las cuales `cargo check` falla con un `explicit panic` del build script de `libdbus-sys` y los tests fallan al enlazar con `cannot find -lsqlite3`. Ni el README ni el `Makefile` lo mencionan | `L1`; secciones 3.2, 3.6, 3.7 y 4 |
| D10 | "**paneles redimensionados**" en la lista de estado persistido, junto a "Response panel claro" | Los divisores vertical del explorador y vertical request/response funcionan. El del panel de respuesta **no**: en el layout apilado del área Debug el "divisor" es un `container` inerte de 1 px y la altura sale de un `.clamp(180.0, 360.0)` escrito en línea (`app.rs:5452-5453`). El campo `responsePanelHeight` se persiste pero el usuario no puede cambiarlo por arrastre | `L29`; lo corrige la tarea 10 de este spec |
| D11 | "configuración por plataforma para **Windows** y **Linux**" en "Distribución y releases", junto a la presencia de `icons/icon.icns` (formato de macOS) en la metadata de packager | Consistente con el pipeline (`formats = ["nsis", "wix", "appimage", "deb"]`, matriz `ubuntu-22.04` + `windows-latest`, comentario "sin macOS" en `release.yml:91`). Se registra porque **macOS es plataforma de primer nivel** por el Req 11.4 y no tiene ninguna cobertura de build ni de release, mientras el `icon.icns` sugiere lo contrario | `L5`; sección 14 |
| D12 | "workflow de **CI** con `cargo check --workspace` y `cargo test --workspace`" | Textualmente correcta, y por eso mismo revela la deuda: eso es **todo** lo que corre CI. Sin fmt, sin clippy, sin docs, sin auditoría de dependencias y sobre una sola plataforma | `L3`, `L5`; lo corrige la tarea 13.1 |
| D13 | "**Dark mode real**" y "dark mode" como foco de diseño | El tema funciona, con una inconsistencia: las tabs de sección del Workspace_Panel derivan su paleta de `DesignSystem::for_mode(ThemeMode::default())` en vez del tema activo, y `section_content` recibe el `DesignSystem` y lo ignora (`_ds`). El tema claro no se aplica de forma consistente en ese panel | `L32` |
| D14 | Presentación general en español, con "**Send**" como "botón primario" | El texto visible mezcla idiomas: junto a las cadenas en español conviven "Send"/"Sending..." (`ui/request_composer.rs:135,137`), las etiquetas "Environments"/"Data"/"History"/"Diagnostics"/"App updates", los identificadores de modo "Debug"/"Test" y el encabezado "Collection Runner" | `L31`; lo corrige parcialmente la tarea 11.3 |
| D15 | "**Estado del proyecto**: ... uso interno fuerte, beta privada con usuarios reales" | No es una afirmación verificable de funcionalidad, sino una valoración de madurez. Se registra porque el mismo README acota después ("Todavía no lo presentaría como release pública final sin antes validar en máquinas reales"), y esa salvedad es la parte que se sostiene con evidencia: en el entorno de auditoría **ninguna funcionalidad gráfica está verificada manualmente** | `L34`; sección 14 |

Nota sobre `#[allow(dead_code)]`: hay **26 ocurrencias** en `midway-core/src` y
`midway-desktop/src`. Cinco de las quince discrepancias de arriba (D2, D3, D4, D5, D6)
tienen su raíz en código completo y no cableado. El patrón, no cada caso individual, es lo
que hace que el README describa un producto más terminado que el que existe.

## 11. Riesgos

Riesgos identificados con su impacto y su mitigación propuesta (Req 3.6). "Mitigación
propuesta" es exactamente eso: propuesta. Ninguna fila afirma que la mitigación esté
aplicada, salvo donde nombra la tarea de este spec que la ejecuta.

| Riesgo | Impacto | Mitigación propuesta |
| --- | --- | --- |
| **R1 — El monolito de `app.rs` bloquea todo cambio paralelo.** 5 610 líneas de producción con el `Message` raíz, catorce handlers, el estado y parte de la vista | Cualquier tarea de cualquier área toca el mismo archivo. Los conflictos de merge son estructurales, no accidentales, y ninguna revisión puede acotar el alcance real de un cambio | Extraer por grupo de mensajes según el ADR 0004, **una vertical por cambio revisable**, con test de caracterización escrito antes de extraer. La tarea 7 de este spec hace la primera (`Theme`) y establece el patrón. Registrar el recuento de líneas en cada extracción (tarea 8.2) |
| **R2 — La descomposición degenera en mover archivos.** Cinco módulos de `src/ui/` ya reciben `&Midway` y devuelven `Element<Message>`; `request_tree_pane.rs` ya recibe `&mut Midway` y devuelve `Task<Message>` | Se produce la apariencia de progreso sin ninguna de sus garantías: seis "módulos" que siguen acoplados al estado global. El Req 14.3 lo prohíbe explícitamente y aun así ya ocurrió seis veces | Las seis condiciones verificables del ADR 0004, más el test de guardia de importaciones de la tarea 6.3 que falla si la vertical referencia `AppState`, `crate::session`, `reqwest` o `midway_core::infra`. El compilador y un test, no el criterio del revisor |
| **R3 — Regresión silenciosa durante la extracción.** El comportamiento de `Theme` hoy solo está descrito por el código que se va a mover | Un cambio de paleta, de modo o de marca de sesión sucia pasa inadvertido y se descubre en uso real, sin forma de saber si el baseline hacía otra cosa | Test de caracterización sobre el baseline **sin modificar** (tarea 4.1), adaptado a la ruta nueva sin cambiar ningún valor esperado (tarea 7.3). Si el test falla sobre el baseline, se registra como defecto con hito propietario en vez de tunearse a verde (Req 6.6) |
| **R4 — El estado de formato, lints, licencias, vulnerabilidades y cobertura es desconocido.** Cinco de los ocho comandos de la Compuerta_Evidencia son `Sin_Herramienta` en el entorno de desarrollo, y CI no los corre | Se acumula deuda de estilo y de lint sin señal, y no hay ninguna evidencia sobre CVEs de las dependencias ni sobre licencias. "Desconocido" se lee como "bueno" por omisión | Job `static-checks` en `ci.yml` con `dtolnay/rust-toolchain@stable` y `components: rustfmt, clippy` (tarea 13.1), más `make verify` extendido a `fmt-check check clippy test` (tarea 13.2). **La primera corrida puede fallar y ese fallo es evidencia legítima**, no un problema del job. `cargo deny` y `cargo audit` quedan en Hito 3 |
| **R5 — El baseline no compila en un Linux limpio y la documentación no lo dice.** Falta de `libdbus-1` y `libsqlite3` de desarrollo | Un colaborador nuevo encuentra un `explicit panic` de un build script de tercero como primera experiencia. CI no lo detecta porque `ubuntu-22.04` trae ambas bibliotecas | Documentar los requisitos de sistema en el README y en el `Makefile`, y hacerlos verificables desde el build. **Hito 3**, registrado como `L1` y en la sección 5.1 de este documento |
| **R6 — Pérdida irreversible de datos de respuesta.** El texto es la representación canónica: `from_utf8_lossy`, tope de 8 MiB y eviction de tabs inactivas | Un cliente API que no puede devolver los bytes exactos que recibió es inservible para depurar un endpoint binario, y el usuario no tiene forma de recuperar el payload | Diferido de forma explícita al **Hito 5** con ADR 0005, no resuelto en silencio. Mientras tanto, los flags `truncated`, `body_evicted` y `total_size_bytes` quedan registrados como comportamiento actual para que la UI pueda avisar (sección 7) |
| **R7 — Pérdida de datos del usuario por cambio de esquema.** SQLite sin `user_version` y `session.json` que se descarta entero si la versión no coincide | Un cambio de esquema hecho sin ceremonia borra tabs abiertas, tamaños de panel y stack de tabs cerradas, o deja la base en un estado intermedio sin rollback | **`SESSION_SCHEMA_VERSION` queda en 1 durante todo este spec**, con un test tripwire que falla si alguien lo cambia sin declararlo (tarea 4.3). Si una tarea descubre que necesita cambiarlo, se detiene y vuelve a diseño bajo el Req 11.5 completo. La re-arquitectura es Hito 6 |
| **R8 — Fuga de secretos al historial.** El envío resuelve con `SecretRenderMode::Resolve` y persiste la URL resuelta en `history.url` | Un `{{secret:...}}` interpolado en la URL o en el query string queda en claro en `workspace.sqlite3`, se muestra en la sección History y viaja en el export nativo cuando incluye historial. El preview sí usa `Redact`, lo que hace la inconsistencia más fácil de pasar por alto | Registrado como `L27`, **Hito 10**. Fuera del alcance de este spec, pero es el riesgo de mayor severidad del inventario: no es deuda de estructura, es exposición de credenciales |
| **R9 — El README fija expectativas que el producto no cumple.** Quince discrepancias verificadas (sección 10), cinco de ellas por código completo y no cableado | Quien evalúa el proyecto por su README concluye que está más terminado de lo que está. Internamente, se planifica sobre funcionalidad que no existe como alcanzable | `docs/feature-matrix.md` con estados cerrados y sin porcentajes (tarea 2.2) como fuente de verdad por encima del README, más esta sección 10 como registro de cada discrepancia. La corrección del README es **Hito 3**: este spec lo audita, no lo reescribe |
| **R10 — Ninguna plataforma tiene verificación gráfica.** `DISPLAY` no está definida en el entorno de auditoría; macOS no tiene build ni release | Cualquier afirmación sobre comportamiento visual es especulación. Un defecto de render específico de Wayland, de X11, de Windows o de macOS es indetectable con la evidencia disponible | `No_Verificable_En_Entorno` como estado explícito en `docs/feature-matrix.md`, y ninguna fila en `Verified` con solo tests unitarios como evidencia (Req 4.4). Matriz multiplataforma en CI: **Hito 3**. Ver sección 14 |
| **R11 — Pánico en el arranque sin degradación.** Si `AppState::initialize` falla, `main.rs::boot` entra en pánico por un `.expect(...)` | La app no abre y el usuario no recibe ningún mensaje accionable: directorio de datos no resoluble o SQLite no abrible se manifiestan como un crash | Registrado como `L20`, **Hito 4**. La mitigación es una pantalla de error con degradación, no un `expect` con mejor mensaje |
| **R12 — Deuda de proceso que erosiona la confianza en los comentarios.** 588 líneas en 20 archivos con referencias a fases y tareas de specs cerrados, varias de ellas ya falsas | Un comentario que describe un estado que ya no existe (`ui/text_editor.rs:121`, `app.rs:83-85`) es peor que ningún comentario: dirige mal la lectura del código | Limpieza en **Hito 3**, después de la descomposición del Hito 2, para no generar ruido de diff sobre los mismos archivos. Excepción: el comentario de cabecera de `ci.yml` se reescribe ya en la tarea 13.1, porque además contiene una afirmación prohibida por el Req 14.5 |
| **R13 — El incremento de UX rompe los divisores existentes.** La tarea 10.2 cambia `PanelResizeMessage::DividerDragged(f32)` por `DividerDragged { x, y }`, que es la firma que usan los divisores vertical del explorador y request/response | Una regresión en el redimensionamiento de paneles que ya funcionaban, en la misma tarea que agrega el que falta | Los tests de ejemplo existentes de `panel_resize_tests` se adaptan a la forma `{ x, y }` **sin cambiar ningún valor esperado** (tarea 10.2): esa invariancia es la guardia del Req 9.7. Más la Property 5, que cubre los cinco límites de redimensionamiento con coordenadas cero, negativas y no finitas (tarea 10.4) |
| **R14 — El release depende de Node.js y cuatro de sus scripts no tienen llamador.** 805 líneas en `scripts/`, 370 de ellas sin invocación | El pipeline de release se rompe por una razón ajena al proyecto (una versión de Node, un script sin dueño) y nadie sabe cuáles de los siete scripts importan | Migrar los nueve pasos de la sección 8 al crate `xtask` y decidir por cada script sin llamador si se migra o se elimina. **Hito 3** |

## 12. Reducción de líneas de `app.rs`

Registro del resultado de la extracción de la vertical Tema/Ajustes (Req 8.9), más el
tamaño del módulo nuevo (Req 8.8). Medido después de la tarea 7.3, sobre el árbol de
trabajo, con comandos de solo lectura.

**Resultado en una línea: la extracción no redujo el recuento de líneas de `app.rs`.** El
recuento crudo subió 640 líneas y el de producción subió 6. Las razones están abajo, con
la medición que las sustenta. Cualquier lectura de esta sección como "el monolito se
redujo" es incorrecta.

### 12.1 Qué se está midiendo, y por qué hay dos números

El recuento crudo de `app.rs` **no** aísla el efecto de la extracción, porque entre la
línea base y hoy pasaron dos cambios independientes sobre el mismo archivo:

1. Las tareas 4.1, 4.4, 4.5 y 4.6 agregaron Test_Caracterización dentro de bloques
   `#[cfg(test)]` de `app.rs`. Eso **agranda** el archivo y no dice nada sobre la
   estructura del código de producción.
2. Las tareas 7.1 y 7.2 movieron el handler de tema y la definición de `ThemeMessage` a
   `src/ui/theme_settings.rs`. Ese es el efecto que el Req 8.9 quiere medir.

Por eso se registran los dos recuentos, con el criterio de separación explícito:

- **Recuento crudo**: `wc -l` del archivo completo.
- **Recuento de producción**: el archivo completo menos las líneas alojadas en bloques
  `#[cfg(test)]`, el mismo criterio de `count_test_lines.py` usado en la sección 5.

El script `docs/baseline-evidence/prod_line_delta.py` (agregado por esta tarea) calcula
ambos y además emite el diff unificado de las líneas de producción, para que el delta sea
auditable línea por línea y no una afirmación.

### 12.2 Antes y después

Línea base: `app.rs` en el HEAD `1fc270326e5c304f24ce08fe1f528da33a007d3e` de la
sección 1, obtenido con `git show HEAD:midway-desktop/src/app.rs` (operación de solo
lectura, sin efectos sobre el árbol de trabajo).

| Recuento | Antes (línea base) | Después (tarea 7.3) | Diferencia |
| --- | --- | --- | --- |
| **Crudo (`wc -l`)** | **10 750** | **11 390** | **+640** |
| Líneas en bloques `#[cfg(test)]` | 5 139 | 5 773 | +634 |
| **Producción (crudo − `#[cfg(test)]`)** | **5 611** | **5 617** | **+6** |
| Producción sin comentarios ni líneas en blanco | 3 689 | 3 685 | −4 |

Salida textual de la medición:

```
$ wc -l midway-desktop/src/app.rs midway-desktop/src/ui/theme_settings.rs
 11390 midway-desktop/src/app.rs
   204 midway-desktop/src/ui/theme_settings.rs
 11594 total

$ git show HEAD:midway-desktop/src/app.rs > /tmp/app_baseline.rs
$ python3 docs/baseline-evidence/prod_line_delta.py /tmp/app_baseline.rs midway-desktop/src/app.rs
antes   /tmp/app_baseline.rs
  total=10750 test=5139 prod=5611 bloques_cfg_test=4
despues midway-desktop/src/app.rs
  total=11390 test=5773 prod=5617 bloques_cfg_test=4

delta total = +640
delta test  = +634
delta prod  = +6

lineas de produccion agregadas = 31
lineas de produccion removidas = 25
```

Nota de precisión sobre la sección 5: allí el split de la línea base quedó registrado como
5 610 producción / 5 140 test, con los bloques `#[cfg(test)]` terminando en la línea
"10751". `prod_line_delta.py` acota el último bloque al largo real del archivo (10 750
líneas), de ahí 5 611 / 5 139. Es una diferencia de una línea por el límite superior del
último bloque, no un recuento distinto: el total crudo de 10 750 es el mismo en las dos
mediciones. La comparación de esta sección usa el mismo criterio en el antes y en el
después, así que el delta de +6 no queda afectado.

### 12.3 De dónde vienen las +634 líneas de test

Los cuatro bloques `#[cfg(test)]` de `app.rs`, antes y después:

| Bloque | Antes | Después | Diferencia | Tarea que lo agrandó |
| --- | --- | --- | --- | --- |
| `mod panel_resize_tests` | 135 (líneas 2675-2809) | 324 (líneas 2681-3004) | +189 | 4.6 (límites de redimensionamiento vigentes) |
| `mod import_name_collision_tests` | 52 (líneas 5179-5230) | 52 (líneas 5374-5425) | 0 | — |
| `mod debug_layout_tests` | 20 (líneas 5468-5487) | 166 (líneas 5663-5828) | +146 | 4.5 (Property 4, layout del área Debug) |
| `mod tests` | 4 932 (líneas 5819-10750) | 5 231 (líneas 6160-11390) | +299 | 4.1 (caracterización del toggle, adaptada en 7.3) y 4.4 (Property 3, restauración de sesión) |
| **Total** | **5 139** | **5 773** | **+634** | |

Ese crecimiento es el efecto buscado de la Compuerta de caracterización: 26 tests nuevos en
el workspace entre §3.8 y §3.16 (301 → 323 en las tareas 4.x, 323 → 327 en la 7.3), de los
cuales los alojados en `app.rs` explican estas +634 líneas. No es deuda y no se descuenta
de nada.

### 12.4 De dónde vienen las +6 líneas de producción

Nueve cambios en el código de producción de `app.rs`, tomados del diff que emite
`prod_line_delta.py`. Las columnas "Antes" y "Después" son recuentos de líneas del bloque
afectado, no números de línea:

| Cambio | Antes | Después | Delta |
| --- | --- | --- | --- |
| `use crate::ui::theme_settings::{self, ThemeEvent, ThemeSettingsState};` | 0 | 1 | +1 |
| `enum ThemeMessage` (derive, doc y variante) → `pub use` reexportado con su justificación | 6 | 5 | −1 |
| Doc del campo de estado (`theme_mode` → `theme`) | 3 | 5 | +2 |
| `let theme_mode = session.theme_mode;` → `ThemeSettingsState::new(...)` | 1 | 1 | 0 |
| Inicializador del struct `Midway` (`theme_mode,` → `theme,`) | 1 | 1 | 0 |
| Brazo `Message::Theme` de `guarded_update`: delegación en `theme_settings::update` más traducción del Evento_Ascendente, con 7 líneas de comentario | 1 | 16 | +15 |
| **`fn update_theme` eliminada**: 9 líneas de función (`app.rs:2123-2131` de la línea base) más su doc y la línea en blanco que la separaba | 11 | 0 | **−11** |
| Snapshot de autosave (`state.theme_mode` → `state.theme.mode()`) | 1 | 1 | 0 |
| `view` (`DesignSystem::for_mode(state.theme_mode)` → `state.theme.design_system()`) | 1 | 1 | 0 |
| **Neto** | | | **+6** |

Lo que se movió fuera de `app.rs` son **17 líneas**: el hueco que dejó `fn update_theme`
con su doc (11) más la definición del `enum ThemeMessage` (6). Lo que entró en su lugar son
**23 líneas**: el
`use` (1), el `pub use` con su comentario (5), dos líneas de doc del campo y el brazo de
enrutamiento (16, de las cuales 7 son comentario). El resultado neto es +6 líneas crudas y
−4 si se descuentan comentarios y líneas en blanco.

Lectura honesta del número: **la lógica de tema salió de `app.rs`, pero el archivo no se
acortó.** El costo de una frontera explícita (importar el módulo, reexportar el mensaje,
traducir el Evento_Ascendente y documentar por qué) es del mismo orden que el código que se
removió. En una vertical de este tamaño, ese costo se come la reducción entera.

### 12.5 El efecto en los otros archivos tocados

El recuento de `app.rs` aislado tampoco cuenta la historia completa, porque la tarea 7.2
también sacó el control de tema propio de `top_bar.rs`:

| Archivo | Producción antes | Producción después | Delta de producción | Crudo antes | Crudo después |
| --- | --- | --- | --- | --- | --- |
| `midway-desktop/src/app.rs` | 5 611 | 5 617 | +6 | 10 750 | 11 390 |
| `midway-desktop/src/ui/top_bar.rs` | 310 | 298 | −12 | 549 | 537 |
| `midway-desktop/src/ui/mod.rs` | 17 | 18 | +1 | 17 | 18 |
| **Subtotal de los consumidores** | **5 938** | **5 933** | **−5** | **11 316** | **11 945** |
| `midway-desktop/src/ui/theme_settings.rs` (nuevo) | — | 129 | +129 | — | 204 |
| **Total** | **5 938** | **6 062** | **+124** | **11 316** | **12 149** |

Los tres archivos que ya existían suman **5 líneas de producción menos** que en la línea
base. El módulo nuevo agrega 129 líneas de producción y 75 de test. El sistema, medido en
líneas, es más grande que antes.

`main.rs`, `ui/onboarding.rs` y `ui/request_tree_pane.rs` cambiaron sus puntos de lectura a
`state.theme.mode()` / `state.theme.design_system()` sin cambiar su recuento de líneas
(75, 239 y 2 115 antes y después).

### 12.6 Tamaño de `src/ui/theme_settings.rs` (Req 8.8)

```
$ wc -l midway-desktop/src/ui/theme_settings.rs
204 midway-desktop/src/ui/theme_settings.rs

$ python3 docs/baseline-evidence/count_test_lines.py midway-desktop/src/ui/theme_settings.rs
midway-desktop/src/ui/theme_settings.rs: total=204 test=75 prod=129 spans=[(130, 204)]
```

| Dato | Valor observado | Límite del Req 8.8 | Estado |
| --- | --- | --- | --- |
| Crudo (`wc -l`) | **204** | < 1 000 | Cumple, con 796 líneas de margen |
| Producción | 129 | — | — |
| Bloque `#[cfg(test)]` (líneas 130-204) | 75 | — | — |

El Req 8.8 no impone un mínimo de líneas: una vertical chica no es un defecto. El módulo
contiene `ThemeSettingsState`, `ThemeMessage`, `ThemeEvent`, `update`, `control` y la
Property 1, y queda muy por debajo del límite. No hay nada que justificar bajo el Req 11.9
para este archivo.

### 12.7 Contraste con lo que el ADR 0003 anticipó

`docs/adr/0003-vertical-tema-ajustes.md` predijo una reducción "pequeña": "9 líneas de
handler más la definición del enum, es decir, orden de decenas, no de miles". La medición
confirma el orden de magnitud y corrige el signo:

- El handler removido son las **9 líneas** exactas de `fn update_theme`
  (`app.rs:2123-2131` de la línea base), que es el número que el ADR anticipó. Contando su
  línea de doc y la línea en blanco que lo separaba del código anterior, el hueco es de
  **11** líneas. El enum con su `derive` y su doc son **6**. Total movido: 17 líneas.
- La predicción de "orden de decenas" se cumple: el diff de producción son 31 líneas
  agregadas y 25 removidas.
- Lo que el ADR no anticipó es que el neto quedara **positivo**. La reducción esperada de
  decenas de líneas terminó siendo un aumento de 6 líneas crudas de producción en
  `app.rs`, y una reducción de 5 si se cuenta también `top_bar.rs` y `ui/mod.rs`.

Esto no invalida la decisión del ADR 0003, cuyo criterio explícito fue que "el valor de
esta tarea es el patrón, no el conteo". Sí invalida cualquier uso del recuento de líneas
como métrica de progreso de la descomposición: en verticales chicas, el overhead de la
frontera domina.

### 12.8 Estado del Req 8.9

El Req 8.9 pide que la App_Raíz **reduzca** su recuento de líneas respecto de las 10 750
de la línea base, en una cantidad registrada acá. **La cantidad registrada no es una
reducción sino un aumento: el requisito no se cumple.** El recuento crudo pasó de 10 750 a
11 390 (+640) y el de producción de 5 611 a 5 617 (+6).

Se registra como brecha, no como cumplimiento:

| Brecha | Medición | Hito propietario |
| --- | --- | --- |
| `app.rs` no redujo su recuento de líneas con la primera vertical. La extracción movió la lógica de tema pero el archivo creció: +640 crudas (+634 de ellas por Test_Caracterización de las tareas 4.x) y +6 de producción | §12.2, §12.4 | **Hito 2** (descomposición por grupos de mensajes, `docs/adr/0004-descomposicion-de-app-rs.md`). El ADR 0006 de la tarea 15.3 registra el recuento final de `app.rs` y la justificación de su tamaño |

Lo que **sí** quedó verificado por las tareas 6 y 7, y que es el resultado real del
Hito 1 en esta dimensión:

- La lógica de tema tiene una frontera con contrato: `update` total, sin `Result`, sin
  `Task` y sin I/O, con Evento_Ascendente traducido en la raíz (tarea 7.1).
- El módulo no importa `AppState`, `crate::session`, `reqwest` ni `midway_core::infra`,
  verificado por `midway-desktop/tests/vertical_import_guard.rs` (tarea 6.3).
- El comportamiento observable es idéntico al de la línea base, verificado por el
  Test_Caracterización de la tarea 4.1 adaptado en la 7.3, sin cambiar valores esperados.
- `app.rs` ya no define `ThemeMessage` ni contiene `update_theme`: solo conserva un
  reexport de nombre.

Reducir el recuento de `app.rs` exige mover los grupos de mensajes grandes
(`RequestComposer`, `Tree`, `Runner`), no verticales de una variante. Eso es Hito 2 y está
fuera del alcance de este spec.

### 12.9 Reproducibilidad

```
git show 1fc270326e5c304f24ce08fe1f528da33a007d3e:midway-desktop/src/app.rs > /tmp/app_baseline.rs
python3 docs/baseline-evidence/prod_line_delta.py /tmp/app_baseline.rs midway-desktop/src/app.rs
python3 docs/baseline-evidence/count_test_lines.py midway-desktop/src/app.rs midway-desktop/src/ui/theme_settings.rs
wc -l midway-desktop/src/app.rs midway-desktop/src/ui/theme_settings.rs midway-desktop/src/ui/top_bar.rs midway-desktop/src/ui/mod.rs
```

`git show` es de solo lectura y no modifica el árbol de trabajo ni el índice (Req 2.1-2.3).

## 13. Afirmaciones no verificadas

Esta sección la escribe la tarea 15.2 y cierra el Req 14.5 y el Req 14.6. No reescribe
ninguna sección anterior: recoge en un solo lugar las afirmaciones que este spec **no
hace**, y registra como **no verificada** toda afirmación para la que no existe evidencia
reproducible en este documento o en `docs/baseline-evidence/`.

Criterio, aplicado sin excepciones:

- Si la evidencia está registrada y es reproducible, la afirmación vive en la sección que
  la mide y acá solo se referencia.
- Si la evidencia no existe, es parcial, o es de otra plataforma o de otra máquina, la
  afirmación se registra acá como **no verificada** (Req 14.6), con lo que faltaría para
  poder hacerla.
- "No verificada" **no** significa "falsa". Significa que no hay evidencia para
  sostenerla, y que por lo tanto no se afirma. El punto del Req 14.5 es evitar que
  "desconocido" se lea como "bueno" por omisión, que es el riesgo `R4`.

### 13.1 Las ocho afirmaciones que el Req 14.5 prohíbe sin evidencia

**Ninguna de las ocho se afirma en este spec.** Una fila por afirmación, con la evidencia
que sí existe, lo que faltaría para poder hacerla, y dónde aparece hoy en el árbol de
trabajo si aparece.

| Afirmación prohibida | ¿Se afirma acá? | Evidencia que sí existe | Qué faltaría para poder afirmarla | Dónde aparece hoy en el árbol |
| --- | --- | --- | --- | --- |
| "paridad total de funcionalidades" | **No.** Se registra como no verificada | `docs/feature-matrix.md` con estados cerrados por funcionalidad, y §10 con quince discrepancias del README. Cinco de ellas (D2, D3, D4, D5, D6) son código completo y **no cableado** | Una comparación funcionalidad por funcionalidad contra un producto de referencia, con criterio de equivalencia declarado y evidencia por fila. No existe y no está en el alcance | `docs/functional-equivalence.md` y `docs/functional-comparison.md` comparan contra la versión Tauri previa; ninguno de los dos fue auditado por este spec y ninguno se toma como evidencia acá |
| "100% compatible" | **No.** Se registra como no verificada | Ninguna. No hay ni definición de "compatible con qué" en este spec | Definir el artefacto de referencia y un conjunto de casos ejecutables. No existe | No aparece con ese texto exacto. Su vecina "100% Rust" sí: ver la fila "enteramente Rust" |
| "listo para producción" | **No.** Se registra como no verificada | Lo contrario: `docs/known-limitations.md` registra **43 limitaciones** (`L1` a `L43`), de las cuales solo `L2` figura como cerrada; `R8` es exposición de secretos en el historial (`L27`, Hito 10) y `R11` es pánico en el arranque sin degradación (`L20`, Hito 4) | Cerrar al menos los riesgos de severidad alta, tener verificación gráfica en las plataformas de primer nivel y evidencia de licencias y vulnerabilidades. Nada de eso existe hoy | `README.md` "Estado del proyecto" habla de "uso interno fuerte, beta privada con usuarios reales" (D15). Es una valoración de madurez, no una afirmación verificable, y no se avala acá |
| "verificado multiplataforma" | **No.** Se registra como no verificada, y es la más lejana de las ocho | Solo Linux, en **una** máquina, **headless**: `cargo check` exit 0 y 363 tests exit 0 (§3.17, §3.18). Windows tiene un job de empaquetado en `release.yml` que **intenta**; macOS no tiene ni eso | Compilación, tests y verificación gráfica en Linux Wayland, Linux X11, Windows y macOS, con evidencia por plataforma. Falta todo salvo la mitad de Linux, y la matriz multiplataforma de CI es Hito 3 (`L5`) | §14 y `docs/feature-matrix.md` §7 ya declaran `No_Verificable_En_Entorno` en las cuatro plataformas. `README.md` D11 declara "configuración por plataforma para Windows y Linux", que es configuración, no verificación |
| "cero regresiones" | **No.** Se registra como no verificada | Fuerte pero acotada: `comm -23` entre la lista de 301 nombres de test del baseline y la de 363 de la corrida final devuelve **0 líneas** (§3.17), así que ningún test del baseline desapareció ni cambió de nombre, y los valores esperados de `panel_resize_tests` no se tocaron (Req 9.7). El Test_Caracterización de la tarea 4.1 pasa por la ruta nueva sin cambiar valores esperados (§12.8) | Cobertura conocida y verificación gráfica. Con `cargo llvm-cov` en `Sin_Herramienta` (§3.13) **no se sabe qué fracción del comportamiento observan esos 363 tests**, y ninguno valida render: una regresión visual de la tarea 11.3 sería indetectable con esta evidencia | Nadie la afirma en el árbol. Se registra porque "la suite pasa" se lee fácil como "no hay regresiones", y no es lo mismo |
| "enteramente Rust" / "100% Rust" | **No.** Se registra como no verificada a nivel de proyecto | Verificable y verificado a nivel de **artefacto y de workspace**: `midway-core/tests/no_tauri_dependency.rs` pasa; `grep -c iced_aw Cargo.lock` = 0; sobre el grafo de 543 paquetes con `--all-features` no hay **ninguna** coincidencia de la pila WebKit/GTK/Tauri (§14.1). **Falsa a nivel de proyecto**: `release.yml` declara `NODE_VERSION: "22"`, usa `actions/setup-node@v4` en dos jobs y ejecuta tres scripts `.mjs` (§8, §9, D1) | Migrar los nueve pasos de §8 al crate `xtask` y eliminar Node del pipeline de release. **Hito 3** | `README.md:155` ("aplicación **100% Rust**"), `CHANGELOG.md:10,12,58`, `release.yml:26`, `docs/functional-comparison.md:9`, `docs/functional-equivalence.md:6,209`. El comentario de `ci.yml` que la afirmaba fue reescrito por la tarea 13.1; los otros siete lugares **siguen vigentes** y quedan como `L33`, Hito 3 |
| "no requiere Node" | **No** se afirma como propiedad del producto. Ver el detalle de abajo: la frase existe hoy en el `Makefile` | Lo que sí se verificó, con un grep reproducible: **no hay tooling de Node, npm ni JavaScript** en `Makefile`, en `.github/workflows/ci.yml` ni en los tres `Cargo.toml`. Lo que **no** se puede afirmar es la versión sin calificar: `release.yml` necesita Node, y `.gitignore` conserva un bloque entero "Node / frontend" para un stack que ya no existe (§5.1) | Eliminar Node del pipeline de release (Hito 3). Mientras `release.yml` lo necesite, la afirmación sin calificar es falsa, no solo no verificada | `Makefile:2`: `# Requiere: rustup/cargo instalado. No requiere Node.` Ver §13.2 |
| "migración completa" | **No.** Se registra como no verificada | El ADR 0001 descarta reejecutar la migración Tauri → iced y fija `main` como única línea base, lo que es una decisión de alcance, no evidencia de completitud. Hay residuo observable: `dist/` con `index.html` y `assets/` del build de Vite (ignorado, no versionado, §5.1), el bloque "Node / frontend" de `.gitignore`, `icons/icon.icns` sin plataforma que lo consuma (§14) y 588 líneas de comentarios que citan fases y directorios que ya no existen (§9) | Un criterio de "completa" declarado y verificable, más la limpieza del residuo. `docs/functional-equivalence.md:209` habla de "repositorio 100% Rust sin código muerto" mientras `grep -rn 'allow(dead_code)' midway-core/src midway-desktop/src --include='*.rs'` devuelve **29 ocurrencias** al cierre (§10 midió **26** en el baseline; las 3 nuevas están todas en `ui/empty_state.rs`, el módulo de la tarea 11.1, y el delta se registra en vez de armonizarse en silencio) y `scripts/` conserva 370 líneas sin llamador (§8) | `CHANGELOG.md:10` titula "Migración de Tauri + React/TypeScript a 100% Rust sobre `iced`". Describe el cambio realizado; no se toma como evidencia de completitud |

Sobre `Makefile:2` y "no requiere Node", con la distinción explícita porque la frase está en
el árbol y este spec tocó ese archivo en la tarea 13.2:

- **Verificable y verificado**: en el camino de desarrollo del workspace no hay ninguna
  herramienta de Node. El comando
  `grep -n -iE 'node|npm|npx|yarn|pnpm|\.mjs|\.js|typescript|vite' Makefile .github/workflows/ci.yml Cargo.toml midway-core/Cargo.toml midway-desktop/Cargo.toml`
  devuelve cuatro líneas y **ninguna es tooling**: `Makefile:2` (la afirmación misma),
  `ci.yml:22` (la declaración negativa que escribió la tarea 13.1) y dos comentarios de
  `midway-desktop/Cargo.toml` que citan `tauri.conf.json` y `tauri.windows.conf.json`, que
  coinciden por el `.js` de `.json`.
- **No verificable como afirmación de producto**: "no requiere Node" sin calificador abarca
  el proyecto entero, y el proyecto entero **sí** requiere Node para producir un release.
  Esa parte no es "no verificada": está contradicha por `release.yml` (§8, §9, D1).
- Segunda imprecisión de la misma línea, independiente de Node: enumera los requisitos de
  desarrollo y **omite** `libdbus-1` y `libsqlite3`, sin las cuales `cargo check` falla en
  esta máquina (§3.2, `L1`, D9). La línea es incompleta en las dos direcciones.
- La tarea 13.2 cambió **solo** el target `verify` del `Makefile` (§3.18) y no tocó el
  comentario de cabecera. Se registra acá como afirmación a corregir, con el mismo hito que
  sus siete hermanas de `README.md`, `CHANGELOG.md` y `release.yml`: **Hito 3**, `L33`. Se
  anota que `L33` hoy enumera `ci.yml` y `README.md` pero **no** el `Makefile`, el
  `CHANGELOG.md` ni `release.yml:26`: esas tres ubicaciones se agregan acá y la corrección
  de `L33` queda al mismo hito.

### 13.2 Afirmaciones sin evidencia reproducible, registradas como no verificadas (Req 14.6)

Todo lo que este spec **no** puede sostener con evidencia registrada, aunque nadie lo haya
afirmado en voz alta. Cada fila dice qué afirmación quedaría tentadora, por qué no se hace,
y qué la cerraría.

| # | Afirmación que **no** se hace | Por qué no hay evidencia | Qué la cerraría | Referencia |
| --- | --- | --- | --- | --- |
| U1 | "El divisor horizontal, los estados vacíos y la jerarquía visual funcionan" | Los 363 tests son **headless**: `DISPLAY` no está definida, solo `WAYLAND_DISPLAY=wayland-1`, y **no se abrió ninguna ventana en ningún momento de este spec**. Ninguno valida render, cursor de redimensionado, arrastre real con mouse, contraste percibido ni los tres niveles de elevación | Un entorno con display o un runner con captura. **Hito 3** | §2, §3.17, `L34`, `docs/feature-matrix.md` |
| U2 | "El código está bien formateado y sin lints" | `cargo fmt` y `cargo clippy` son `Sin_Herramienta` en este entorno (exit 101, `no such command`): rustc 1.95.0 desde tarball y `rustup` ausente, así que no hay forma de agregar los componentes. El estado es **desconocido**, no "bueno" | El job `static-checks` de CI (tarea 13.1), que **nunca corrió**. Ver §13.3 | §3.5, §3.15, §3.17, §3.18, `L4`, `R4` |
| U3 | "Las licencias de las dependencias están en orden" | `cargo deny` es `Sin_Herramienta` y ningún job de CI lo ejecuta | `cargo deny check` en un entorno con la herramienta y un gate en CI. **Hito 3** | §3.11, §14.1, `L4`, `L5` |
| U4 | "No hay vulnerabilidades conocidas en las dependencias" | `cargo audit` es `Sin_Herramienta` y ningún job de CI lo ejecuta. **No hay ninguna evidencia sobre CVEs** de los 543 paquetes del grafo | `cargo audit` con la herramienta instalada y un gate en CI. **Hito 3** | §3.12, §14.1, `L4`, `L5` |
| U5 | Cualquier número de cobertura | `cargo llvm-cov` es `Sin_Herramienta`. **No hay número y ninguno se estima.** Los 363 tests son un recuento de tests, no una medida de cobertura | `cargo llvm-cov --workspace` en un entorno con la herramienta | §3.13, `L4` |
| U6 | "Windows funciona" | No hay toolchain de Windows ni cross-compilation en este entorno, y `ci.yml` no incluye Windows. `release.yml` **intenta** empaquetarlo, lo que no es lo mismo, y no hay evidencia registrada de una corrida exitosa de esa pata | Una pata de Windows en `ci.yml` y evidencia de una corrida de release. **Hito 3** | §14, `L5`, D11 |
| U7 | "macOS está soportado" | **Ninguna cobertura**: ni CI, ni release, ni formato de empaquetado. `release.yml:91` dice explícitamente "sin macOS". `icons/icon.icns` es residuo de la configuración de Tauri, no evidencia de soporte | Runner de macOS, formato de empaquetado y verificación. **Hito 3** | §14, `L5`, D11 |
| U8 | "Linux X11 y Linux Wayland son equivalentes" | El mismo binario cubre ambos por winit y la compilación no los distingue, pero **ninguno de los dos** tiene verificación gráfica y no hay pata de CI específica de X11 | Verificación gráfica en las dos sesiones. **Hito 3** | §14, `L34` |
| U9 | "El empaquetado funciona" | `cargo packager` **no se ejecutó** en ninguna tarea de este spec. Que `cargo-packager 0.11.8` invoque `patchelf` para corregir el `RPATH` del AppImage es una **presunción explícitamente no verificada**, y es la razón por la que `patchelf` se conserva | Una corrida de `cargo packager --release` con los formatos por plataforma. **Hito 1** para completar la verificación de dependencias, **Hito 3** para `xtask` | §14.1, `L6` |
| U10 | "Las cuatro dependencias del sistema de `ci.yml` son innecesarias" (ni la contraria) | La corrida por dependencia que pide el Req 10.4 **no se hizo**: requiere commit y push, prohibidos por el Req 2.1-2.3. Lo que se corrió fue el experimento complementario, con las cuatro ausentes a la vez, en **NixOS y no en `ubuntu-22.04`**. Tres quedan con propuesta de eliminación, ninguna eliminada | Cuatro corridas de CI en `ubuntu-22.04`, una por dependencia, más la prueba de empaquetado. **Hito 1** | §14.1, `L6` |
| U11 | "`make verify` pasa" | **Falla** en este entorno, y falla en su primer prerrequisito por herramienta ausente: exit 2 de make, 101 del `cargo fmt` subyacente. Tampoco se afirma que el gate extendido sea **equivalente verificado** a CI, solo que declara la misma intención | Correrlo en una máquina con `rustup` y los componentes instalados. No se hizo acá | §3.18, `L2`, `L4` |
| U12 | "La suite de 363 tests es evidencia suficiente para pasar filas a `Verified`" | Por el Req 4.4, una fila cuya única evidencia es un test unitario **no** puede estar en `Verified`. Ninguna lo está | Verificación manual con display, por funcionalidad | `docs/feature-matrix.md`, Req 4.4, 4.8 |
| U13 | "El comportamiento de la vertical Tema/Ajustes es idéntico al baseline **en pantalla**" | Lo verificado es la ruta de `update`: mismo `ThemeMode`, misma paleta derivada y misma marca de sesión sucia, por el Test_Caracterización de la tarea 4.1 adaptado en la 7.3 sin cambiar valores esperados. **La paleta se compara como dato, no como render** | Verificación gráfica del control de tema, hoy `No_Verificable_En_Entorno` | §12.8, §3.16, `L34` |
| U14 | "Los tests son reproducibles en cualquier máquina" | Los tres comandos que corren en este entorno **requieren** el `PKG_CONFIG_PATH` de dbus + sqlite de §4; el `Makefile` no lo define. En un Linux limpio el baseline no compila | Documentar los requisitos de sistema y hacerlos verificables desde el build. **Hito 3** | §3.2, §4, `L1`, `R5`, D9 |

### 13.3 La comparación de advertencias nuevas del Req 13.3

El Req 13.3 pide que el entregable **no agregue advertencias nuevas de compilación ni de
lint**. La comparación se hace contra un punto de referencia registrado, y ese punto de
referencia existe para una de las tres clases de advertencia y no para las otras dos. **La
comparación completa se hace contra la salida de CI, no contra una ejecución local que en
este entorno no existe.**

| Clase de advertencia | Referencia del baseline | Estado al cierre | ¿Comparable acá? |
| --- | --- | --- | --- |
| **Compilación** (`rustc`) | §3.4: `cargo check --workspace --all-targets --all-features` exit 0, **cero advertencias** | §3.17: exit 0, **cero líneas `warning:` y cero `error:`**, tras `cargo clean -p midway-core -p midway-desktop` para forzar la recompilación de ambos crates. §3.18: `cargo check --workspace` exit 0 sin advertencias | **Sí.** Es la única mitad con referencia y con medición en la misma máquina. Alcance: esta máquina, con el `PKG_CONFIG_PATH` de §4, con este `rustc 1.95.0` |
| **Lint** (`clippy`) | **No existe.** §3.5 registra `cargo clippy` como `Sin_Herramienta` **sobre el baseline sin cambios** | Desconocido. §3.15, §3.17 y §3.18 lo reconfirman en `Sin_Herramienta` | **No.** Y el problema es más profundo que la falta de una corrida final: **no hay salida de clippy del baseline contra la que comparar** |
| **Documentación** (`rustdoc`) | §3.10: `cargo doc --workspace --no-deps` exit 0, **sin advertencias de rustdoc** | §3.17: exit 0, **con 1 advertencia**: `redundant explicit link target` en `midway-desktop/src/ui/empty_state.rs:200` | **Sí**, y el resultado es negativo: ver abajo |

Consecuencia para la mitad de lint, dicha sin adornos:

- La verificación real de formato y lints la hace el job `static-checks` de CI (tarea 13.1).
  Ese job **nunca corrió**: llegar a un runner exige commit y push, prohibidos por el
  Req 2.1-2.3, igual que en §14.1. No hay ninguna salida de clippy de este entregable.
- Cuando corra por primera vez, **`-D warnings` es un gate absoluto, no un delta**. Si
  falla, la salida no distingue una advertencia introducida por este entregable de una
  preexistente del baseline, porque el baseline nunca produjo una lista de advertencias de
  clippy. Atribuir cada advertencia exigirá correr clippy también sobre el HEAD
  `1fc270326e5c304f24ce08fe1f528da33a007d3e`, en el mismo toolchain, y comparar las dos
  salidas.
- **La primera corrida puede fallar y ese fallo es evidencia legítima**, no un defecto del
  job, tal como lo anticipa `R4`. Este documento no predice su resultado.
- Lo mismo vale para `cargo fmt --all -- --check`: sin salida de rustfmt del baseline, un
  fallo de formato en la primera corrida no es atribuible a este entregable ni al baseline
  sin ese contraste.

Consecuencia para la mitad de documentación, que sí es comparable y sí es negativa:

- La advertencia `redundant explicit link target` de `ui/empty_state.rs:200` es **nueva
  respecto de §3.10**, que registró `cargo doc` sin ninguna advertencia. La introdujo la
  tarea 11.1 y **quedó sin corregir**: la tarea 12 es un checkpoint de verificación y no
  modifica ningún `.rs`.
- Por lo tanto **el Req 13.3 no se cumple por completo**, y se registra como brecha en vez
  de omitirse: es una advertencia de documentación, `cargo doc` cierra con exit 0 y genera
  las docs, pero es una advertencia nueva y el requisito dice "ninguna".
- Nada la bloquea automáticamente: `cargo doc` **no corre en ningún job de CI**
  (Deuda_Registrada de §14.1, `L5`). Si corriera, la habría señalado.

| Brecha del Req 13.3 | Medición | Hito propietario |
| --- | --- | --- |
| 1 advertencia de rustdoc nueva respecto de §3.10, sin corregir: `redundant explicit link target` en `midway-desktop/src/ui/empty_state.rs:200` | §3.17, transcrita íntegra | **Hito 1**, para quien toque `ui/empty_state.rs`: quitar el target explícito del enlace. Es una línea de doc-comment |
| La mitad de lint del Req 13.3 no tiene comparación posible: ni corrida del entregable ni salida del baseline | §3.5 (baseline `Sin_Herramienta`), §3.17, §3.18 | **Hito 1**, primera corrida de `static-checks` en CI, más una corrida de clippy sobre el HEAD del baseline para poder atribuir cada advertencia. `L4` |
| `cargo doc` no tiene gate en CI, así que las advertencias de rustdoc no se detectan solas | §14.1 | **Hito 3**, `L5` |

Estado declarado del Req 13.3, para que quede una sola lectura posible: **cumplido en la
clase de compilación en esta máquina, no cumplido en la clase de documentación con una
advertencia nueva registrada, y no verificable en la clase de lint hasta que corra CI.**

### 13.4 Brechas que no son afirmaciones no verificadas

Distinción deliberada, porque mezclarlas debilita las dos categorías. Una **brecha** es un
requisito medido que **no se cumplió**: hay evidencia y el resultado es negativo. Una
**afirmación no verificada** es una para la que no hay evidencia en ninguna dirección. Las
brechas de este spec ya están registradas en su lugar y se listan acá solo como referencia
cruzada.

| Brecha | Tipo | Dónde está registrada |
| --- | --- | --- |
| **Req 8.9 no cumplido**: `app.rs` no redujo su recuento de líneas con la primera vertical. Pasó de 10 750 a 11 390 crudas (+640) y de 5 611 a 5 617 de producción (+6) | Brecha medida, resultado negativo | §12.8 y `docs/adr/0006-archivos-sobre-mil-lineas.md`. **Hito 2** |
| **Req 13.3 no cumplido en documentación**: 1 advertencia de rustdoc nueva sin corregir | Brecha medida, resultado negativo | §13.3, §3.17. **Hito 1** |
| **Req 10.4 incompleto**: la corrida por dependencia en `ubuntu-22.04` no se ejecutó y ninguna dependencia fue eliminada | Verificación incompleta por imposibilidad del entorno | §14.1, `L6`. **Hito 1** |
| **`L35`**: `f32::clamp` deja pasar `NaN`, así que el contrato "el resultado queda clampeado" no se cumple para `NaN` en las tres funciones de ancho vigentes | Comportamiento del baseline previamente desconocido, fijado por test | `docs/known-limitations.md` `L35`. **Hito 1** |
| **`L36`**: por debajo de 451 px de ancho disponible el panel derecho recibe menos que su propio mínimo | Comportamiento del baseline previamente desconocido, fijado por test | `docs/known-limitations.md` `L36`. **Hito 1** |
| **Tres discrepancias del spec** halladas al medir: `scripts/` sí existe (siete archivos, 805 líneas), el release depende de Node.js, y solo doce de los quince grupos del `Message` raíz son costuras reales | Corrección de la premisa del spec por valor observado | Preámbulo de este documento, §6, §8 |

### 13.5 Reproducibilidad de esta sección

Los tres comandos que sustentan las afirmaciones nuevas de esta sección, todos de solo
lectura y sin efectos de git (Req 2.1-2.3):

```
grep -rn -iE '100% (compatible|rust)|paridad total|listo para producci|cero regresiones|enteramente rust|no requiere node|migraci[oó]n completa|multiplataforma' \
  README.md CHANGELOG.md Makefile .github docs/*.md midway-core/Cargo.toml midway-desktop/Cargo.toml \
  | grep -v 'product-audit.md'
grep -n -iE 'node|npm|npx|yarn|pnpm|\.mjs|\.js|typescript|vite' \
  Makefile .github/workflows/ci.yml Cargo.toml midway-core/Cargo.toml midway-desktop/Cargo.toml
grep -rn 'allow(dead_code)' midway-core/src midway-desktop/src --include='*.rs' | wc -l
grep -c iced_aw Cargo.lock
```

El primero enumera las ocurrencias vigentes de las ocho frases prohibidas en el árbol y es
la base de la columna "Dónde aparece hoy" de §13.1; el `grep -v` final excluye este mismo
documento, que las cita para negarlas. El segundo devuelve cuatro líneas y **ninguna es
tooling**: sustenta la distinción de "no requiere Node". El tercero devuelve `29` y
sustenta el delta sobre las 26 de §10. El cuarto devuelve `0` y sustenta D7.

Esta sección **no ejecutó ningún comando que modifique el árbol de trabajo, no tocó ningún
`.rs` y no creó commits, push, cambio de rama ni operación destructiva de git.**

## 14. Plataformas de primer nivel

Las cuatro plataformas de primer nivel del Req 11.4, con su **estado de verificación real**.
La columna "Verificación gráfica manual" registra lo que se hizo, no lo que se espera que
funcione: en este entorno de auditoría, ninguna plataforma tiene verificación gráfica.

| Plataforma | Compilación verificada | Tests verificados | Build de release / empaquetado en CI | Verificación gráfica manual | Evidencia |
| --- | --- | --- | --- | --- | --- |
| **Linux Wayland** | Sí, en el entorno de auditoría con `PKG_CONFIG_PATH` de dbus + sqlite: `cargo check --workspace --all-targets --all-features` exit 0 | Sí: 301 tests, 0 fallos, 0 ignorados, **headless** | `ubuntu-22.04`: `ci.yml` corre `check` + `test`; `release.yml` empaqueta AppImage y deb x86_64 | **No.** `DISPLAY` no está definida; solo `WAYLAND_DISPLAY=wayland-1`. La app es un binario iced de escritorio y no se abrió ninguna ventana. `No_Verificable_En_Entorno` | §2, §3.4, §3.8 |
| **Linux X11** | No verificada de forma separada de Wayland: el mismo binario cubre ambos backends vía winit, y la compilación no distingue | Los mismos 301 tests, que son lógica pura y no tocan el backend de ventanas | Mismo job de `ubuntu-22.04`: no hay pata de CI específica de X11 | **No.** Sin `DISPLAY` no hay sesión X11 en el entorno. `No_Verificable_En_Entorno` | §2 |
| **Windows** | **No verificada en este entorno.** No hay toolchain de Windows ni cross-compilation configurada | No ejecutados en este entorno | `windows-latest` en `release.yml` (job `build`, formatos `nsis` y `wix`). **`ci.yml` no incluye Windows**: check y test solo corren en `ubuntu-22.04` | **No.** Ninguna verificación gráfica registrada | `ci.yml`, `release.yml` |
| **macOS** | **No verificada.** Sin toolchain ni runner | No ejecutados | **Ninguno.** `release.yml:91` lo dice explícitamente: "matriz Windows/Linux x86_64 (sin macOS)". `formats = ["nsis", "wix", "appimage", "deb"]` no incluye ningún formato de macOS, aunque `icons/icon.icns` está declarado en la metadata de packager | **No.** Ninguna verificación gráfica registrada | `midway-desktop/Cargo.toml`, `release.yml` |

Lo que esta tabla **no** afirma:

- No se afirma "verificado multiplataforma" (prohibido por el Req 14.5 sin evidencia).
  La única plataforma con compilación y tests verificados es Linux, en una sola máquina,
  headless.
- No se afirma que Windows funcione. Se afirma que `release.yml` **intenta** empaquetarlo,
  lo que no es lo mismo. No hay evidencia registrada de una corrida exitosa de esa pata.
- No se afirma que macOS esté soportado. Está declarado como plataforma de primer nivel por
  el Req 11.4 y **no tiene ninguna cobertura**: ni CI, ni release, ni formato de
  empaquetado. La presencia de `icon.icns` es un residuo de la configuración de Tauri, no
  evidencia de soporte.
- No se afirma que Linux X11 y Linux Wayland sean equivalentes. Se afirma que el mismo
  binario los cubre por winit y que **ninguno de los dos** tiene verificación gráfica.

Deuda_Registrada derivada, con hito propietario:

| Deuda | Hito propietario |
| --- | --- |
| `ci.yml` corre en una sola plataforma (`ubuntu-22.04`): sin matriz multiplataforma, sin generación de docs y sin auditoría de dependencias | **Hito 3**; ver `L5`. La tarea 14.2 de este spec agrega el registro por dependencia del sistema, no la matriz |
| macOS es plataforma de primer nivel sin build, sin release y sin formato de empaquetado | **Hito 3**; ver `L5` |
| Ninguna funcionalidad gráfica está verificada manualmente en ninguna plataforma | **Hito 3** (requiere un entorno con display o un runner con capacidad de captura); ver `L34` |
| `ci.yml` instala `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev` y `patchelf` sin evidencia de que sigan siendo necesarias tras retirar Tauri | **Hito 1**, tarea 14: verificación empírica una dependencia a la vez. Hasta que se complete, las cuatro permanecen; ver `L6`. Resultado parcial en §14.1: la corrida por dependencia en CI **no se hizo**, y las cuatro siguen instaladas |

### 14.1 Verificación empírica de las cuatro dependencias del sistema (Req 10.4, 10.5)

Esta subsección la agrega la tarea 14.2 y transcribe el hallazgo de la tarea 14.1. No
reescribe la tabla de plataformas de arriba ni sus filas de Deuda_Registrada (la última fila
solo gana un puntero a acá): la completa con el registro por dependencia del paso "Install
Linux system dependencies" de `.github/workflows/ci.yml`.

Evidencia cruda: `docs/baseline-evidence/09-system-dep-probe.txt`. Hallazgo de trabajo con
el detalle por dependencia: `docs/baseline-evidence/09b-system-dep-findings.md`. Sonda:
`docs/baseline-evidence/system_dep_probe.sh`, de solo lectura sobre el árbol y sin tocar
git. Fecha de la corrida: 2026-08-21. Entorno: NixOS 26.05, Linux 6.18.36 x86_64,
rustc 1.95.0, cargo 1.95.0, con el `PKG_CONFIG_PATH` de dbus + sqlite de §3.8 y §4.

#### El experimento ejecutado no es el que describe el procedimiento

El procedimiento del Req 10.4 pide quitar **una** dependencia a la vez del paso de
instalación y observar `cargo check --workspace` y `cargo test --workspace` en el runner.
Ese experimento **no se ejecutó**: requiere una corrida de CI, y por lo tanto un commit y
un push, que el Req 2.1-2.3 prohíbe; ningún cambio de `ci.yml` puede llegar a un runner
desde acá. Además el runner es `ubuntu-22.04` y esta máquina no lo es: sin `dpkg` ni `apt`
no puede reproducir ni el estado "las cuatro instaladas" ni el estado "tres instaladas".

Lo que sí se ejecutó es el experimento **complementario**, que en esta plataforma subsume
los cuatro casos de "falta una": `cargo check --workspace` y `cargo test --workspace` con
las **cuatro dependencias ausentes a la vez**.

| Comando | Exit code | Resultado observado |
| --- | --- | --- |
| `cargo check --workspace` | 0 | pasó, sin advertencias de compilación |
| `cargo test --workspace` | 0 | **363 tests, 0 fallos, 0 ignorados** (54 + 1 + 4 + 2 + 299 + 3 + 0 doc-tests), idéntico al recuento de §3.18 |

La ausencia se verificó **antes** de correr los comandos; sin ese paso un exit 0 no
probaría nada:

| Dependencia | Comprobación de ausencia | Resultado observado |
| --- | --- | --- |
| `libwebkit2gtk-4.1-dev` | `pkg-config` sobre `webkit2gtk-4.1`, `webkit2gtk-4.0`, `javascriptcoregtk-4.1`, `libsoup-3.0`; `ldconfig -p`; `/usr/lib/x86_64-linux-gnu`; `~/.nix-profile/lib` | ausente en todas |
| `libappindicator3-dev` | `pkg-config` sobre `appindicator3-0.1` y `ayatana-appindicator3-0.1`; mismas rutas de bibliotecas | ausente en todas |
| `librsvg2-dev` | `pkg-config` sobre `librsvg-2.0`; mismas rutas de bibliotecas | ausente en todas |
| `patchelf` | `command -v patchelf` | ausente del `PATH` |

`dpkg` no existe en la máquina, así que ninguno de los cuatro **paquetes Debian** puede
estar instalado como tal. Hay derivaciones de `webkitgtk`, `librsvg` y `patchelf` en
`/nix/store`, pero no están en el perfil del usuario, ni en `PKG_CONFIG_PATH`, ni en
`PATH`: el build no las alcanza.

Evidencia estructural que acompaña al resultado: sobre el grafo resuelto con
`--all-features` (**543 paquetes**), solo **5** declaran `links` — `libdbus-sys` (`dbus`),
`libsqlite3-sys` (`sqlite3`), `objc-sys` (solo macOS), `ring` (código vendorizado propio) y
`wasm-bindgen-shared` (solo wasm). Ninguno de los cuatro paquetes del workflow tiene
consumidor en el grafo. La búsqueda en `Cargo.lock` de crates de la pila
WebKit/GTK/Tauri (`webkit`, `soup`, `javascriptcore`, `appindicator`, `rsvg`, `gtk`, `gdk`,
`glib`, `gobject`, `cairo`, `pango`, `atk`, `tauri`, `wry`, `tao`, `tray`) no arroja
**ninguna** coincidencia, consistente con el test `no_tauri_dependency`. Las dos
bibliotecas de sistema que el workspace realmente necesita en Linux, `dbus-1` y `sqlite3`,
**no figuran** en el paso de instalación: llegan por la imagen base de `ubuntu-22.04`.

#### Resultado por dependencia

Una fila por dependencia, con el **resultado observado**, no la expectativa. La columna
"Resultado sin ella" registra la corrida del experimento complementario descrito arriba:
la corrida por dependencia en `ubuntu-22.04` **no existe** y así queda dicho en cada fila.

| Dependencia | Resultado sin ella | Decisión | Razón |
| --- | --- | --- | --- |
| `libwebkit2gtk-4.1-dev` | **No medido aisladamente en `ubuntu-22.04`** (no hubo corrida de CI). Observado con las cuatro ausentes a la vez en NixOS: `cargo check --workspace` exit 0 y `cargo test --workspace` exit 0, 363 tests, 0 fallos | **Proponer eliminación; no aplicada.** Permanece en `ci.yml` | Ausencia verificada en cuatro módulos de `pkg-config` y en las rutas de bibliotecas, y aun así check y test pasan. Cero crates de la pila WebKit en el grafo de 543 paquetes. Era dependencia de Tauri, y Tauri ya no está en el grafo (`no_tauri_dependency` pasa). iced 0.14 renderiza con wgpu/tiny-skia sobre winit |
| `libappindicator3-dev` | **No medido aisladamente en `ubuntu-22.04`**. Observado en la misma corrida con las cuatro ausentes: check exit 0, test exit 0, 363 tests | **Proponer eliminación; no aplicada.** Permanece en `ci.yml` | Misma verificación de ausencia. Ningún crate de bandeja o indicador en el grafo, y ningún módulo del workspace implementa icono de bandeja |
| `librsvg2-dev` | **No medido aisladamente en `ubuntu-22.04`**. Observado en la misma corrida con las cuatro ausentes: check exit 0, test exit 0, 363 tests | **Proponer eliminación; no aplicada.** Permanece en `ci.yml` | Ausencia verificada; ningún crate del grafo la enlaza; los íconos de `[package.metadata.packager]` son PNG/ICNS/ICO, no SVG. La reserva es de empaquetado, no de compilación: la ruta `deb`/AppImage podría convertir íconos y eso **no se midió** |
| `patchelf` | **No medido aisladamente en `ubuntu-22.04`**. Observado en la misma corrida con las cuatro ausentes del `PATH`: check exit 0, test exit 0, 363 tests | **Conservar** (Req 10.5), por empaquetado y no por compilación | No es necesaria para compilar ni testear: `patchelf` reescribe `RPATH`/intérprete de un ELF **ya compilado**, `cargo check` no produce ELF y `cargo test` no post-procesa binarios; ningún build script del grafo la invoca. Se conserva por el job `build` de `release.yml`, que genera AppImage, donde las herramientas de AppImage la usan para corregir el `RPATH`. Que `cargo-packager 0.11.8` la invoque es una **presunción explícitamente no verificada acá**: `cargo packager` no se ejecutó |

`.github/workflows/ci.yml` **no fue modificado por esta tarea y las cuatro dependencias
siguen instaladas** (igual que en `release.yml`, que repite el mismo paso en tres jobs). La
eliminación de tres de las cuatro queda registrada como **propuesta**, no como cambio
aplicado.

#### Qué establece este resultado y qué no

- **Establece** que ninguna de las cuatro es necesaria para `cargo check --workspace` ni
  `cargo test --workspace` en esta máquina, con la ausencia verificada de las cuatro.
- **No establece** el resultado en `ubuntu-22.04`. La evidencia es de NixOS. El grafo de
  Cargo es el mismo en las dos máquinas y es el grafo el que determina qué bibliotecas de
  sistema se necesitan, pero una diferencia propia de la imagen del runner no queda
  descartada. Cerrarlo requiere las cuatro corridas de CI que este entorno no puede hacer.
- **No establece** nada sobre el empaquetado. El Req 10.5 conserva una dependencia si es
  necesaria para compilar, testear **o empaquetar**, y `cargo packager` no se ejecutó. Es
  la razón por la que `patchelf` se conserva y por la que la propuesta sobre `librsvg2-dev`
  lleva reserva.
- **No establece** nada sobre tiempo de ejecución: `cargo check` y `cargo test` son
  headless, no abren ventana y no inicializan el backend gráfico.
- Por lo tanto, la verificación del Req 10.4 está **incompleta** según la letra del
  procedimiento, y por el Req 10.5 las cuatro permanecen mientras lo esté.

#### Deuda_Registrada con hito propietario (Req 10.7)

Estas entradas no reemplazan a las de §14 ni a las de `docs/known-limitations.md`: son las
mismas limitaciones `L5` y `L6`, con el estado actualizado por esta tarea.

| Deuda | Estado observado | Hito propietario |
| --- | --- | --- |
| **Ausencia de matriz multiplataforma en CI**: `ci.yml` corre `static-checks` y `workspace` únicamente en `ubuntu-22.04`; no hay pata de Windows ni de macOS ni de X11 | Confirmado leyendo `ci.yml` (dos jobs, ambos `runs-on: ubuntu-22.04`). Ya cubierto por `L5` de `docs/known-limitations.md`; acá no se amplía, se referencia | **Hito 3**, `L5` |
| **Ausencia de generación de docs en CI**: ningún job ejecuta `cargo doc`, así que las advertencias de rustdoc no tienen gate. §3.10 muestra que en local pasa con exit 0, lo que no es lo mismo que verificarlo en CI | Confirmado leyendo `ci.yml`. Ya cubierto por `L5` | **Hito 3**, `L5` |
| **Ausencia de auditoría de dependencias en CI**: ningún job ejecuta `cargo deny` ni `cargo audit`, y en este entorno tampoco están instalados (§3.11, §3.12), así que el estado de licencias y de vulnerabilidades conocidas es **desconocido**, no "bueno" | Confirmado leyendo `ci.yml`. Ya cubierto por `L5` y `L4` | **Hito 3**, `L5` |
| **`ci.yml` instala cuatro dependencias del sistema sin evidencia completa de que sean necesarias** | Estado actualizado por esta tarea: tres con propuesta de eliminación y una a conservar por empaquetado, ninguna eliminada, verificación incompleta por falta de corrida en CI y de prueba de empaquetado. `L6` sigue abierta con ese matiz | **Hito 1** para completar la verificación (corrida de CI por dependencia + `cargo packager`); la eliminación efectiva del paso queda al mismo hito, `L6` |

Declaraciones explícitas:

- **No se afirma** que las cuatro dependencias sean innecesarias en CI. Se afirma que no lo
  son para `cargo check` ni `cargo test` en esta máquina, con las cuatro ausentes.
- **No se afirma** que la propuesta de eliminación esté aplicada. `ci.yml` y `release.yml`
  no fueron tocados por esta tarea.
- **No se afirma** que `patchelf` sea necesaria: se conserva por una presunción de
  empaquetado que queda registrada como no verificada, en vez de eliminarla sin medir el
  AppImage.
