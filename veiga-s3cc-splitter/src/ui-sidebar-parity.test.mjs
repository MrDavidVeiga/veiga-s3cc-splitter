import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("./manager-layout-parity.css", import.meta.url), "utf8");
const tauri = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));

test("social footer is exactly once, as final direct child of the sidebar", () => {
  const aside = html.match(/<aside class="left-panel card">([\s\S]*?)<\/aside>/)?.[1];
  assert.ok(aside);
  assert.equal((html.match(/<footer class="social-footer winui-social-footer">/g) || []).length, 1);
  assert.match(aside, /<\/div>\s*<footer class="social-footer winui-social-footer">/);
  assert.match(aside, /<\/footer>\s*$/);
  assert.equal((aside.match(/class="social-icons-list"/g) || []).length, 1);
  assert.equal((aside.match(/<li>/g) || []).length, 5);
  assert.match(css, /\.layout > \.left-panel > \.winui-social-footer\s*\{[^}]*margin-top:auto/s);
});

test("language button has the selected code at left and dropdown chevron at right", () => {
  assert.match(html, /id="lang-toggle-btn"[\s\S]*?id="lang-current-code"[\s\S]*?id="lang-toggle-icon"/);
  assert.match(css, /#lang-toggle-btn\s*\{[^}]*justify-content:space-between/s);
  assert.match(css, /#lang-current-code\s*\{[^}]*margin-right:auto/s);
  assert.match(css, /#lang-toggle-icon\s*\{[^}]*margin-left:auto/s);
});

test("native window title matches the simple Beta identifier", () => {
  const expected = "Veiga's S3CC Splitter v2.1.0 | Beta 2";
  assert.equal(tauri.app.windows[0].title, expected);
  assert.ok(html.includes('class="winui-window-title" data-tauri-drag-region>' + expected + "</span>"));
  assert.ok(html.includes("<title>" + expected + "</title>"));
});
