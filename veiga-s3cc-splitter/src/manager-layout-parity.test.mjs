import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

const html = read("../index.html");
const css = read("./manager-layout-parity.css");

test("both tools share the Manager's final CSS layer and equivalent list shell", () => {
  assert.match(html, /href="\.\/src\/manager-layout-parity\.css"/);
  assert.ok(html.indexOf("manager-layout-parity.css") > html.indexOf("winui-splitter.css"));
  assert.match(html, /<section class="list-card manager-list-card">/);
  assert.match(html, /id="resource-list" class="resource-list package-list"/);
  assert.match(css, /--list-end-gap:28px/);
  assert.match(css, /padding:0 0 var\(--list-end-gap\)/);
  assert.match(css, /min-height:var\(--list-end-gap\)/);
});

test("native scrollbar matches the Manager's Fluent fixed-width thumb without content jitter", () => {
  assert.match(css, /#resource-list,#merger-source-list,#content-filter-menu,\.preview-card/);
  assert.match(css, /scrollbar-width:auto/);
  assert.match(css, /width:12px;/);
  assert.match(css, /border:4px solid transparent/);
  assert.match(css, /border-width:2px/);
  assert.match(css, /border-width:1px/);
  assert.match(css, /var\(--fluent-scrollbar-thumb/);
  assert.doesNotMatch(css, /display:none\s*!important[^}]*#resource-list/);
});

test("Manager search, language, theme and outer panel shape are ported", () => {
  assert.match(html, /id="search-input"[^>]*type="search"/);
  for (const id of ["lang-label", "lang-current-code", "theme-button-label", "content-filter-menu"]) {
    assert.match(html, new RegExp('id="' + id + '"'));
  }
  assert.match(css, /#lang-toggle-btn[\s\S]*?width:76px/);
  assert.match(css, /#theme-button-label[\s\S]*?font:600 14px/);
  assert.match(css, /#results-state #search-input[\s\S]*?font:400 13px/);
  assert.match(css, /border-radius:var\(--winui-overlay-radius\) 0 0 0/);
});

test("modal uses Manager's icons, WinUI colors and context-dependent actions", () => {
  assert.match(html, /class="app-modal-icon" aria-hidden="true"/);
  assert.match(css, /#app-modal\[data-modal-kind="confirm"\] \.app-modal-icon/);
  assert.match(css, /#app-modal\[data-modal-kind="success"\] \.app-modal-icon/);
  assert.match(css, /#app-modal\[data-modal-kind="error"\] \.app-modal-icon/);
  assert.match(css, /#app-modal\[data-modal-kind="warning"\] \.app-modal-icon/);
  assert.match(css, /content:"\\f071"/);
  assert.match(css, /width:min\(460px,92vw\)/);
  assert.match(css, /min-width:120px/);
});
