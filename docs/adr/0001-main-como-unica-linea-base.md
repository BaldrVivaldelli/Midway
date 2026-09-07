# ADR 0001 — `main` es la única línea base

- **Estado**: Aceptado
- **Fecha**: 2026-08-15
- **Requisitos**: 7.1, 2.4, 2.5
- **Evidencia**: `docs/product-audit.md` §1 (identidad del baseline), más los comandos de
  solo lectura transcritos abajo

## Contexto

El repositorio tiene dos ramas, verificadas con `git branch -a` (solo lectura):

- `main` y `remotes/origin/main` (con `remotes/origin/HEAD -> origin/main`)
- `feat/tauri-to-iced-migration` y `remotes/origin/feat/tauri-to-iced-migration`

Estado de `main` al momento de esta decisión, ya registrado en `docs/product-audit.md` §1
y reverificado acá con comandos de solo lectura:

| Dato | Comando | Valor observado |
| --- | --- | --- |
| Rama | `git rev-parse --abbrev-ref HEAD` | `main` |
| HEAD | `git rev-parse HEAD` | `1fc270326e5c304f24ce08fe1f528da33a007d3e` |
| Remoto | `git rev-parse origin/main` | `1fc270326e5c304f24ce08fe1f528da33a007d3e` |
| Divergencia | `git rev-list --left-right --count HEAD...origin/main` | `0 0` |
| Árbol de trabajo | `git status --short` | salida vacía al momento de la captura: limpio |

Sobre la última fila: `git status --short` está limpio **respecto del código**. Al
reverificar durante la escritura de este ADR, el comando ya lista como no rastreados los
documentos que produce este mismo spec (`docs/adr/`, `docs/architecture.md`,
`docs/baseline-evidence/`, `docs/feature-matrix.md`, `docs/known-limitations.md`,
`docs/product-audit.md`, `docs/product-principles.md`). Ningún `.rs`, ni `Cargo.toml`, ni
`Cargo.lock`, ni workflow aparece modificado. Se registra así para que la reproducción del
comando no contradiga lo escrito.

`HEAD` y `origin/main` apuntan al mismo commit: no hay trabajo local por delante ni por
detrás del remoto.

La migración de Tauri a iced ya está incorporada en `main`, y eso también se verificó
leyendo el árbol en ese HEAD:

- `Cargo.toml` declara `members = ["midway-core", "midway-desktop"]`. No hay miembro de
  workspace de Tauri.
- `midway-desktop/Cargo.toml` depende de `iced = { version = "=0.14.0", ... }`. La cadena
  `tauri` aparece en cuatro líneas de ese manifiesto (`grep -ci 'tauri'
  midway-desktop/Cargo.toml` devuelve `4`: líneas 35, 41, 48 y 77), y las cuatro son
  comentarios de la sección de empaquetado que explican qué reemplazó `cargo-packager`;
  ninguna es una dependencia.
- `Cargo.lock` no contiene ningún paquete `tauri` ni `tauri-*`:
  `grep -c '^name = "tauri' Cargo.lock` devuelve `0`.
- `midway-core/tests/no_tauri_dependency.rs` fija esa ausencia como test del baseline:
  recorre el subgrafo transitivo de `midway-core` en `cargo metadata` y falla si aparece
  `tauri` o cualquier `tauri-*`.
- El único resto de Tauri en el disco es el directorio `src-tauri/target`, cuyo contenido
  son artefactos de build (`CACHEDIR.TAG`, `release/`, `.rustc_info.json`). Está cubierto
  por `.gitignore` (`git check-ignore -v src-tauri/target/CACHEDIR.TAG` →
  `.gitignore:42:src-tauri/target`) y no tiene un solo archivo versionado:
  `git ls-files src-tauri` no devuelve nada.

Y el dato que cierra la discusión sobre la rama histórica:

```
$ git rev-list --left-right --count HEAD...feat/tauri-to-iced-migration
1       0
```

Cero commits del lado derecho. `feat/tauri-to-iced-migration`
(`9e0173ac4421c25f274404318c9a308e0d46c9a2`) **no tiene ningún commit que no esté ya en
`main`**: es ancestro de `main`, que además la aventaja en un commit. La relación de
ancestría se confirma aparte, sin ambigüedad:

```
$ git merge-base --is-ancestor feat/tauri-to-iced-migration HEAD
$ echo $?
0
```

No hay trabajo pendiente de traer desde ahí.

Pese a eso, la rama sigue existiendo local y remotamente. Sin una decisión escrita, cada
revisión futura puede reabrir la pregunta de si falta rescatar algo de ahí, o si conviene
reejecutar la migración desde cero para "hacerla bien". Ambas preguntas cuestan tiempo y
arriesgan operaciones de git destructivas (merge, rebase, `cherry-pick`, force-push) sobre
la única rama que hoy compila y tiene evidencia registrada.

## Decisión

1. `main` en `1fc270326e5c304f24ce08fe1f528da33a007d3e` es la **única línea base
   autorizada**. Toda auditoría, todo test de caracterización y todo refactor de este spec
   se mide contra ese commit.
2. Se **descarta reejecutar** la migración de Tauri a iced (Req 2.5). La migración se
   considera hecha y su resultado es el código de `main`. Cualquier defecto que se
   encuentre se trata como defecto del baseline, con hito propietario en
   `docs/known-limitations.md`, no como motivo para volver a migrar.
3. Queda **fuera de alcance todo aporte de código** proveniente de
   `feat/tauri-to-iced-migration` o de cualquier otra rama histórica (Req 2.4). No se hace
   merge, ni rebase, ni `cherry-pick`, ni copia manual de archivos desde esas ramas. El
   conteo `1 0` muestra que no hay nada que aportar, así que la restricción no cuesta nada.
4. No se cambia de rama y no se crean ramas nuevas durante la ejecución de este spec
   (Req 2.1). Los comandos de git admitidos son de solo lectura (`git rev-parse`,
   `git rev-list`, `git status`, `git branch`, `git ls-files`).
5. Si aparece una operación que exigiría `git reset --hard`, `git clean -fd`,
   `git checkout -- .`, rebase, merge, force-push o `cherry-pick`, el trabajo se **detiene
   y se registra la necesidad** en lugar de ejecutar la operación (Req 2.3).
6. La rama `feat/tauri-to-iced-migration` no se borra. Se deja como registro histórico,
   sin autoridad sobre el código.
7. El directorio `src-tauri/target` no se toca: es artefacto de build ignorado, no código.
   Borrarlo no aporta nada al spec y salir a limpiar el árbol no es tarea de esta línea
   base.

## Consecuencias

- La comparación de regresiones tiene una referencia única y verificable. "Igual que
  antes" significa "igual que `1fc2703`", no "igual que mi recuerdo".
- Se evita el riesgo de perder trabajo: sin merges ni rebases sobre `main`, el árbol
  limpio registrado en la auditoría se mantiene, y este spec no puede destruir historia.
- Si alguien creía que `feat/tauri-to-iced-migration` guardaba algo valioso, la evidencia
  dice que no: está contenida en `main`. Y si en el futuro apareciera un commit nuevo en
  esa rama, tampoco entra por esta vía: la forma de incorporarlo es un spec propio que lo
  reimplemente sobre `main`, con su propia evidencia.
- La creación de commits, el push y la apertura de pull requests quedan fuera del alcance
  de este spec (Req 2.2): el usuario decide cuándo y cómo publicar los cambios. En
  particular, los documentos y ADRs que produce este spec quedan como cambios sin
  commitear en el árbol de trabajo.
- Cualquier propuesta futura de volver a migrar, de cambiar de framework de UI o de
  adoptar código de una rama histórica necesita un ADR nuevo que supersede a este. No
  alcanza con una discusión en una revisión.
