import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (relative) => readFileSync(new URL(relative, import.meta.url), "utf8");
const html = read("../index.html");
const app = read("./app.js");

test("both drag hints preserve a separate icon, main text and extension line", () => {
  for (const id of ["package-drop-hint", "merger-add-hint"]) {
    const fragment = html.match(new RegExp('<div id="' + id + '" class="drop-hint" role="note">([\\s\\S]*?)<\\/div>'));
    assert.ok(fragment, id + " not found");
    assert.match(fragment[1], /class="drop-hint-icon"/);
    assert.match(fragment[1], /class="drop-hint-title"/);
    assert.match(fragment[1], /class="drop-hint-formats">\.package • \.dbc<\/span>/);
  }
  assert.match(html, /class="drop-hint-title">Select or drag a file here<\/span>/);
  assert.match(html, /class="drop-hint-title">Select or drag folders or files here<\/span>/);
});

test("Portuguese, English and Spanish instructions stay localized", () => {
  for (const phrase of [
    'dropHint: "Selecione ou arraste um arquivo aqui"',
    'mergerAddHint: "Selecione ou arraste pastas ou arquivos aqui"',
    'dropHint: "Select or drag a file here"',
    'mergerAddHint: "Select or drag folders or files here"',
    'dropHint: "Selecciona o arrastra un archivo aquí"',
    'mergerAddHint: "Selecciona o arrastra carpetas o archivos aquí"',
  ]) assert.ok(app.includes(phrase), "Missing translation: " + phrase);
});

test("language changes only replace hint text and preserve icons and file formats", () => {
  assert.match(app, /el\.dropHint\.querySelector\("\.drop-hint-title"\)\?\.replaceChildren\(t\("dropHint"\)\)/);
  assert.match(app, /el\.mergerAddHint\.querySelector\("\.drop-hint-title"\)\?\.replaceChildren\(t\("mergerAddHint"\)\)/);
  assert.doesNotMatch(app, /el\.(?:dropHint|mergerAddHint)\.textContent\s*=/);
});
