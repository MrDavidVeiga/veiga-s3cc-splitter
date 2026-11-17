import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const app = read("./app.js");
const backend = read("../src-tauri/src/commands.rs");
const lib = read("../src-tauri/src/lib.rs");
const managerCss = read("./manager-layout-parity.css");
const manifest = read("../src-tauri/src/merge_manifest.rs");

function blockBetween(source, from, to) {
  const start = source.indexOf(from);
  const end = source.indexOf(to, start + from.length);
  assert.ok(start >= 0 && end > start, `Cannot extract source section: ${from}`);
  return source.slice(start, end);
}

test("Beta 2 still scans through a blocking worker, not the UI event loop", () => {
  assert.match(backend, /pub async fn scan_package_resources\(/);
  assert.match(backend, /spawn_blocking\(move \|\| scan_package_resources_inner/);
  assert.match(lib, /commands::scan_package_resources,/);
});

test("Beta 1 nonblocking first preview is preserved without removing Beta 2 cache", () => {
  const scan = blockBetween(app, "async function scanPackage() {", "async function revealPreviewSource()");
  assert.match(scan, /void loadPreview\(firstVisible\)/);
  assert.doesNotMatch(scan, /await loadPreview\(/);
  assert.ok(scan.indexOf("clearBusy()") < scan.indexOf("void loadPreview(firstVisible)"));
  assert.match(backend, /pub async fn get_resource_preview_async\(/);
  assert.match(backend, /spawn_blocking\(move \|\| get_resource_preview\(app, payload\)\)/);
  assert.match(lib, /commands::get_resource_preview_async,/);
  assert.match(app, /invoke\("get_resource_preview_async"/);
  assert.match(backend, /build_preview_cache\(&path\)/);
  assert.match(backend, /pose_list_preview_base64/);
});

test("cancelled scans and obsolete thumbnails cannot replace a newer selection", () => {
  const scan = blockBetween(app, "async function scanPackage() {", "async function revealPreviewSource()");
  const preview = blockBetween(app, "async function loadPreview(item) {", "function isMergerPackagePath");
  assert.match(scan, /state\.operationCancelRequested \|\| state\.packagePath !== requestedPath/);
  assert.match(scan, /!state\.operationCancelRequested && state\.packagePath === requestedPath/);
  assert.match(preview, /token !== previewRequestToken \|\| state\.packagePath !== requestedPath/);
  assert.match(app, /\+\+previewRequestToken/);
  assert.match(app, /invoke\("cancel_package_operation"\)/);
});

test("large relation checks explicitly indicate skipped or deferred work", () => {
  assert.match(backend, /const MAX_RELATION_ANALYSIS_ITEMS: usize = 8_000;/);
  assert.match(backend, /items\.len\(\) > MAX_RELATION_ANALYSIS_ITEMS/);
  assert.match(backend, /relations_status: relations_status\.to_string\(\)/);
  assert.match(backend, /\(BTreeMap::new\(\), "skipped"\)/);
  assert.match(backend, /\(BTreeMap::new\(\), "deferred"\)/);
  assert.match(app, /state\.relationsStatus = result\.relationsStatus/);
  assert.match(app, /state\.relationsStatus = "complete"/);
  for (const translation of ["relationCheckSkipped", "relationCheckRunning"]) {
    assert.equal(app.split(translation + ":").length - 1, 3, `${translation} must exist in EN, PT and ES`);
  }
});

test("scan stage and cancellation notices are localized while preserving backend events", () => {
  assert.match(app, /const scanStageMessages = \{/);
  for (const stage of ["Opening package", "Reading names", "Building resource list",
    "Preparing relationship metadata", "Ready"]) {
    assert.equal(app.split('"' + stage + '":').length - 1, 3);
  }
  assert.equal(app.split("scanCancelling:").length - 1, 3);
  assert.match(app, /state\.scanInProgress = true/);
  assert.match(app, /state\.scanInProgress = false/);
});

test("late duplicate badge updates are rejected after a newer scan", () => {
  assert.match(backend, /static SCAN_GENERATION: AtomicU64 = AtomicU64::new\(0\)/);
  assert.match(backend, /SCAN_GENERATION\.fetch_add\(1, Ordering::AcqRel\)/);
  assert.match(backend, /SCAN_GENERATION\.load\(Ordering::Acquire\) != relation_generation/);
  assert.match(backend, /CANCEL_REQUESTED\.load\(Ordering::Acquire\)/);
});

test("Beta 1 sidebar geometry is retained in the Beta 2 interface", () => {
  assert.match(managerCss, /\.left-panel > \.winui-social-footer/);
  assert.match(managerCss, /#lang-toggle-btn/);
  assert.match(managerCss, /#lang-current-code/);
  assert.match(managerCss, /#lang-toggle-icon/);
});

test("Beta 2-exclusive reversible poses, skins and manifest remain present", () => {
  assert.match(backend, /fn pose_list_merge_plan\(/);
  assert.match(backend, /fn build_clip_group_items\(/);
  assert.match(backend, /fn recovered_skin_resource_stem\(/);
  assert.match(backend, /fn copy_stored_merge_resource\(/);
  assert.match(manifest, /MANIFEST_TYPE: u32 = 0x5653474D/);
  assert.match(manifest, /LEGACY_MANIFEST_TYPE: u32 = 0x0333406C/);
});
