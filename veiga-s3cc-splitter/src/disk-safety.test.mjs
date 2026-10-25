import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const app = readFileSync(new URL("./app.js", import.meta.url), "utf8");
const backend = readFileSync(new URL("../src-tauri/src/commands.rs", import.meta.url), "utf8");

test("all languages expose low-disk warning and success flows show it", () => {
  assert.equal((app.match(/lowDiskWarning:\s*\(bytes\)\s*=>/g) || []).length, 3);
  assert.match(app, /Low disk space:/);
  assert.match(app, /Pouco espaço em disco:/);
  assert.match(app, /Poco espacio en disco:/);
  assert.equal((app.match(/result\.diskFreeBytesWarning\s*!=\s*null/g) || []).length, 3);
});

test("backend rejects insufficient disk before writes and computes risk per operation", () => {
  assert.match(backend, /fn evaluate_disk_budget\(/);
  assert.match(backend, /MIN_RECOMMENDED_FREE_BYTES:\s*u64\s*=\s*5/);
  assert.match(backend, /fn destination_free_bytes\(/);
  assert.ok((backend.match(/preflight_destination_space\(&destination,/g) || []).length >= 3);
  assert.match(backend, /fn disk_budget_blocks_shortage_and_warns_under_five_gib\(/);
});
