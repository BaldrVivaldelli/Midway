import type { FormDataRow, RequestDraft, RequestPreview, ResolvedPair } from "../tauri/types";

export type RequestCodeExportFormat = "curl" | "fetch" | "axios";

export const REQUEST_CODE_EXPORT_FORMATS: RequestCodeExportFormat[] = [
  "curl",
  "fetch",
  "axios"
];

export function requestCodeExportLabel(format: RequestCodeExportFormat): string {
  switch (format) {
    case "curl":
      return "cURL";
    case "fetch":
      return "fetch";
    case "axios":
      return "axios";
  }
}

export function requestCodeExportFilename(
  format: RequestCodeExportFormat,
  requestName: string
): string {
  const base = slugify(requestName || "request");

  switch (format) {
    case "curl":
      return `${base}.sh`;
    case "fetch":
      return `${base}.fetch.ts`;
    case "axios":
      return `${base}.axios.ts`;
  }
}

export function requestCodeExportLanguage(
  format: RequestCodeExportFormat
): "shell" | "text" {
  return format === "curl" ? "shell" : "text";
}

export function generateRequestCodeSnippet({
  format,
  draft,
  preview
}: {
  format: RequestCodeExportFormat;
  draft: RequestDraft;
  preview: RequestPreview;
}): string {
  switch (format) {
    case "curl":
      return preview.curlCommand;
    case "fetch":
      return generateFetchSnippet(draft, preview);
    case "axios":
      return generateAxiosSnippet(draft, preview);
  }
}

function generateFetchSnippet(draft: RequestDraft, preview: RequestPreview): string {
  const lines = [
    `const url = ${JSON.stringify(preview.resolvedUrl)};`,
    ...buildSharedPrelude(draft, preview, "fetch"),
    "",
    buildFetchRequestBlock(draft, preview),
    "",
    ...buildResponseReadBlock("fetch")
  ];

  return lines.filter(Boolean).join("\n");
}

function generateAxiosSnippet(draft: RequestDraft, preview: RequestPreview): string {
  const lines = [
    'import axios from "axios";',
    "",
    `const url = ${JSON.stringify(preview.resolvedUrl)};`,
    ...buildSharedPrelude(draft, preview, "axios"),
    "",
    buildAxiosRequestBlock(draft, preview),
    "",
    "console.log(response.status);",
    "console.log(response.data);"
  ];

  return lines.filter(Boolean).join("\n");
}

function buildSharedPrelude(
  draft: RequestDraft,
  preview: RequestPreview,
  target: "fetch" | "axios"
): string[] {
  const lines: string[] = [];

  if (draft.body.mode === "json") {
    const body = safeParseJson(preview.bodyText ?? draft.body.value);
    if (body !== null) {
      lines.push(`const payload = ${JSON.stringify(body, null, 2)};`);
    } else {
      lines.push(`const payload = ${JSON.stringify(preview.bodyText ?? draft.body.value)};`);
    }
  }

  if (draft.body.mode === "text") {
    lines.push(`const bodyText = ${JSON.stringify(preview.bodyText ?? draft.body.value)};`);
  }

  if (draft.body.mode === "formData") {
    lines.push("const formData = new FormData();");

    for (const statement of buildFormDataStatements(draft, preview, target)) {
      lines.push(statement);
    }
  }

  const headersObject = buildHeadersObjectLiteral(
    preview.headers,
    draft.body.mode === "formData"
  );
  if (headersObject) {
    lines.push(`const headers = ${headersObject};`);
  }

  return lines;
}

function buildFetchRequestBlock(draft: RequestDraft, preview: RequestPreview): string {
  const optionsLines = [`  method: ${JSON.stringify(draft.method)}`];
  const hasHeaders = buildHeadersObjectLiteral(
    preview.headers,
    draft.body.mode === "formData"
  );

  if (hasHeaders) {
    optionsLines.push("  headers");
  }

  const bodyExpression = requestBodyExpression(draft);
  if (bodyExpression) {
    optionsLines.push(`  body: ${bodyExpression}`);
  }

  return [
    "const response = await fetch(url, {",
    optionsLines.join(",\n"),
    "});"
  ].join("\n");
}

function buildAxiosRequestBlock(draft: RequestDraft, preview: RequestPreview): string {
  const optionsLines = [
    `  method: ${JSON.stringify(draft.method.toLowerCase())}`,
    "  url"
  ];
  const hasHeaders = buildHeadersObjectLiteral(
    preview.headers,
    draft.body.mode === "formData"
  );

  if (hasHeaders) {
    optionsLines.push("  headers");
  }

  const bodyExpression = requestBodyExpression(draft, "axios");
  if (bodyExpression) {
    optionsLines.push(`  data: ${bodyExpression}`);
  }

  return [
    "const response = await axios({",
    optionsLines.join(",\n"),
    "});"
  ].join("\n");
}

function buildResponseReadBlock(target: "fetch" | "axios"): string[] {
  if (target === "axios") {
    return [];
  }

  return [
    "const responseText = await response.text();",
    "",
    "try {",
    "  console.log(response.status);",
    "  console.log(JSON.parse(responseText));",
    "} catch {",
    "  console.log(response.status);",
    "  console.log(responseText);",
    "}"
  ];
}

function requestBodyExpression(
  draft: RequestDraft,
  target: "fetch" | "axios" = "fetch"
): string | null {
  switch (draft.body.mode) {
    case "none":
      return null;
    case "json":
      return target === "fetch" ? "JSON.stringify(payload)" : "payload";
    case "text":
      return "bodyText";
    case "formData":
      return "formData";
  }
}

function buildHeadersObjectLiteral(
  headers: ResolvedPair[],
  excludeMultipartContentType: boolean
): string | null {
  const filtered = headers.filter((header) => {
    if (!header.key.trim()) {
      return false;
    }

    if (
      excludeMultipartContentType &&
      header.key.trim().toLowerCase() === "content-type"
    ) {
      return false;
    }

    return true;
  });

  if (filtered.length === 0) {
    return null;
  }

  const body = filtered
    .map(
      (header) =>
        `  ${JSON.stringify(header.key)}: ${JSON.stringify(header.value)}`
    )
    .join(",\n");

  return `{
${body}
}`;
}

type ResolvedFormDataHint = {
  key: string;
  value: string;
  kind: "text" | "file";
};

function buildFormDataStatements(
  draft: RequestDraft,
  preview: RequestPreview,
  target: "fetch" | "axios"
): string[] {
  const rows = draft.body.formData ?? [];
  const previewRows = parsePreviewFormData(preview.bodyText ?? "");

  return rows
    .filter((row) => row.enabled && row.key.trim())
    .flatMap((row, index) => formDataStatementsForRow(row, previewRows[index], target));
}

function formDataStatementsForRow(
  row: FormDataRow,
  hint: ResolvedFormDataHint | undefined,
  _target: "fetch" | "axios"
): string[] {
  const key = hint?.key?.trim() || row.key.trim();

  if (!key) {
    return [];
  }

  if (row.kind === "file") {
    const fileName = row.fileName?.trim();
    const originalPath = hint?.kind === "file" ? hint.value : row.value;
    const appendArgs = [JSON.stringify(key), "fileInput.files[0]"];

    if (fileName) {
      appendArgs.push(JSON.stringify(fileName));
    }

    return [
      `// Reemplazá fileInput.files[0] por un File/Blob real. Midway ejecutó este campo desde ${JSON.stringify(originalPath)}.`,
      `formData.append(${appendArgs.join(", ")});`
    ];
  }

  const value = hint?.value ?? row.value;
  return [`formData.append(${JSON.stringify(key)}, ${JSON.stringify(value)});`];
}

function parsePreviewFormData(bodyText: string): ResolvedFormDataHint[] {
  return bodyText
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const separatorIndex = line.indexOf("=");
      if (separatorIndex === -1) {
        return null;
      }

      const key = line.slice(0, separatorIndex).trim();
      const rawValue = line.slice(separatorIndex + 1);

      if (rawValue.startsWith("@")) {
        return {
          key,
          value: rawValue.slice(1),
          kind: "file" as const
        };
      }

      return {
        key,
        value: rawValue,
        kind: "text" as const
      };
    })
    .filter((entry): entry is ResolvedFormDataHint => entry !== null);
}

function safeParseJson(value: string | null | undefined): unknown | null {
  const trimmed = value?.trim();

  if (!trimmed) {
    return null;
  }

  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

function slugify(value: string): string {
  const normalized = value
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized || "request";
}
