# Verificación de la compuerta de bloqueo hacia la Fase 8 — Midway_Desktop

> **Artefacto de compuerta (Tarea 17.4, Requisito 10, Criterio 10.8).**
> Este documento verifica formalmente la compuerta que gobierna el inicio de la
> **limpieza final (Fase 8, Tarea 18)**. Toma como entrada el reporte de diferencias
> clasificadas (Tarea 17.3 → [`docs/differences-report.md`](./differences-report.md)),
> la comparación funcional manual final (Tarea 17.2 →
> [`docs/functional-comparison.md`](./functional-comparison.md)) y el resumen escrito
> de equivalencia (Tarea 17.1 →
> [`docs/functional-equivalence.md`](./functional-equivalence.md)), y emite un
> **veredicto GO / NO-GO** sobre si la Fase 8 puede proceder.

## Criterio que gobierna esta compuerta

**Requisito 10, Criterio 10.8 (texto normativo):**

> IF el reporte de diferencias identifica una regresión bloqueante que **no está
> documentada** o **no ha sido aceptada explícitamente** por el responsable técnico de
> la migración, THEN THE Workspace_Cargo SHALL **bloquear la limpieza final** descrita
> en el Requisito 9 hasta que dicha regresión sea **resuelta o aceptada explícitamente**.

En otras palabras, la compuerta se **activa (bloquea)** únicamente cuando existe al
menos una **regresión bloqueante** que quede *sin documentar* o *sin aceptación
explícita*. La Fase 8 (eliminación del crate `midway`, de `src-tauri/` y de `src/`) es
una operación **irreversible**, por lo que esta verificación es la última salvaguarda
antes de proceder.

## Método de verificación (Tarea 17.4)

1. Enumerar, desde `docs/differences-report.md`, **toda** diferencia clasificada como
   **regresión bloqueante**.
2. Para cada regresión bloqueante, comprobar que esté (a) **documentada** y (b)
   **aceptada explícitamente** por el responsable técnico de la migración, **o** bien
   **resuelta**.
3. Registrar el estado de cada una como **RESUELTA**, **ACEPTADA** (con quién y cuándo)
   o **PENDIENTE** (sin documentar o sin aceptación explícita).
4. Emitir el veredicto: si **alguna** regresión bloqueante queda **PENDIENTE**, el
   veredicto es **NO-GO** y la Fase 8 queda **bloqueada**; en caso contrario, evaluar
   las precondiciones documentadas de cierre antes de autorizar.

## 1. Inventario de regresiones bloqueantes

Fuente: tabla de clasificación de `docs/differences-report.md` (D1–D9) e ítem de estado
O1, más las notas por fase de `docs/functional-equivalence.md`.

| Diferencia | Área (Fase) | Clasificación en el reporte | ¿Es regresión bloqueante? |
| --- | --- | --- | --- |
| D1 | Resaltado JSON (Fase 1) | No bloqueante (mecanismo) | No |
| D2 | Búsqueda in-editor (Fase 1) | No bloqueante (mecanismo, 11.3) | No |
| D3 | History 50→500 (Fase 3) | No bloqueante (ajuste deliberado, Req 4.6) | No |
| D4 | Diagnostics en archivo JSON (Fase 3) | No bloqueante (mecanismo) | No |
| D5 | Transporte de progreso del runner (Fase 4) | No bloqueante (mecanismo) | No |
| D6 | Autosave `session.json` (Fase 5) | No bloqueante (mecanismo) | No |
| D7 | Error boundary `catch_unwind` (Fase 5) | No bloqueante (mecanismo, 11.3) | No |
| D8 | Updater manual sobre `reqwest` (Fase 6) | No bloqueante (mecanismo, 7.1/11.3) | No |
| D9 | Empaquetado con `cargo-packager` (Fase 7) | No bloqueante (mecanismo, 8.1) | No |
| O1 | Cierre de Fase 7 (pipeline/CI) | Ítem abierto → **Resuelto** | No |

**Regresiones bloqueantes identificadas en el reporte de diferencias: 0.**

## 2. Estado de aceptación / resolución de cada regresión bloqueante

| # | Regresión bloqueante | Estado | Detalle |
| --- | --- | --- | --- |
| — | *(ninguna)* | — | El reporte de diferencias (Tarea 17.3) **no identifica ninguna regresión bloqueante**. Las 9 diferencias (D1–D9) son de mecanismo interno o ajuste deliberado, todas respaldadas por la suite automatizada (Propiedades 1–28 + unit/integración) y/o por la reutilización sin reescritura de `midway-core`. |

Como no existe ninguna regresión bloqueante, **no hay ninguna regresión bloqueante que
haya quedado sin documentar ni sin aceptación explícita**. En consecuencia, la
condición IF del Criterio 10.8 **no se cumple** y dicho criterio, por sí mismo, **no
impone un bloqueo** sobre la Fase 8.

> Nota sobre la firma del responsable técnico: la línea *"Aprobación del responsable
> técnico de la migración"* de la **Parte D** de `docs/functional-comparison.md` está
> **en blanco** al momento de esta verificación. Esto **no** activa el bloqueo del
> Criterio 10.8 (que exige aceptación explícita solo *cuando existe una regresión
> bloqueante*, y no existe ninguna), pero sí es una **precondición operativa** de la
> autorización final de la Fase 8 (ver Sección 3). No se fabrica ni se asume aquí
> ninguna aprobación que no conste firmada en el documento.

## 3. Precondiciones de cierre pendientes (no son regresiones)

La conclusión "0 regresiones bloqueantes" es **provisional** hasta cerrar las
verificaciones que ningún test automatizado puede sustituir. Los propios documentos de
las Tareas 17.2 y 17.3 lo advierten explícitamente: *"si durante esa verificación humana
apareciera una diferencia de comportamiento observable, deberá añadirse a la tabla de
clasificación y evaluarse como potencial regresión bloqueante antes de la firma de la
Parte D"*.

Precondiciones abiertas:

- **P1 — Verificaciones manuales visuales/de plataforma (5):** siguen `☐ Pendiente` en
  `docs/functional-comparison.md`:
  1. Aspecto visual del resaltado JSON en pantalla (relacionado con D1).
  2. Diálogo de archivo del SO al importar/exportar (A.4.6).
  3. Portapapeles del SO al copiar el comando cURL (A.5.4).
  4. Relanzamiento del proceso tras instalar una actualización (B.6.5).
  5. Instalación en sistema limpio en Windows/Linux x86_64 (B.7.1–B.7.2).
- **P2 — Firma de la Parte D:** faltan la firma del/los tester(es) de verificación
  visual/SO y la **aprobación del responsable técnico de la migración** en la Parte D de
  `docs/functional-comparison.md`.

Ninguna de estas precondiciones es, hoy, una regresión bloqueante; son confirmaciones
aún no ejecutadas. Sin embargo, dado que la Fase 8 es **irreversible**, deben cerrarse
antes de proceder, y si P1 revelara una diferencia de comportamiento observable, deberá
clasificarse en `docs/differences-report.md` y re-evaluarse esta compuerta.

## 4. Veredicto

### 4.1 Respecto al Criterio 10.8 (compuerta normativa)

**NO BLOQUEA.** No existe ninguna regresión bloqueante en el reporte de diferencias
(Tarea 17.3); por tanto ninguna regresión bloqueante quedó sin documentar ni sin
aceptación explícita. La condición que activaría el bloqueo del Criterio 10.8 **no se
cumple**.

### 4.2 Autorización operativa de la Fase 8 (Tarea 18)

**NO-GO CONDICIONAL.** La Fase 8 **no debe iniciarse todavía**, y ello **no** se debe a
una regresión bloqueante (no existe ninguna), sino a que permanecen abiertas dos
precondiciones documentadas de cierre:

- **P1**: ejecutar las 5 verificaciones manuales visuales/de plataforma sin que aparezca
  una nueva diferencia de comportamiento observable.
- **P2**: obtener la firma del/los tester(es) y la **aprobación explícita del
  responsable técnico** en la Parte D de `docs/functional-comparison.md`.

**Transición a GO:** una vez cerradas P1 y P2 sin que surja ninguna nueva regresión
bloqueante, esta compuerta pasa a **GO** y la Tarea 18 (limpieza de la Fase 8) queda
autorizada. Si P1 revelara una regresión bloqueante, deberá documentarse y aceptarse
explícitamente (o resolverse) antes de reintentar la compuerta, conforme al Criterio
10.8.

### 4.3 Resumen

| Ítem verificado | Resultado |
| --- | --- |
| Regresiones bloqueantes en el reporte | 0 |
| Regresiones bloqueantes PENDIENTES (sin documentar / sin aceptar) | 0 |
| ¿Criterio 10.8 bloquea la Fase 8? | No |
| Precondiciones operativas abiertas | 2 (P1 verificaciones manuales, P2 firma Parte D) |
| Autorización final para iniciar la Tarea 18 | **NO-GO condicional** (hasta cerrar P1 y P2) |

## 5. Acción sobre la Fase 8 (Tarea 18)

Mientras el veredicto operativo sea **NO-GO condicional**, **no** se ejecuta la Tarea 18:
no se elimina el crate `midway`, ni `src-tauri/`, ni `src/`, ni el tooling npm. La
baseline de Midway_Tauri permanece funcional como referencia hasta que la compuerta pase
a GO.

## Documentos relacionados

- Resumen escrito de equivalencia funcional (Tarea 17.1): [`docs/functional-equivalence.md`](./functional-equivalence.md).
- Comparación funcional manual final (Tarea 17.2): [`docs/functional-comparison.md`](./functional-comparison.md).
- Reporte de diferencias clasificadas (Tarea 17.3): [`docs/differences-report.md`](./differences-report.md).
- Distribución y release: [`docs/distribution.md`](./distribution.md).
