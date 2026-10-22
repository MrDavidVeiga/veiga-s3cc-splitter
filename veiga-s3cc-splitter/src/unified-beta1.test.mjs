import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (relativePath) => readFileSync(new URL(relativePath, import.meta.url), "utf8");

test("unified Beta 1 is consistently versioned 2.0.0", () => {
  const npm = JSON.parse(read("../package.json"));
  const npmLock = JSON.parse(read("../package-lock.json"));
  const tauri = JSON.parse(read("../src-tauri/tauri.conf.json"));
  const cargo = read("../src-tauri/Cargo.toml");
  const cargoLock = read("../../Cargo.lock");
  const html = read("../index.html");

  assert.equal(npm.version, "2.0.0");
  assert.equal(npmLock.version, "2.0.0");
  assert.equal(npmLock.packages[""].version, "2.0.0");
  assert.equal(tauri.version, "2.0.0");
  assert.match(cargo, /^version = "2\.0\.0"$/m);
  assert.match(cargoLock, /name = "veiga-s3cc-splitter"\r?\nversion = "2\.0\.0"/);
  assert.match(html, /Veiga's S3CC Splitter v2\.0\.0 \| Beta 1/);
  assert.doesNotMatch(html, /Beta 2/);
  assert.match(html, /Versão Beta Final 2\.0/);
  assert.match(tauri.app.windows[0].title, /Versão Beta Final 2\.0/);
});

test("unified Beta 1 retains Splitter and Merger entry points", () => {
  const html = read("../index.html");
  const app = read("./app.js");
  assert.match(html, /id="splitter-workspace"/);
  assert.match(html, /id="merger-workspace"/);
  assert.match(html, /id="merger-run-btn"/);
  assert.match(app, /function renderMergerSources\(/);
  assert.match(app, /function renderPreview\(/);
  assert.match(app, /function removeSelectionFromMerged\(/);
});

test("no-packages Merger message centers only while its list is empty", () => {
  const css = read("./winui-splitter.css");
  assert.match(css, /#merger-workspace:not\(\.hidden\):has\(#merger-source-list:empty\)/);
  assert.match(css, /justify-content:\s*center/);
});
