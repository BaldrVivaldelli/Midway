import type {
  AuthConfig,
  BodyMode,
  FormDataRow,
  HttpMethod,
  KeyValueRow,
  RequestDraft
} from "../tauri/types";

const METHODS: HttpMethod[] = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS"
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

function createFormDataRow(
  key = "",
  value = "",
  kind: FormDataRow["kind"] = "text",
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

export function createBlankDraft(): RequestDraft {
  return {
    id: null,
    name: "Nuevo request",
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

function decodeQueryValue(value: string): string {
  try {
    return decodeURIComponent(value.replace(/\+/g, " "));
  } catch {
    return value;
  }
}

function parseQueryStringToRows(queryString: string): KeyValueRow[] {
  return queryString
    .split("&")
    .map((part) => part.trim())
    .filter(Boolean)
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
    return "Nuevo request";
  }

  const withoutQuery = trimmed.split("?")[0] ?? trimmed;
  const segments = withoutQuery.split("/").filter(Boolean);
  const lastSegment = segments[segments.length - 1] ?? "request";
  return `${method} ${lastSegment}`;
}

function looksLikeJsonText(value: string): boolean {
  const trimmed = value.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

function prettifyJsonText(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
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

export function looksLikeCurlCommand(value: string): boolean {
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

function parseCurlFormToken(token: string, asString = false): FormDataRow | null {
  const separatorIndex = token.indexOf("=");
  if (separatorIndex === -1) {
    return null;
  }

  const key = token.slice(0, separatorIndex).trim();
  const rawValue = token.slice(separatorIndex + 1);
  if (!key) {
    return null;
  }

  if (!asString && rawValue.startsWith("@")) {
    return createFormDataRow(key, rawValue.slice(1), "file", true, null);
  }

  return createFormDataRow(key, rawValue, "text", true, null);
}

export function parseCurlCommandToDraft(command: string): {
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
  const formTokens: FormDataRow[] = [];
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

    const inlineData = [
      "--data=",
      "--data-raw=",
      "--data-binary=",
      "--data-ascii=",
      "--data-urlencode="
    ].find((prefix) => token.startsWith(prefix));

    if (inlineData) {
      dataTokens.push(token.slice(inlineData.length));
      if (!method && !forceQueryString) {
        method = "POST";
      }
      continue;
    }

    const inlineForm = ["--form=", "--form-string="]
      .find((prefix) => token.startsWith(prefix));
    if (inlineForm) {
      const formToken = parseCurlFormToken(
        token.slice(inlineForm.length),
        inlineForm === "--form-string="
      );
      if (formToken) {
        formTokens.push(formToken);
      }
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
      token === "--data-urlencode"
    ) {
      dataTokens.push(takeValue(token, index));
      if (!method && !forceQueryString) {
        method = "POST";
      }
      index += 1;
      continue;
    }

    if (token === "-F" || token === "--form" || token === "--form-string") {
      const rawValue = takeValue(token, index);
      const formToken = parseCurlFormToken(rawValue, token === "--form-string");
      if (formToken) {
        formTokens.push(formToken);
      }
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
      warnings.push(`Header ignorado: ${headerToken}`);
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
  let formData: FormDataRow[] = [];

  if (!forceQueryString && formTokens.length > 0) {
    bodyMode = "formData";
    formData = formTokens;
  } else if (!forceQueryString && dataTokens.length > 0) {
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
    value: bodyValue,
    formData
  };
  draft.responseTests = [];

  return {
    draft,
    warnings
  };
}
