import test from "node:test";
import assert from "node:assert/strict";
import vm from "node:vm";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const app = read("./app.js");
const backend = read("../src-tauri/src/commands.rs");
const commands = read("../src-tauri/src/lib.rs");
const html = read("../index.html");

const between = (source, from, to) => {
  const a = source.indexOf(from);
  const b = source.indexOf(to, a + from.length);
  assert.ok(a >= 0 && b > a, `source block not found: ${from}`);
  return source.slice(a, b);
};

test("resource rows preserve untrusted CC names as literal text rather than parsing HTML", () => {
  const renderList = between(app, "function renderList() {", "function renderPreview()");
  assert.doesNotMatch(renderList, /\.innerHTML\s*=/);
  assert.match(renderList, /name\.textContent = item\.label/);
  assert.match(renderList, /document\.createTextNode\(String\(value\)\)/);

  class Node {
    constructor(tag) {
      this.tag = tag;
      this.children = [];
      this.style = {};
      this.classList = { add() {} };
      this.scrollTop = 0;
      this.clientHeight = 90;
      this.firstElementChild = null;
    }
    set innerHTML(value) { throw new Error("Unsafe HTML insertion: " + value); }
    set textContent(value) { this.text = value; }
    get childNodes() { return this.children; }
    append(...children) { this.children.push(...children); }
    appendChild(child) { this.children.push(child); return child; }
    replaceChildren(...children) {
      this.children = [...children];
      this.firstElementChild = children[0] || null;
    }
    addEventListener() {}
  }

  const label = '<img src=x onerror="evil()"> & Pessoa';
  const details = '<script>evil()</script> - custom name';
  const item = { key: "test-key", label, details, kind: "CASP" };
  const list = new Node("list");
  const context = {
    document: {
      createElement: (name) => new Node(name),
      createTextNode: (value) => ({ text: value, tag: "text" }),
    },
    el: { resourceList: list },
    state: { listScrollTop: 0, selectedKeys: [], preview: null, language: "pt" },
    renderedListWindow: { items: null, first: -1, last: -1, selection: null, previewKey: null, language: null },
    cachedListCounts: { source: null, counts: null },
    getVisibleResources: () => [item],
    selectedResourceKeys: () => new Set(),
    buildSameNameCounts: () => ({}),
    sameNameCountForItem: () => 0,
    displayRelationForItem: () => null,
    getDisplayType: (value) => value.kind,
    getDisplayDetails: (value) => value.details,
    handleSelectionClick: () => {},
    loadPreview: () => {},
    t: (key) => key,
  };
  vm.runInNewContext(renderList + "\nrenderList();", context);
  const rows = list.children[0].children;
  assert.equal(rows.length, 1);
  const button = rows[0].children.find((node) => node.tag === "button");
  const heading = button.children[0], meta = button.children[1];
  assert.equal(heading.children[0].text, label);
  assert.equal(meta.children.find((node) => node.tag === "text" && node.text === details).text, details);
  assert.equal(button.children.length, 2);
});

test("scroll-window caching and Set membership avoid repeat O(n) checks", () => {
  const render = between(app, "function renderList() {", "function renderPreview()");
  assert.match(render, /renderedListWindow\.items === visibleResources/);
  assert.match(render, /renderedListWindow\.selection === state\.selectedKeys/);
  assert.match(render, /selection\.has\(item\.key\)/);
  assert.match(app, /new Set\(state\.selectedKeys\)/);
});

test("full relation verification is opt-in and cancellable without blocking list", () => {
  assert.match(backend, /pub async fn analyze_package_relations_async\(/);
  assert.match(backend, /pub fn cancel_package_relation_analysis\(/);
  assert.match(backend, /fn build_resource_relations_monitored<F>\(/);
  assert.match(backend, /item_index % 16 == 0/);
  assert.match(backend, /RELATION_ANALYSIS_GENERATION\.load\(Ordering::Acquire\) != generation/);
  assert.match(backend, /SCAN_GENERATION\.load\(Ordering::Acquire\) != active_scan/);
  assert.match(commands, /commands::analyze_package_relations_async/);
  assert.match(commands, /commands::cancel_package_relation_analysis/);
  assert.match(app, /invoke\("analyze_package_relations_async"/);
  assert.match(app, /invoke\("cancel_package_relation_analysis"/);
  assert.match(app, /listen\("package-relations-progress"/);
  assert.match(html, /id="relation-analyze-btn"/);
  assert.match(html, /id="relation-cancel-btn"/);
});

test("duplicate verification controls are localized in PT EN and ES", () => {
  for (const key of ["relationCheckAction", "relationCheckCancel", "relationCheckProgress", "relationCheckError"]) {
    assert.equal(app.split(key + ":").length - 1, 3, key);
  }
  assert.match(app, /state\.relationsStatus !== "skipped"/);
  assert.match(app, /state\.relationAnalysisInProgress/);
});
