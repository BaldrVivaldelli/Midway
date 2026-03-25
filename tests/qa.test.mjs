import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve('.');

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
}

test('frontend integra command palette, CodeMirror y updater in-app', () => {
  const app = read('src/App.tsx');
  const editor = read('src/components/CodeEditor.tsx');
  const updater = read('src/lib/updater.ts');
  const updateCard = read('src/components/UpdateCenterCard.tsx');
  const main = read('src/main.tsx');

  assert.match(app, /CommandPalette/);
  assert.match(app, /importWorkspacePayload/);
  assert.match(app, /FormDataEditor/);
  assert.match(app, /handleCancelActiveRequest/);
  assert.match(app, /UpdateCenterCard/);
  assert.match(editor, /@uiw\/react-codemirror/);
  assert.match(editor, /jsonParseLinter/);
  assert.match(updater, /checkForAppUpdate/);
  assert.match(updateCard, /Download & install/);
  assert.match(main, /AppErrorBoundary/);
});

test('backend integra import OpenAPI, updater y cancelación', () => {
  const commands = read('src-tauri/src/commands/mod.rs');
  const interop = read('src-tauri/src/domain/interop.rs');
  const tauriLib = read('src-tauri/src/lib.rs');
  const capabilities = read('src-tauri/capabilities/default.json');

  assert.match(commands, /cancel_request/);
  assert.match(commands, /import_workspace_payload/);
  assert.match(interop, /OpenApiV3/);
  assert.match(interop, /import_openapi_document/);
  assert.match(tauriLib, /tauri_plugin_updater/);
  assert.match(tauriLib, /tauri_plugin_process/);
  assert.match(capabilities, /updater:default/);
  assert.match(capabilities, /process:default/);
});

test('motor HTTP mantiene cancelación, cookies, multipart y errores enriquecidos', () => {
  const executor = read('src-tauri/src/runtime/request_executor.rs');
  const state = read('src-tauri/src/state.rs');
  const http = read('src-tauri/src/infra/http_reqwest.rs');

  assert.match(executor, /Request cancelado por el usuario/);
  assert.match(state, /cookie_store\(true\)/);
  assert.match(state, /app\.config\(\)\.identifier/);
  assert.match(http, /multipart::\{Form,\s*Part\}/);
  assert.match(http, /build_multipart_form/);
  assert.match(http, /normalize_reqwest_error/);
  assert.match(http, /TLS \/ SSL|DNS|timeout/i);
});


test('interop cubre snippets de request y export nativo de colección', () => {
  const app = read('src/App.tsx');
  const requestExport = read('src/lib/requestExport.ts');
  const commands = read('src-tauri/src/commands/mod.rs');
  const interop = read('src-tauri/src/domain/interop.rs');

  assert.match(app, /RequestExportPanel/);
  assert.match(requestExport, /generateRequestCodeSnippet/);
  assert.match(requestExport, /fetch/);
  assert.match(requestExport, /axios/);
  assert.match(commands, /snapshot_for_native_collection_export/);
  assert.match(interop, /serde_yaml::from_str/);
});
