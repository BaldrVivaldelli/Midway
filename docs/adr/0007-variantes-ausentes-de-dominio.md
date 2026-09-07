# ADR 0007 — Variantes ausentes de `HttpMethod`, `BodyMode` y `AuthConfig`

- **Estado**: Aceptado
- **Fecha**: 2026-08-15
- **Requisitos**: 4.7, 12.5, 12.6
- **Consumido por**: `docs/feature-matrix.md` §5 (la columna de estado de cada fila de variante
  ausente se toma de las tablas de decisión de este ADR, sin reinterpretarlas)

## Contexto

La Matriz_Funcionalidades exige una fila por variante ausente de `HttpMethod`, `BodyMode` y
`AuthConfig`, con estado `Not started` o `Intentionally unsupported` "según la decisión
registrada en su ADR" (Req 4.7). Este es ese ADR. Sin él la matriz no puede completarse: no
hay un estado por defecto para "falta".

### Superficie real del baseline, verificada leyendo el fuente

Los tres tipos viven en **el mismo archivo**, `midway-core/src/domain/http.rs`. No en
`midway-core/src/domain/auth.rs`, que no declara `AuthConfig` sino que opera sobre él
(`enum AppliedAuth`, `apply_auth`, `auth_type_name`). Esto importa para el costo: cualquier
variante nueva de auth toca los dos archivos, no uno.

- **`HttpMethod`: 7 variantes** — `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS`.
  Además `pub const ALL: [HttpMethod; 7]` fija el mismo orden en que se presentan al usuario;
  el tipo deriva `Copy`. `midway-desktop/src/ui/request_composer.rs` alimenta el `pick_list` de
  método con `HttpMethod::ALL`, así que la lista de la UI **es** la superficie del dominio: no
  hay métodos alcanzables por otra vía.
- **`BodyMode`: 4 variantes** — `None`, `Json`, `Text`, `FormData`. `FormData` se acompaña de
  `FormDataFieldKind { Text, File }`, o sea que la subida de archivos existe, pero solo como
  campo de un multipart.
- **`AuthConfig`: 4 variantes** — `None`, `Bearer { token }`, `Basic { username, password }`,
  `ApiKey { key, value, placement }`, con `ApiKeyPlacement { Header, Query }`.

Los recuentos están fijados como test en el módulo `domain_surface_tests` de ese mismo archivo
(tarea 4.7): `http_method_all_has_exactly_seven_variants`,
`body_mode_has_exactly_four_variants` y `auth_config_has_exactly_four_variants`. Los tres usan
un `match` exhaustivo sin comodín, así que agregar o quitar una variante primero **no compila**
y después falla la aserción de recuento, y el mensaje de fallo apunta a este ADR y a la matriz.
Este ADR usa esos recuentos verificados, no los de ningún documento de plan.

### Cómo se comporta hoy lo que no está

Los caminos de importación degradan en silencio o con aviso, y eso es evidencia de demanda
real, no especulación:

| Camino | Archivo | Comportamiento |
| --- | --- | --- |
| Método desconocido al importar | `midway-core/src/domain/interop.rs::parse_http_method` | el `match` cierra con `_ => HttpMethod::GET`: cae a `GET`, **sin warning** |
| Método desconocido al leer de SQLite | `midway-core/src/infra/sqlite_repository.rs::method_from_string` | devuelve `None`, y `parse_history_entry_row` lo convierte en error de fila ("Método HTTP inválido: …"): la entrada de historial no se puede leer |
| Tipo de auth Postman no soportado | `midway-core/src/domain/interop.rs::parse_postman_auth` | el `match` cierra con `_ => AuthConfig::None`: cae a `None`, **sin warning** |
| Modo de body Postman no soportado | `midway-core/src/domain/interop.rs::parse_postman_body` | body vacío **con warning** al usuario ("Encontré un body Postman con modo '…' y lo importé como vacío.") |
| `application/x-www-form-urlencoded` en OpenAPI | `midway-core/src/domain/interop.rs` | se importa como `BodyMode::Text`, con el par `k=v&k=v` armado a mano desde el ejemplo del schema y el header `content-type` inyectado si no venía |

Ese último caso importa: hay un **workaround en producción** que simula un modo de body que no
existe. Eso decide por sí solo el estado de `UrlEncoded`.

### Por qué la distinción no es cosmética

- `Not started` = lo queremos, no está hecho, tiene dueño y criterio de cierre. Es deuda.
- `Intentionally unsupported` = decidimos no tenerlo en **este** tipo, con condición explícita
  de reapertura. No es deuda; es alcance.

Marcar todo como `Not started` convertiría la matriz en una lista de deseos sin dueño. Marcar
todo como `Intentionally unsupported` esconde deuda detrás de una decisión que nadie tomó.

## Decisión

Se registra un estado **por variante**, con razón y hito propietario. Este ADR **no crea hitos
nuevos**: usa los del registro de `docs/known-limitations.md` §1, que es la fuente única de
nombres de hito. Los que aplican acá son:

- **Hito 5 — Rediseño de la representación de respuesta a bytes** (ver ADR 0005).
- **Hito 7 — Adaptadores multiprotocolo**, dueño del alcance ya `Diferido` por el Req 12.1.
- **Hito 8 — Superficie de colaboración: mocks, monitores, flows y proxy de captura**, dueño
  del proxy de captura, que es donde se resolvería `CONNECT` si su condición de reapertura se
  cumpliera.
- **Hito 10 — Transporte avanzado, autenticación y endurecimiento del manejo de secretos**,
  que es el hito propietario que la limitación L24 ya asigna a la angostura de estos tres
  tipos.

La columna "Hito objetivo" de `docs/feature-matrix.md` copia estos nombres tal cual. Para las
filas `Intentionally unsupported` que no tienen trabajo previsto en ningún hito (`Xml`,
`OAuth1`, `Ntlm`/`Hawk`) esa columna queda en `—`, porque asignarles un hito sería declarar
un trabajo que la decisión justamente descarta; lo que sí es obligatorio ahí es la condición
de reapertura.

### `HttpMethod`

| Variante ausente | Decisión | Razón |
| --- | --- | --- |
| `TRACE` | **Not started** | Es un método HTTP legítimo y mapea directo a `Method::TRACE` del stack que ya usamos. El costo es acotado y conocido, y está inventariado: la variante, `ALL` de 7 a 8, `curl.rs::METHODS` (otro `[HttpMethod; 7]`), un hue en `design_system::method_color` que respete la separación de ≥ 15° que su test exige, y los seis `match` exhaustivos sobre el tipo (`sqlite_repository.rs::{method_to_string, method_from_string}`, `http_reqwest.rs::to_reqwest_method`, `preview.rs::Display`, `interop.rs::{parse_http_method, http_method_text}`, `curl.rs::method_to_str`), más `app.rs::default_tab_for_method` y su oráculo de test, que agrupan métodos por tab por defecto. Que la mayoría de servidores lo rechacen no es razón para no poder enviarlo: comprobar ese rechazo es diagnóstico válido. Simplemente no se hizo. **Hito 10**. |
| `CONNECT` | **Intentionally unsupported** | `CONNECT` no es un request a un endpoint: establece un túnel y su semántica de respuesta no es un body que se pueda mostrar en el Response_Inspector. En nuestra arquitectura el túnel lo maneja el cliente HTTP por debajo, no el usuario. Exponerlo en el `pick_list` daría un método que produce una respuesta que la app no sabe representar, y eso es una funcionalidad simulada (Req 14.2). **Reapertura**: si alguna vez existe el proxy de captura (hoy `Diferido`, Req 12.2, **Hito 8**), `CONNECT` se resuelve ahí y no como variante de este enum. |
| Método personalizado (p. ej. `Custom(String)`, `PROPFIND`, `LOCK`) | **Not started** | Hay demanda real (WebDAV y APIs internas con verbos propios) y el baseline ni lo soporta ni lo avisa: `parse_http_method` cae a `GET` en silencio, así que un request importado con verbo propio **se corrompe sin decirlo**. Es la peor de las tres ausencias de método. El costo no es cosmético: `HttpMethod` deriva `Copy` y expone `const ALL: [HttpMethod; 7]`; una variante con `String` rompe las dos cosas y obliga a revisar los seis `match` exhaustivos del workspace, más `design_system::method_color`, que hoy asigna un hue por variante y no tiene rama para un valor abierto. Por eso no se cuela en un cambio menor. **Hito 10**. |

### `BodyMode`

| Variante ausente | Decisión | Razón |
| --- | --- | --- |
| `UrlEncoded` (`application/x-www-form-urlencoded`) | **Not started** | El workaround ya existe en producción: el importador de OpenAPI arma `k=v&k=v` a mano, lo mete en `BodyMode::Text` e inyecta el header. Un formato tan común que ya obligó a simularlo es deuda, no alcance descartado. Además es la más barata de las cuatro filas de esta tabla: reutiliza la grilla de filas clave/valor que `FormData` ya tiene. **Hito 10**. |
| `Binary` / archivo crudo (`application/octet-stream`) | **Not started** | Subir un archivo hoy solo se puede como campo de un multipart (`FormDataFieldKind::File`, que además recibe la ruta como texto escrito a mano); un `PUT` de un archivo crudo no se puede expresar. Es un caso legítimo y frecuente en APIs de storage. Salvedad honesta: mientras la respuesta siga siendo texto (ADR 0005), poder **enviar** bytes sin poder **ver** los bytes que vuelven es media funcionalidad. **Hito 10**, con dependencia declarada del **Hito 5**: conviene entregarlo junto o después de él. |
| `GraphQL` | **Intentionally unsupported** | GraphQL no llega como modo de body. El Req 12.1 ya lo declaró `Diferido` como protocolo completo, y un protocolo necesita su propio editor de query y variables, su introspección de esquema y su forma de error, no un `String` con otra etiqueta. Agregar `BodyMode::GraphQL` daría un editor de texto disfrazado de soporte de GraphQL: exactamente la funcionalidad simulada que el Req 14.2 prohíbe. **Reapertura**: no como variante de `BodyMode`; si llega, llega por el adaptador del **Hito 7**. |
| `Xml` | **Intentionally unsupported** | XML ya se envía hoy con `BodyMode::Text` más el header `content-type` correspondiente, y eso no es un workaround: es el uso correcto del modo de texto. Una variante propia solo agregaría resaltado de sintaxis, que es una mejora del editor y no un modo de body. **Reapertura**: si el resaltado se implementa, se resuelve dentro de `Text`. |

### `AuthConfig`

| Variante ausente | Decisión | Razón |
| --- | --- | --- |
| `OAuth2` | **Not started** | Es la ausencia de mayor demanda real de las tres tablas: hoy, para pegarle a una API con OAuth2, hay que conseguir el token por fuera y pegarlo en `Bearer`. No es un tipo nuevo y nada más: necesita flujo de autorización con listener local de loopback, guardado del refresh token en el secret store (`midway-core/src/runtime/secret_executor.rs` ya tiene `get`/`set`/`delete`, pero `set` y `delete` **no tienen llamador en `midway-desktop`**: hoy no hay forma de guardar un secreto desde la app, registrado como `L16`), renovación y manejo de expiración, todo sin filtrar el token a logs, exportaciones, historial ni mensajes de error (Req 11.6). Por eso no es un incremento menor. **Hito 10**. |
| `Digest` | **Not started** | Es demanda real de APIs internas y de dispositivos. Consecuencia estructural que hay que resolver en su hito: `Digest` es desafío-respuesta, o sea **dos** viajes (401 con `WWW-Authenticate`, luego el request firmado), y hoy `midway-core/src/infra/http_reqwest.rs::execute_request_with_body_limit` hace un solo `send()` y devuelve el `ResponseEnvelope`. No es solo una variante del enum: cambia la forma de la ejecución. **Hito 10**. |
| `AwsSigV4` | **Not started** | Firmar peticiones a AWS es un caso concreto y frecuente, y hoy es imposible de expresar: la firma depende del método, la URL canónica, los headers y el hash del payload, así que no hay forma de simularla con `ApiKey`. **Hito 10**. |
| `OAuth1` | **Intentionally unsupported** | Esquema legado, demanda baja, y una superficie de firma criptográfica desproporcionada respecto de su uso actual. **Reapertura**: una necesidad concreta y documentada de un usuario real, no la simetría con otra herramienta. |
| `Ntlm` / `Hawk` | **Intentionally unsupported** | Nicho, y `NTLM` además arrastra negociación multi-viaje dependiente de plataforma, que choca con las cuatro plataformas de primer nivel (Req 11.4). **Reapertura**: igual que `OAuth1`. |
| Certificado de cliente (mTLS) | **Intentionally unsupported** *como variante de `AuthConfig`* | No es un esquema de autenticación de la petición: es configuración de transporte del cliente TLS, y modelarlo como variante de `AuthConfig` lo pondría en el lugar equivocado (se elige por request cuando en realidad aplica al cliente). La necesidad se reconoce y ya está registrada como limitación L28: `midway-desktop/src/state.rs::AppState::initialize` construye el `reqwest::Client` con `redirect::Policy::limited(10)` y `cookie_provider`, y nada más —sin proxy, sin mTLS y sin CA propia—, bajo el **Hito 10**; el lugar no es este enum. **Reapertura**: como ajuste de cliente/entorno, con su propio análisis. |

### Reglas que se derivan y que la matriz debe respetar

1. Los dos únicos estados admitidos en una fila de variante ausente son `Not started` e
   `Intentionally unsupported` (Req 4.7). Quedan descartados `Verified`, `Functional`,
   `Partially wired`, `Scaffolded` y `Blocked`: si la variante no existe en el tipo no hay
   nada cableado, no hay código que no se ejecute y nada externo lo bloquea.
2. `Not started` obliga a hito propietario en la columna "Hito objetivo", tomado del registro
   de `docs/known-limitations.md` §1. `Intentionally unsupported` obliga a condición de
   reapertura escrita, y su "Hito objetivo" es el hito donde se resolvería la reapertura si se
   cumpliera, o `—` cuando no hay ninguno.
3. Las tres columnas de implementación (dominio, infraestructura, persistencia) llevan el
   mismo valor que la columna de UI, porque la ausencia es del tipo de dominio: no hay una
   capa donde exista y otra donde falte. La única excepción es mTLS, que se explica en su
   fila.
4. Este ADR **no** implementa ninguna variante ni diseña su forma final (Req 12.6). Cambiar
   `HttpMethod`, `BodyMode` o `AuthConfig` está fuera del alcance de este spec: los tests de
   `domain_surface_tests` fijan los recuentos 7/4/4 justamente para que un cambio así no pase
   inadvertido.

## Consecuencias

- La matriz queda completable sin inventar estados: cada una de las trece filas de variante
  ausente de `docs/feature-matrix.md` §5 tiene una decisión trazable a este documento.
- Los tests de `domain_surface_tests` 7/4/4 pasan a ser un tripwire con intención declarada. Si
  alguien agrega una variante, el `match` exhaustivo deja de compilar y el mensaje de la
  aserción nombra este ADR: la discusión arranca acá en vez de en un diff.
- Se acepta que las degradaciones silenciosas siguen vigentes en este spec: método desconocido
  → `GET` sin aviso (`interop.rs::parse_http_method`) y tipo de auth desconocido → `None` sin
  aviso (`interop.rs::parse_postman_auth`). Son peores que la ausencia de la variante, porque el
  usuario cree que importó lo que pidió. Estado del registro al escribirse este ADR, verificado
  documento por documento: `docs/known-limitations.md` §2 registra la angostura de los tres
  tipos como `L24` (Hito 10) y **no** tiene una entrada propia para estas dos degradaciones;
  `docs/feature-matrix.md` §1 sí las nombra, en las columnas "Limitaciones" y "Riesgos" de las
  filas de método y de auth. O sea que están dichas, pero no como limitación con identificador
  ni criterio de cierre. Este ADR no las registra —no es su alcance, y agregar filas a la §2 le
  corresponde a las tareas que ya tienen ese mandato— y deja anotado por qué convendría
  hacerlo: avisar en vez de degradar callado es independiente de implementar las variantes, es
  más barato y se puede entregar antes del Hito 10.
- Lo marcado `Intentionally unsupported` no vuelve a discutirse sin que se cumpla su condición
  de reapertura. Eso es el punto de escribirlo.
- Riesgo asumido: `Intentionally unsupported` puede leerse como "nunca". No lo es, y por eso
  cada fila lleva condición de reapertura explícita. Si una condición se cumple, se enmienda
  este ADR con uno nuevo que lo supersede, no editando esta decisión en el lugar.
