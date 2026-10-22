import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (p) => readFileSync(new URL(p, import.meta.url), "utf8");
const source = read("./app.js");
const html = read("../index.html");
const tauri = JSON.parse(read("../src-tauri/tauri.conf.json"));

test("native open-dialog labels are *.package in both Splitter and Merger", () => {
  const expected = 'filters: [{ name: "*.package", extensions: ["package", "dbc"] }]';
  assert.equal(source.split(expected).length - 1, 2);
  assert.doesNotMatch(source, /name: "Package", extensions/);
  assert.match(source, /async function pickPackage\(/);
  assert.match(source, /async function pickMergerPackages\(/);
});

test("visible title and runtime title share the same release version and beta number", () => {
  const n = /Beta 1/.test(tauri.app.windows[0].title) ? 1 : 2;
  const expected = "Veiga's S3CC Splitter v2.0.0 | Beta " + n;
  assert.equal(tauri.version, "2.0.0");
  assert.equal(tauri.app.windows[0].title, expected);
  assert.equal((html.match(/<title>([^<]+)<\/title>/) || [])[1], expected);
  assert.equal((html.match(/<span class="winui-window-title"[^>]*>([^<]+)<\/span>/) || [])[1], expected);
});
