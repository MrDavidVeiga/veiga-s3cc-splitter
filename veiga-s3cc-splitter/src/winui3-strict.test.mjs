import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (relative) => readFileSync(new URL(relative, import.meta.url), "utf8");
const html = read("../index.html");
const css = read("./winui3-strict.css");

test("WinUI 3 geometry stylesheet loads after every previous CSS layer", () => {
  const head = html.slice(0, html.indexOf("</head>"));
  const cssLinks = [...head.matchAll(/<link\b[^>]*rel="stylesheet"[^>]*href="([^"]+)"/g)].map(x => x[1]);
  assert.ok(cssLinks.length, "No stylesheets in HTML");
  assert.equal(cssLinks.at(-1), "./src/winui3-strict.css");
});

test("Microsoft ControlCornerRadius 4px, OverlayCornerRadius 8px, joined edges 0px", () => {
  for (const token of [
    "--veiga-winui-control-radius: 4px;",
    "--veiga-winui-overlay-radius: 8px;",
    "--veiga-winui-joined-radius: 0px;",
  ]) assert.ok(css.includes(token), "Missing " + token);
  assert.match(css, /body :is\(\s*\.list-card, \.manager-list-card, \.preview-card/);
  assert.match(css, /\.lang-menu, \.theme-flyout, \.content-filter-menu/);
  assert.match(css, /border-radius: var\(--veiga-winui-control-radius\) !important/);
  assert.match(css, /border-radius: var\(--veiga-winui-overlay-radius\) !important/);
});

test("navigation, themed controls, dialogs and WebView2 scrollbars retain Fluent semantics", () => {
  for (const className of [
    ".left-panel", ".right-panel", ".manager-list-card", ".preview-card",
    ".primary-btn", ".lang-menu", ".theme-flyout", ".confirm-modal",
    ".modal-overlay", ".resource-list", ".package-list", "#merger-source-list",
  ]) assert.ok(css.includes(className), "Missing selector " + className);
  assert.match(css, /scrollbar-width: auto/);
  assert.match(css, /width: 12px/);
  assert.match(css, /border-width: 2px/);
  assert.match(css, /border-width: 1px/);
  assert.match(css, /@media \(forced-colors: active\)/);
  assert.match(css, /@media \(prefers-reduced-motion: reduce\)/);
});
