import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const load = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const config = JSON.parse(load("../src-tauri/tauri.conf.json"));
const html = load("../index.html");
const modal = load("./modal.js");
const app = load("./app.js");
const css = load("./manager-layout-parity.css");

test("Beta 2 Windows title and publisher use requested branding", () => {
  const expectedTitle = "Veiga's S3CC Splitter v2.0.0 | Beta 2";
  assert.equal(config.app.windows[0].title, expectedTitle);
  assert.equal(config.bundle.publisher, "Veiga Sims");
  assert.equal(html.split(expectedTitle).length - 1, 2,
    "Both document title and custom caption should display the expected title");
});

test("Backup warning uses safely formatted bold text, not injected HTML", () => {
  assert.match(app, /emphasizeLastParagraph: true/);
  assert.match(modal, /doc\.createElement\("strong"\)/);
  assert.match(modal, /message\.replaceChildren\(doc\.createTextNode/);
  assert.doesNotMatch(modal, /message\.innerHTML/);
  assert.match(css, /\.app-modal-emphasis\s*\{\s*font-weight:700;/);
});
