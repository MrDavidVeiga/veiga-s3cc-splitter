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
      this.dataset = {};
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
    queueListThumbnail: () => {},
    listThumbnailCache: new Map(),
    noPreviewImage: "llama-placeholder.png",
    t: (key) => key,
  };
  vm.runInNewContext(renderList + "\nrenderList();", context);
  const rows = list.children[0].children;
  assert.equal(rows.length, 1);
  const button = rows[0].children.find((node) => node.tag === "button");
  const thumbnail = button.children[0];
  const textWrap = button.children[1];
  const heading = textWrap.children[0], meta = textWrap.children[1];
  assert.equal(thumbnail.tag, "img");
  assert.equal(thumbnail.dataset.resourceKey, "test-key");
  assert.equal(thumbnail.src, "llama-placeholder.png");
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
  assert.match(backend, /SCAN_GENERATION\.load\(Ordering::Acquire\) != payload\.scan_generation/);
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

test("stale automatic and manual relation results never replace the active scan", () => {
  const source = between(app, "function applyRelationUpdate(payload) {", "function clearBusy()");
  const item = { key: "0xA-0xB-0xC", relation: null };
  const state = {
    packagePath: "same.package",
    scanGeneration: 12,
    relationsStatus: "deferred",
    relationAnalysisInProgress: false,
    activeRelationRequestId: null,
    resources: [item],
  };
  const ctx = { state };
  vm.runInNewContext(source, ctx);

  const updated = { "0xA-0xB-0xC": { duplicateCount: 3 } };
  const automatic = {
    packagePath: "same.package", scanGeneration: 11,
    requestId: null, relations: updated,
  };
  assert.equal(ctx.applyRelationUpdate(automatic), false);
  assert.equal(state.resources[0], item);

  automatic.scanGeneration = 12;
  assert.equal(ctx.applyRelationUpdate(automatic), true);
  assert.equal(state.resources[0].relation.duplicateCount, 3);

  state.relationsStatus = "skipped";
  state.relationAnalysisInProgress = true;
  state.activeRelationRequestId = 17;
  const manual = { ...automatic, requestId: 16, relations: {} };
  assert.equal(ctx.applyRelationUpdate(manual), false);
  assert.equal(state.resources[0].relation.duplicateCount, 3);
  manual.requestId = 17;
  assert.equal(ctx.applyRelationUpdate(manual), true);
  assert.equal(state.relationsStatus, "complete");
  assert.equal(state.resources[0].relation, null);

  state.relationAnalysisInProgress = false;
  assert.equal(ctx.applyRelationUpdate(manual), false);
  state.scanGeneration = null;
  assert.equal(ctx.applyRelationUpdate(automatic), false);
});

test("progress handlers ignore previous requests even when the package path is unchanged", async () => {
  const source = between(app, "async function setupProgressEvents() {", "void setupProgressEvents();");
  const callbacks = new Map();
  const state = {
    packagePath: "same.package",
    scanGeneration: 45,
    activeRelationRequestId: 9,
    relationAnalysisInProgress: true,
    relationAnalysisProgress: 0,
    isBusy: false,
    language: "en",
  };
  let renders = 0;
  const context = {
    state,
    listen: async (name, cb) => { callbacks.set(name, cb); },
    applyRelationUpdate: () => false,
    render: () => { renders++; },
    console,
    el: {},
    t: () => "",
    scanStageMessages: { en: {} },
    Math,
    Number,
  };
  const installed = vm.runInNewContext(source + "\nsetupProgressEvents()", context);
  // Await the promise from the VM realm, not the host realm's microtask ticks.
  await installed;
  const progress = callbacks.get("package-relations-progress");
  assert.equal(typeof progress, "function");

  progress({ payload: { packagePath: "same.package", scanGeneration: 44, requestId: 9, percent: 70 } });
  progress({ payload: { packagePath: "same.package", scanGeneration: 45, requestId: 8, percent: 80 } });
  assert.equal(state.relationAnalysisProgress, 0);
  assert.equal(renders, 0);
  progress({ payload: { packagePath: "same.package", scanGeneration: 45, requestId: 9, percent: 25 } });
  assert.equal(state.relationAnalysisProgress, 25);
  assert.equal(renders, 1);
});

test("Rust request IDs and cancellation are correlated, including cancel-before-start", () => {
  assert.match(backend, /pub struct RelationCancelPayload/);
  assert.match(backend, /scan_generation: u64,\s*pub request_id: u64/);
  assert.match(backend, /cancelled_before_start\.insert\(id\)/);
  assert.match(backend, /cancelled_before_start\.remove\(&id\)/);
  assert.match(backend, /operations\.active == Some\(id\)/);
  assert.match(backend, /request_id: Some\(payload\.request_id\)/);
  assert.match(backend, /request_id: None/);
  assert.match(backend, /scan_generation: relation_generation/);
  assert.match(app, /payload\.scanGeneration !== state\.scanGeneration/);
  assert.match(app, /payload\.requestId !== state\.activeRelationRequestId/);
  assert.match(app, /payload: \{ requestId, scanGeneration \}/);
  assert.match(app, /packagePath: requestedPath, scanGeneration, requestId: token/);
});

test("expensive comparison loops cooperatively check for cancellation", () => {
  for (const name of ["relation_tgi_index", "relation_entries_have_same_payload",
    "relation_tgi_conflicts", "logical_profile_for_item", "logical_profiles_exactly_equal"]) {
    const start = backend.indexOf("fn " + name + "<F>");
    assert.ok(start >= 0, name + " must be instrumented");
    const remainder = backend.slice(start, start + 3400);
    assert.match(remainder, /checkpoint\(\)\?/);
  }
  assert.match(backend, /build_resource_relations_monitored\([\s\S]*?Análise automática substituída ou cancelada/);
});
