import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("./winui-splitter.css", import.meta.url), "utf8");
const source = readFileSync(new URL("./app.js", import.meta.url), "utf8");

test("List-only actions appear below the CC list, not below the preview", () => {
  const results = html.slice(html.indexOf('id="results-state"'));
  const grid = results.indexOf('class="results-grid"');
  const column = results.indexOf('class="results-list-column"');
  const list = results.indexOf('id="resource-list"');
  const footer = results.indexOf('class="footer-actions results-footer-actions');
  const preview = results.indexOf('class="preview-card"');
  assert.ok(grid >= 0 && column > grid && list > column && footer > list && preview > footer);
  const listColumnEnd = results.indexOf("</div>", footer);
  assert.ok(listColumnEnd > footer && results.slice(footer, listColumnEnd).includes('id="export-btn"'));
  for (const id of ["export-btn", "remove-merged-btn", "clear-btn"]) {
    const matches = [...results.matchAll(new RegExp('id="' + id + '"', "g"))];
    assert.equal(matches.length, 1, id);
    assert.ok(matches[0].index > footer && matches[0].index < preview, id);
    assert.ok(source.includes('"' + id + '"'), "existing event binding " + id);
  }
  if (html.includes('id="restore-manifest-btn"')) {
    const undo = results.indexOf('id="restore-manifest-btn"');
    assert.ok(undo > footer && undo < preview);
    assert.ok(source.includes('"#restore-manifest-btn"'));
  }
});

test("Filter and search share a row with selection controls above preview", () => {
  const results = html.slice(html.indexOf('id="results-state"'));
  const actions = results.indexOf('class="results-actions"');
  const grid = results.indexOf('class="results-grid"');
  for (const id of [
    "content-filter-toggle-btn","search-input","select-all-btn","select-none-btn"
  ]) {
    const match = results.indexOf('id="' + id + '"');
    assert.ok(match > actions && match < grid, id);
    assert.equal([...html.matchAll(new RegExp('id="' + id + '"',"g"))].length, 1, id);
    assert.ok(source.includes('"#' + id + '"'), "existing event binding " + id);
  }
  assert.match(css,/\.results-selection-controls\s*\{[\s\S]*?grid-column:2/);
  assert.match(css,/\.results-selection-controls > button,[\s\S]*?\.results-search-input\s*\{[\s\S]*?height:36px/);
  assert.match(css,/\.results-footer-actions\s*\{[\s\S]*?grid-template-columns:repeat\(4/);
});
