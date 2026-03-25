export type HttpMethod =
  | "GET"
  | "POST"
  | "PUT"
  | "PATCH"
  | "DELETE"
  | "HEAD"
  | "OPTIONS";

export type BodyMode = "none" | "json" | "text" | "formData";
export type ApiKeyPlacement = "header" | "query";
export type FormDataFieldKind = "text" | "file";
export type AssertionSource =
  | "status"
  | "header"
  | "bodyText"
  | "jsonPointer"
  | "finalUrl";
export type AssertionOperator =
  | "equals"
  | "contains"
  | "notContains"
  | "exists"
  | "notExists"
  | "greaterOrEqual"
  | "lessOrEqual";

export type KeyValueRow = {
  id: string;
  key: string;
  value: string;
  enabled: boolean;
};

export type FormDataRow = {
  id: string;
  key: string;
  value: string;
  enabled: boolean;
  kind: FormDataFieldKind;
  fileName?: string | null;
};

export type AuthConfig =
  | { type: "none" }
  | { type: "bearer"; token: string }
  | { type: "basic"; username: string; password: string }
  | { type: "apiKey"; key: string; value: string; placement: ApiKeyPlacement };

export type RequestBodyDraft = {
  mode: BodyMode;
  value: string;
  formData?: FormDataRow[];
};

export type ResponseAssertion = {
  id: string;
  name: string;
  enabled: boolean;
  source: AssertionSource;
  operator: AssertionOperator;
  selector?: string | null;
  expected: string;
};

export type RequestDraft = {
  id?: string | null;
  name: string;
  method: HttpMethod;
  url: string;
  query: KeyValueRow[];
  headers: KeyValueRow[];
  auth: AuthConfig;
  body: RequestBodyDraft;
  timeoutMs: number;
  environmentId?: string | null;
  responseTests: ResponseAssertion[];
};

export type ResolvedPair = {
  key: string;
  value: string;
};

export type RequestPreview = {
  method: HttpMethod;
  resolvedUrl: string;
  headers: ResolvedPair[];
  bodyText?: string | null;
  curlCommand: string;
  environmentName?: string | null;
  usedSecretAliases: string[];
  missingSecretAliases: string[];
};

export type ResponseEnvelope = {
  status: number;
  statusText: string;
  headers: ResolvedPair[];
  bodyText: string;
  durationMs: number;
  sizeBytes: number;
  finalUrl: string;
  receivedAt: string;
};

export type AssertionResult = {
  id: string;
  name: string;
  passed: boolean;
  source: AssertionSource;
  operator: AssertionOperator;
  selector?: string | null;
  expected: string;
  actual?: string | null;
  message: string;
};

export type AssertionReport = {
  total: number;
  passed: number;
  failed: number;
  results: AssertionResult[];
};

export type RequestExecutionOutcome = {
  response: ResponseEnvelope;
  assertionReport: AssertionReport;
};

export type CancelRequestResult = {
  executionId: string;
  canceled: boolean;
};

export type CollectionSummary = {
  id: string;
  name: string;
  requestCount: number;
  createdAt: string;
  updatedAt: string;
};

export type SaveRequestInput = {
  requestId?: string | null;
  collectionId: string;
  draft: RequestDraft;
};

export type SavedRequestRecord = {
  id: string;
  collectionId: string;
  name: string;
  draft: RequestDraft;
  createdAt: string;
  updatedAt: string;
};

export type CollectionWithRequests = {
  collection: CollectionSummary;
  requests: SavedRequestRecord[];
};

export type EnvironmentRecord = {
  id: string;
  name: string;
  variables: KeyValueRow[];
  createdAt: string;
  updatedAt: string;
};

export type SaveEnvironmentInput = {
  environmentId?: string | null;
  name: string;
  variables: KeyValueRow[];
};

export type SecretMetadata = {
  alias: string;
  createdAt: string;
  updatedAt: string;
};

export type SaveSecretInput = {
  alias: string;
  value: string;
};

export type HistoryEntry = {
  id: string;
  requestName: string;
  method: HttpMethod;
  url: string;
  environmentName?: string | null;
  responseStatus?: number | null;
  durationMs?: number | null;
  errorMessage?: string | null;
  createdAt: string;
};

export type RunCollectionInput = {
  collectionId: string;
  environmentOverrideId?: string | null;
  stopOnError: boolean;
};

export type CollectionRunItem = {
  requestId: string;
  requestName: string;
  environmentName?: string | null;
  resolvedUrl?: string | null;
  responseStatus?: number | null;
  durationMs?: number | null;
  errorMessage?: string | null;
  assertionReport?: AssertionReport | null;
  executedAt: string;
};

export type CollectionRunReport = {
  collectionId: string;
  collectionName: string;
  startedAt: string;
  finishedAt: string;
  totalRequests: number;
  completedRequests: number;
  erroredRequests: number;
  passedAssertions: number;
  failedAssertions: number;
  items: CollectionRunItem[];
};

export type WorkspaceSnapshot = {
  collections: CollectionWithRequests[];
  environments: EnvironmentRecord[];
  history: HistoryEntry[];
  secrets: SecretMetadata[];
};

export type CollectionRunPhase =
  | "started"
  | "requestStarted"
  | "requestFinished"
  | "finished";

export type CollectionRunProgressEvent = {
  runId: string;
  phase: CollectionRunPhase;
  collectionId: string;
  collectionName: string;
  totalRequests: number;
  processedRequests: number;
  currentIndex: number;
  completedRequests: number;
  erroredRequests: number;
  passedAssertions: number;
  failedAssertions: number;
  requestId?: string | null;
  requestName?: string | null;
  environmentName?: string | null;
  resolvedUrl?: string | null;
  responseStatus?: number | null;
  durationMs?: number | null;
  errorMessage?: string | null;
  startedAt: string;
  finishedAt?: string | null;
  emittedAt: string;
};

export type WorkspaceExportFormat =
  | "nativeWorkspaceV1"
  | "postmanCollectionV21";

export type WorkspaceImportFormat =
  | "auto"
  | "nativeWorkspaceV1"
  | "postmanCollectionV21"
  | "openApiV3";

export type ExportWorkspaceInput = {
  path: string;
  format: WorkspaceExportFormat;
  collectionId?: string | null;
  includeHistory: boolean;
  includeSecretMetadata: boolean;
};

export type ExportWorkspaceResult = {
  path: string;
  format: WorkspaceExportFormat;
  collectionsExported: number;
  requestsExported: number;
  environmentsExported: number;
  historyExported: number;
  secretMetadataExported: number;
  bytesWritten: number;
};

export type ImportWorkspaceInput = {
  path: string;
  format: WorkspaceImportFormat;
  merge: boolean;
  collectionNameOverride?: string | null;
};

export type ImportWorkspacePayloadInput = {
  payload: string;
  format: WorkspaceImportFormat;
  merge: boolean;
  collectionNameOverride?: string | null;
  sourceLabel?: string | null;
};

export type ImportWorkspaceResult = {
  path: string;
  detectedFormat: WorkspaceImportFormat;
  collectionsImported: number;
  requestsImported: number;
  environmentsImported: number;
  historyImported: number;
  secretMetadataImported: number;
  collectionIds: string[];
  warnings: string[];
};
