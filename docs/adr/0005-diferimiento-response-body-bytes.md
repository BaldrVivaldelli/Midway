# ADR 0005 — Diferimiento del rediseño de `ResponseBody` a bytes

- **Estado**: Aceptado
- **Fecha**: 2026-08-15
- **Requisitos**: 7.5, 12.4, 12.5, 12.6
- **Hito propietario del trabajo diferido**: **Hito 5 — Rediseño de la representación de
  respuesta a bytes**, según el registro de hitos de `docs/known-limitations.md` §1. Las
  limitaciones L22 y L23 de ese documento son las entradas correspondientes.

## Contexto

La representación canónica de una respuesta HTTP en el baseline es **texto**, no bytes.
Verificado leyendo el fuente:

- `midway-core/src/domain/http.rs`, `ResponseEnvelope`, guarda el payload en
  `body_text: String`. No existe ningún campo de bytes. Los campos vecinos son
  `status`, `status_text`, `headers`, `duration_ms`, `size_bytes`, `final_url`,
  `received_at`, `truncated`, `body_evicted` y `total_size_bytes: Option<u64>`.
- `midway-core/src/infra/http_reqwest.rs`, en
  `execute_request_with_body_limit`, acumula el stream en un `Vec<u8>` y lo convierte con
  `String::from_utf8(buffer)`; cuando el payload no es UTF-8 válido cae en
  `String::from_utf8_lossy(error.as_bytes()).into_owned()`. **Ahí se pierde información de
  forma irreversible**: cada secuencia inválida se sustituye por `U+FFFD` y el buffer
  original se descarta. El `Vec<u8>` es local a esa función y no sale de ella: el único
  campo que viaja al llamador es el `String` ya convertido.
- `DEFAULT_MAX_BODY_BYTES = 8 * 1024 * 1024` (8 MiB) acota lo que se retiene. Al alcanzar el
  límite se deja de acumular, se marca `truncated = true` y el resto del stream se descarta.
  Si el corte cae en medio de un carácter multibyte, la conversión con reemplazo también
  actúa (comportamiento fijado por
  `midway-core/tests/response_body_limit.rs::truncation_does_not_corrupt_multibyte_characters`).
- `size_bytes` se calcula como `body_text.len() as u64`
  (`midway-core/src/infra/http_reqwest.rs`): es el tamaño en bytes del **texto ya
  convertido**, no del payload original ni del buffer recibido. Con reemplazo lossy los dos
  números difieren, porque cada `U+FFFD` ocupa 3 bytes en UTF-8 y sustituye una secuencia
  inválida que podía ocupar menos. `total_size_bytes` guarda el `Content-Length` informado
  por el servidor cuando existe (`response.content_length()`).
- `body_evicted` lo activa `midway-desktop/src/app.rs::evict_inactive_response_bodies`, que
  vacía el `body_text` de las tabs inactivas y conserva la metadata; antes de vaciarlo,
  rellena `total_size_bytes` con `size_bytes` si venía en `None`, para que la UI pueda seguir
  informando un tamaño después de la liberación.

Consumidores actuales de `body_text`, todos acoplados al tipo `String`:

| Consumidor | Uso |
| --- | --- |
| `midway-core/src/domain/testing.rs::extract_actual_value` | `AssertionSource::BodyText` clona el texto; `AssertionSource::JsonPointer` hace `serde_json::from_str` sobre él |
| `midway-desktop/src/ui/response_inspector.rs` | `clamp_rendered_body` recorta el texto antes de pasarlo al widget; el aviso de body liberado usa `body_evicted` |
| `midway-desktop/src/app.rs::evict_inactive_response_bodies` | asigna `String::new()` y marca `body_evicted` |
| `midway-desktop/src/collection_runner.rs` | recibe el `RequestExecutionOutcome`, usa `status` y `duration_ms` para el reporte y **descarta el body** al persistir el historial |

Consecuencia funcional del diseño actual: una respuesta binaria (imagen, protobuf, gzip sin
descomprimir, PDF) no se puede recuperar tal cual llegó, ni guardar a disco byte a byte, ni
verificar por hash. Es Deuda_Registrada declarada en `docs/product-audit.md` (Req 5.4, 5.5).

Dato relevante para el costo del rediseño: **ningún camino de código escribe un
`ResponseEnvelope` a disco**. Verificado leyendo los tres caminos de escritura que existen:

- La tabla `history` (`midway-core/src/infra/sqlite_repository.rs`, `migrate`) tiene nueve
  columnas: `id`, `request_name`, `method`, `url`, `environment_name`, `response_status`,
  `duration_ms`, `error_message`, `created_at`. No hay columna de body. `append_history`
  recibe `Option<ResponseEnvelope>` y solo le extrae `status` y `duration_ms`; el resto,
  incluido `body_text`, se descarta en el mismo `INSERT`. Sus dos llamadores
  (`midway-desktop/src/app.rs::execute_send` y `midway-desktop/src/collection_runner.rs`)
  pasan por ahí.
- `SessionSnapshot` (`midway-desktop/src/session.rs`) y `TabSnapshot`
  (`midway-desktop/src/app.rs`, tres campos: `id`, `draft`, `active_request_tab`) persisten
  el `RequestDraft` y la tab activa. Ninguno tiene campo de respuesta.
- El export nativo (`midway-core/src/domain/interop.rs::make_native_bundle`) serializa el
  `WorkspaceSnapshot`, cuyas filas de historial son las de la tabla de arriba.

Salvedad: `ResponseEnvelope` **sí** deriva `Serialize`/`Deserialize` y sus tres flags llevan
`#[serde(default)]`, o sea que la forma serializada existe como tipo. Lo que no existe es un
llamador que la escriba en disco. Esto no se afirma como diseño intencional: es lo que hay
hoy en el fuente.

## Decisión

**Se difiere el rediseño de la representación de respuesta a bytes al Hito 5.** No se aborda en
este spec.

Durante este spec:

1. `ResponseEnvelope.body_text: String` permanece **sin cambios**, igual que
   `DEFAULT_MAX_BODY_BYTES`, `truncated`, `body_evicted`, `size_bytes` y `total_size_bytes`.
   El comportamiento observable del baseline se conserva.
2. No se introduce un campo de bytes paralelo, ni un `enum ResponseBody`, ni una capa de
   compatibilidad que sostenga las dos representaciones a la vez. Un rediseño a medias
   duplicaría el payload en memoria, que es exactamente el problema que el límite de 8 MiB
   vino a resolver.
3. No se agrega ninguna funcionalidad que dependa de bytes fieles: guardar la respuesta a
   disco, previsualizar imágenes, ver el body en hexadecimal, verificar checksums o
   descomprimir manualmente. Prometerlas sobre `body_text` sería funcionalidad simulada
   (Req 14.2).
4. La pérdida queda **declarada**, no oculta: se registra en `docs/product-audit.md` como
   Deuda_Registrada y en `docs/known-limitations.md` como `Diferido` con este hito
   propietario (Req 12.4, 12.5).
5. No se diseña aquí la forma final del tipo (Req 12.6). Este ADR fija el diferimiento y su
   dueño, no la solución.

Razón del diferimiento, y no de la implementación inmediata: el cambio toca la firma del tipo
que cruza `midway-core` → `midway-desktop`, la evaluación de assertions del dominio, el
render del inspector y la eviction de bodies por tab. Es un rediseño transversal cuyo riesgo
no tiene nada que ver con el objetivo de este spec, que es auditar el baseline y extraer una
vertical **sin cambiar comportamiento**. Mezclarlos haría imposible atribuir una regresión a
una causa.

## Consecuencias

Lo que se acepta a cambio:

- Las respuestas no UTF-8 siguen mostrándose con caracteres de reemplazo y no hay forma de
  recuperar el payload original desde la app. Es una pérdida real de datos del usuario,
  visible en cada respuesta binaria.
- Las respuestas de más de 8 MiB siguen recortándose, y `size_bytes` sigue midiendo el texto
  retenido y convertido, no el payload original. Quien lea la métrica sin leer `truncated`
  saca una conclusión equivocada.
- Las assertions sobre body (`AssertionSource::BodyText`, `AssertionSource::JsonPointer`)
  operan sobre el texto ya degradado. Un payload no UTF-8 puede hacer fallar una assertion por
  la conversión, no por el contenido.
- La Matriz_Funcionalidades no puede declarar `Verified` ninguna funcionalidad que dependa de
  fidelidad de bytes, porque el baseline no la ofrece.

Lo que el diferimiento preserva:

- El límite de 8 MiB y la eviction por tab siguen acotando el uso de memoria, que era la razón
  original de este diseño. Diferir no reintroduce el problema de RAM.
- El rediseño **no arrastra migración de persistencia**: como ni `history` ni `session.json`
  guardan el body, el Hito 5 no necesita cambiar `SESSION_SCHEMA_VERSION` ni el esquema
  SQLite. Esto reduce de forma sustancial el riesgo del trabajo futuro y es la razón por la
  que se puede posponer sin que el costo crezca. Queda separado del Hito 6, que es el dueño de
  la re-arquitectura de persistencia.
- Los tests existentes de `midway-core/tests/response_body_limit.rs` quedan como
  caracterización del comportamiento actual y serán la referencia contra la que el Hito 5
  demuestre que no rompió el corte por límite (Req 6.8).

Invariantes que el Hito 5 no puede romper, sea cual sea la forma que elija (esto **no** es el
diseño del rediseño, que queda para ese hito según el Req 12.6; son las restricciones que el
baseline ya tiene y que hay que conservar):

1. El corte por `DEFAULT_MAX_BODY_BYTES` y la eviction por tab siguen vigentes, con los mismos
   límites observables. La razón de existir de ambos —acotar la RAM— no cambia.
2. Las assertions de dominio siguen dando el mismo resultado para payloads UTF-8 válidos.
   `midway-core/src/domain/testing.rs` es el juez y sus resultados no pueden moverse por un
   cambio de representación.
3. La conversión con pérdida deja de ser silenciosa. Hoy `from_utf8_lossy` es un camino sin
   señal; cualquier forma que el Hito 5 adopte tiene que poder decir que el body no es texto.
4. Los cuatro tests de `midway-core/tests/response_body_limit.rs` son la referencia de
   caracterización contra la que ese hito demuestra que no rompió el corte por límite
   (Req 6.8).

Este ADR se revisa solo si el Hito 5 arranca o si aparece un requisito que no se pueda cumplir
sobre `body_text`. Mientras eso no pase, la decisión está tomada y no se reabre.
