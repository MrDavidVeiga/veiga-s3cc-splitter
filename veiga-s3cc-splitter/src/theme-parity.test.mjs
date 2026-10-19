import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const html = read("../index.html");
const css = read("./winui-splitter.css");
const foundation = read("./winui-foundation.css");
const engine = read("./winui-theme.js");

test("Theme selector displays the localized label beside the icon", () => {
  assert.match(html, /<span id="theme-button-label">Theme<\/span>/);
  assert.match(css, /Packer-compatible icon \+ localized label in theme button/);
  assert.match(css, /html\[data-theme-choice\] body\.release-v1 \.topbar \.theme-button\s*\{[\s\S]*?width:auto;[\s\S]*?display:inline-flex;[\s\S]*?gap:7px;/);
  assert.match(css, /html\[data-theme-choice\] body\.release-v1 \.topbar #theme-button-label\s*\{[\s\S]*?white-space:nowrap;/);
  assert.match(engine, /const label = lang === "pt" \? "Tema" : lang === "es" \? "Tema" : "Theme";/);
});

test("EA App accent and theme icon colors match Manager", () => {
  assert.match(engine, /const EA_APP_ACCENT = "#276AFC";/);
  assert.match(engine, /if \(choice==="ea-app"\) return \{\s*\.\.\.accentRamp\(EA_APP_ACCENT,"dark",false\),\s*light1: "#3978FC",\s*dark1: "#215BD8"\s*\};/);
  assert.match(foundation, /\.ea-app-swatch\s*\{[^}]*#276AFC/s);
  assert.match(foundation, /\.theme-check\s*\{[^}]*color:var\(--accent\)/);
  assert.match(css, /:root \{--theme-icon-hover:var\(--accent-light-1,var\(--accent\)\);\}/);
  assert.match(css, /html\[data-resolved-mode="light"\]\s*\{--theme-icon-hover:var\(--accent-dark-2,var\(--accent\)\);\}/);
  assert.match(foundation, /\.theme-button svg\s*\{[^}]*fill:currentColor;/);
});
