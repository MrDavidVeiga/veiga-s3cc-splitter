import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (relative) => readFileSync(new URL(relative, import.meta.url), "utf8");
const app = read("./app.js");
const html = read("../index.html");
const css = read("./manager-layout-parity.css");

test("Splitter opening title follows localized mode heading without changing the Merger", () => {
  assert.match(app, /emptyTitle: "Packages to Split"/);
  assert.equal((app.match(/emptyTitle: "Packages para Separar"/g) || []).length, 2);
  assert.match(app, /mergerTitle: "Packages para Mesclar"/);
  assert.match(html, /class="state-title">Packages to Split<\/div>/);
  assert.match(css, /#splitter-workspace #empty-state \.state-title,\s*[\s\S]*?#merger-workspace #merger-title\s*\{/);
  assert.match(css, /color:var\(--winui-text-primary\)/);
});

test("Theme and language topbar match Manager geometry and Fluent text sizes", () => {
  assert.match(css, /topbar\.winui-navview\s*\{\s*column-gap:12px/);
  assert.match(css, /#lang-label\s*\{[\s\S]*?font:400 14px\/20px/);
  assert.match(css, /#lang-toggle-btn\s*\{[\s\S]*?width:76px;[\s\S]*?justify-content:center;[\s\S]*?gap:8px/);
  assert.match(css, /#lang-current-code\s*\{[\s\S]*?font:600 14px\/20px/);
  assert.match(css, /#theme-button\s*\{[\s\S]*?align-items:center;[\s\S]*?justify-content:center;[\s\S]*?gap:8px/);
  assert.match(css, /#theme-button > svg\s*\{[\s\S]*?width:16px;[\s\S]*?height:16px/);
  assert.match(css, /#theme-button-label\s*\{[\s\S]*?font:600 14px\/20px/);
});

test("language list displays Manager regular 12px codes and accurate selected state", () => {
  assert.match(css, /#lang-menu \.lang-menu-item\s*\{[\s\S]*?font:400 12px\/18px/);
  assert.match(app, /btn\.hidden = isActive/);
  assert.match(app, /btn\.setAttribute\("aria-hidden", String\(isActive\)\)/);
  assert.match(app, /btn\.setAttribute\("aria-selected", String\(isActive\)\)/);
});
