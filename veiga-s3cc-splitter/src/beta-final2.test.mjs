import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (relativePath) => readFileSync(new URL(relativePath, import.meta.url), "utf8");

test("Beta 2 uses v2.1.0 consistently and carries the Beta Final 2.1 marker", () => {
  const npm = JSON.parse(read("../package.json"));
  const lock = JSON.parse(read("../package-lock.json"));
  const tauri = JSON.parse(read("../src-tauri/tauri.conf.json"));
  const cargo = read("../src-tauri/Cargo.toml");
  const cargoLock = read("../../Cargo.lock");
  const html = read("../index.html");
  assert.equal(npm.version, "2.1.0");
  assert.equal(lock.version, "2.1.0");
  assert.equal(lock.packages[""].version, "2.1.0");
  assert.equal(tauri.version, "2.1.0");
  assert.match(cargo, /^version = "2\.1\.0"$/m);
  assert.match(cargoLock, /name = "veiga-s3cc-splitter"\r?\nversion = "2\.1\.0"/);
  assert.match(html, /v2\.1\.0 \| Beta 2 — Versão Beta Final 2\.1/);
  assert.match(tauri.app.windows[0].title, /v2\.1\.0 \| Beta 2 — Versão Beta Final 2\.1/);
  assert.doesNotMatch(html, /Beta 3/);
});

test("Beta Final 2.1 preserves Splitter, Merger, thumbnails and animated progress", () => {
  const html = read("../index.html");
  const app = read("./app.js");
  const css = read("./winui-splitter.css");
  assert.match(html, /id="splitter-workspace"/);
  assert.match(html, /id="merger-workspace"/);
  assert.match(html, /id="merger-run-btn"/);
  assert.match(app, /function renderMergerSources\(/);
  assert.match(app, /function removeSelectionFromMerged\(/);
  assert.match(app, /thumbBase64/);
  assert.match(app, /noPreviewImage/);
  assert.match(css, /#merger-workspace:not\(\.hidden\):has\(#merger-source-list:empty\)/);
  assert.match(css, /splitter-busy-marquee/);
});
