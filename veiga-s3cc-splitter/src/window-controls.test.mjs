import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const capabilities = JSON.parse(read("../src-tauri/capabilities/default.json"));
const config = JSON.parse(read("../src-tauri/tauri.conf.json"));
const html = read("../index.html");

test("All three window actions are explicitly authorized for the main Tauri window", () => {
  assert.ok(capabilities.windows.includes("main"));
  for (const permission of [
    "core:window:allow-minimize",
    "core:window:allow-toggle-maximize",
    "core:window:allow-close"
  ]) assert.ok(capabilities.permissions.includes(permission), permission);
  assert.equal(new Set(capabilities.permissions).size, capabilities.permissions.length);
  const mainWindow = config.app.windows[0];
  assert.notEqual(mainWindow.minimizable, false);
  assert.notEqual(mainWindow.maximizable, false);
  assert.notEqual(mainWindow.closable, false);
});

test("Custom WinUI 3 titlebar buttons are wired to Tauri window commands", () => {
  assert.equal(config.app.windows[0].decorations, false);
  const script = read("./winui-theme.js");
  for (const id of ["minimize", "maximize", "close"]) {
    assert.match(html, new RegExp('id="titlebar-' + id + '"'));
  }
  assert.match(script, /getCurrentWindow\(\)/);
  assert.match(script, /appWindow\.minimize\(\)/);
  assert.match(script, /appWindow\.toggleMaximize\(\)/);
  assert.match(script, /appWindow\.close\(\)/);
  assert.match(script, /appWindow\.isMaximized\(\)/);
  assert.doesNotMatch(html, /<button[^>]*id="titlebar-(?:minimize|maximize|close)"[^>]*data-tauri-drag-region/);
});
