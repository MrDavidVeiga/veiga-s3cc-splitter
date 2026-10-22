import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (relativePath) => readFileSync(new URL(relativePath, import.meta.url), "utf8");

test("Beta 2 is versioned 2.0.0 and titled as Beta 2", () => {
  const npm = JSON.parse(read("../package.json"));
  const lock = JSON.parse(read("../package-lock.json"));
  const tauri = JSON.parse(read("../src-tauri/tauri.conf.json"));
  const cargo = read("../src-tauri/Cargo.toml");
  const cargoLock = read("../../Cargo.lock");
  const html = read("../index.html");
  assert.equal(npm.version, "2.0.0");
  assert.equal(lock.version, "2.0.0");
  assert.equal(lock.packages[""].version, "2.0.0");
  assert.equal(tauri.version, "2.0.0");
  assert.match(cargo, /^version = "2\.0\.0"$/m);
  assert.match(cargoLock, /name = "veiga-s3cc-splitter"\r?\nversion = "2\.0\.0"/);
  assert.equal((html.match(/<title>([^<]+)<\/title>/) || [])[1], "Veiga's S3CC Splitter v2.0.0 | Beta 2");
  assert.equal(tauri.app.windows[0].title, "Veiga's S3CC Splitter v2.0.0 | Beta 2");
  assert.doesNotMatch(html, /Beta 3/);
});

test("Beta 2 preserves Splitter, Merger, thumbnails and animated progress", () => {
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
