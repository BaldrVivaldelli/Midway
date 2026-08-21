# ADR 0002 — iced queda pinneado en `=0.14.0`

- **Estado**: Aceptado
- **Fecha**: 2026-08-15
- **Requisitos**: 7.2, 11.10
- **Evidencia**: `midway-desktop/Cargo.toml`, `Cargo.lock`, `docs/product-audit.md` §2
  (entorno de ejecución) y §14 (plataformas de primer nivel)

## Contexto

`midway-desktop` es un binario iced de escritorio; iced gobierna el render, el layout y el
manejo de eventos de entrada. Un cambio de versión ahí toca la aplicación entera, no un
módulo.

Estado verificado leyendo los manifiestos en el HEAD del ADR 0001.

`midway-desktop/Cargo.toml` — pin exacto, sin rango abierto:

```toml
iced = { version = "=0.14.0", features = ["tokio", "highlighter", "tiny-skia"] }
iced_highlighter = "=0.14.0"
```

`Cargo.lock` — versiones resueltas de la familia iced (trece paquetes), obtenidas con
`grep -A1 '^name = "iced' Cargo.lock`:

| Crate | Versión en `Cargo.lock` |
| --- | --- |
| `iced` | `0.14.0` |
| `iced_core` | `0.14.0` |
| `iced_debug` | `0.14.0` |
| `iced_futures` | `0.14.0` |
| `iced_graphics` | `0.14.0` |
| `iced_highlighter` | `0.14.0` |
| `iced_program` | `0.14.0` |
| `iced_renderer` | `0.14.0` |
| `iced_runtime` | `0.14.0` |
| `iced_tiny_skia` | `0.14.0` |
| `iced_wgpu` | `0.14.0` |
| `iced_widget` | `0.14.2` |
| `iced_winit` | `0.14.0` |

`iced_widget` resolvió a `0.14.2` y no a `0.14.0` porque el pin exacto está sobre `iced`,
no sobre cada subcrate. El manifiesto de `iced 0.14.0` en el caché del registro declara
(`~/.cargo/registry/src/index.crates.io-*/iced-0.14.0/Cargo.toml:166-167`):

```toml
[dependencies.iced_widget]
version = "0.14.0"
```

En semver eso es el rango compatible `^0.14.0`, que admite cualquier `0.14.x` con
`x >= 0`. `0.14.2` cae dentro del rango, así que la resolución es legítima. **No se
afirma** que `0.14.2` sea el patch más alto publicado: verificarlo exigiría consultar el
índice de crates.io, y este ADR se escribe sin esa consulta. Lo que importa para la
decisión es lo otro, que sí es verificable en el repo: el pin de la raíz sigue siendo
`=0.14.0` y el lockfile congela las trece versiones de la tabla, así que la resolución no
se mueve mientras `Cargo.lock` no se regenere.

El entorno donde se audita no puede validar un cambio de versión de UI
(`docs/product-audit.md` §2): `DISPLAY` **no está definida**, solo hay
`WAYLAND_DISPLAY=wayland-1`, y `command -v` no encuentra `rustfmt`, `clippy-driver`,
`cargo-deny`, `cargo-audit` ni `cargo-llvm-cov` (rustc 1.95.0 desde tarball, sin `rustup`,
así que no hay mecanismo de componentes para agregarlos). Toda verificación gráfica manual
se registra como `No_Verificable_En_Entorno`.

Además, las cuatro plataformas de primer nivel del Req 11.4 —Linux Wayland, Linux X11,
Windows y macOS— **no tienen ninguna verificación gráfica registrada**, y así consta en
`docs/product-audit.md` §14. Solo Linux tiene compilación y tests verificados, en una sola
máquina y headless. Windows y macOS no tienen ni compilación verificada en este entorno.

Actualizar iced en estas condiciones significaría cambiar el motor de la UI sin poder abrir
la ventana para ver el resultado.

## Decisión

1. iced queda **fijo en `=0.14.0`**, tal como está pinneado en
   `midway-desktop/Cargo.toml` y resuelto en `Cargo.lock`. `iced_highlighter` queda en
   `=0.14.0`.
2. `Cargo.lock` es autoritativo y está versionado en git (`git ls-files Cargo.lock` lo
   confirma). Este spec **no lo regenera**, no corre `cargo update` y no toca las versiones
   resueltas de la familia iced, incluido el `iced_widget 0.14.2` ya presente. Ese `0.14.2`
   es parte del baseline, no una desviación a corregir.
3. Cualquier actualización de la versión de iced queda **fuera del alcance de este spec**
   (Req 11.10), sin excepción: también si apareciera una versión nueva mientras el spec
   está en curso, y también si esa versión resolviera algo que acá se registra como
   limitación.
4. Una actualización futura exige, como condiciones acumulativas y previas al merge
   (Req 7.2):
   - **ADR propio** que supersede a este, con la versión destino y el motivo concreto (no
     "estar al día");
   - **verificación multiplataforma** sobre las cuatro plataformas de primer nivel —Linux
     Wayland, Linux X11, Windows y macOS— con evidencia registrada por plataforma, no
     deducida de que una compile;
   - **plan de rollback** escrito y probado: volver `Cargo.toml` y `Cargo.lock` a
     `=0.14.0` debe dejar la app en el comportamiento previo, y eso se verifica antes de
     adoptar la versión nueva, no después.
5. Si un requisito futuro solo puede cumplirse con una versión más nueva de iced, se
   registra como limitación conocida con hito propietario en `docs/known-limitations.md`.
   No se sube la versión de forma oportunista para desbloquear una tarea.

## Consecuencias

- El comportamiento de render, layout y eventos de entrada se mantiene constante mientras
  se extrae la vertical Tema/Ajustes y se agrega el divisor horizontal del panel de
  respuesta. Si algo cambia visualmente, la causa está en el código del repo, no en el
  framework. Eso es lo que hace utilizable la comparación contra el baseline.
- La imposibilidad de verificar gráficamente en este entorno deja de ser un riesgo de
  entrega: no hay cambio de framework que verificar.
- Se acepta quedarse sin las mejoras y las correcciones de versiones posteriores de iced
  durante este spec, incluidas las que podrían simplificar el divisor arrastrable. El
  divisor se implementa con los widgets y el manejo de eventos de `0.14.0`.
- Un `cargo update` accidental que mueva la familia iced es una regresión de esta decisión
  y debe revertirse, no documentarse como hecho consumado. La forma de detectarlo es el
  diff de `Cargo.lock`.
- El pin queda alineado con la política de dependencias del proyecto: sin rangos abiertos
  en dependencias que definen la superficie de la aplicación. Con la salvedad registrada de
  que el pin exacto de la raíz no se propaga a los subcrates, y por eso el lockfile es la
  segunda mitad indispensable de la garantía.
