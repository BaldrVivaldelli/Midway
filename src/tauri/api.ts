import { invoke } from "@tauri-apps/api/core";
import type {
  CancelRequestResult,
  CollectionRunReport,
  CollectionSummary,
  EnvironmentRecord,
  ExportWorkspaceInput,
  ExportWorkspaceResult,
  ImportWorkspaceInput,
  ImportWorkspacePayloadInput,
  ImportWorkspaceResult,
  RequestDraft,
  RequestExecutionOutcome,
  RequestPreview,
  RunCollectionInput,
  SaveEnvironmentInput,
  SaveRequestInput,
  SaveSecretInput,
  SavedRequestRecord,
  SecretMetadata,
  WorkspaceSnapshot
} from "./types";

export async function previewRequest(
  draft: RequestDraft
): Promise<RequestPreview> {
  return invoke<RequestPreview>("preview_request", { draft });
}

export async function executeRequest(
  draft: RequestDraft,
  executionId?: string | null
): Promise<RequestExecutionOutcome> {
  return invoke<RequestExecutionOutcome>("execute_request", { draft, executionId });
}

export async function cancelRequest(
  executionId: string
): Promise<CancelRequestResult> {
  return invoke<CancelRequestResult>("cancel_request", { executionId });
}

export async function runCollection(
  input: RunCollectionInput
): Promise<CollectionRunReport> {
  return invoke<CollectionRunReport>("run_collection", { input });
}

export async function workspaceSnapshot(): Promise<WorkspaceSnapshot> {
  return invoke<WorkspaceSnapshot>("workspace_snapshot");
}

export async function createCollection(
  name: string
): Promise<CollectionSummary> {
  return invoke<CollectionSummary>("create_collection", { name });
}

export async function saveRequest(
  input: SaveRequestInput
): Promise<SavedRequestRecord> {
  return invoke<SavedRequestRecord>("save_request", { input });
}

export async function saveEnvironment(
  input: SaveEnvironmentInput
): Promise<EnvironmentRecord> {
  return invoke<EnvironmentRecord>("save_environment", { input });
}

export async function deleteEnvironment(environmentId: string): Promise<void> {
  return invoke<void>("delete_environment", { environmentId });
}

export async function saveSecret(
  input: SaveSecretInput
): Promise<SecretMetadata> {
  return invoke<SecretMetadata>("save_secret", { input });
}

export async function deleteSecret(alias: string): Promise<void> {
  return invoke<void>("delete_secret", { alias });
}

export const COLLECTION_RUN_PROGRESS_EVENT = "collection-run-progress";

export async function exportWorkspaceData(
  input: ExportWorkspaceInput
): Promise<ExportWorkspaceResult> {
  return invoke<ExportWorkspaceResult>("export_workspace_data", { input });
}

export async function importWorkspaceData(
  input: ImportWorkspaceInput
): Promise<ImportWorkspaceResult> {
  return invoke<ImportWorkspaceResult>("import_workspace_data", { input });
}

export async function importWorkspacePayload(
  input: ImportWorkspacePayloadInput
): Promise<ImportWorkspaceResult> {
  return invoke<ImportWorkspaceResult>("import_workspace_payload", { input });
}
