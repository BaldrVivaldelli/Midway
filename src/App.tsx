import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  COLLECTION_RUN_PROGRESS_EVENT,
  cancelRequest,
  createCollection,
  deleteEnvironment,
  deleteSecret,
  executeRequest,
  exportWorkspaceData,
  importWorkspaceData,
  importWorkspacePayload,
  previewRequest,
  runCollection,
  saveEnvironment,
  saveRequest,
  saveSecret,
  workspaceSnapshot
} from "./tauri/api";
import type {
  ApiKeyPlacement,
  AssertionOperator,
  AssertionReport,
  AssertionSource,
  AuthConfig,
  BodyMode,
  CollectionRunProgressEvent,
  CollectionRunReport,
  CollectionWithRequests,
  EnvironmentRecord,
  ExportWorkspaceResult,
  FormDataFieldKind,
  FormDataRow,
  HistoryEntry,
  HttpMethod,
  ImportWorkspaceResult,
  KeyValueRow,
  RequestDraft,
  RequestExecutionOutcome,
  RequestPreview,
  ResponseAssertion,
  SaveEnvironmentInput,
  SavedRequestRecord,
  SecretMetadata,
  WorkspaceExportFormat,
  WorkspaceImportFormat,
  WorkspaceSnapshot
} from "./tauri/types";

import CodeEditor from "./components/CodeEditor";
import UpdateCenterCard from "./components/UpdateCenterCard";
import { searchPaletteItems, type CommandPaletteItem } from "./lib/commandPalette";
import {
  appendCrashRecord,
  clearCrashRecords,
  installCrashCapture,
  readCrashRecords,
  type CrashRecord
} from "./lib/diagnostics";
import {
  looksLikeCurlCommand as looksLikeCurlCommandImported,
  parseCurlCommandToDraft as parseCurlCommandToDraftImported
} from "./lib/curl";
import {
  generateRequestCodeSnippet,
  requestCodeExportFilename,
  requestCodeExportLabel,
  requestCodeExportLanguage,
  type RequestCodeExportFormat
} from "./lib/requestExport";

const METHODS: HttpMethod[] = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS"
];

const BODY_MODES: BodyMode[] = ["none", "json", "text", "formData"];
const AUTH_TYPES: AuthConfig["type"][] = ["none", "bearer", "basic", "apiKey"];
const API_KEY_PLACEMENTS: ApiKeyPlacement[] = ["header", "query"];
const ASSERTION_SOURCES: AssertionSource[] = [
  "status",
  "header",
  "bodyText",
  "jsonPointer",
  "finalUrl"
];
const ASSERTION_OPERATORS: AssertionOperator[] = [
  "equals",
  "contains",
  "notContains",
  "exists",
  "notExists",
  "greaterOrEqual",
  "lessOrEqual"
];

type RequestTab = "params" | "headers" | "auth" | "body" | "tests";
type ResponseTab = "body" | "headers" | "tests";
type WorkspaceTab = "environments" | "data" | "history";
type DataTab = "export" | "import";
type ExportScope = "workspace" | "collection";
type SidePanel = "workspace" | "runner" | null;
type AssertionTemplateKey = "status" | "header" | "bodyText" | "jsonPointer";

const ASSERTION_TEMPLATE_OPTIONS: Array<{
  key: AssertionTemplateKey;
  label: string;
  description: string;
}> = [
  {
    key: "status",
    label: "Estado 200",
    description: "Verifica el código HTTP"
  },
  {
    key: "header",
    label: "Encabezado JSON",
    description: "Valida el content-type"
  },
  {
    key: "bodyText",
    label: "Cuerpo contiene",
    description: "Busca un texto útil"
  },
  {
    key: "jsonPointer",
    label: "JSON /id",
    description: "Confirma que existe un campo"
  }
];

function createId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return Math.random().toString(36).slice(2);
}

function createRow(key = "", value = "", enabled = true): KeyValueRow {
  return {
    id: createId(),
    key,
    value,
    enabled
  };
}

function createAssertion(
  partial: Partial<ResponseAssertion> = {}
): ResponseAssertion {
  return {
    id: partial.id ?? createId(),
    name: partial.name ?? "estado ok",
    enabled: partial.enabled ?? true,
    source: partial.source ?? "status",
    operator: partial.operator ?? "equals",
    selector: partial.selector ?? null,
    expected: partial.expected ?? "200"
  };
}

function createAssertionTemplate(template: AssertionTemplateKey): ResponseAssertion {
  switch (template) {
    case "status":
      return createAssertion({
        name: "estado 200",
        source: "status",
        operator: "equals",
        expected: "200"
      });
    case "header":
      return createAssertion({
        name: "content-type json",
        source: "header",
        operator: "contains",
        selector: "content-type",
        expected: "application/json"
      });
    case "bodyText":
      return createAssertion({
        name: "cuerpo contiene title",
        source: "bodyText",
        operator: "contains",
        expected: "title"
      });
    case "jsonPointer":
      return createAssertion({
        name: "json /id existe",
        source: "jsonPointer",
        operator: "exists",
        selector: "/id",
        expected: ""
      });
  }

  const exhaustiveCheck: never = template;
  return exhaustiveCheck;
}

function createInitialDraft(): RequestDraft {
  return {
    id: null,
    name: "Obtener post 1",
    method: "GET",
    url: "https://jsonplaceholder.typicode.com/posts/1",
    query: [],
    headers: [createRow("accept", "application/json")],
    auth: { type: "none" },
    body: {
      mode: "none",
      value: '{\n  "title": "foo",\n  "body": "bar",\n  "userId": 1\n}'
    },
    timeoutMs: 30000,
    environmentId: null,
    responseTests: [
      createAssertion({
        name: "estado 200",
        source: "status",
        operator: "equals",
        expected: "200"
      }),
      createAssertion({
        name: "cuerpo contiene title",
        source: "bodyText",
        operator: "contains",
        expected: "title"
      })
    ]
  };
}

function createBlankDraft(): RequestDraft {
  return {
    id: null,
    name: "Nueva petición",
    method: "GET",
    url: "",
    query: [],
    headers: [],
    auth: { type: "none" },
    body: {
      mode: "none",
      value: "",
      formData: []
    },
    timeoutMs: 30000,
    environmentId: null,
    responseTests: []
  };
}

function createEmptyEnvironmentEditor(): SaveEnvironmentInput {
  return {
    environmentId: null,
    name: "",
    variables: [
      createRow("base_url", "https://jsonplaceholder.typicode.com"),
      createRow("post_id", "1")
    ]
  };
}

function normalizeError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;

  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }

  try {
    return JSON.stringify(error, null, 2);
  } catch {
    return "Ocurrió un error inesperado.";
  }
}

function normalizeDraft(draft: RequestDraft): RequestDraft {
  return {
    ...draft,
    body: {
      ...draft.body,
      formData: draft.body.formData ?? []
    },
    responseTests: draft.responseTests ?? []
  };
}

function needsSelector(source: AssertionSource): boolean {
  return source === "header" || source === "jsonPointer";
}

function selectorPlaceholder(source: AssertionSource): string {
  switch (source) {
    case "header":
      return "content-type";
    case "jsonPointer":
      return "/data/id";
    default:
      return "sin selector";
  }
}

function KeyValueEditor({
  title,
  rows,
  onChange,
  addLabel = "+ fila"
}: {
  title: string;
  rows: KeyValueRow[];
  onChange: (rows: KeyValueRow[]) => void;
  addLabel?: string;
}) {
  const updateRow = (id: string, patch: Partial<KeyValueRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  const removeRow = (id: string) => {
    onChange(rows.filter((row) => row.id !== id));
  };

  const addRow = () => {
    onChange([...rows, createRow()]);
  };

  return (
    <section className="card">
      <div className="header-row">
        <h3 className="section-title">{title}</h3>
        <button className="button secondary" onClick={addRow} type="button">
          {addLabel}
        </button>
      </div>

      <div className="rows-grid">
        {rows.map((row) => (
          <div className="kv-row" key={row.id}>
            <label className="checkbox-cell">
              <input
                checked={row.enabled}
                onChange={(event) =>
                  updateRow(row.id, { enabled: event.target.checked })
                }
                type="checkbox"
              />
            </label>

            <input
              className="input"
              onChange={(event) => updateRow(row.id, { key: event.target.value })}
              placeholder="key"
              value={row.key}
            />

            <input
              className="input"
              onChange={(event) => updateRow(row.id, { value: event.target.value })}
              placeholder="valor"
              value={row.value}
            />

            <button
              className="button danger"
              onClick={() => removeRow(row.id)}
              type="button"
            >
              borrar
            </button>
          </div>
        ))}

        {rows.length === 0 ? (
          <div className="muted small">No hay filas todavía.</div>
        ) : null}
      </div>
    </section>
  );
}

function createFormDataRow(
  key = "",
  value = "",
  kind: FormDataFieldKind = "text",
  enabled = true,
  fileName: string | null = null
): FormDataRow {
  return {
    id: createId(),
    key,
    value,
    enabled,
    kind,
    fileName
  };
}

function FormDataEditor({
  rows,
  onChange
}: {
  rows: FormDataRow[];
  onChange: (rows: FormDataRow[]) => void;
}) {
  const updateRow = (id: string, patch: Partial<FormDataRow>) => {
    onChange(rows.map((row) => (row.id === id ? { ...row, ...patch } : row)));
  };

  const removeRow = (id: string) => {
    onChange(rows.filter((row) => row.id !== id));
  };

  const addTextRow = () => {
    onChange([...rows, createFormDataRow()]);
  };

  const addFileRow = () => {
    onChange([...rows, createFormDataRow("", "", "file")]);
  };

  return (
    <section className="card">
      <div className="header-row">
        <div>
          <h3 className="section-title">Campos multipart</h3>
          <div className="muted small">
            Usá texto o rutas de archivo locales. El backend arma el multipart real.
          </div>
        </div>
        <div className="header-actions">
          <button className="button secondary compact-button" onClick={addTextRow} type="button">
            + texto
          </button>
          <button className="button secondary compact-button" onClick={addFileRow} type="button">
            + archivo
          </button>
        </div>
      </div>

      <div className="rows-grid formdata-grid">
        {rows.map((row) => (
          <div className="formdata-row" key={row.id}>
            <label className="checkbox-cell">
              <input
                checked={row.enabled}
                onChange={(event) => updateRow(row.id, { enabled: event.target.checked })}
                type="checkbox"
              />
            </label>

            <select
              className="select"
              onChange={(event) =>
                updateRow(row.id, {
                  kind: event.target.value as FormDataFieldKind,
                  fileName:
                    event.target.value === "file" ? row.fileName ?? null : null
                })
              }
              value={row.kind}
            >
              <option value="text">texto</option>
              <option value="file">archivo</option>
            </select>

            <input
              className="input"
              onChange={(event) => updateRow(row.id, { key: event.target.value })}
              placeholder="campo"
              value={row.key}
            />

            <input
              className="input"
              onChange={(event) => updateRow(row.id, { value: event.target.value })}
              placeholder={row.kind === "file" ? "/ruta/local/archivo.png" : "valor"}
              value={row.value}
            />

            {row.kind === "file" ? (
              <input
                className="input"
                onChange={(event) =>
                  updateRow(row.id, {
                    fileName: event.target.value.trim() || null
                  })
                }
                placeholder="nombre opcional"
                value={row.fileName ?? ""}
              />
            ) : (
              <div className="muted small formdata-hint">se envía como texto</div>
            )}

            <button
              className="button danger compact-button"
              onClick={() => removeRow(row.id)}
              type="button"
            >
              borrar
            </button>
          </div>
        ))}

        {rows.length === 0 ? (
          <div className="muted small">No hay campos multipart todavía.</div>
        ) : null}
      </div>
    </section>
  );
}

function AssertionEditor({
  assertions,
  onChange
}: {
  assertions: ResponseAssertion[];
  onChange: (assertions: ResponseAssertion[]) => void;
}) {
  const updateAssertion = (
    id: string,
    patch: Partial<ResponseAssertion>
  ) => {
    onChange(
      assertions.map((assertion) =>
        assertion.id === id ? { ...assertion, ...patch } : assertion
      )
    );
  };

  const removeAssertion = (id: string) => {
    onChange(assertions.filter((assertion) => assertion.id !== id));
  };

  const addAssertion = () => {
    onChange([...assertions, createAssertion()]);
  };

  const addAssertionTemplate = (template: AssertionTemplateKey) => {
    onChange([...assertions, createAssertionTemplate(template)]);
  };

  const addStarterPack = () => {
    onChange([
      ...assertions,
      createAssertionTemplate("status"),
      createAssertionTemplate("header"),
      createAssertionTemplate("bodyText")
    ]);
  };

  return (
    <section className="card">
      <div className="header-row">
        <div>
          <h3 className="section-title">Pruebas</h3>
          <div className="muted small">
            Se ejecutan en Rust después de recibir la respuesta.
          </div>
        </div>
        <div className="header-actions">
          {assertions.length === 0 ? (
            <button className="button ghost compact-button" onClick={addStarterPack} type="button">
              Base rápida
            </button>
          ) : null}
          <button className="button secondary" onClick={addAssertion} type="button">
            + prueba
          </button>
        </div>
      </div>

      <div className="rows-grid">
        {assertions.map((assertion) => (
          <div className="assertion-row" key={assertion.id}>
            <label className="checkbox-cell">
              <input
                checked={assertion.enabled}
                onChange={(event) =>
                  updateAssertion(assertion.id, { enabled: event.target.checked })
                }
                type="checkbox"
              />
            </label>

            <input
              className="input"
              onChange={(event) =>
                updateAssertion(assertion.id, { name: event.target.value })
              }
              placeholder="estado ok"
              value={assertion.name}
            />

            <select
              className="select"
              onChange={(event) => {
                const source = event.target.value as AssertionSource;
                updateAssertion(assertion.id, {
                  source,
                  selector: needsSelector(source) ? assertion.selector ?? "" : null
                });
              }}
              value={assertion.source}
            >
              {ASSERTION_SOURCES.map((source) => (
                <option key={source} value={source}>
                  {formatAssertionSourceLabel(source)}
                </option>
              ))}
            </select>

            <select
              className="select"
              onChange={(event) =>
                updateAssertion(assertion.id, {
                  operator: event.target.value as AssertionOperator
                })
              }
              value={assertion.operator}
            >
              {ASSERTION_OPERATORS.map((operator) => (
                <option key={operator} value={operator}>
                  {formatAssertionOperatorLabel(operator)}
                </option>
              ))}
            </select>

            <input
              className="input"
              disabled={!needsSelector(assertion.source)}
              onChange={(event) =>
                updateAssertion(assertion.id, { selector: event.target.value })
              }
              placeholder={selectorPlaceholder(assertion.source)}
              value={assertion.selector ?? ""}
            />

            <input
              className="input"
              disabled={
                assertion.operator === "exists" || assertion.operator === "notExists"
              }
              onChange={(event) =>
                updateAssertion(assertion.id, { expected: event.target.value })
              }
              placeholder="esperado"
              value={assertion.expected}
            />

            <button
              className="button danger"
              onClick={() => removeAssertion(assertion.id)}
              type="button"
            >
              borrar
            </button>
          </div>
        ))}

        {assertions.length === 0 ? (
          <div className="card assertion-empty-state" style={{ display: "grid", gap: 18 }}>
            <div style={{ display: "grid", gap: 8 }}>
              <div className="eyebrow">arranque rápido</div>
              <strong style={{ fontSize: 16, letterSpacing: "-0.02em" }}>
                Armá una base mínima de validaciones en un click
              </strong>
              <span className="muted small">
                Sembrá pruebas comunes para estado, encabezados, cuerpo o JSON y después ajustalas a tu API.
              </span>
            </div>

            <div
              style={{
                display: "grid",
                gap: 10,
                gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))"
              }}
            >
              {ASSERTION_TEMPLATE_OPTIONS.map((template) => (
                <button
                  className="button ghost"
                  key={template.key}
                  onClick={() => addAssertionTemplate(template.key)}
                  style={{
                    display: "grid",
                    gap: 8,
                    minHeight: 88,
                    padding: 16,
                    textAlign: "left",
                    justifyItems: "start",
                    alignContent: "start"
                  }}
                  type="button"
                >
                  <span style={{ fontSize: 14, fontWeight: 600, letterSpacing: "-0.01em" }}>
                    {template.label}
                  </span>
                  <small className="muted" style={{ letterSpacing: "0.04em", textTransform: "uppercase" }}>
                    {template.description}
                  </small>
                </button>
              ))}
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

function AssertionReportView({
  report,
  emptyMessage,
  title = "Pruebas"
}: {
  report: AssertionReport | null | undefined;
  emptyMessage: string;
  title?: string;
}) {
  if (!report) {
    return <div className="muted small">{emptyMessage}</div>;
  }

  return (
    <div className="stack">
      <div className="inline-row">
        <strong>{title}</strong>
        <span className="badge">{report.total}</span>
        <span className="result-pass">{report.passed} ok</span>
        <span className="result-fail">{report.failed} error</span>
      </div>

      <div className="rows-grid">
        {report.results.map((result) => (
          <div className="history-item" key={result.id}>
            <div className="header-row">
              <strong>{result.name || "sin nombre"}</strong>
              <span className={result.passed ? "result-pass" : "result-fail"}>
                {result.passed ? "OK" : "ERROR"}
              </span>
            </div>
            <div className="muted small">
              {formatAssertionSourceLabel(result.source)} · {formatAssertionOperatorLabel(result.operator)}
              {result.selector ? ` · ${result.selector}` : ""}
            </div>
            {result.actual !== undefined && result.actual !== null ? (
              <div className="muted small">actual: {result.actual}</div>
            ) : null}
            {result.expected ? (
              <div className="muted small">esperado: {result.expected}</div>
            ) : null}
            <div className="muted small">{result.message}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

function AuthEditor({
  auth,
  onChange
}: {
  auth: AuthConfig;
  onChange: (auth: AuthConfig) => void;
}) {
  const setType = (type: AuthConfig["type"]) => {
    switch (type) {
      case "none":
        onChange({ type: "none" });
        break;
      case "bearer":
        onChange({ type: "bearer", token: "" });
        break;
      case "basic":
        onChange({ type: "basic", username: "", password: "" });
        break;
      case "apiKey":
        onChange({ type: "apiKey", key: "", value: "", placement: "header" });
        break;
    }
  };

  return (
    <section className="card">
      <h3 className="section-title">Autorización</h3>

      <div className="stack">
        <label className="label">
          <span>Tipo</span>
          <select
            className="select"
            onChange={(event) => setType(event.target.value as AuthConfig["type"])}
            value={auth.type}
          >
            {AUTH_TYPES.map((type) => (
              <option key={type} value={type}>
                {formatAuthTypeLabel(type)}
              </option>
            ))}
          </select>
        </label>

        {auth.type === "bearer" ? (
          <label className="label">
            <span>Plantilla del token</span>
            <input
              className="input"
              onChange={(event) => onChange({ ...auth, token: event.target.value })}
              placeholder="{{secret:api_token}}"
              value={auth.token}
            />
          </label>
        ) : null}

        {auth.type === "basic" ? (
          <div className="grid-2">
            <label className="label">
              <span>Usuario</span>
              <input
                className="input"
                onChange={(event) =>
                  onChange({ ...auth, username: event.target.value })
                }
                placeholder="demo"
                value={auth.username}
              />
            </label>

            <label className="label">
              <span>Plantilla de contraseña</span>
              <input
                className="input"
                onChange={(event) =>
                  onChange({ ...auth, password: event.target.value })
                }
                placeholder="{{secret:demo_password}}"
                value={auth.password}
              />
            </label>
          </div>
        ) : null}

        {auth.type === "apiKey" ? (
          <div className="grid-3">
            <label className="label">
              <span>Clave</span>
              <input
                className="input"
                onChange={(event) => onChange({ ...auth, key: event.target.value })}
                placeholder="x-api-key"
                value={auth.key}
              />
            </label>

            <label className="label">
              <span>Plantilla del valor</span>
              <input
                className="input"
                onChange={(event) => onChange({ ...auth, value: event.target.value })}
                placeholder="{{secret:api_key}}"
                value={auth.value}
              />
            </label>

            <label className="label">
              <span>Ubicación</span>
              <select
                className="select"
                onChange={(event) =>
                  onChange({
                    ...auth,
                    placement: event.target.value as ApiKeyPlacement
                  })
                }
                value={auth.placement}
              >
                {API_KEY_PLACEMENTS.map((placement) => (
                  <option key={placement} value={placement}>
                    {formatApiKeyPlacementLabel(placement)}
                  </option>
                ))}
              </select>
            </label>
          </div>
        ) : null}
      </div>
    </section>
  );
}

function CollectionsSidebar({
  collections,
  selectedCollectionId,
  onSelectCollection,
  onLoadRequest,
  onRunCollection,
  busy
}: {
  collections: CollectionWithRequests[];
  selectedCollectionId: string | null;
  onSelectCollection: (collectionId: string) => void;
  onLoadRequest: (record: SavedRequestRecord) => void;
  onRunCollection: () => void;
  busy: boolean;
}) {
  const selectedCollection =
    collections.find((item) => item.collection.id === selectedCollectionId) ?? null;

  return (
    <section className="card">
      <div className="sidebar-section-title">
        <div>
          <h3 className="section-title">Colecciones</h3>
          <div className="muted small">Una lista corta, foco en lo seleccionado.</div>
        </div>
        <button
          className="button secondary"
          disabled={!selectedCollection || busy}
          onClick={onRunCollection}
          type="button"
        >
          correr
        </button>
      </div>

      <div className="sidebar-list">
        {collections.length === 0 ? (
          <div className="muted small">Todavía no hay colecciones.</div>
        ) : null}

        {collections.map((item) => {
          const selected = item.collection.id === selectedCollectionId;

          return (
            <div className="sidebar-cluster" key={item.collection.id}>
              <button
                className={`sidebar-group-title ${selected ? "selected" : ""}`}
                onClick={() => onSelectCollection(item.collection.id)}
                type="button"
              >
                <span>{item.collection.name}</span>
                <span className="badge">{item.collection.requestCount}</span>
              </button>

              {selected ? (
                <div className="sidebar-sublist">
                  {item.requests.map((record) => (
                    <button
                      className="sidebar-item"
                      key={record.id}
                      onClick={() => onLoadRequest(record)}
                      type="button"
                    >
                      <div className="sidebar-item-head">
                        <span className="method-pill">{record.draft.method}</span>
                        <strong>{record.name}</strong>
                      </div>
                      <div className="muted small">
                        {record.draft.responseTests.length} pruebas · {record.updatedAt}
                      </div>
                    </button>
                  ))}

                  {item.requests.length === 0 ? (
                    <div className="muted small">Sin peticiones guardadas.</div>
                  ) : null}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </section>
  );
}

function HistorySidebar({ history }: { history: HistoryEntry[] }) {
  return (
    <section className="card">
      <h3 className="section-title">Historial</h3>

      <div className="sidebar-list">
        {history.length === 0 ? (
          <div className="muted small">Sin historial todavía.</div>
        ) : null}

        {history.map((entry) => (
          <div className="history-item" key={entry.id}>
            <div>
              <span className="method-pill">{entry.method}</span>
              <strong>{entry.requestName}</strong>
            </div>
            <div className="muted small">{entry.url}</div>
            <div className="muted small">
              {entry.responseStatus ? `estado ${entry.responseStatus}` : "sin estado"}
              {entry.durationMs ? ` · ${entry.durationMs} ms` : ""}
            </div>
            {entry.errorMessage ? (
              <div className="muted small">error: {entry.errorMessage}</div>
            ) : null}
            <div className="muted small">{entry.createdAt}</div>
          </div>
        ))}
      </div>
    </section>
  );
}

function SecretManager({
  secrets,
  onSave,
  onDelete,
  busy
}: {
  secrets: SecretMetadata[];
  onSave: (alias: string, value: string) => Promise<void>;
  onDelete: (alias: string) => Promise<void>;
  busy: boolean;
}) {
  const [alias, setAlias] = useState("");
  const [value, setValue] = useState("");
  const canSave = alias.trim().length > 0 && value.trim().length > 0;

  return (
    <section className="card">
      <div className="header-row">
        <div>
          <h3 className="section-title">Secretos</h3>
          <div className="muted small">
            Alias locales para autorización, encabezados y variables sensibles. Se guardan en el llavero del sistema.
          </div>
        </div>
      </div>

      <div className="stack">
        <div className="grid-2">
          <label className="label">
            <span>Alias</span>
            <input
              className="input"
              onChange={(event) => setAlias(event.target.value)}
              placeholder="api_token"
              value={alias}
            />
          </label>

          <label className="label">
            <span>Valor</span>
            <input
              className="input"
              onChange={(event) => setValue(event.target.value)}
              placeholder="••••••"
              type="password"
              value={value}
            />
          </label>
        </div>

        <div className="section-footer">
          <div className="muted small">Los valores se guardan en el llavero o almacén de credenciales del sistema y no se exponen en la vista previa ni en la exportación.</div>
          <div className="header-actions">
            <button
              className="button"
              disabled={busy || !canSave}
              onClick={async () => {
                await onSave(alias, value);
                setAlias("");
                setValue("");
              }}
              type="button"
            >
              Guardar secreto
            </button>
          </div>
        </div>

        <div className="sidebar-list">
          {secrets.length === 0 ? (
            <div className="muted small">No hay alias de secretos todavía.</div>
          ) : null}

          {secrets.map((item) => (
            <div className="header-row history-item" key={item.alias}>
              <div>
                <strong>{item.alias}</strong>
                <div className="muted small">{item.updatedAt}</div>
              </div>
              <button
                className="button danger compact-button"
                disabled={busy}
                onClick={() => onDelete(item.alias)}
                type="button"
              >
                Borrar
              </button>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function EnvironmentManager({
  environments,
  editor,
  onEditorChange,
  onLoad,
  onSave,
  onDelete,
  busy
}: {
  environments: EnvironmentRecord[];
  editor: SaveEnvironmentInput;
  onEditorChange: (editor: SaveEnvironmentInput) => void;
  onLoad: (env: EnvironmentRecord) => void;
  onSave: () => Promise<void>;
  onDelete: (environmentId: string) => Promise<void>;
  busy: boolean;
}) {
  const activeEnvironment =
    environments.find((env) => env.id === editor.environmentId) ?? null;

  return (
    <div className="split-pane environment-split">
      <section className="card split-pane-nav">
        <div className="sidebar-section-title">
          <div>
            <h3 className="section-title">Entornos</h3>
            <div className="muted small">Organizá variables por contexto de trabajo.</div>
          </div>
          <button
            className="button secondary compact-button"
            onClick={() => onEditorChange(createEmptyEnvironmentEditor())}
            type="button"
          >
            Nuevo
          </button>
        </div>

        <div className="sidebar-list environment-nav">
          {environments.map((env) => {
            const selected = env.id === editor.environmentId;

            return (
              <button
                className={`list-button ${selected ? "selected" : ""}`}
                key={env.id}
                onClick={() => onLoad(env)}
                type="button"
              >
                <strong>{env.name}</strong>
                <div className="muted small">{env.variables.length} vars</div>
              </button>
            );
          })}

          {environments.length === 0 ? (
            <div className="muted small">No hay entornos todavía.</div>
          ) : null}
        </div>
      </section>

      <section className="card split-pane-content">
        <div className="header-row">
          <div>
            <h3 className="section-title">
              {activeEnvironment ? "Editar entorno" : "Nuevo entorno"}
            </h3>
            <div className="muted small">
              {activeEnvironment
                ? `Actualizado ${activeEnvironment.updatedAt}`
                : "Definí variables reutilizables para URLs, ids y credenciales."}
            </div>
          </div>

          {activeEnvironment ? (
            <button
              className="button danger compact-button"
              disabled={busy}
              onClick={() => onDelete(activeEnvironment.id)}
              type="button"
            >
              Borrar
            </button>
          ) : null}
        </div>

        <div className="stack">
          <label className="label">
            <span>Nombre</span>
            <input
              className="input"
              onChange={(event) =>
                onEditorChange({ ...editor, name: event.target.value })
              }
              placeholder="dev"
              value={editor.name}
            />
          </label>

          <KeyValueEditor
            addLabel="+ variable"
            onChange={(variables) => onEditorChange({ ...editor, variables })}
            rows={editor.variables}
            title="Variables"
          />
        </div>

        <div className="section-footer">
          <div className="muted small">
            {editor.environmentId
              ? "Este entorno ya se puede usar desde el selector Entorno de la petición."
              : "Guardalo para usarlo en la petición actual o en la ejecución por lote."}
          </div>
          <div className="header-actions">
            <button
              className="button"
              disabled={busy || !editor.name.trim()}
              onClick={() => void onSave()}
              type="button"
            >
              {editor.environmentId ? "Guardar cambios" : "Guardar entorno"}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

function CollectionRunReportView({ report }: { report: CollectionRunReport | null }) {
  return (
    <section className="card">
      <h3 className="section-title">Reporte de ejecución</h3>

      {report ? (
        <div className="stack">
          <div className="inline-row">
            <strong>{report.collectionName}</strong>
            <span className="badge">{report.totalRequests} peticiones</span>
            <span className="result-pass">{report.passedAssertions} pruebas ok</span>
            <span className="result-fail">{report.failedAssertions} pruebas con error</span>
          </div>

          <div className="muted small">
            {report.startedAt} → {report.finishedAt}
          </div>

          <div className="rows-grid">
            {report.items.map((item) => (
              <div className="history-item" key={`${item.requestId}-${item.executedAt}`}>
                <div className="header-row">
                  <div>
                    <strong>{item.requestName}</strong>
                    <div className="muted small">
                      {item.environmentName ? `entorno ${item.environmentName}` : "sin entorno"}
                    </div>
                  </div>
                  {item.responseStatus ? (
                    <span className="badge">{item.responseStatus}</span>
                  ) : (
                    <span className="result-fail">error</span>
                  )}
                </div>

                {item.resolvedUrl ? (
                  <div className="muted small">{item.resolvedUrl}</div>
                ) : null}

                {item.durationMs ? (
                  <div className="muted small">{item.durationMs} ms</div>
                ) : null}

                {item.errorMessage ? (
                  <div className="muted small">{item.errorMessage}</div>
                ) : null}

                <div style={{ marginTop: 10 }}>
                  <AssertionReportView
                    emptyMessage="Esa petición no llegó a evaluar pruebas."
                    report={item.assertionReport ?? null}
                    title="Resultados"
                  />
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <div className="muted small">
          Elegí una colección y ejecutá el lote para ver el reporte consolidado.
        </div>
      )}
    </section>
  );
}

function CollectionRunProgressView({
  progress
}: {
  progress: CollectionRunProgressEvent | null;
}) {
  if (!progress) {
    return (
      <section className="card">
        <h3 className="section-title">Progreso</h3>
        <div className="muted small">Todavía no hay eventos de progreso de la ejecución.</div>
      </section>
    );
  }

  const processed =
    progress.phase === "finished"
      ? progress.totalRequests
      : Math.min(progress.processedRequests, progress.totalRequests);
  const percent =
    progress.totalRequests === 0
      ? 0
      : Math.round((processed / progress.totalRequests) * 100);

  return (
    <section className="card">
      <div className="header-row">
        <div>
          <h3 className="section-title">Progreso</h3>
          <div className="muted small">
            {progress.collectionName} · ejecución {progress.runId.slice(0, 8)}
          </div>
        </div>
        <span className="badge">{formatCollectionRunPhaseLabel(progress.phase)}</span>
      </div>

      <div className="stack">
        <div className="progress-track">
          <div className="progress-fill" style={{ width: `${percent}%` }} />
        </div>

        <div className="inline-row">
          <strong>{percent}%</strong>
          <span className="badge">
            {processed}/{progress.totalRequests}
          </span>
          <span className="result-pass">{progress.passedAssertions} ok</span>
          <span className="result-fail">{progress.failedAssertions} error</span>
        </div>

        <div className="stats-grid">
          <div className="stat-card">
            <div className="muted small">Completados</div>
            <strong>{progress.completedRequests}</strong>
          </div>
          <div className="stat-card">
            <div className="muted small">Errores</div>
            <strong>{progress.erroredRequests}</strong>
          </div>
        </div>

        {progress.requestName ? (
          <div>
            <strong>Petición actual</strong>
            <div className="code-block">
              #{progress.currentIndex} {progress.requestName}
            </div>
          </div>
        ) : null}

        {progress.resolvedUrl ? (
          <div className="muted small mono">{progress.resolvedUrl}</div>
        ) : null}

        {progress.errorMessage ? (
          <div className="error-banner">{progress.errorMessage}</div>
        ) : null}

        <div className="muted small">
          {progress.startedAt}
          {progress.finishedAt ? ` → ${progress.finishedAt}` : ""}
        </div>
      </div>
    </section>
  );
}

function TabButton({
  active,
  onClick,
  label,
  meta
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  meta?: string | number;
}) {
  return (
    <button
      className={`tab-button ${active ? "active" : ""}`}
      onClick={onClick}
      type="button"
    >
      <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
        <span
          aria-hidden="true"
          style={{
            width: 6,
            height: 6,
            borderRadius: 999,
            background: active ? "rgba(246, 244, 239, 0.92)" : "rgba(255, 255, 255, 0.18)",
            boxShadow: active ? "0 0 0 4px rgba(246, 244, 239, 0.08)" : "none",
            transform: active ? "scale(1.16)" : undefined,
            transition: "transform 160ms ease, background-color 160ms ease, box-shadow 160ms ease"
          }}
        />
        <span>{label}</span>
      </span>
      {meta !== undefined ? <span className="tab-meta">{meta}</span> : null}
    </button>
  );
}

function defaultRequestTabForMethod(method: HttpMethod): RequestTab {
  return method === "POST" || method === "PUT" || method === "PATCH"
    ? "body"
    : "params";
}

function formatBodyModeLabel(mode: BodyMode): string {
  switch (mode) {
    case "none":
      return "Sin cuerpo";
    case "json":
      return "JSON";
    case "text":
      return "Texto";
    case "formData":
      return "Multipart";
  }

  const exhaustiveCheck: never = mode;
  return exhaustiveCheck;
}

function formatAuthTypeLabel(type: AuthConfig["type"]): string {
  switch (type) {
    case "none":
      return "Sin autorización";
    case "bearer":
      return "Bearer";
    case "basic":
      return "Basic";
    case "apiKey":
      return "Clave API";
  }

  const exhaustiveCheck: never = type;
  return exhaustiveCheck;
}

function formatApiKeyPlacementLabel(placement: ApiKeyPlacement): string {
  switch (placement) {
    case "header":
      return "Encabezado";
    case "query":
      return "Consulta";
  }

  const exhaustiveCheck: never = placement;
  return exhaustiveCheck;
}

function formatAssertionSourceLabel(source: AssertionSource): string {
  switch (source) {
    case "status":
      return "Estado";
    case "header":
      return "Encabezado";
    case "bodyText":
      return "Cuerpo";
    case "jsonPointer":
      return "Puntero JSON";
    case "finalUrl":
      return "URL final";
  }

  const exhaustiveCheck: never = source;
  return exhaustiveCheck;
}

function formatAssertionOperatorLabel(operator: AssertionOperator): string {
  switch (operator) {
    case "equals":
      return "igual a";
    case "contains":
      return "contiene";
    case "notContains":
      return "no contiene";
    case "exists":
      return "existe";
    case "notExists":
      return "no existe";
    case "greaterOrEqual":
      return "mayor o igual";
    case "lessOrEqual":
      return "menor o igual";
  }

  const exhaustiveCheck: never = operator;
  return exhaustiveCheck;
}

function formatCollectionRunPhaseLabel(
  phase: CollectionRunProgressEvent["phase"]
): string {
  switch (phase) {
    case "started":
      return "iniciada";
    case "requestStarted":
      return "ejecutando";
    case "requestFinished":
      return "petición lista";
    case "finished":
      return "finalizada";
  }

  const exhaustiveCheck: never = phase;
  return exhaustiveCheck;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  if (bytes < 1024 * 1024) {
    const kb = bytes / 1024;
    return `${kb >= 10 ? Math.round(kb) : kb.toFixed(1)} KB`;
  }

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatBodyText(bodyText: string): string {
  if (!bodyText) {
    return "(vacío)";
  }

  try {
    const parsed = JSON.parse(bodyText) as unknown;
    return JSON.stringify(parsed, null, 2);
  } catch {
    return bodyText;
  }
}

async function copyTextToClipboard(value: string): Promise<void> {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }

  if (typeof document === "undefined") {
    throw new Error("El portapapeles no está disponible en este entorno.");
  }

  const textarea = document.createElement("textarea");
  textarea.value = value;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();

  const success = document.execCommand("copy");
  document.body.removeChild(textarea);

  if (!success) {
    throw new Error("No pude copiar al portapapeles.");
  }
}

function downloadTextFile(filename: string, content: string): void {
  if (typeof document === "undefined" || typeof URL === "undefined") {
    return;
  }

  const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.rel = "noopener";
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}

function detectEditorLanguage(value: string): "json" | "text" | "shell" {
  const trimmed = value.trim();
  if (!trimmed) {
    return "text";
  }

  if ((trimmed.startsWith("{") && trimmed.endsWith("}")) || (trimmed.startsWith("[") && trimmed.endsWith("]"))) {
    return "json";
  }

  if (/^(curl|GET|POST|PUT|PATCH|DELETE|HEAD|OPTIONS)/i.test(trimmed)) {
    return "shell";
  }

  return "text";
}

function statusTone(
  status?: number | null
): "ok" | "warn" | "client" | "error" | "neutral" {
  if (status === undefined || status === null) {
    return "neutral";
  }

  if (status >= 200 && status < 300) {
    return "ok";
  }

  if (status >= 300 && status < 400) {
    return "warn";
  }

  if (status >= 400 && status < 500) {
    return "client";
  }

  if (status >= 500) {
    return "error";
  }

  return "neutral";
}

function ResponseHeadersTable({
  rows,
  emptyMessage = "Sin encabezados."
}: {
  rows: Array<{ key: string; value: string }>;
  emptyMessage?: string;
}) {
  if (rows.length === 0) {
    return <div className="empty-surface muted small">{emptyMessage}</div>;
  }

  return (
    <div className="table-shell">
      <table className="data-table">
        <thead>
          <tr>
            <th>Clave</th>
            <th>Valor</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row, index) => (
            <tr key={`${row.key}-${index}`}>
              <td>{row.key}</td>
              <td>{row.value}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ResponseEmptyState({
  currentTab,
  busy,
  hasUrl,
  sending,
  environmentName,
  testsCount,
  onPreview,
  onSend,
  onCancel,
  onOpenTests
}: {
  currentTab: ResponseTab;
  busy: boolean;
  hasUrl: boolean;
  sending: boolean;
  environmentName: string | null;
  testsCount: number;
  onPreview: () => void;
  onSend: () => void;
  onCancel: () => void;
  onOpenTests: () => void;
}) {
  const tabMessages: Record<ResponseTab, { title: string; description: string }> = {
    body: {
      title: "Todavía no hay cuerpo de respuesta",
      description:
        "Mandá la petición o generá una vista previa para validar URL, encabezados y entorno antes de ejecutar."
    },
    headers: {
      title: "Los encabezados van a aparecer acá",
      description:
        "Usá la vista previa para revisar la resolución previa y después ejecutá la petición para inspeccionar los encabezados reales."
    },
    tests: {
      title: "Las pruebas viven mejor con contexto",
      description:
        "Ejecutá la petición para ver resultados reales o abrí la pestaña de pruebas para preparar una base mínima."
    }
  };

  const current = tabMessages[currentTab];
  const description = hasUrl
    ? current.description
    : "Definí una URL o pegá un cURL en la barra superior para empezar a trabajar esta petición.";

  return (
    <div className="response-empty-state">
      <div className="card response-empty-primary">
        <div className="eyebrow">sin respuesta</div>
        <h3 className="surface-title response-empty-title">
          {sending ? "Esperando respuesta…" : current.title}
        </h3>
        <div className="muted small response-empty-description">{description}</div>

        <div className="header-actions response-empty-actions">
          <button
            className="button secondary"
            disabled={busy || !hasUrl || sending}
            onClick={onPreview}
            type="button"
          >
            {busy && !sending ? "Procesando…" : "Vista previa"}
          </button>
          <button
            className={`button ${sending ? "danger" : ""}`}
            disabled={busy && !sending}
            onClick={sending ? onCancel : onSend}
            type="button"
          >
            {sending ? "Cancelar" : busy ? "Procesando…" : "Enviar"}
          </button>
        </div>
      </div>

      <div className="response-empty-grid">
        <div className="card response-empty-card">
          <div className="eyebrow">contexto activo</div>
          <strong>{environmentName ?? "Sin entorno"}</strong>
          <span className="muted small">
            Cambialo desde la barra superior para resolver variables antes de ejecutar.
          </span>
        </div>

        <div className="card response-empty-card">
          <div className="eyebrow">pruebas</div>
          <strong>{testsCount === 0 ? "Sin pruebas todavía" : `${testsCount} listas`}</strong>
          <button className="button ghost compact-button" onClick={onOpenTests} type="button">
            {testsCount === 0 ? "Configurar pruebas" : "Ver pruebas"}
          </button>
        </div>

        <div className="card response-empty-card">
          <div className="eyebrow">atajos</div>
          <strong>⌘/Ctrl + Enter · Enviar</strong>
          <span className="muted small">⌘/Ctrl + Shift + P genera una vista previa.</span>
        </div>
      </div>
    </div>
  );
}

function RequestStatusRail({
  response,
  preview,
  environmentName,
  collectionName,
  testsCount,
  timeoutMs,
  bodyMode,
  isDirty,
  sending
}: {
  response: RequestExecutionOutcome["response"] | null;
  preview: RequestPreview | null;
  environmentName: string | null;
  collectionName: string | null;
  testsCount: number;
  timeoutMs: number;
  bodyMode: BodyMode;
  isDirty: boolean;
  sending: boolean;
}) {
  const tone = sending ? "neutral" : statusTone(response?.status);
  const toneBorder: Record<typeof tone, string> = {
    neutral: "rgba(255, 255, 255, 0.06)",
    ok: "rgba(112, 186, 137, 0.16)",
    warn: "rgba(116, 153, 220, 0.16)",
    client: "rgba(231, 176, 92, 0.17)",
    error: "rgba(205, 101, 123, 0.18)"
  };

  let statusValue = "Listo para enviar";
  let statusDetail = isDirty
    ? "Hay cambios locales pendientes"
    : "Todo sincronizado localmente";

  if (sending) {
    statusValue = "Enviando petición";
    statusDetail = "Esperando respuesta del servidor. Podés cancelar la ejecución actual.";
  } else if (response) {
    statusValue = `${response.status} ${response.statusText}`;
    statusDetail = `Última ejecución ${formatSessionTimestamp(response.receivedAt)}`;
  } else if (preview) {
    statusValue = "Vista previa lista";
    statusDetail = preview.environmentName
      ? `Resuelto con ${preview.environmentName}`
      : "Resolución previa disponible para revisar URL y encabezados.";
  }

  return (
    <section aria-label="Estado de la petición" className="status-rail">
      <div
        className="card status-rail-card status-rail-primary"
        style={{ borderColor: toneBorder[tone] }}
      >
        <div className="eyebrow">estado</div>
        <strong>{statusValue}</strong>
        <span className="muted small">{statusDetail}</span>
      </div>

      <div className="card status-rail-card">
        <div className="eyebrow">entorno</div>
        <strong>{environmentName ?? "Sin entorno"}</strong>
      </div>

      <div className="card status-rail-card">
        <div className="eyebrow">colección</div>
        <strong>{collectionName ?? "Borrador"}</strong>
      </div>

      <div className="card status-rail-card">
        <div className="eyebrow">{response ? "duración" : "límite"}</div>
        <strong>{response ? `${response.durationMs} ms` : `${timeoutMs} ms`}</strong>
      </div>

      <div className="card status-rail-card">
        <div className="eyebrow">{response ? "tamaño" : "cuerpo"}</div>
        <strong>{response ? formatBytes(response.sizeBytes) : formatBodyModeLabel(bodyMode)}</strong>
      </div>

      <div className="card status-rail-card">
        <div className="eyebrow">pruebas</div>
        <strong>{testsCount === 0 ? "Sin pruebas" : `${testsCount} listas`}</strong>
      </div>
    </section>
  );
}

function RequestPreviewPanel({
  preview,
  busy,
  onRefresh
}: {
  preview: RequestPreview | null;
  busy: boolean;
  onRefresh: () => void;
}) {
  return (
    <section className="card preview-card">
      <div className="header-row">
        <div>
          <h3 className="section-title">Vista previa</h3>
          <div className="muted small">
            Resuelve el entorno, los encabezados y el cURL sin salir de la configuración.
          </div>
        </div>

        <button
          className="button secondary compact-button"
          disabled={busy}
          onClick={onRefresh}
          type="button"
        >
          {busy ? "Generando…" : preview ? "Regenerar" : "Vista previa"}
        </button>
      </div>

      {preview ? (
        <div className="stack">
          <div>
            <strong>{preview.method}</strong>
            <CodeEditor className="compact-code-editor" language="text" minHeight={96} readOnly value={preview.resolvedUrl} />
          </div>

          <div>
            <strong>Encabezados</strong>
            <ResponseHeadersTable
              emptyMessage="Sin encabezados resueltos."
              rows={preview.headers}
            />
          </div>

          <div>
            <strong>cURL</strong>
            <CodeEditor className="compact-code-editor" language="shell" minHeight={140} readOnly value={preview.curlCommand} />
          </div>

          {preview.bodyText ? (
            <div>
              <strong>Cuerpo</strong>
              <CodeEditor className="compact-code-editor" language={detectEditorLanguage(preview.bodyText)} minHeight={140} readOnly value={formatBodyText(preview.bodyText)} />
            </div>
          ) : null}

          {preview.environmentName ? (
            <div className="muted small">
              Entorno resuelto: {preview.environmentName}
            </div>
          ) : null}

          {preview.usedSecretAliases.length > 0 ? (
            <div className="muted small">
              Usa estos secretos: {preview.usedSecretAliases.join(", ")}
            </div>
          ) : null}

          {preview.missingSecretAliases.length > 0 ? (
            <div>
              <strong>Secretos faltantes</strong>
              <ul className="warning-list">
                {preview.missingSecretAliases.map((alias) => (
                  <li key={alias}>{alias}</li>
                ))}
              </ul>
            </div>
          ) : null}
        </div>
      ) : (
        <div className="empty-surface muted small">
          Tocá <strong>Vista previa</strong> para ver la petición resuelta.
        </div>
      )}
    </section>
  );
}

function RequestExportPanel({
  draft,
  preview,
  exportFormat,
  onExportFormatChange,
  onCopy,
  onDownload
}: {
  draft: RequestDraft;
  preview: RequestPreview | null;
  exportFormat: RequestCodeExportFormat;
  onExportFormatChange: (format: RequestCodeExportFormat) => void;
  onCopy: (content: string, format: RequestCodeExportFormat) => void;
  onDownload: (content: string, format: RequestCodeExportFormat) => void;
}) {
  const snippet = useMemo(
    () =>
      preview
        ? generateRequestCodeSnippet({
            format: exportFormat,
            draft,
            preview
          })
        : "",
    [draft, preview, exportFormat]
  );

  return (
    <section className="card preview-card">
      <div className="header-row interop-header-row">
        <div>
          <h3 className="section-title">Exportar código</h3>
          <div className="muted small">
            Copiá la petición actual como cURL, fetch o axios con variables ya resueltas.
          </div>
        </div>

        <div className="header-actions interop-actions">
          {(["curl", "fetch", "axios"] as RequestCodeExportFormat[]).map((format) => (
            <button
              className={`button ${exportFormat === format ? "secondary" : "ghost"} compact-button`}
              key={format}
              onClick={() => onExportFormatChange(format)}
              type="button"
            >
              {requestCodeExportLabel(format)}
            </button>
          ))}
        </div>
      </div>

      {preview ? (
        <div className="stack">
          <div className="section-footer">
            <div className="muted small">
              {requestCodeExportLabel(exportFormat)} listo para copiar o descargar.
            </div>
            <div className="header-actions">
              <button
                className="button secondary compact-button"
                onClick={() => onCopy(snippet, exportFormat)}
                type="button"
              >
                Copiar
              </button>
              <button
                className="button ghost compact-button"
                onClick={() => onDownload(snippet, exportFormat)}
                type="button"
              >
                Descargar
              </button>
            </div>
          </div>

          <CodeEditor
            className="compact-code-editor"
            language={requestCodeExportLanguage(exportFormat)}
            minHeight={180}
            readOnly
            value={snippet}
          />

          {draft.body.mode === "formData" ? (
            <div className="muted small">
              Para multipart, el snippet deja marcado dónde reemplazar archivos locales por un File/Blob real.
            </div>
          ) : null}
        </div>
      ) : (
        <div className="empty-surface muted small">
          Generá <strong>Vista previa</strong> primero para exportar la petición resuelta.
        </div>
      )}
    </section>
  );
}

function CollectionsTree({
  collections,
  selectedCollectionId,
  selectedRequestId,
  onSelectCollection,
  onLoadRequest,
  onOpenRunner,
  busy
}: {
  collections: CollectionWithRequests[];
  selectedCollectionId: string | null;
  selectedRequestId: string | null;
  onSelectCollection: (collectionId: string) => void;
  onLoadRequest: (record: SavedRequestRecord) => void;
  onOpenRunner: (collectionId: string) => void;
  busy: boolean;
}) {
  if (collections.length === 0) {
    return (
      <div className="empty-surface muted small">
        Todavía no hay colecciones. Creá una con el botón <strong>+</strong>.
      </div>
    );
  }

  return (
    <div className="collections-tree">
      {collections.map((item) => {
        const selectedCollection = item.collection.id === selectedCollectionId;

        return (
          <div className="tree-group" key={item.collection.id}>
            <div className={`tree-folder-shell ${selectedCollection ? "selected" : ""}`}>
              <button
                className={`tree-row tree-folder ${selectedCollection ? "selected" : ""}`}
                onClick={() => onSelectCollection(item.collection.id)}
                type="button"
              >
                <span className="tree-caret">{selectedCollection ? "▾" : "▸"}</span>
                <span className="tree-label">{item.collection.name}</span>
                <span className="tree-count">{item.collection.requestCount}</span>
              </button>

              {selectedCollection ? (
                <button
                  className="button secondary compact-button tree-action-button"
                  disabled={busy || item.requests.length === 0}
                  onClick={() => onOpenRunner(item.collection.id)}
                  type="button"
                >
                  Ejecutar
                </button>
              ) : null}
            </div>

            {selectedCollection ? (
              <div className="tree-children">
                {item.requests.length === 0 ? (
                  <div className="tree-empty muted small">Sin peticiones guardadas.</div>
                ) : null}

                {item.requests.map((record) => {
                  const selectedRequest = record.id === selectedRequestId;

                  return (
                    <button
                      className={`tree-row tree-request ${
                        selectedRequest ? "selected" : ""
                      }`}
                      key={record.id}
                      onClick={() => onLoadRequest(record)}
                      type="button"
                    >
                      <span className="tree-method">{record.draft.method}</span>
                      <span className="tree-label">{record.name}</span>
                    </button>
                  );
                })}
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}



type OpenRequestTab = {
  key: string;
  collectionId: string | null;
  draft: RequestDraft;
  requestTab: RequestTab;
  responseTab: ResponseTab;
  preview: RequestPreview | null;
  executionOutcome: RequestExecutionOutcome | null;
  showSettings: boolean;
  isDirty: boolean;
  originRequestId: string | null;
};

type ResizeKind = "sidebar" | "response";
type ResizeState = {
  kind: ResizeKind;
  startX: number;
  startWidth: number;
};
type ReorderPlacement = "before" | "after";
type ClosedRequestTab = {
  tab: OpenRequestTab;
  index: number;
};

const DEFAULT_SIDEBAR_WIDTH = 260;
const DEFAULT_RESPONSE_WIDTH = 400;
const MIN_SIDEBAR_WIDTH = 220;
const MAX_SIDEBAR_WIDTH = 360;
const MIN_RESPONSE_WIDTH = 320;
const MAX_RESPONSE_WIDTH = 560;
const SIDEBAR_WIDTH_STORAGE_KEY = "midway.ui.sidebarWidth";
const RESPONSE_WIDTH_STORAGE_KEY = "midway.ui.responseWidth";
const WORKSPACE_SESSION_STORAGE_KEY = "midway.session.workspace.v1";
const SESSION_ACTIVE_MARKER_STORAGE_KEY = "midway.session.active";

type PersistedOpenRequestTab = {
  key: string;
  collectionId: string | null;
  draft: RequestDraft;
  requestTab: RequestTab;
  responseTab: ResponseTab;
  showSettings: boolean;
  isDirty: boolean;
  originRequestId: string | null;
};

type PersistedClosedRequestTab = {
  tab: PersistedOpenRequestTab;
  index: number;
};

type PersistedWorkspaceSession = {
  version: 1;
  savedAt: string;
  requestTabs: PersistedOpenRequestTab[];
  activeRequestTabKey: string | null;
  selectedCollectionId: string | null;
  workspaceTab: WorkspaceTab;
  dataTab: DataTab;
  runnerEnvironmentOverrideId: string | null;
  stopOnError: boolean;
  closedRequestTabs: PersistedClosedRequestTab[];
};

type RestoredWorkspaceSession = {
  savedAt: string;
  requestTabs: OpenRequestTab[];
  activeRequestTabKey: string;
  selectedCollectionId: string | null;
  workspaceTab: WorkspaceTab;
  dataTab: DataTab;
  runnerEnvironmentOverrideId: string | null;
  stopOnError: boolean;
  closedRequestTabs: ClosedRequestTab[];
};

type SessionRestoreState = {
  session: RestoredWorkspaceSession | null;
  recoveredAfterCrash: boolean;
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function getMaxResponseWidth(): number {
  if (typeof window === "undefined") {
    return MAX_RESPONSE_WIDTH;
  }

  return Math.max(
    MIN_RESPONSE_WIDTH,
    Math.min(MAX_RESPONSE_WIDTH, Math.round(window.innerWidth * 0.48))
  );
}

function readStoredPanelWidth(
  key: string,
  fallback: number,
  min: number,
  max: number
): number {
  if (typeof window === "undefined") {
    return fallback;
  }

  const raw = window.localStorage.getItem(key);

  if (!raw) {
    return fallback;
  }

  const parsed = Number(raw);

  if (!Number.isFinite(parsed)) {
    return fallback;
  }

  return clamp(Math.round(parsed), min, max);
}

function createOpenRequestTab({
  draft,
  collectionId = null,
  preview = null,
  executionOutcome = null,
  requestTab = defaultRequestTabForMethod(draft.method),
  responseTab = "body",
  showSettings = false,
  isDirty = false,
  originRequestId = draft.id ?? null
}: {
  draft: RequestDraft;
  collectionId?: string | null;
  preview?: RequestPreview | null;
  executionOutcome?: RequestExecutionOutcome | null;
  requestTab?: RequestTab;
  responseTab?: ResponseTab;
  showSettings?: boolean;
  isDirty?: boolean;
  originRequestId?: string | null;
}): OpenRequestTab {
  return {
    key: createId(),
    collectionId,
    draft,
    requestTab,
    responseTab,
    preview,
    executionOutcome,
    showSettings,
    isDirty,
    originRequestId
  };
}

function requestTabLabel(tab: OpenRequestTab): string {
  const explicitName = tab.draft.name.trim();

  if (explicitName.length > 0) {
    return explicitName;
  }

  if (tab.draft.url.trim().length > 0) {
    return tab.draft.url.trim();
  }

  return "Nueva petición";
}

function isPlaceholderRequestTab(tab: OpenRequestTab): boolean {
  return (
    !tab.isDirty &&
    !tab.originRequestId &&
    tab.draft.method === "GET" &&
    tab.draft.url.trim().length === 0 &&
    tab.draft.name.trim() === "Nueva petición" &&
    tab.draft.query.length === 0 &&
    tab.draft.headers.length === 0 &&
    tab.draft.auth.type === "none" &&
    tab.draft.body.mode === "none" &&
    tab.draft.body.value.trim().length === 0 &&
    tab.draft.responseTests.length === 0 &&
    tab.preview === null &&
    tab.executionOutcome === null
  );
}


function isRequestTab(value: unknown): value is RequestTab {
  return (
    value === "params" ||
    value === "headers" ||
    value === "auth" ||
    value === "body" ||
    value === "tests"
  );
}

function isResponseTab(value: unknown): value is ResponseTab {
  return value === "body" || value === "headers" || value === "tests";
}

function isWorkspaceTab(value: unknown): value is WorkspaceTab {
  return value === "environments" || value === "data" || value === "history";
}

function isDataTab(value: unknown): value is DataTab {
  return value === "export" || value === "import";
}

function safeReadLocalStorage(key: string): string | null {
  if (typeof window === "undefined") {
    return null;
  }

  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function safeWriteLocalStorage(key: string, value: string): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(key, value);
  } catch {
    // ignore storage write errors
  }
}

function safeRemoveLocalStorage(key: string): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.removeItem(key);
  } catch {
    // ignore storage remove errors
  }
}

function normalizeStoredMethod(method: unknown): HttpMethod {
  return METHODS.includes(method as HttpMethod) ? (method as HttpMethod) : "GET";
}

function serializeOpenRequestTab(tab: OpenRequestTab): PersistedOpenRequestTab {
  return {
    key: tab.key,
    collectionId: tab.collectionId,
    draft: tab.draft,
    requestTab: tab.requestTab,
    responseTab: tab.responseTab,
    showSettings: tab.showSettings,
    isDirty: tab.isDirty,
    originRequestId: tab.originRequestId
  };
}

function deserializePersistedOpenRequestTab(
  raw: PersistedOpenRequestTab | null | undefined
): OpenRequestTab | null {
  if (!raw || !raw.draft) {
    return null;
  }

  const draft = normalizeDraft({
    ...raw.draft,
    method: normalizeStoredMethod(raw.draft.method)
  });

  const restored = createOpenRequestTab({
    draft,
    collectionId: raw.collectionId ?? null,
    requestTab: isRequestTab(raw.requestTab)
      ? raw.requestTab
      : defaultRequestTabForMethod(draft.method),
    responseTab: isResponseTab(raw.responseTab) ? raw.responseTab : "body",
    showSettings: Boolean(raw.showSettings),
    isDirty: Boolean(raw.isDirty),
    originRequestId: raw.originRequestId ?? draft.id ?? null
  });

  return {
    ...restored,
    key: raw.key?.trim() ? raw.key : restored.key
  };
}

function serializeClosedRequestTab(item: ClosedRequestTab): PersistedClosedRequestTab {
  return {
    tab: serializeOpenRequestTab(item.tab),
    index: item.index
  };
}

function deserializePersistedClosedRequestTab(
  raw: PersistedClosedRequestTab | null | undefined
): ClosedRequestTab | null {
  if (!raw) {
    return null;
  }

  const tab = deserializePersistedOpenRequestTab(raw.tab);

  if (!tab) {
    return null;
  }

  return {
    tab,
    index: Number.isFinite(raw.index) ? raw.index : 0
  };
}

function buildPersistedWorkspaceSession({
  requestTabs,
  activeRequestTabKey,
  selectedCollectionId,
  workspaceTab,
  dataTab,
  runnerEnvironmentOverrideId,
  stopOnError,
  closedRequestTabs
}: {
  requestTabs: OpenRequestTab[];
  activeRequestTabKey: string;
  selectedCollectionId: string | null;
  workspaceTab: WorkspaceTab;
  dataTab: DataTab;
  runnerEnvironmentOverrideId: string | null;
  stopOnError: boolean;
  closedRequestTabs: ClosedRequestTab[];
}): PersistedWorkspaceSession {
  return {
    version: 1,
    savedAt: new Date().toISOString(),
    requestTabs: requestTabs.map(serializeOpenRequestTab),
    activeRequestTabKey,
    selectedCollectionId,
    workspaceTab,
    dataTab,
    runnerEnvironmentOverrideId,
    stopOnError,
    closedRequestTabs: closedRequestTabs.slice(-12).map(serializeClosedRequestTab)
  };
}

function readStoredWorkspaceSession(): SessionRestoreState {
  const recoveryMarker = safeReadLocalStorage(SESSION_ACTIVE_MARKER_STORAGE_KEY);
  const raw = safeReadLocalStorage(WORKSPACE_SESSION_STORAGE_KEY);

  if (!raw) {
    return {
      session: null,
      recoveredAfterCrash: false
    };
  }

  try {
    const parsed = JSON.parse(raw) as Partial<PersistedWorkspaceSession>;
    const requestTabs = Array.isArray(parsed.requestTabs)
      ? parsed.requestTabs
          .map((item) => deserializePersistedOpenRequestTab(item as PersistedOpenRequestTab))
          .filter((item): item is OpenRequestTab => item !== null)
      : [];

    if (requestTabs.length === 0) {
      return {
        session: null,
        recoveredAfterCrash: false
      };
    }

    const closedRequestTabs = Array.isArray(parsed.closedRequestTabs)
      ? parsed.closedRequestTabs
          .map((item) =>
            deserializePersistedClosedRequestTab(item as PersistedClosedRequestTab)
          )
          .filter((item): item is ClosedRequestTab => item !== null)
      : [];

    const activeRequestTabKey = requestTabs.some(
      (tab) => tab.key === parsed.activeRequestTabKey
    )
      ? (parsed.activeRequestTabKey as string)
      : requestTabs[0].key;

    return {
      session: {
        savedAt:
          typeof parsed.savedAt === "string"
            ? parsed.savedAt
            : new Date().toISOString(),
        requestTabs,
        activeRequestTabKey,
        selectedCollectionId:
          typeof parsed.selectedCollectionId === "string"
            ? parsed.selectedCollectionId
            : null,
        workspaceTab: isWorkspaceTab(parsed.workspaceTab)
          ? parsed.workspaceTab
          : "environments",
        dataTab: isDataTab(parsed.dataTab) ? parsed.dataTab : "export",
        runnerEnvironmentOverrideId:
          typeof parsed.runnerEnvironmentOverrideId === "string"
            ? parsed.runnerEnvironmentOverrideId
            : null,
        stopOnError: Boolean(parsed.stopOnError),
        closedRequestTabs
      },
      recoveredAfterCrash: Boolean(recoveryMarker)
    };
  } catch {
    return {
      session: null,
      recoveredAfterCrash: false
    };
  }
}

function writeStoredWorkspaceSession(session: PersistedWorkspaceSession): void {
  safeWriteLocalStorage(WORKSPACE_SESSION_STORAGE_KEY, JSON.stringify(session));
}

function formatSessionTimestamp(value: string | null | undefined): string {
  if (!value) {
    return "sin registro";
  }

  const parsed = new Date(value);

  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("es-AR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  }).format(parsed);
}

function looksLikeJsonText(value: string): boolean {
  const trimmed = value.trim();

  if (!trimmed) {
    return false;
  }

  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    return true;
  }

  return false;
}

function prettifyJsonText(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

function decodeQueryValue(value: string): string {
  try {
    return decodeURIComponent(value.replace(/\+/g, " "));
  } catch {
    return value;
  }
}

function parseQueryStringToRows(queryString: string): KeyValueRow[] {
  if (!queryString.trim()) {
    return [];
  }

  return queryString
    .split("&")
    .filter((part) => part.length > 0)
    .map((part) => {
      const separatorIndex = part.indexOf("=");

      if (separatorIndex === -1) {
        return createRow(decodeQueryValue(part), "", true);
      }

      return createRow(
        decodeQueryValue(part.slice(0, separatorIndex)),
        decodeQueryValue(part.slice(separatorIndex + 1)),
        true
      );
    });
}

function splitUrlAndQuery(rawUrl: string): {
  url: string;
  queryRows: KeyValueRow[];
} {
  const questionMarkIndex = rawUrl.indexOf("?");

  if (questionMarkIndex === -1) {
    return {
      url: rawUrl,
      queryRows: []
    };
  }

  const base = rawUrl.slice(0, questionMarkIndex);
  const queryAndHash = rawUrl.slice(questionMarkIndex + 1);
  const hashIndex = queryAndHash.indexOf("#");
  const query = hashIndex === -1 ? queryAndHash : queryAndHash.slice(0, hashIndex);
  const hash = hashIndex === -1 ? "" : queryAndHash.slice(hashIndex);

  return {
    url: `${base}${hash}`,
    queryRows: parseQueryStringToRows(query)
  };
}

function buildRequestNameFromUrl(method: HttpMethod, rawUrl: string): string {
  const trimmed = rawUrl.trim();

  if (!trimmed) {
    return "Nueva petición";
  }

  const withoutQuery = trimmed.split("?")[0] ?? trimmed;
  const segments = withoutQuery.split("/").filter(Boolean);
  const lastSegment = segments[segments.length - 1] ?? "peticion";
  return `${method} ${lastSegment}`;
}

function tokenizeCurlCommand(command: string): string[] {
  const normalized = command.replace(/\\\r?\n/g, " ").replace(/\r/g, " ").trim();
  const tokens: string[] = [];
  let current = "";
  let quote: "single" | "double" | null = null;
  let escaping = false;

  for (const character of normalized) {
    if (escaping) {
      current += character;
      escaping = false;
      continue;
    }

    if (quote === "single") {
      if (character === "'") {
        quote = null;
      } else {
        current += character;
      }
      continue;
    }

    if (quote === "double") {
      if (character === '"') {
        quote = null;
      } else if (character === "\\") {
        escaping = true;
      } else {
        current += character;
      }
      continue;
    }

    if (character === "\\") {
      escaping = true;
      continue;
    }

    if (character === "'") {
      quote = "single";
      continue;
    }

    if (character === '"') {
      quote = "double";
      continue;
    }

    if (/\s/.test(character)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
      continue;
    }

    current += character;
  }

  if (current) {
    tokens.push(current);
  }

  return tokens;
}

function looksLikeCurlCommand(value: string): boolean {
  return /^curl(?:\.exe)?\b/i.test(value.trim());
}

function normalizeHttpMethodFromCurl(value: string, warnings: string[]): HttpMethod {
  const normalized = value.trim().toUpperCase();

  if (METHODS.includes(normalized as HttpMethod)) {
    return normalized as HttpMethod;
  }

  warnings.push(`Método no soportado: ${value}. Se usó GET.`);
  return "GET";
}

function parseCurlHeader(headerLine: string): {
  key: string;
  value: string;
} | null {
  const separatorIndex = headerLine.indexOf(":");

  if (separatorIndex === -1) {
    return null;
  }

  return {
    key: headerLine.slice(0, separatorIndex).trim(),
    value: headerLine.slice(separatorIndex + 1).trim()
  };
}

function parseCurlBasicAuth(rawValue: string): AuthConfig {
  const separatorIndex = rawValue.indexOf(":");

  if (separatorIndex === -1) {
    return {
      type: "basic",
      username: rawValue,
      password: ""
    };
  }

  return {
    type: "basic",
    username: rawValue.slice(0, separatorIndex),
    password: rawValue.slice(separatorIndex + 1)
  };
}

function parseCurlCommandToDraft(command: string): {
  draft: RequestDraft;
  warnings: string[];
} {
  const tokens = tokenizeCurlCommand(command);

  if (tokens.length === 0 || !/^curl(?:\.exe)?$/i.test(tokens[0])) {
    throw new Error('Pegá un comando completo que empiece con "curl".');
  }

  const warnings: string[] = [];
  let method: HttpMethod | null = null;
  let url = "";
  let forceQueryString = false;
  let inferJsonBody = false;
  const headerTokens: string[] = [];
  const dataTokens: string[] = [];
  let auth: AuthConfig = { type: "none" };

  const takeValue = (label: string, index: number): string => {
    const value = tokens[index + 1];

    if (!value) {
      throw new Error(`Falta un valor para ${label}.`);
    }

    return value;
  };

  for (let index = 1; index < tokens.length; index += 1) {
    const token = tokens[index];

    if (token === "-X" || token === "--request") {
      method = normalizeHttpMethodFromCurl(takeValue(token, index), warnings);
      index += 1;
      continue;
    }

    if (token.startsWith("--request=")) {
      method = normalizeHttpMethodFromCurl(token.slice("--request=".length), warnings);
      continue;
    }

    if (token.startsWith("-X") && token.length > 2) {
      method = normalizeHttpMethodFromCurl(token.slice(2), warnings);
      continue;
    }

    if (token === "--url") {
      url = takeValue(token, index);
      index += 1;
      continue;
    }

    if (token.startsWith("--url=")) {
      url = token.slice("--url=".length);
      continue;
    }

    if (token === "-H" || token === "--header") {
      headerTokens.push(takeValue(token, index));
      index += 1;
      continue;
    }

    if (token.startsWith("--header=")) {
      headerTokens.push(token.slice("--header=".length));
      continue;
    }

    if (token.startsWith("-H") && token.length > 2) {
      headerTokens.push(token.slice(2));
      continue;
    }

    if (token === "-u" || token === "--user") {
      auth = parseCurlBasicAuth(takeValue(token, index));
      index += 1;
      continue;
    }

    if (token.startsWith("--user=")) {
      auth = parseCurlBasicAuth(token.slice("--user=".length));
      continue;
    }

    if (token.startsWith("-u") && token.length > 2) {
      auth = parseCurlBasicAuth(token.slice(2));
      continue;
    }

    if (token === "-I" || token === "--head") {
      method = "HEAD";
      continue;
    }

    if (token === "-G" || token === "--get") {
      method = "GET";
      forceQueryString = true;
      continue;
    }

    if (token === "--json") {
      dataTokens.push(takeValue(token, index));
      inferJsonBody = true;
      if (!method) {
        method = "POST";
      }
      index += 1;
      continue;
    }

    const dataOptionPrefixes = [
      "--data=",
      "--data-raw=",
      "--data-binary=",
      "--data-ascii=",
      "--data-urlencode=",
      "--form=",
      "--form-string="
    ];

    const inlineData = dataOptionPrefixes.find((prefix) => token.startsWith(prefix));

    if (inlineData) {
      dataTokens.push(token.slice(inlineData.length));
      if (!method && !forceQueryString) {
        method = "POST";
      }
      continue;
    }

    if (
      token === "-d" ||
      token === "--data" ||
      token === "--data-raw" ||
      token === "--data-binary" ||
      token === "--data-ascii" ||
      token === "--data-urlencode" ||
      token === "-F" ||
      token === "--form" ||
      token === "--form-string"
    ) {
      dataTokens.push(takeValue(token, index));
      if (!method && !forceQueryString) {
        method = "POST";
      }
      index += 1;
      continue;
    }

    if (
      token === "--location" ||
      token === "-L" ||
      token === "--silent" ||
      token === "-s" ||
      token === "--compressed" ||
      token === "--fail" ||
      token === "--include" ||
      token === "-i" ||
      token === "--verbose" ||
      token === "-v" ||
      token === "--insecure" ||
      token === "-k"
    ) {
      continue;
    }

    if (!token.startsWith("-") && !url) {
      url = token;
      continue;
    }

    if (token.startsWith("-")) {
      warnings.push(`Flag ignorada: ${token}`);
    }
  }

  if (!url.trim()) {
    throw new Error("No pude encontrar la URL dentro del cURL.");
  }

  const headers: KeyValueRow[] = [];
  let hasJsonContentType = false;

  for (const headerToken of headerTokens) {
    const parsedHeader = parseCurlHeader(headerToken);

    if (!parsedHeader) {
      warnings.push(`Encabezado ignorado: ${headerToken}`);
      continue;
    }

    const lowerKey = parsedHeader.key.toLowerCase();

    if (lowerKey === "authorization" && auth.type === "none") {
      const bearerMatch = parsedHeader.value.match(/^Bearer\s+(.+)$/i);

      if (bearerMatch) {
        auth = {
          type: "bearer",
          token: bearerMatch[1]
        };
        continue;
      }
    }

    if (
      lowerKey === "content-type" &&
      parsedHeader.value.toLowerCase().includes("application/json")
    ) {
      hasJsonContentType = true;
    }

    headers.push(createRow(parsedHeader.key, parsedHeader.value, true));
  }

  if (inferJsonBody) {
    const hasContentTypeHeader = headers.some(
      (row) => row.key.toLowerCase() === "content-type"
    );
    const hasAcceptHeader = headers.some((row) => row.key.toLowerCase() === "accept");

    if (!hasContentTypeHeader) {
      headers.push(createRow("content-type", "application/json", true));
      hasJsonContentType = true;
    }

    if (!hasAcceptHeader) {
      headers.push(createRow("accept", "application/json", true));
    }
  }

  const splitUrl = splitUrlAndQuery(url.trim());
  const queryRows = [...splitUrl.queryRows];

  if (forceQueryString && dataTokens.length > 0) {
    queryRows.push(...parseQueryStringToRows(dataTokens.join("&")));
  }

  let bodyMode: BodyMode = "none";
  let bodyValue = "";

  if (!forceQueryString && dataTokens.length > 0) {
    bodyValue = dataTokens.join("&");
    bodyMode = hasJsonContentType || inferJsonBody || looksLikeJsonText(bodyValue)
      ? "json"
      : "text";

    if (bodyMode === "json") {
      bodyValue = prettifyJsonText(bodyValue);
    }
  }

  const inferredMethod = method ?? (bodyMode === "none" ? "GET" : "POST");
  const draft = createBlankDraft();

  draft.name = buildRequestNameFromUrl(inferredMethod, splitUrl.url);
  draft.method = inferredMethod;
  draft.url = splitUrl.url;
  draft.query = queryRows;
  draft.headers = headers;
  draft.auth = auth;
  draft.body = {
    mode: bodyMode,
    value: bodyValue
  };
  draft.responseTests = [];

  return {
    draft,
    warnings
  };
}

function isEditableElement(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  if (target.isContentEditable) {
    return true;
  }

  const tagName = target.tagName;
  return tagName === "INPUT" || tagName === "TEXTAREA" || tagName === "SELECT";
}

const INITIAL_OPEN_REQUEST_TAB = createOpenRequestTab({
  draft: createInitialDraft(),
  isDirty: false
});

const INITIAL_SESSION_RESTORE = readStoredWorkspaceSession();

function RequestWorkspaceTabsStrip({
  tabs,
  activeKey,
  onSelect,
  onClose,
  onNewRequest,
  onReorder
}: {
  tabs: OpenRequestTab[];
  activeKey: string;
  onSelect: (key: string) => void;
  onClose: (key: string) => void;
  onNewRequest: () => void;
  onReorder: (
    dragKey: string,
    targetKey: string,
    placement: ReorderPlacement
  ) => void;
}) {
  const [draggingKey, setDraggingKey] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<{
    key: string;
    placement: ReorderPlacement;
  } | null>(null);

  const clearDragState = () => {
    setDraggingKey(null);
    setDropTarget(null);
  };

  return (
    <div className="request-workspace-tabs" role="tablist" aria-label="Peticiones abiertas">
      <div className="request-workspace-tabs-scroll">
        {tabs.map((tab) => {
          const active = tab.key === activeKey;
          const label = requestTabLabel(tab);
          const dropClass =
            dropTarget?.key === tab.key ? `drop-${dropTarget.placement}` : "";
          const dragging = draggingKey === tab.key;

          return (
            <div
              aria-grabbed={dragging}
              className={`request-workspace-tab ${active ? "active" : ""} ${
                dragging ? "dragging" : ""
              } ${dropClass}`.trim()}
              draggable
              key={tab.key}
              onDragEnd={clearDragState}
              onDragOver={(event) => {
                if (!draggingKey || draggingKey === tab.key) {
                  return;
                }

                event.preventDefault();
                const bounds = event.currentTarget.getBoundingClientRect();
                const placement: ReorderPlacement =
                  event.clientX - bounds.left < bounds.width / 2 ? "before" : "after";

                setDropTarget((current) =>
                  current?.key === tab.key && current.placement === placement
                    ? current
                    : {
                        key: tab.key,
                        placement
                      }
                );
              }}
              onDragStart={(event) => {
                setDraggingKey(tab.key);
                event.dataTransfer.effectAllowed = "move";
                event.dataTransfer.setData("text/plain", tab.key);
              }}
              onDrop={(event) => {
                event.preventDefault();

                if (draggingKey && dropTarget && dropTarget.key === tab.key) {
                  onReorder(draggingKey, dropTarget.key, dropTarget.placement);
                }

                clearDragState();
              }}
            >
              <button
                aria-selected={active}
                className="request-workspace-tab-main"
                onClick={() => onSelect(tab.key)}
                role="tab"
                title={`${label} · Arrastrá para reordenar`}
                type="button"
              >
                <span className={`request-workspace-method method-${tab.draft.method.toLowerCase()}`}>
                  {tab.draft.method}
                </span>
                <span className="request-workspace-label">{label}</span>
                {tab.isDirty ? <span className="request-workspace-dirty" /> : null}
              </button>

              <button
                aria-label={`Cerrar ${label}`}
                className="request-workspace-tab-close"
                draggable={false}
                onClick={() => onClose(tab.key)}
                type="button"
              >
                ×
              </button>
            </div>
          );
        })}
      </div>

      <button
        aria-label="Nueva petición"
        className="icon-button request-workspace-add"
        onClick={onNewRequest}
        type="button"
      >
        +
      </button>
    </div>
  );
}


type CommandPaletteProps = {
  open: boolean;
  query: string;
  results: CommandPaletteItem[];
  selectedIndex: number;
  onQueryChange: (value: string) => void;
  onClose: () => void;
  onSelect: (item: CommandPaletteItem) => void;
};

function CommandPalette({
  open,
  query,
  results,
  selectedIndex,
  onQueryChange,
  onClose,
  onSelect
}: CommandPaletteProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }

    const timer = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(timer);
  }, [open]);

  if (!open) {
    return null;
  }

  return (
    <div className="command-palette-overlay" onClick={onClose} role="presentation">
      <div
        aria-label="Paleta de comandos"
        aria-modal="true"
        className="command-palette"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
      >
        <div className="command-palette-header">
          <input
            className="input command-palette-input"
            onChange={(event) => onQueryChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                onClose();
              }
            }}
            placeholder="Buscar peticiones, colecciones o acciones…"
            ref={inputRef}
            value={query}
          />
          <button className="icon-button" onClick={onClose} type="button">
            ×
          </button>
        </div>

        <div className="command-palette-results">
          {results.length === 0 ? (
            <div className="empty-surface muted small">No encontré coincidencias.</div>
          ) : (
            results.map((item, index) => (
              <button
                className={`command-palette-item ${index === selectedIndex ? "active" : ""}`}
                key={item.id}
                onClick={() => onSelect(item)}
                type="button"
              >
                <div>
                  <div className="command-palette-title">{item.title}</div>
                  {item.subtitle ? (
                    <div className="muted small">{item.subtitle}</div>
                  ) : null}
                </div>
                <span className="badge">{item.section}</span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

export default function App() {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null);
  const [requestTabs, setRequestTabs] = useState<OpenRequestTab[]>(
    INITIAL_SESSION_RESTORE.session?.requestTabs ?? [INITIAL_OPEN_REQUEST_TAB]
  );
  const [activeRequestTabKey, setActiveRequestTabKey] = useState<string>(
    INITIAL_SESSION_RESTORE.session?.activeRequestTabKey ??
      INITIAL_SESSION_RESTORE.session?.requestTabs[0]?.key ??
      INITIAL_OPEN_REQUEST_TAB.key
  );
  const [collectionRunReport, setCollectionRunReport] =
    useState<CollectionRunReport | null>(null);
  const [runnerProgress, setRunnerProgress] =
    useState<CollectionRunProgressEvent | null>(null);
  const [lastExportResult, setLastExportResult] =
    useState<ExportWorkspaceResult | null>(null);
  const [lastImportResult, setLastImportResult] =
    useState<ImportWorkspaceResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [recoveryNotice, setRecoveryNotice] = useState<string | null>(
    INITIAL_SESSION_RESTORE.recoveredAfterCrash && INITIAL_SESSION_RESTORE.session
      ? INITIAL_SESSION_RESTORE.session.savedAt
      : null
  );
  const [lastAutosavedAt, setLastAutosavedAt] = useState<string | null>(
    INITIAL_SESSION_RESTORE.session?.savedAt ?? null
  );
  const autosaveTimeoutRef = useRef<number | null>(null);
  const [busy, setBusy] = useState<
    "preview" | "send" | "save" | "sync" | "runCollection" | "interop" | null
  >(null);
  const [selectedCollectionId, setSelectedCollectionId] = useState<string | null>(
    INITIAL_SESSION_RESTORE.session?.selectedCollectionId ?? null
  );
  const [newCollectionName, setNewCollectionName] = useState("");
  const [showCreateCollection, setShowCreateCollection] = useState(false);
  const [environmentEditor, setEnvironmentEditor] =
    useState<SaveEnvironmentInput>(createEmptyEnvironmentEditor);
  const [runnerEnvironmentOverrideId, setRunnerEnvironmentOverrideId] = useState<
    string | null
  >(INITIAL_SESSION_RESTORE.session?.runnerEnvironmentOverrideId ?? null);
  const [stopOnError, setStopOnError] = useState(
    INITIAL_SESSION_RESTORE.session?.stopOnError ?? false
  );
  const [exportPath, setExportPath] = useState("");
  const [exportFormat, setExportFormat] =
    useState<WorkspaceExportFormat>("nativeWorkspaceV1");
  const [exportScope, setExportScope] = useState<ExportScope>("workspace");
  const [requestExportFormat, setRequestExportFormat] =
    useState<RequestCodeExportFormat>("curl");
  const [includeHistoryInExport, setIncludeHistoryInExport] = useState(true);
  const [includeSecretMetadataInExport, setIncludeSecretMetadataInExport] =
    useState(true);
  const [importPath, setImportPath] = useState("");
  const [importFormat, setImportFormat] =
    useState<WorkspaceImportFormat>("auto");
  const [importMerge, setImportMerge] = useState(true);
  const [workspaceTab, setWorkspaceTab] = useState<WorkspaceTab>(
    INITIAL_SESSION_RESTORE.session?.workspaceTab ?? "environments"
  );
  const [dataTab, setDataTab] = useState<DataTab>(
    INITIAL_SESSION_RESTORE.session?.dataTab ?? "export"
  );
  const [sidePanel, setSidePanel] = useState<SidePanel>(null);
  const [closedRequestTabs, setClosedRequestTabs] = useState<ClosedRequestTab[]>(
    INITIAL_SESSION_RESTORE.session?.closedRequestTabs ?? []
  );
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [commandPaletteQuery, setCommandPaletteQuery] = useState("");
  const [commandPaletteSelectedIndex, setCommandPaletteSelectedIndex] = useState(0);
  const [crashRecords, setCrashRecords] = useState<CrashRecord[]>(() => readCrashRecords());
  const [importPayloadText, setImportPayloadText] = useState("");
  const [activeExecutionId, setActiveExecutionId] = useState<string | null>(null);
  const [sidebarWidth, setSidebarWidth] = useState(() =>
    readStoredPanelWidth(
      SIDEBAR_WIDTH_STORAGE_KEY,
      DEFAULT_SIDEBAR_WIDTH,
      MIN_SIDEBAR_WIDTH,
      MAX_SIDEBAR_WIDTH
    )
  );
  const [responseWidth, setResponseWidth] = useState(() =>
    readStoredPanelWidth(
      RESPONSE_WIDTH_STORAGE_KEY,
      DEFAULT_RESPONSE_WIDTH,
      MIN_RESPONSE_WIDTH,
      getMaxResponseWidth()
    )
  );
  const [resizeState, setResizeState] = useState<ResizeState | null>(null);

  const activeTab = useMemo(
    () =>
      requestTabs.find((tab) => tab.key === activeRequestTabKey) ?? requestTabs[0] ?? null,
    [requestTabs, activeRequestTabKey]
  );

  const activeCollectionId = activeTab?.collectionId ?? selectedCollectionId;

  const activeCollection = useMemo(
    () =>
      snapshot?.collections.find((item) => item.collection.id === activeCollectionId) ??
      null,
    [snapshot, activeCollectionId]
  );

  const activeEnvironment = useMemo(
    () =>
      snapshot?.environments.find((env) => env.id === activeTab?.draft.environmentId) ??
      null,
    [snapshot, activeTab]
  );

  const selectedCollection = useMemo(
    () =>
      snapshot?.collections.find((item) => item.collection.id === selectedCollectionId) ??
      null,
    [snapshot, selectedCollectionId]
  );

  const response = activeTab?.executionOutcome?.response ?? null;
  const busyNow = busy !== null;
  const collectionCount = snapshot?.collections.length ?? 0;
  const totalRequestCount = (snapshot?.collections ?? []).reduce(
    (total, item) => total + item.requests.length,
    0
  );
  const environmentCount = snapshot?.environments.length ?? 0;
  const hasUnsavedChanges = useMemo(
    () => requestTabs.some((tab) => tab.isDirty),
    [requestTabs]
  );

  const commandPaletteItems = useMemo(() => {
    const items: CommandPaletteItem[] = [
      {
        id: "action:new-request",
        title: "Nueva petición",
        subtitle: "Abrir una pestaña vacía",
        section: "Acción",
        keywords: ["new", "request", "crear"]
      },
      {
        id: "action:save-request",
        title: "Guardar petición",
        subtitle: activeCollection?.collection.name ?? "Guardar en la colección actual",
        section: "Acción",
        keywords: ["save", "guardar", "request"]
      },
      {
        id: "action:send-request",
        title: activeExecutionId ? "Cancelar petición en curso" : "Enviar petición",
        subtitle: activeExecutionId ? "Cancela la ejecución actual" : "Ejecutar la petición activa",
        section: "Acción",
        keywords: activeExecutionId ? ["cancel", "stop"] : ["send", "run", "execute"]
      },
      {
        id: "action:workspace",
        title: "Abrir panel",
        subtitle: "Entornos, datos, historial y diagnósticos",
        section: "Acción",
        keywords: ["workspace", "tools", "settings"]
      },
      {
        id: "action:copy-curl",
        title: "Copiar cURL",
        subtitle: "Exportar la petición activa como cURL",
        section: "Acción",
        keywords: ["curl", "export", "copy"]
      },
      {
        id: "action:copy-fetch",
        title: "Copiar fetch",
        subtitle: "Exportar la petición activa como fetch",
        section: "Acción",
        keywords: ["fetch", "export", "copy"]
      },
      {
        id: "action:copy-axios",
        title: "Copiar axios",
        subtitle: "Exportar la petición activa como axios",
        section: "Acción",
        keywords: ["axios", "export", "copy"]
      }
    ];

    for (const collection of snapshot?.collections ?? []) {
      items.push({
        id: `collection:${collection.collection.id}`,
        title: collection.collection.name,
        subtitle: `${collection.requests.length} peticiones`,
        section: "Colección",
        keywords: ["collection", collection.collection.name]
      });

      for (const request of collection.requests) {
        items.push({
          id: `request:${request.id}`,
          title: request.name,
          subtitle: `${request.draft.method} ${request.draft.url}`,
          section: "Petición",
          keywords: [
            request.draft.method,
            request.name,
            request.draft.url,
            collection.collection.name
          ]
        });
      }
    }

    return items;
  }, [snapshot, activeCollection, activeExecutionId]);

  const filteredCommandPaletteItems = useMemo(
    () => searchPaletteItems(commandPaletteItems, commandPaletteQuery, 14),
    [commandPaletteItems, commandPaletteQuery]
  );

  const updateRequestTabByKey = useCallback(
    (key: string, updater: (tab: OpenRequestTab) => OpenRequestTab) => {
      setRequestTabs((tabs) =>
        tabs.map((tab) => (tab.key === key ? updater(tab) : tab))
      );
    },
    []
  );

  const updateActiveRequestTab = useCallback(
    (updater: (tab: OpenRequestTab) => OpenRequestTab) => {
      setRequestTabs((tabs) =>
        tabs.map((tab) => (tab.key === activeRequestTabKey ? updater(tab) : tab))
      );
    },
    [activeRequestTabKey]
  );

  const refreshWorkspace = useCallback(async () => {
    setBusy("sync");

    try {
      const next = await workspaceSnapshot();
      setSnapshot(next);
      setError(null);

      setSelectedCollectionId((current) => {
        if (current && next.collections.some((item) => item.collection.id === current)) {
          return current;
        }

        return next.collections[0]?.collection.id ?? null;
      });

      setRunnerEnvironmentOverrideId((current) => {
        if (current && next.environments.some((env) => env.id === current)) {
          return current;
        }

        return null;
      });

      setRequestTabs((tabs) =>
        tabs.map((tab) => {
          let nextTab = tab;

          if (
            tab.draft.environmentId &&
            !next.environments.some((env) => env.id === tab.draft.environmentId)
          ) {
            nextTab = {
              ...nextTab,
              draft: { ...nextTab.draft, environmentId: null }
            };
          }

          if (
            tab.collectionId &&
            !next.collections.some((item) => item.collection.id === tab.collectionId)
          ) {
            nextTab = {
              ...nextTab,
              collectionId: null
            };
          }

          return nextTab;
        })
      );
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  }, []);

  useEffect(() => {
    void refreshWorkspace();
  }, [refreshWorkspace]);

  useEffect(() => {
    const uninstall = installCrashCapture();
    setCrashRecords(readCrashRecords());
    return uninstall;
  }, []);

  useEffect(() => {
    if (sidePanel === "workspace" && workspaceTab === "history") {
      setCrashRecords(readCrashRecords());
    }
  }, [sidePanel, workspaceTab]);

  useEffect(() => {
    if (!commandPaletteOpen) {
      setCommandPaletteQuery("");
      setCommandPaletteSelectedIndex(0);
    }
  }, [commandPaletteOpen]);

  useEffect(() => {
    setCommandPaletteSelectedIndex(0);
  }, [commandPaletteQuery]);

  useEffect(() => {
    const unlistenPromise = listen<CollectionRunProgressEvent>(
      COLLECTION_RUN_PROGRESS_EVENT,
      (event) => {
        setRunnerProgress(event.payload);
      }
    );

    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    if (requestTabs.length === 0) {
      return;
    }

    if (!requestTabs.some((tab) => tab.key === activeRequestTabKey)) {
      setActiveRequestTabKey(requestTabs[0].key);
    }
  }, [requestTabs, activeRequestTabKey]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    if (autosaveTimeoutRef.current) {
      window.clearTimeout(autosaveTimeoutRef.current);
    }

    autosaveTimeoutRef.current = window.setTimeout(() => {
      const session = buildPersistedWorkspaceSession({
        requestTabs,
        activeRequestTabKey,
        selectedCollectionId,
        workspaceTab,
        dataTab,
        runnerEnvironmentOverrideId,
        stopOnError,
        closedRequestTabs
      });
      writeStoredWorkspaceSession(session);
      setLastAutosavedAt(session.savedAt);
    }, 320);

    return () => {
      if (autosaveTimeoutRef.current) {
        window.clearTimeout(autosaveTimeoutRef.current);
      }
    };
  }, [
    requestTabs,
    activeRequestTabKey,
    selectedCollectionId,
    workspaceTab,
    dataTab,
    runnerEnvironmentOverrideId,
    stopOnError,
    closedRequestTabs
  ]);

  useEffect(() => {
    safeWriteLocalStorage(
      SESSION_ACTIVE_MARKER_STORAGE_KEY,
      JSON.stringify({ startedAt: new Date().toISOString() })
    );

    const clearSessionMarker = () => {
      safeRemoveLocalStorage(SESSION_ACTIVE_MARKER_STORAGE_KEY);
    };

    window.addEventListener("pagehide", clearSessionMarker);
    window.addEventListener("unload", clearSessionMarker);

    return () => {
      window.removeEventListener("pagehide", clearSessionMarker);
      window.removeEventListener("unload", clearSessionMarker);
      clearSessionMarker();
    };
  }, []);

  useEffect(() => {
    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!hasUnsavedChanges) {
        return;
      }

      event.preventDefault();
      event.returnValue = "";
    };

    window.addEventListener("beforeunload", handleBeforeUnload);
    return () => window.removeEventListener("beforeunload", handleBeforeUnload);
  }, [hasUnsavedChanges]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    void import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        const nextUnlisten = await getCurrentWindow().onCloseRequested((event) => {
          if (!hasUnsavedChanges) {
            return;
          }

          const confirmed = window.confirm(
            "Tenés cambios sin guardar. ¿Querés cerrar igual? La sesión local queda recuperable al volver a abrir la app."
          );

          if (!confirmed) {
            event.preventDefault();
          }
        });

        if (cancelled) {
          nextUnlisten();
          return;
        }

        unlisten = nextUnlisten;
      })
      .catch(() => {
        // browser runtime without Tauri window bridge
      });

    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [hasUnsavedChanges]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    safeWriteLocalStorage(SIDEBAR_WIDTH_STORAGE_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    safeWriteLocalStorage(RESPONSE_WIDTH_STORAGE_KEY, String(responseWidth));
  }, [responseWidth]);

  useEffect(() => {
    const handleWindowResize = () => {
      setSidebarWidth((current) =>
        clamp(current, MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
      );
      setResponseWidth((current) =>
        clamp(current, MIN_RESPONSE_WIDTH, getMaxResponseWidth())
      );
    };

    handleWindowResize();
    window.addEventListener("resize", handleWindowResize);

    return () => window.removeEventListener("resize", handleWindowResize);
  }, []);

  useEffect(() => {
    if (!resizeState) {
      return;
    }

    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    const handleMouseMove = (event: MouseEvent) => {
      if (resizeState.kind === "sidebar") {
        const nextWidth = clamp(
          resizeState.startWidth + (event.clientX - resizeState.startX),
          MIN_SIDEBAR_WIDTH,
          MAX_SIDEBAR_WIDTH
        );
        setSidebarWidth(nextWidth);
        return;
      }

      const nextWidth = clamp(
        resizeState.startWidth - (event.clientX - resizeState.startX),
        MIN_RESPONSE_WIDTH,
        getMaxResponseWidth()
      );
      setResponseWidth(nextWidth);
    };

    const handleMouseUp = () => {
      setResizeState(null);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);

    return () => {
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [resizeState]);

  const openWorkspace = (tab?: WorkspaceTab) => {
    if (tab) {
      setWorkspaceTab(tab);
    }

    setSidePanel("workspace");
  };

  const openRunner = (collectionId: string) => {
    setSelectedCollectionId(collectionId);
    setSidePanel("runner");

    if (collectionRunReport && collectionRunReport.collectionId !== collectionId) {
      setCollectionRunReport(null);
    }

    if (runnerProgress && runnerProgress.collectionId !== collectionId) {
      setRunnerProgress(null);
    }
  };

  const focusRequestTab = (key: string) => {
    const tab = requestTabs.find((item) => item.key === key) ?? null;
    setActiveRequestTabKey(key);

    if (tab?.collectionId) {
      setSelectedCollectionId(tab.collectionId);
    }
  };

  const handleReorderRequestTabs = (
    dragKey: string,
    targetKey: string,
    placement: ReorderPlacement
  ) => {
    if (dragKey === targetKey) {
      return;
    }

    setRequestTabs((tabs) => {
      const sourceIndex = tabs.findIndex((tab) => tab.key === dragKey);
      const targetIndex = tabs.findIndex((tab) => tab.key === targetKey);

      if (sourceIndex === -1 || targetIndex === -1) {
        return tabs;
      }

      const nextTabs = [...tabs];
      const [draggedTab] = nextTabs.splice(sourceIndex, 1);
      const insertionTargetIndex = nextTabs.findIndex((tab) => tab.key === targetKey);

      if (!draggedTab || insertionTargetIndex === -1) {
        return tabs;
      }

      const insertionIndex =
        insertionTargetIndex + (placement === "after" ? 1 : 0);
      nextTabs.splice(insertionIndex, 0, draggedTab);
      return nextTabs;
    });
  };

  const handleSelectCollection = (collectionId: string) => {
    setSelectedCollectionId(collectionId);

    if (activeTab && !activeTab.originRequestId) {
      updateActiveRequestTab((tab) => ({
        ...tab,
        collectionId
      }));
    }
  };

  const handleLoadRequest = (record: SavedRequestRecord) => {
    const existing = requestTabs.find((tab) => tab.originRequestId === record.id) ?? null;

    if (existing) {
      setActiveRequestTabKey(existing.key);
      setSelectedCollectionId(record.collectionId);
      setError(null);
      setSuccess(null);
      return;
    }

    const nextTab = createOpenRequestTab({
      draft: normalizeDraft(record.draft),
      collectionId: record.collectionId,
      originRequestId: record.id,
      isDirty: false
    });

    setRequestTabs((tabs) => [...tabs, nextTab]);
    setActiveRequestTabKey(nextTab.key);
    setSelectedCollectionId(record.collectionId);
    setError(null);
    setSuccess(null);
  };

  const handleCloseRequestTab = (key: string) => {
    const index = requestTabs.findIndex((tab) => tab.key === key);

    if (index === -1) {
      return;
    }

    const closingTab = requestTabs[index];
    const remaining = requestTabs.filter((tab) => tab.key !== key);
    const wasActive = activeRequestTabKey === key;

    setClosedRequestTabs((stack) =>
      [...stack, { tab: closingTab, index }].slice(-12)
    );

    if (remaining.length === 0) {
      const blankTab = createOpenRequestTab({
        draft: createBlankDraft(),
        collectionId: selectedCollectionId,
        isDirty: false
      });
      setRequestTabs([blankTab]);
      setActiveRequestTabKey(blankTab.key);
      setSelectedCollectionId(blankTab.collectionId ?? selectedCollectionId);
      return;
    }

    setRequestTabs(remaining);

    if (!wasActive) {
      return;
    }

    const fallbackTab =
      remaining[index] ?? remaining[index - 1] ?? remaining[0];
    setActiveRequestTabKey(fallbackTab.key);
    setSelectedCollectionId(fallbackTab.collectionId ?? selectedCollectionId);
  };

  const handleReopenClosedRequestTab = () => {
    const lastClosed = closedRequestTabs[closedRequestTabs.length - 1] ?? null;

    if (!lastClosed) {
      return;
    }

    const reopenedTab = lastClosed.tab;

    setClosedRequestTabs((stack) => stack.slice(0, -1));
    setRequestTabs((tabs) => {
      if (tabs.length === 1 && isPlaceholderRequestTab(tabs[0])) {
        return [reopenedTab];
      }

      const insertAt = clamp(lastClosed.index, 0, tabs.length);
      const nextTabs = [...tabs];
      nextTabs.splice(insertAt, 0, reopenedTab);
      return nextTabs;
    });
    setActiveRequestTabKey(reopenedTab.key);
    setSelectedCollectionId(reopenedTab.collectionId ?? selectedCollectionId);
    setError(null);
    setSuccess("Tab reabierta.");
  };

  const handlePreview = async (): Promise<RequestPreview | null> => {
    if (!activeTab) {
      return null;
    }

    const tabKey = activeTab.key;
    const draftToPreview = activeTab.draft;

    setBusy("preview");
    setError(null);
    setSuccess(null);

    try {
      const next = await previewRequest(draftToPreview);
      updateRequestTabByKey(tabKey, (tab) => ({
        ...tab,
        preview: next
      }));
      return next;
    } catch (err) {
      setError(normalizeError(err));
      return null;
    } finally {
      setBusy(null);
    }
  };

  const handleCopyRequestExport = async (
    preferredFormat: RequestCodeExportFormat = requestExportFormat
  ) => {
    if (!activeTab) {
      return;
    }

    const preview = activeTab.preview ?? (await handlePreview());

    if (!preview) {
      return;
    }

    try {
      const snippet = generateRequestCodeSnippet({
        format: preferredFormat,
        draft: activeTab.draft,
        preview
      });
      await copyTextToClipboard(snippet);
      setRequestExportFormat(preferredFormat);
      setError(null);
      setSuccess(`${requestCodeExportLabel(preferredFormat)} copiado al portapapeles.`);
    } catch (err) {
      setError(normalizeError(err));
    }
  };

  const handleDownloadRequestExport = async (
    preferredFormat: RequestCodeExportFormat = requestExportFormat
  ) => {
    if (!activeTab) {
      return;
    }

    const preview = activeTab.preview ?? (await handlePreview());

    if (!preview) {
      return;
    }

    try {
      const snippet = generateRequestCodeSnippet({
        format: preferredFormat,
        draft: activeTab.draft,
        preview
      });
      downloadTextFile(
        requestCodeExportFilename(preferredFormat, activeTab.draft.name),
        snippet
      );
      setRequestExportFormat(preferredFormat);
      setError(null);
      setSuccess(`${requestCodeExportLabel(preferredFormat)} descargado.`);
    } catch (err) {
      setError(normalizeError(err));
    }
  };

  const handleSend = async () => {
    if (!activeTab) {
      return;
    }

    const tabKey = activeTab.key;
    const draftToExecute = activeTab.draft;
    const executionId = createId();

    setBusy("send");
    setActiveExecutionId(executionId);
    setError(null);
    setSuccess(null);

    updateRequestTabByKey(tabKey, (tab) => ({
      ...tab,
      responseTab: "body"
    }));

    try {
      const nextOutcome = await executeRequest(draftToExecute, executionId);
      const nextPreview = await previewRequest(draftToExecute);

      updateRequestTabByKey(tabKey, (tab) => ({
        ...tab,
        executionOutcome: nextOutcome,
        preview: nextPreview,
        responseTab: "body"
      }));

      setSuccess(
        `Request ejecutado. ${nextOutcome.assertionReport.passed}/${nextOutcome.assertionReport.total} tests pasaron.`
      );
      await refreshWorkspace();
    } catch (err) {
      const message = normalizeError(err);
      setError(message);
      appendCrashRecord({
        source: "react-boundary",
        message: `request error: ${message}`,
        stack: null
      });
      setCrashRecords(readCrashRecords());
    } finally {
      setActiveExecutionId((current) => (current === executionId ? null : current));
      setBusy(null);
    }
  };

  const handleCreateCollection = async () => {
    if (!newCollectionName.trim()) {
      return;
    }

    setBusy("save");
    setError(null);
    setSuccess(null);

    try {
      const collection = await createCollection(newCollectionName.trim());
      setNewCollectionName("");
      setShowCreateCollection(false);
      setSelectedCollectionId(collection.id);
      setSuccess("Colección creada.");

      if (activeTab && !activeTab.collectionId) {
        updateActiveRequestTab((tab) => ({
          ...tab,
          collectionId: collection.id
        }));
      }

      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleSaveRequest = async () => {
    if (!activeTab) {
      return;
    }

    const collectionId = activeTab.collectionId ?? selectedCollectionId;

    if (!collectionId) {
      setError("Necesitás seleccionar o crear una colección primero.");
      return;
    }

    const tabKey = activeTab.key;
    const draftToSave = activeTab.draft;

    setBusy("save");
    setError(null);
    setSuccess(null);

    try {
      const saved = await saveRequest({
        requestId: activeTab.originRequestId ?? activeTab.draft.id ?? null,
        collectionId,
        draft: draftToSave
      });

      updateRequestTabByKey(tabKey, (tab) => ({
        ...tab,
        collectionId,
        originRequestId: saved.id,
        isDirty: false,
        draft: normalizeDraft({ ...tab.draft, id: saved.id, name: saved.name })
      }));

      setSelectedCollectionId(collectionId);
      setSuccess(`Request guardado con ${draftToSave.responseTests.length} tests.`);
      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleSaveEnvironment = async () => {
    const tabKey = activeTab?.key ?? null;

    setBusy("save");
    setError(null);
    setSuccess(null);

    try {
      const saved = await saveEnvironment(environmentEditor);
      setEnvironmentEditor({
        environmentId: saved.id,
        name: saved.name,
        variables: saved.variables
      });

      if (tabKey) {
        updateRequestTabByKey(tabKey, (tab) =>
          tab.draft.environmentId
            ? tab
            : {
                ...tab,
                draft: { ...tab.draft, environmentId: saved.id },
                isDirty: true
              }
        );
      }

      setSuccess("Environment guardado.");
      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleDeleteEnvironment = async (environmentId: string) => {
    setBusy("save");
    setError(null);
    setSuccess(null);

    try {
      await deleteEnvironment(environmentId);
      setEnvironmentEditor(createEmptyEnvironmentEditor());
      setRequestTabs((tabs) =>
        tabs.map((tab) =>
          tab.draft.environmentId === environmentId
            ? { ...tab, draft: { ...tab.draft, environmentId: null } }
            : tab
        )
      );

      if (runnerEnvironmentOverrideId === environmentId) {
        setRunnerEnvironmentOverrideId(null);
      }

      setSuccess("Environment borrado.");
      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleSaveSecret = async (alias: string, value: string) => {
    setBusy("save");
    setError(null);
    setSuccess(null);

    try {
      await saveSecret({ alias, value });
      setSuccess("Secreto guardado.");
      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleDeleteSecret = async (alias: string) => {
    setBusy("save");
    setError(null);
    setSuccess(null);

    try {
      await deleteSecret(alias);
      setSuccess("Secreto eliminado.");
      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleRunCollection = async () => {
    if (!selectedCollectionId) {
      setError("Seleccioná una colección para ejecutar la colección.");
      return;
    }

    setBusy("runCollection");
    setError(null);
    setSuccess(null);
    setRunnerProgress(null);
    setSidePanel("runner");

    try {
      const report = await runCollection({
        collectionId: selectedCollectionId,
        environmentOverrideId: runnerEnvironmentOverrideId,
        stopOnError
      });
      setCollectionRunReport(report);
      setSuccess(
        `Collection corrida: ${report.completedRequests}/${report.totalRequests} requests, ${report.failedAssertions} tests fallidos.`
      );
      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleExportWorkspace = async () => {
    if (!exportPath.trim()) {
      setError("Indicá una ruta para exportar.");
      return;
    }

    const exportingSelectedCollection =
      exportFormat === "postmanCollectionV21" || exportScope === "collection";

    if (exportingSelectedCollection && !selectedCollectionId) {
      setError(
        exportFormat === "postmanCollectionV21"
          ? "Para exportar Postman v2.1 primero seleccioná una colección."
          : "Para exportar una colección Midway primero seleccioná una colección."
      );
      return;
    }

    setBusy("interop");
    setError(null);
    setSuccess(null);

    try {
      const result = await exportWorkspaceData({
        path: exportPath,
        format: exportFormat,
        collectionId: exportingSelectedCollection ? selectedCollectionId : null,
        includeHistory: exportScope === "collection" ? false : includeHistoryInExport,
        includeSecretMetadata: includeSecretMetadataInExport
      });
      setLastExportResult(result);
      setSuccess(`Export listo: ${result.path}`);
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleImportWorkspace = async () => {
    if (!importPath.trim()) {
      setError("Indicá una ruta para importar.");
      return;
    }

    setBusy("interop");
    setError(null);
    setSuccess(null);

    try {
      const result = await importWorkspaceData({
        path: importPath,
        format: importFormat,
        merge: importMerge,
        collectionNameOverride: null
      });
      setLastImportResult(result);

      if (result.collectionIds.length > 0) {
        setSelectedCollectionId(result.collectionIds[0]);
      }

      setSuccess(
        `Importación lista: ${result.collectionsImported} colecciones, ${result.requestsImported} peticiones.`
      );
      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleImportPayload = async () => {
    if (!importPayloadText.trim()) {
      setError("Pegá un JSON o YAML de Postman/OpenAPI para importar.");
      return;
    }

    setBusy("interop");
    setError(null);
    setSuccess(null);

    try {
      const result = await importWorkspacePayload({
        payload: importPayloadText,
        format: importFormat,
        merge: importMerge,
        collectionNameOverride: null,
        sourceLabel: "contenido pegado"
      });
      setLastImportResult(result);
      if (result.collectionIds.length > 0) {
        setSelectedCollectionId(result.collectionIds[0]);
      }
      setSuccess(
        `Contenido importado: ${result.collectionsImported} colecciones, ${result.requestsImported} peticiones.`
      );
      await refreshWorkspace();
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setBusy(null);
    }
  };

  const handleCancelActiveRequest = async () => {
    if (!activeExecutionId) {
      return;
    }

    try {
      await cancelRequest(activeExecutionId);
      setSuccess("Petición cancelada.");
    } catch (err) {
      setError(normalizeError(err));
    } finally {
      setActiveExecutionId(null);
      setBusy(null);
    }
  };

  const handleImportCurl = (command: string) => {
    const { draft: importedDraft, warnings } = parseCurlCommandToDraftImported(command);
    const baseCollectionId = selectedCollectionId ?? activeTab?.collectionId ?? null;
    const baseEnvironmentId = activeTab?.draft.environmentId ?? null;
    const baseTimeout = activeTab?.draft.timeoutMs ?? 30000;
    const nextDraft: RequestDraft = {
      ...importedDraft,
      environmentId: baseEnvironmentId,
      timeoutMs: baseTimeout
    };

    const reuseActiveTab =
      activeTab !== null &&
      (isPlaceholderRequestTab(activeTab) ||
        (!activeTab.originRequestId && !activeTab.isDirty && !activeTab.draft.url.trim()));

    if (activeTab && reuseActiveTab) {
      updateRequestTabByKey(activeTab.key, (tab) => ({
        ...tab,
        collectionId: tab.collectionId ?? baseCollectionId,
        draft: nextDraft,
        requestTab: defaultRequestTabForMethod(nextDraft.method),
        responseTab: "body",
        preview: null,
        executionOutcome: null,
        showSettings: false,
        originRequestId: null,
        isDirty: true
      }));
      setActiveRequestTabKey(activeTab.key);
    } else {
      const importedTab = createOpenRequestTab({
        draft: nextDraft,
        collectionId: baseCollectionId,
        isDirty: true,
        originRequestId: null
      });
      setRequestTabs((tabs) => [...tabs, importedTab]);
      setActiveRequestTabKey(importedTab.key);
    }

    setSelectedCollectionId(baseCollectionId ?? null);
    setError(null);
    setSuccess(
      warnings.length > 0
        ? `cURL importado. Se ${warnings.length === 1 ? "ignoró" : "ignoraron"} ${warnings.length} ${warnings.length === 1 ? "opción avanzada" : "opciones avanzadas"}.`
        : "cURL importado. Método, URL, encabezados y cuerpo inferidos."
    );
  };

  const handleUrlPaste = (event: React.ClipboardEvent<HTMLInputElement>) => {
    const pastedText = event.clipboardData.getData("text");

    if (!looksLikeCurlCommandImported(pastedText)) {
      return;
    }

    event.preventDefault();

    try {
      handleImportCurl(pastedText);
    } catch (err) {
      setError(normalizeError(err));
      setSuccess(null);
    }
  };

  const handleNewRequest = () => {
    const baseCollectionId = selectedCollectionId ?? activeTab?.collectionId ?? null;
    const blank = createOpenRequestTab({
      draft: createBlankDraft(),
      collectionId: baseCollectionId,
      isDirty: false
    });

    setRequestTabs((tabs) => [...tabs, blank]);
    setActiveRequestTabKey(blank.key);
    setSelectedCollectionId(baseCollectionId ?? null);
    setError(null);
    setSuccess(null);
  };

  const handleCommandPaletteSelect = async (item: CommandPaletteItem) => {
    setCommandPaletteOpen(false);

    if (item.id === "action:new-request") {
      handleNewRequest();
      return;
    }

    if (item.id === "action:save-request") {
      await handleSaveRequest();
      return;
    }

    if (item.id === "action:send-request") {
      if (activeExecutionId) {
        await handleCancelActiveRequest();
      } else {
        await handleSend();
      }
      return;
    }

    if (item.id === "action:workspace") {
      openWorkspace();
      return;
    }

    if (item.id === "action:copy-curl") {
      await handleCopyRequestExport("curl");
      return;
    }

    if (item.id === "action:copy-fetch") {
      await handleCopyRequestExport("fetch");
      return;
    }

    if (item.id === "action:copy-axios") {
      await handleCopyRequestExport("axios");
      return;
    }

    if (item.id.startsWith("collection:")) {
      const collectionId = item.id.slice("collection:".length);
      setSelectedCollectionId(collectionId);
      return;
    }

    if (item.id.startsWith("request:")) {
      const requestId = item.id.slice("request:".length);
      const record = snapshot?.collections
        .flatMap((collection) => collection.requests)
        .find((request) => request.id === requestId);
      if (record) {
        handleLoadRequest(record);
      }
    }
  };

  const startResize = (kind: ResizeKind, clientX: number) => {
    setResizeState({
      kind,
      startX: clientX,
      startWidth: kind === "sidebar" ? sidebarWidth : responseWidth
    });
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const metaPressed = event.metaKey || event.ctrlKey;

      if (commandPaletteOpen) {
        if (event.key === "Escape") {
          event.preventDefault();
          setCommandPaletteOpen(false);
          return;
        }

        if (event.key === "ArrowDown") {
          event.preventDefault();
          setCommandPaletteSelectedIndex((current) =>
            filteredCommandPaletteItems.length === 0
              ? 0
              : Math.min(current + 1, filteredCommandPaletteItems.length - 1)
          );
          return;
        }

        if (event.key === "ArrowUp") {
          event.preventDefault();
          setCommandPaletteSelectedIndex((current) => Math.max(current - 1, 0));
          return;
        }

        if (event.key === "Enter") {
          const selected = filteredCommandPaletteItems[commandPaletteSelectedIndex] ?? null;
          if (selected) {
            event.preventDefault();
            void handleCommandPaletteSelect(selected);
          }
          return;
        }
      }

      if (event.key === "Escape") {
        if (sidePanel) {
          event.preventDefault();
          setSidePanel(null);
          return;
        }

        if (activeTab?.showSettings) {
          event.preventDefault();
          updateActiveRequestTab((tab) => ({
            ...tab,
            showSettings: false
          }));
        }

        return;
      }

      if (
        event.altKey &&
        !metaPressed &&
        !event.shiftKey &&
        /^[1-9]$/.test(event.key)
      ) {
        const index = Number(event.key) - 1;
        const nextTab = requestTabs[index] ?? null;

        if (nextTab) {
          event.preventDefault();
          setActiveRequestTabKey(nextTab.key);

          if (nextTab.collectionId) {
            setSelectedCollectionId(nextTab.collectionId);
          }
        }

        return;
      }

      if (!metaPressed) {
        return;
      }

      const lowerKey = event.key.toLowerCase();
      const editable = isEditableElement(event.target);

      if (lowerKey === "k") {
        event.preventDefault();
        setCommandPaletteOpen(true);
        return;
      }

      if (lowerKey === "enter") {
        event.preventDefault();
        if (activeExecutionId) {
          void handleCancelActiveRequest();
        } else if (!busyNow) {
          void handleSend();
        }
        return;
      }

      if (lowerKey === "s") {
        event.preventDefault();
        if (!busyNow) {
          void handleSaveRequest();
        }
        return;
      }

      if (lowerKey === "n" && event.shiftKey) {
        event.preventDefault();
        if (!busyNow) {
          handleNewRequest();
        }
        return;
      }

      if (lowerKey === "w" && !event.shiftKey) {
        event.preventDefault();
        if (!busyNow) {
          handleCloseRequestTab(activeRequestTabKey);
        }
        return;
      }

      if (lowerKey === "t" && event.shiftKey) {
        event.preventDefault();
        if (!busyNow) {
          handleReopenClosedRequestTab();
        }
        return;
      }

      if (lowerKey === "p" && event.shiftKey) {
        event.preventDefault();
        if (!busyNow) {
          void handlePreview();
        }
        return;
      }

      if (event.key === ".") {
        event.preventDefault();
        openWorkspace();
        return;
      }

      if (editable) {
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    activeExecutionId,
    activeRequestTabKey,
    activeTab,
    busyNow,
    commandPaletteOpen,
    commandPaletteSelectedIndex,
    filteredCommandPaletteItems,
    requestTabs,
    sidePanel,
    updateActiveRequestTab,
    selectedCollectionId
  ]);

  if (!activeTab) {
    return null;
  }

  return (
    <>
      <div
        className="workspace-shell"
        style={
          {
            "--sidebar-width": `${sidebarWidth}px`,
            "--response-width": `${responseWidth}px`
          } as React.CSSProperties
        }
      >
        <aside className="sidebar">
          <div className="sidebar-hero">
            <div className="sidebar-hero-top">
              <div className="sidebar-hero-badge">Midway</div>
              <div className="sidebar-hero-caption">espacio API</div>
            </div>
            <div className="sidebar-hero-copy">
              <strong>Una biblioteca más calmada para tus peticiones</strong>
              <span className="muted small">
                Explorá colecciones, retomá borradores y corré lotes sin salir del foco.
              </span>
            </div>
            <div className="sidebar-hero-stats" aria-label="Resumen del espacio de trabajo">
              <div className="sidebar-hero-stat">
                <span>Colecciones</span>
                <strong>{collectionCount}</strong>
              </div>
              <div className="sidebar-hero-stat">
                <span>Peticiones</span>
                <strong>{totalRequestCount}</strong>
              </div>
              <div className="sidebar-hero-stat">
                <span>Entornos</span>
                <strong>{environmentCount}</strong>
              </div>
            </div>
          </div>

          <div className="sidebar-header">
            <h1 className="sidebar-title">Biblioteca</h1>
            <button
              aria-label="Nueva colección"
              className="icon-button"
              onClick={() => {
                setShowCreateCollection((current) => !current);
                setNewCollectionName("");
              }}
              type="button"
            >
              +
            </button>
          </div>

          {showCreateCollection ? (
            <section className="inline-popover compact-popover">
              <label className="label">
                <span>Nueva colección</span>
                <input
                  className="input"
                  onChange={(event) => setNewCollectionName(event.target.value)}
                  placeholder="Pagos"
                  value={newCollectionName}
                />
              </label>

              <div className="header-actions">
                <button
                  className="button ghost compact-button"
                  onClick={() => {
                    setShowCreateCollection(false);
                    setNewCollectionName("");
                  }}
                  type="button"
                >
                  Cancelar
                </button>
                <button
                  className="button secondary compact-button"
                  disabled={busyNow || !newCollectionName.trim()}
                  onClick={() => void handleCreateCollection()}
                  type="button"
                >
                  Crear
                </button>
              </div>
            </section>
          ) : null}

          <div className="sidebar-scroll">
            <CollectionsTree
              busy={busyNow}
              collections={snapshot?.collections ?? []}
              onLoadRequest={handleLoadRequest}
              onOpenRunner={openRunner}
              onSelectCollection={handleSelectCollection}
              selectedCollectionId={selectedCollectionId}
              selectedRequestId={activeTab.originRequestId ?? activeTab.draft.id ?? null}
            />
          </div>
        </aside>

        <div
          aria-orientation="vertical"
          className={`panel-resizer sidebar-resizer ${
            resizeState?.kind === "sidebar" ? "active" : ""
          }`}
          onDoubleClick={() => setSidebarWidth(DEFAULT_SIDEBAR_WIDTH)}
          onMouseDown={(event) => startResize("sidebar", event.clientX)}
          role="separator"
        />

        <main className="request-stage">
          <RequestWorkspaceTabsStrip
            activeKey={activeRequestTabKey}
            onClose={handleCloseRequestTab}
            onNewRequest={handleNewRequest}
            onReorder={handleReorderRequestTabs}
            onSelect={focusRequestTab}
            tabs={requestTabs}
          />

          <section className="request-shell">
            <div className="panel-header">
              <div className="panel-context">
                <div className="eyebrow">petición</div>
                <div className="context-line">
                  <span>{activeCollection?.collection.name ?? "Sin colección"}</span>
                  <span className="context-divider">·</span>
                  <span>{activeTab.originRequestId ? "Guardada" : "Borrador"}</span>
                  <span className="context-divider">·</span>
                  <span className="context-emphasis">{activeTab.draft.name}</span>
                </div>
                <div className="session-meta muted small">
                  <span>{activeTab.isDirty ? "Cambios sin guardar" : "Sin cambios locales pendientes"}</span>
                  <span className="context-divider">·</span>
                  <span>
                    {lastAutosavedAt
                      ? `Autoguardado ${formatSessionTimestamp(lastAutosavedAt)}`
                      : "Autoguardado local activo"}
                  </span>
                </div>
              </div>

              <div className="header-actions">
                <button
                  className="button ghost compact-button"
                  disabled={busyNow}
                  onClick={handleNewRequest}
                  type="button"
                >
                  Nueva petición
                </button>
                <button
                  className="button secondary compact-button"
                  disabled={busyNow || !activeCollectionId}
                  onClick={() => void handleSaveRequest()}
                  type="button"
                >
                  Guardar
                </button>
                <button
                  className="button ghost compact-button"
                  onClick={() => openWorkspace()}
                  type="button"
                >
                  Panel
                </button>
              </div>
            </div>

            <div className="request-bar">
              <select
                className="select method-select"
                onChange={(event) => {
                  const method = event.target.value as HttpMethod;
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    draft: {
                      ...tab.draft,
                      method
                    },
                    requestTab:
                      tab.requestTab === "params" || tab.requestTab === "body"
                        ? defaultRequestTabForMethod(method)
                        : tab.requestTab,
                    isDirty: true
                  }));
                }}
                value={activeTab.draft.method}
              >
                {METHODS.map((method) => (
                  <option key={method} value={method}>
                    {method}
                  </option>
                ))}
              </select>

              <input
                className="input url-input"
                onChange={(event) =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    draft: { ...tab.draft, url: event.target.value },
                    isDirty: true
                  }))
                }
                onPaste={handleUrlPaste}
                placeholder="https://api.example.com/users o pegá un cURL"
                title="También podés pegar un comando cURL completo acá"
                value={activeTab.draft.url}
              />

              <select
                className="select compact-select"
                onChange={(event) =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    collectionId: tab.collectionId,
                    draft: {
                      ...tab.draft,
                      environmentId: event.target.value || null
                    },
                    isDirty: true
                  }))
                }
                value={activeTab.draft.environmentId ?? ""}
              >
                <option value="">Entorno</option>
                {(snapshot?.environments ?? []).map((env) => (
                  <option key={env.id} value={env.id}>
                    {env.name}
                  </option>
                ))}
              </select>

              <button
                aria-label="Configuración de la petición"
                className={`icon-button ${activeTab.showSettings ? "active" : ""}`}
                onClick={() =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    showSettings: !tab.showSettings
                  }))
                }
                type="button"
              >
                ⚙
              </button>

              <button
                className={`button send-button ${activeExecutionId ? "danger" : ""}`}
                disabled={busyNow && !activeExecutionId}
                onClick={() => void (activeExecutionId ? handleCancelActiveRequest() : handleSend())}
                type="button"
              >
                {activeExecutionId ? "Cancelar" : busy === "send" ? "Enviando…" : "Enviar"}
              </button>
            </div>

            <RequestStatusRail
              bodyMode={activeTab.draft.body.mode}
              collectionName={activeCollection?.collection.name ?? null}
              environmentName={activeEnvironment?.name ?? activeTab.preview?.environmentName ?? null}
              isDirty={activeTab.isDirty}
              preview={activeTab.preview}
              response={response}
              sending={Boolean(activeExecutionId)}
              testsCount={activeTab.draft.responseTests.length}
              timeoutMs={activeTab.draft.timeoutMs}
            />

            {activeTab.showSettings ? (
              <section className="inline-popover request-settings-panel">
                <div className="settings-grid">
                  <label className="label">
                    <span>Nombre</span>
                    <input
                      className="input"
                      onChange={(event) =>
                        updateActiveRequestTab((tab) => ({
                          ...tab,
                          draft: { ...tab.draft, name: event.target.value },
                          isDirty: true
                        }))
                      }
                      value={activeTab.draft.name}
                    />
                  </label>

                  <label className="label">
                    <span>Tiempo límite (ms)</span>
                    <input
                      className="input"
                      min={1}
                      onChange={(event) =>
                        updateActiveRequestTab((tab) => ({
                          ...tab,
                          draft: {
                            ...tab.draft,
                            timeoutMs: Number(event.target.value || 0)
                          },
                          isDirty: true
                        }))
                      }
                      type="number"
                      value={activeTab.draft.timeoutMs}
                    />
                  </label>
                </div>

                <div className="request-settings-footer">
                  <div className="settings-meta muted small">
                    <span>
                      Guardando en: <strong>{activeCollection?.collection.name ?? "seleccioná una colección"}</strong>
                    </span>
                    <span>
                      {activeEnvironment
                        ? `Entorno activo: ${activeEnvironment.name}`
                        : "Sin entorno activo"}
                    </span>
                    <span>
                      {lastAutosavedAt
                        ? `Autoguardado local: ${formatSessionTimestamp(lastAutosavedAt)}`
                        : "Autoguardado local activo"}
                    </span>
                    <span>Pegá un cURL completo en la URL para importarlo.</span>
                    <span>⌘/Ctrl + Shift + P · vista previa</span>
                  </div>
                </div>

                <RequestPreviewPanel
                  busy={busy === "preview"}
                  onRefresh={() => void handlePreview()}
                  preview={activeTab.preview}
                />
                <RequestExportPanel
                  draft={activeTab.draft}
                  exportFormat={requestExportFormat}
                  onCopy={(content, format) => {
                    void handleCopyRequestExport(format);
                  }}
                  onDownload={(content, format) => {
                    void handleDownloadRequestExport(format);
                  }}
                  onExportFormatChange={setRequestExportFormat}
                  preview={activeTab.preview}
                />
              </section>
            ) : null}

            <div className="tabs request-tabs">
              <TabButton
                active={activeTab.requestTab === "params"}
                label="Parámetros"
                meta={activeTab.draft.query.length}
                onClick={() =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    requestTab: "params"
                  }))
                }
              />
              <TabButton
                active={activeTab.requestTab === "headers"}
                label="Encabezados"
                meta={activeTab.draft.headers.length}
                onClick={() =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    requestTab: "headers"
                  }))
                }
              />
              <TabButton
                active={activeTab.requestTab === "auth"}
                label="Autorización"
                meta={formatAuthTypeLabel(activeTab.draft.auth.type)}
                onClick={() =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    requestTab: "auth"
                  }))
                }
              />
              <TabButton
                active={activeTab.requestTab === "body"}
                label="Cuerpo"
                meta={formatBodyModeLabel(activeTab.draft.body.mode)}
                onClick={() =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    requestTab: "body"
                  }))
                }
              />
              <TabButton
                active={activeTab.requestTab === "tests"}
                label="Pruebas"
                meta={activeTab.draft.responseTests.length}
                onClick={() =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    requestTab: "tests"
                  }))
                }
              />
            </div>

            <div className="tab-panel">
              {activeTab.requestTab === "params" ? (
                <KeyValueEditor
                  onChange={(query) =>
                    updateActiveRequestTab((tab) => ({
                      ...tab,
                      draft: { ...tab.draft, query },
                      isDirty: true
                    }))
                  }
                  rows={activeTab.draft.query}
                  title="Parámetros de consulta"
                />
              ) : null}

              {activeTab.requestTab === "headers" ? (
                <KeyValueEditor
                  onChange={(headers) =>
                    updateActiveRequestTab((tab) => ({
                      ...tab,
                      draft: { ...tab.draft, headers },
                      isDirty: true
                    }))
                  }
                  rows={activeTab.draft.headers}
                  title="Encabezados"
                />
              ) : null}

              {activeTab.requestTab === "auth" ? (
                <AuthEditor
                  auth={activeTab.draft.auth}
                  onChange={(auth) =>
                    updateActiveRequestTab((tab) => ({
                      ...tab,
                      draft: { ...tab.draft, auth },
                      isDirty: true
                    }))
                  }
                />
              ) : null}

              {activeTab.requestTab === "body" ? (
                <section className="card">
                  <div className="header-row">
                    <h3 className="section-title">Cuerpo</h3>
                    <div className="header-actions align-right">
                      {activeTab.draft.body.mode === "json" ? (
                        <button
                          className="button ghost compact-button"
                          onClick={() =>
                            updateActiveRequestTab((tab) => ({
                              ...tab,
                              draft: {
                                ...tab.draft,
                                body: {
                                  ...tab.draft.body,
                                  value: prettifyJsonText(tab.draft.body.value)
                                }
                              },
                              isDirty: true
                            }))
                          }
                          type="button"
                        >
                          Formatear JSON
                        </button>
                      ) : null}
                      <select
                        className="select compact-select"
                        onChange={(event) =>
                          updateActiveRequestTab((tab) => ({
                            ...tab,
                            draft: {
                              ...tab.draft,
                              body: {
                                ...tab.draft.body,
                                mode: event.target.value as BodyMode
                              }
                            },
                            isDirty: true
                          }))
                        }
                        value={activeTab.draft.body.mode}
                      >
                        {BODY_MODES.map((mode) => (
                          <option key={mode} value={mode}>
                            {formatBodyModeLabel(mode)}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>

                  {activeTab.draft.body.mode === "json" || activeTab.draft.body.mode === "text" ? (
                    <div className="stack small-gap">
                      <div className="muted small">
                        Editor CodeMirror con resaltado, búsqueda y validación JSON en vivo.
                      </div>
                      <CodeEditor
                        language={activeTab.draft.body.mode === "json" ? "json" : "text"}
                        minHeight={260}
                        onChange={(value) =>
                          updateActiveRequestTab((tab) => ({
                            ...tab,
                            draft: {
                              ...tab.draft,
                              body: { ...tab.draft.body, value }
                            },
                            isDirty: true
                          }))
                        }
                        placeholderText={
                          activeTab.draft.body.mode === "json"
                            ? `{
  "hello": "world"
}`
                            : "Cuerpo de la petición"
                        }
                        value={activeTab.draft.body.value}
                      />
                    </div>
                  ) : null}

                  {activeTab.draft.body.mode === "formData" ? (
                    <FormDataEditor
                      onChange={(formData) =>
                        updateActiveRequestTab((tab) => ({
                          ...tab,
                          draft: {
                            ...tab.draft,
                            body: { ...tab.draft.body, formData }
                          },
                          isDirty: true
                        }))
                      }
                      rows={activeTab.draft.body.formData ?? []}
                    />
                  ) : null}

                  {activeTab.draft.body.mode === "none" ? (
                    <div className="empty-surface muted small">
                      Sin cuerpo para esta petición.
                    </div>
                  ) : null}
                </section>
              ) : null}

              {activeTab.requestTab === "tests" ? (
                <AssertionEditor
                  assertions={activeTab.draft.responseTests}
                  onChange={(responseTests) =>
                    updateActiveRequestTab((tab) => ({
                      ...tab,
                      draft: { ...tab.draft, responseTests },
                      isDirty: true
                    }))
                  }
                />
              ) : null}
            </div>
          </section>

          {recoveryNotice ? (
            <div className="info-banner banner-with-actions">
              <span>
                Recuperamos tus pestañas y borradores locales después de un cierre inesperado.
                Último autoguardado: <strong>{formatSessionTimestamp(recoveryNotice)}</strong>.
              </span>
              <button
                className="button ghost compact-button"
                onClick={() => setRecoveryNotice(null)}
                type="button"
              >
                Cerrar
              </button>
            </div>
          ) : null}

          {error ? <div className="error-banner">{error}</div> : null}
          {success ? <div className="success-banner">{success}</div> : null}
        </main>

        <div
          aria-orientation="vertical"
          className={`panel-resizer response-resizer ${
            resizeState?.kind === "response" ? "active" : ""
          }`}
          onDoubleClick={() => setResponseWidth(DEFAULT_RESPONSE_WIDTH)}
          onMouseDown={(event) => startResize("response", event.clientX)}
          role="separator"
        />

        <section className="response-shell">
          <div className="panel-header">
            <div className="panel-context">
              <div className="eyebrow">respuesta</div>

              <div className="response-status-row">
                <span className={`status-pill ${statusTone(response?.status)}`}>
                  {response
                    ? `${response.status} ${response.statusText}`
                    : "Sin respuesta"}
                </span>
                {response ? (
                  <>
                    <span className="metric-pill">{response.durationMs} ms</span>
                    <span className="metric-pill">{formatBytes(response.sizeBytes)}</span>
                  </>
                ) : null}
              </div>
            </div>

            <div className="header-actions">
              <button
                className="button ghost compact-button"
                onClick={() => openWorkspace("history")}
                type="button"
              >
                Historial
              </button>
            </div>
          </div>

          <div className="tabs tabs-inline response-tabs">
            <TabButton
              active={activeTab.responseTab === "body"}
              label="Cuerpo"
              onClick={() =>
                updateActiveRequestTab((tab) => ({
                  ...tab,
                  responseTab: "body"
                }))
              }
            />
            <TabButton
              active={activeTab.responseTab === "headers"}
              label="Encabezados"
              meta={response?.headers.length ?? 0}
              onClick={() =>
                updateActiveRequestTab((tab) => ({
                  ...tab,
                  responseTab: "headers"
                }))
              }
            />
            <TabButton
              active={activeTab.responseTab === "tests"}
              label="Pruebas"
              meta={activeTab.executionOutcome?.assertionReport.total ?? 0}
              onClick={() =>
                updateActiveRequestTab((tab) => ({
                  ...tab,
                  responseTab: "tests"
                }))
              }
            />
          </div>

          {response ? (
            <div className="response-info muted small">
              <div className="response-url">{response.finalUrl}</div>
              <div>{response.receivedAt}</div>
            </div>
          ) : null}

          <div className={`tab-panel response-panel-body ${response ? "" : "empty"}`.trim()}>
            {response ? (
              <>
                {activeTab.responseTab === "body" ? (
                  <CodeEditor
                    className="response-code-editor"
                    language={detectEditorLanguage(response.bodyText)}
                    minHeight={420}
                    readOnly
                    value={formatBodyText(response.bodyText)}
                  />
                ) : null}

                {activeTab.responseTab === "headers" ? (
                  <ResponseHeadersTable rows={response.headers} />
                ) : null}

                {activeTab.responseTab === "tests" ? (
                  <section className="card">
                    <AssertionReportView
                      emptyMessage="No hay pruebas evaluadas todavía."
                      report={activeTab.executionOutcome?.assertionReport ?? null}
                    />
                  </section>
                ) : null}
              </>
            ) : (
              <ResponseEmptyState
                busy={busyNow}
                currentTab={activeTab.responseTab}
                environmentName={activeEnvironment?.name ?? activeTab.preview?.environmentName ?? null}
                hasUrl={activeTab.draft.url.trim().length > 0}
                onCancel={() => {
                  void handleCancelActiveRequest();
                }}
                onOpenTests={() =>
                  updateActiveRequestTab((tab) => ({
                    ...tab,
                    requestTab: "tests"
                  }))
                }
                onPreview={() => {
                  void handlePreview();
                }}
                onSend={() => {
                  void handleSend();
                }}
                sending={Boolean(activeExecutionId)}
                testsCount={activeTab.draft.responseTests.length}
              />
            )}
          </div>
        </section>
      </div>

      <div
        className={`drawer-scrim ${sidePanel ? "open" : ""}`}
        onClick={() => setSidePanel(null)}
        role="presentation"
      />

      <aside className={`tools-drawer ${sidePanel ? "open" : ""}`}>
        <div className="drawer-header">
          <div>
            <div className="eyebrow">
              {sidePanel === "runner" ? "colección" : "panel"}
            </div>
            <h2 className="surface-title">
              {sidePanel === "runner" ? "Ejecución de colección" : "Espacio de trabajo"}
            </h2>
          </div>

          <button
            aria-label="Cerrar panel"
            className="icon-button"
            onClick={() => setSidePanel(null)}
            type="button"
          >
            ×
          </button>
        </div>

        {sidePanel === "workspace" ? (
          <>
            <div className="tabs tabs-inline">
              <TabButton
                active={workspaceTab === "environments"}
                label="Entornos"
                meta={snapshot?.environments.length ?? 0}
                onClick={() => setWorkspaceTab("environments")}
              />
              <TabButton
                active={workspaceTab === "data"}
                label="Datos"
                onClick={() => setWorkspaceTab("data")}
              />
              <TabButton
                active={workspaceTab === "history"}
                label="Historial"
                meta={snapshot?.history.length ?? 0}
                onClick={() => setWorkspaceTab("history")}
              />
            </div>

            <div className="drawer-body">
              <UpdateCenterCard className="workspace-update-card" />

              {workspaceTab === "environments" ? (
                <div className="stack">
                  <EnvironmentManager
                    busy={busyNow}
                    editor={environmentEditor}
                    environments={snapshot?.environments ?? []}
                    onDelete={(environmentId) => handleDeleteEnvironment(environmentId)}
                    onEditorChange={setEnvironmentEditor}
                    onLoad={(env) =>
                      setEnvironmentEditor({
                        environmentId: env.id,
                        name: env.name,
                        variables: env.variables
                      })
                    }
                    onSave={handleSaveEnvironment}
                  />

                  <SecretManager
                    busy={busyNow}
                    onDelete={handleDeleteSecret}
                    onSave={handleSaveSecret}
                    secrets={snapshot?.secrets ?? []}
                  />
                </div>
              ) : null}

              {workspaceTab === "data" ? (
                <section className="card stack">
                  <div className="header-row">
                    <div>
                      <h3 className="section-title">Interoperabilidad</h3>
                      <div className="muted small">
                        Importá o exportá Midway, Postman y OpenAPI sin mezclar ambos flujos a la vez.
                      </div>
                    </div>
                  </div>

                  <div className="tabs tabs-inline data-subtabs">
                    <TabButton
                      active={dataTab === "export"}
                      label="Exportar"
                      onClick={() => setDataTab("export")}
                    />
                    <TabButton
                      active={dataTab === "import"}
                      label="Importar"
                      onClick={() => setDataTab("import")}
                    />
                  </div>

                  {dataTab === "export" ? (
                    <div className="stack">
                      <label className="label">
                        <span>Ruta de exportación</span>
                        <input
                          className="input"
                          onChange={(event) => setExportPath(event.target.value)}
                          placeholder="/ruta/absoluta/espacio-trabajo.json"
                          value={exportPath}
                        />
                      </label>

                      <label className="label">
                        <span>Formato de exportación</span>
                        <select
                          className="select"
                          onChange={(event) =>
                            setExportFormat(event.target.value as WorkspaceExportFormat)
                          }
                          value={exportFormat}
                        >
                          <option value="nativeWorkspaceV1">Midway nativo v1</option>
                          <option value="postmanCollectionV21">Postman v2.1</option>
                        </select>
                      </label>

                      {exportFormat === "nativeWorkspaceV1" ? (
                        <>
                          <label className="label">
                            <span>Alcance</span>
                            <select
                              className="select"
                              onChange={(event) =>
                                setExportScope(event.target.value as ExportScope)
                              }
                              value={exportScope}
                            >
                              <option value="workspace">espacio completo</option>
                              <option value="collection">colección seleccionada</option>
                            </select>
                          </label>

                          {exportScope === "workspace" ? (
                            <label className="inline-row muted small">
                              <input
                                checked={includeHistoryInExport}
                                onChange={(event) =>
                                  setIncludeHistoryInExport(event.target.checked)
                                }
                                type="checkbox"
                              />
                              incluir historial
                            </label>
                          ) : (
                            <div className="muted small">
                              El paquete nativo de colección exporta solo la colección elegida y los entornos y secretos relacionados.
                            </div>
                          )}

                          <label className="inline-row muted small">
                            <input
                              checked={includeSecretMetadataInExport}
                              onChange={(event) =>
                                setIncludeSecretMetadataInExport(event.target.checked)
                              }
                              type="checkbox"
                            />
                            incluir alias de secretos
                          </label>
                        </>
                      ) : (
                        <div className="muted small">
                          Exporta la colección seleccionada para Postman v2.1.
                        </div>
                      )}

                      <div className="section-footer">
                        <div className="muted small">
                          {exportFormat === "postmanCollectionV21"
                            ? selectedCollection
                              ? `Colección seleccionada: ${selectedCollection.collection.name}`
                              : "Seleccioná una colección para exportar Postman."
                            : exportScope === "collection"
                              ? selectedCollection
                                ? `Bundle nativo listo para reimportar: ${selectedCollection.collection.name}`
                                : "Seleccioná una colección para exportar una colección de Midway."
                              : "Exportación completa del espacio de trabajo."}
                        </div>
                        <div className="header-actions">
                          <button
                            className="button"
                            disabled={busyNow}
                            onClick={() => void handleExportWorkspace()}
                            type="button"
                          >
                            {busy === "interop" ? "Procesando…" : "Exportar"}
                          </button>
                        </div>
                      </div>

                      {lastExportResult ? (
                        <div className="code-block">
                          {lastExportResult.format}
                          {"\n"}
                          {lastExportResult.path}
                          {"\n"}
                          {lastExportResult.collectionsExported} colecciones · {" "}
                          {lastExportResult.requestsExported} peticiones
                          {"\n"}
                          {lastExportResult.bytesWritten} bytes
                        </div>
                      ) : null}
                    </div>
                  ) : null}

                  {dataTab === "import" ? (
                    <div className="stack">
                      <label className="label">
                        <span>Ruta de importación</span>
                        <input
                          className="input"
                          onChange={(event) => setImportPath(event.target.value)}
                          placeholder="/ruta/absoluta/archivo.json"
                          value={importPath}
                        />
                      </label>

                      <label className="label">
                        <span>Formato de importación</span>
                        <select
                          className="select"
                          onChange={(event) =>
                            setImportFormat(event.target.value as WorkspaceImportFormat)
                          }
                          value={importFormat}
                        >
                          <option value="auto">automático</option>
                          <option value="nativeWorkspaceV1">Midway nativo v1</option>
                          <option value="postmanCollectionV21">Postman v2.1</option>
                          <option value="openApiV3">OpenAPI v3</option>
                        </select>
                      </label>

                      <label className="inline-row muted small">
                        <input
                          checked={importMerge}
                          onChange={(event) => setImportMerge(event.target.checked)}
                          type="checkbox"
                        />
                        combinar con el espacio actual
                      </label>

                      <div className="section-footer">
                        <div className="muted small">
                          Detecta formato automáticamente o elegilo manualmente. Soporta Midway nativo, Postman v2.1 y OpenAPI v3 en JSON o YAML.
                        </div>
                        <div className="header-actions">
                          <button
                            className="button"
                            disabled={busyNow}
                            onClick={() => void handleImportWorkspace()}
                            type="button"
                          >
                            {busy === "interop" ? "Procesando…" : "Importar archivo"}
                          </button>
                        </div>
                      </div>

                      <section className="card stack compact-card">
                        <div className="header-row">
                          <div>
                            <h3 className="section-title">Pegar contenido</h3>
                            <div className="muted small">
                              Pegá una colección Postman o una especificación OpenAPI en JSON o YAML.
                            </div>
                          </div>
                          <button
                            className="button secondary compact-button"
                            disabled={busyNow || !importPayloadText.trim()}
                            onClick={() => void handleImportPayload()}
                            type="button"
                          >
                            {busy === "interop" ? "Procesando…" : "Importar contenido"}
                          </button>
                        </div>

                        <CodeEditor
                          language={detectEditorLanguage(importPayloadText)}
                          minHeight={240}
                          onChange={setImportPayloadText}
                          placeholderText={`openapi: 3.0.0
info:
  title: Demo API`}
                          value={importPayloadText}
                        />
                      </section>

                      {lastImportResult ? (
                        <div className="stack">
                          <div className="code-block">
                            {lastImportResult.detectedFormat}
                            {"\n"}
                            {lastImportResult.path}
                            {"\n"}
                            {lastImportResult.collectionsImported} colecciones · {" "}
                            {lastImportResult.requestsImported} peticiones · {" "}
                            {lastImportResult.environmentsImported} entornos
                          </div>

                          {lastImportResult.warnings.length > 0 ? (
                            <ul className="warning-list">
                              {lastImportResult.warnings.map((warning) => (
                                <li key={warning}>{warning}</li>
                              ))}
                            </ul>
                          ) : null}
                        </div>
                      ) : null}
                    </div>
                  ) : null}
                </section>
              ) : null}

              {workspaceTab === "history" ? (
                <div className="stack">
                  <HistorySidebar history={snapshot?.history ?? []} />

                  <section className="card stack">
                    <div className="header-row">
                      <div>
                        <h3 className="section-title">Diagnósticos</h3>
                        <div className="muted small">
                          Logs locales de cierres inesperados y errores no controlados para soporte y QA.
                        </div>
                      </div>
                      <button
                        className="button secondary compact-button"
                        disabled={crashRecords.length === 0}
                        onClick={() => {
                          clearCrashRecords();
                          setCrashRecords([]);
                        }}
                        type="button"
                      >
                        Limpiar
                      </button>
                    </div>

                    {crashRecords.length === 0 ? (
                      <div className="empty-surface muted small">
                        No hay cierres inesperados registrados en esta sesión/almacenamiento local.
                      </div>
                    ) : (
                      <div className="rows-grid diagnostics-grid">
                        {crashRecords.map((record) => (
                          <div className="history-item" key={record.id}>
                            <div className="header-row">
                              <strong>{record.source}</strong>
                              <span className="badge">{record.createdAt}</span>
                            </div>
                            <div>{record.message}</div>
                            {record.stack ? (
                              <CodeEditor
                                className="compact-code-editor"
                                language="text"
                                minHeight={120}
                                readOnly
                                value={record.stack}
                              />
                            ) : null}
                          </div>
                        ))}
                      </div>
                    )}
                  </section>
                </div>
              ) : null}

              <section className="card shortcuts-card">
                <div className="header-row">
                  <div>
                    <h3 className="section-title">Atajos</h3>
                    <div className="muted small">
                      Microinteracciones para navegar más rápido.
                    </div>
                  </div>
                </div>

                <div className="shortcut-grid">
                  <div className="shortcut-row">
                    <span>Enviar</span>
                    <kbd>⌘/Ctrl + Enter</kbd>
                  </div>
                  <div className="shortcut-row">
                    <span>Guardar</span>
                    <kbd>⌘/Ctrl + S</kbd>
                  </div>
                  <div className="shortcut-row">
                    <span>Nueva petición</span>
                    <kbd>⌘/Ctrl + Shift + N</kbd>
                  </div>
                  <div className="shortcut-row">
                    <span>Cerrar pestaña</span>
                    <kbd>⌘/Ctrl + W</kbd>
                  </div>
                  <div className="shortcut-row">
                    <span>Reabrir pestaña</span>
                    <kbd>⌘/Ctrl + Shift + T</kbd>
                  </div>
                  <div className="shortcut-row">
                    <span>Vista previa</span>
                    <kbd>⌘/Ctrl + Shift + P</kbd>
                  </div>
                  <div className="shortcut-row">
                    <span>Panel</span>
                    <kbd>⌘/Ctrl + .</kbd>
                  </div>
                  <div className="shortcut-row">
                    <span>Ir a pestaña</span>
                    <kbd>Alt + 1..9</kbd>
                  </div>
                  <div className="shortcut-row">
                    <span>Reordenar tabs</span>
                    <kbd>Arrastrar y soltar</kbd>
                  </div>
                </div>
              </section>
            </div>
          </>
        ) : null}

        {sidePanel === "runner" ? (
          <div className="drawer-body">
            <div className="stack">
              <section className="card">
                <div className="header-row">
                  <div>
                    <h3 className="section-title">Ejecución</h3>
                    <div className="muted small">
                      {selectedCollection
                        ? selectedCollection.collection.name
                        : "sin colección seleccionada"}
                    </div>
                  </div>

                  <button
                    className="button"
                    disabled={!selectedCollection || busyNow}
                    onClick={() => void handleRunCollection()}
                    type="button"
                  >
                    {busy === "runCollection" ? "Ejecutando…" : "Ejecutar"}
                  </button>
                </div>

                <div className="grid-2">
                  <label className="label">
                    <span>Forzar entorno</span>
                    <select
                      className="select"
                      onChange={(event) =>
                        setRunnerEnvironmentOverrideId(event.target.value || null)
                      }
                      value={runnerEnvironmentOverrideId ?? ""}
                    >
                      <option value="">usar el de cada petición</option>
                      {(snapshot?.environments ?? []).map((env) => (
                        <option key={env.id} value={env.id}>
                          {env.name}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="inline-toggle">
                    <input
                      checked={stopOnError}
                      onChange={(event) => setStopOnError(event.target.checked)}
                      type="checkbox"
                    />
                    detener al primer error
                  </label>
                </div>
              </section>

              <CollectionRunProgressView progress={runnerProgress} />
              <CollectionRunReportView report={collectionRunReport} />
            </div>
          </div>
        ) : null}
      </aside>

      <CommandPalette
        onClose={() => setCommandPaletteOpen(false)}
        onQueryChange={setCommandPaletteQuery}
        onSelect={(item) => void handleCommandPaletteSelect(item)}
        open={commandPaletteOpen}
        query={commandPaletteQuery}
        results={filteredCommandPaletteItems}
        selectedIndex={commandPaletteSelectedIndex}
      />
    </>
  );
}
