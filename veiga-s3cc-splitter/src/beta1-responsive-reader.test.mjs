import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const app = readFileSync(new URL("./app.js", import.meta.url), "utf8");
const backend = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");
const lib = readFileSync(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");

test("Beta 1 package loading uses the asynchronous Tauri API", () => {
  assert.match(app, /invoke\("scan_package_resources_async"/);
  assert.match(lib, /commands::scan_package_resources_async/);
  assert.match(backend, /pub async fn scan_package_resources_async/);
  assert.match(backend, /tauri::async_runtime::spawn_blocking/);
});

test("Beta 1 preview does not block the UI thread or initial list rendering", () => {
  assert.match(app, /invoke\("get_resource_preview_async"/);
  assert.match(lib, /commands::get_resource_preview_async/);
  assert.match(backend, /pub async fn get_resource_preview_async/);
  assert.match(app, /void loadPreview\(firstVisible\)/);
});

test("Real stage progress, explicit deferred diagnostics, and scan cancel are wired", () => {
  assert.match(app, /listen\("splitter-scan-progress"/);
  assert.match(backend, /app\.emit\("splitter-scan-progress", info\)/);
  assert.match(app, /result\.relationsDeferred/);
  assert.match(app, /Verificação de duplicatas\/conflitos adiada/);
  assert.match(app, /invoke\("cancel_scan_package"\)/);
  assert.match(html, /id="cancel-scan-btn"/);
  assert.match(lib, /commands::cancel_scan_package/);
});
