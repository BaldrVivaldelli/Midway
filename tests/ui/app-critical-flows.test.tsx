import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const apiMocks = vi.hoisted(() => ({
  workspaceSnapshot: vi.fn(),
  previewRequest: vi.fn(),
  executeRequest: vi.fn(),
  cancelRequest: vi.fn(),
  createCollection: vi.fn(),
  saveRequest: vi.fn(),
  saveEnvironment: vi.fn(),
  deleteEnvironment: vi.fn(),
  saveSecret: vi.fn(),
  deleteSecret: vi.fn(),
  runCollection: vi.fn(),
  exportWorkspaceData: vi.fn(),
  importWorkspaceData: vi.fn(),
  importWorkspacePayload: vi.fn()
}));

const workspaceFixture = {
  collections: [
    {
      collection: {
        id: 'collection-1',
        name: 'Core APIs',
        requestCount: 1,
        createdAt: '2026-03-23T00:00:00Z',
        updatedAt: '2026-03-23T00:00:00Z'
      },
      requests: []
    }
  ],
  environments: [],
  history: [],
  secrets: []
};

vi.mock('../../src/tauri/api', () => ({
  COLLECTION_RUN_PROGRESS_EVENT: 'collection-run-progress',
  workspaceSnapshot: apiMocks.workspaceSnapshot,
  previewRequest: apiMocks.previewRequest,
  executeRequest: apiMocks.executeRequest,
  cancelRequest: apiMocks.cancelRequest,
  createCollection: apiMocks.createCollection,
  saveRequest: apiMocks.saveRequest,
  saveEnvironment: apiMocks.saveEnvironment,
  deleteEnvironment: apiMocks.deleteEnvironment,
  saveSecret: apiMocks.saveSecret,
  deleteSecret: apiMocks.deleteSecret,
  runCollection: apiMocks.runCollection,
  exportWorkspaceData: apiMocks.exportWorkspaceData,
  importWorkspaceData: apiMocks.importWorkspaceData,
  importWorkspacePayload: apiMocks.importWorkspacePayload
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {})
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    onCloseRequested: vi.fn(async () => () => {})
  })
}));

vi.mock('../../src/components/UpdateCenterCard', () => ({
  default: () => <div data-testid="update-center-stub">updates</div>
}));

describe('App critical flows', () => {
  beforeEach(() => {
    localStorage.clear();
    apiMocks.workspaceSnapshot.mockResolvedValue(workspaceFixture);
    apiMocks.previewRequest.mockResolvedValue({
      method: 'GET',
      resolvedUrl: 'https://jsonplaceholder.typicode.com/posts/1',
      headers: [],
      bodyText: null,
      curlCommand: 'curl https://jsonplaceholder.typicode.com/posts/1',
      environmentName: null,
      usedSecretAliases: [],
      missingSecretAliases: []
    });
    apiMocks.executeRequest.mockResolvedValue({
      response: {
        status: 200,
        statusText: 'OK',
        headers: [],
        bodyText: '{"ok":true}',
        durationMs: 42,
        sizeBytes: 12,
        finalUrl: 'https://jsonplaceholder.typicode.com/posts/1',
        receivedAt: '2026-03-23T00:00:00Z'
      },
      assertionReport: {
        total: 0,
        passed: 0,
        failed: 0,
        results: []
      }
    });
    apiMocks.cancelRequest.mockResolvedValue({ executionId: 'x', canceled: true });
    apiMocks.createCollection.mockResolvedValue(workspaceFixture.collections[0].collection);
    apiMocks.saveRequest.mockResolvedValue({
      id: 'request-1',
      collectionId: 'collection-1',
      name: 'Saved request',
      draft: {
        id: null,
        name: 'Saved request',
        method: 'GET',
        url: 'https://jsonplaceholder.typicode.com/posts/1',
        query: [],
        headers: [],
        auth: { type: 'none' },
        body: { mode: 'none', value: '', formData: [] },
        timeoutMs: 30000,
        environmentId: null,
        responseTests: []
      },
      createdAt: '2026-03-23T00:00:00Z',
      updatedAt: '2026-03-23T00:00:00Z'
    });
    apiMocks.saveEnvironment.mockResolvedValue({
      id: 'env-1',
      name: 'Default',
      variables: [],
      createdAt: '2026-03-23T00:00:00Z',
      updatedAt: '2026-03-23T00:00:00Z'
    });
    apiMocks.deleteEnvironment.mockResolvedValue(undefined);
    apiMocks.saveSecret.mockResolvedValue({
      alias: 'token',
      createdAt: '2026-03-23T00:00:00Z',
      updatedAt: '2026-03-23T00:00:00Z'
    });
    apiMocks.deleteSecret.mockResolvedValue(undefined);
    apiMocks.runCollection.mockResolvedValue({
      collectionId: 'collection-1',
      collectionName: 'Core APIs',
      startedAt: '2026-03-23T00:00:00Z',
      finishedAt: '2026-03-23T00:00:02Z',
      totalRequests: 0,
      completedRequests: 0,
      erroredRequests: 0,
      passedAssertions: 0,
      failedAssertions: 0,
      items: []
    });
    apiMocks.exportWorkspaceData.mockResolvedValue({
      path: '/tmp/midway.json',
      format: 'nativeWorkspaceV1',
      collectionsExported: 1,
      requestsExported: 1,
      environmentsExported: 0,
      historyExported: 0,
      secretMetadataExported: 0,
      bytesWritten: 10
    });
    apiMocks.importWorkspaceData.mockResolvedValue({
      path: '/tmp/midway.json',
      detectedFormat: 'nativeWorkspaceV1',
      collectionsImported: 1,
      requestsImported: 1,
      environmentsImported: 0,
      historyImported: 0,
      secretMetadataImported: 0,
      collectionIds: ['collection-1'],
      warnings: []
    });
    apiMocks.importWorkspacePayload.mockResolvedValue({
      path: 'payload',
      detectedFormat: 'postmanCollectionV21',
      collectionsImported: 1,
      requestsImported: 1,
      environmentsImported: 0,
      historyImported: 0,
      secretMetadataImported: 0,
      collectionIds: ['collection-1'],
      warnings: []
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('importa un cURL pegado y lo autosavea en la sesión local', async () => {
    vi.resetModules();
    const { default: App } = await import('../../src/App');
    render(<App />);

    const urlInput = await screen.findByPlaceholderText(/pegá un cURL/i);
    fireEvent.paste(urlInput, {
      clipboardData: {
        getData: () =>
          `curl -X POST https://api.example.com/posts -H "content-type: application/json" -d '{"hello":"world"}'`
      }
    });

    await waitFor(() => {
      expect(screen.getByDisplayValue('POST')).toBeInTheDocument();
      expect(screen.getByDisplayValue('https://api.example.com/posts')).toBeInTheDocument();
      expect(screen.getByText(/editor codemirror/i)).toBeInTheDocument();
    });

    await new Promise((resolve) => setTimeout(resolve, 500));

    const savedSession = localStorage.getItem('midway.session.workspace.v1');
    expect(savedSession).toBeTruthy();
    expect(savedSession).toContain('https://api.example.com/posts');
    expect(savedSession).toContain('POST');
  });

  it('restaura la sesión abierta al reimportar la app', async () => {
    localStorage.setItem(
      'midway.session.workspace.v1',
      JSON.stringify({
        version: 1,
        savedAt: '2026-03-23T00:00:00Z',
        requestTabs: [
          {
            key: 'restored-tab',
            collectionId: 'collection-1',
            draft: {
              id: null,
              name: 'Restore me',
              method: 'GET',
              url: 'https://restored.example.com/users',
              query: [],
              headers: [],
              auth: { type: 'none' },
              body: { mode: 'none', value: '', formData: [] },
              timeoutMs: 30000,
              environmentId: null,
              responseTests: []
            },
            requestTab: 'params',
            responseTab: 'body',
            showSettings: false,
            isDirty: true,
            originRequestId: null
          }
        ],
        activeRequestTabKey: 'restored-tab',
        selectedCollectionId: 'collection-1',
        workspaceTab: 'environments',
        dataTab: 'export',
        runnerEnvironmentOverrideId: null,
        stopOnError: false,
        closedRequestTabs: []
      })
    );

    vi.resetModules();
    const { default: App } = await import('../../src/App');
    render(<App />);

    expect(await screen.findByDisplayValue('https://restored.example.com/users')).toBeInTheDocument();
    expect(screen.getAllByText(/cambios sin guardar/i).length).toBeGreaterThan(0);
  });
});
