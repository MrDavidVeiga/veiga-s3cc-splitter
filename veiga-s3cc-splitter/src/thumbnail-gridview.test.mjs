import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
const base=new URL(".",import.meta.url);
const css=readFileSync(new URL("./ui/fluent/thumbnail-items.css",base),"utf8");
const html=readFileSync(new URL("../index.html",base),"utf8");
const app=readFileSync(new URL("./app.js",base),"utf8");
test("Thumbnail UI imports WinUI ItemsView/GridView styling after legacy CSS",()=>{
  assert.match(html,/src\/ui\/fluent\/thumbnail-items\.css/);
  assert.match(css,/--winui-card-fill-alt/);
  assert.match(css,/--fluent-radius-control/);
  assert.match(css,/--fluent-focus-outer/);
  assert.match(css,/--fluent-accent-foreground/);
  assert.match(css,/object-fit:contain/);
  assert.doesNotMatch(css,/#(?:9acd3f|7fad2d|618422|435c17)|rgba?\(\s*96\s*,\s*132\s*,\s*34/i);
});
test("Selection glyph is exclusive to truly selectable thumbnails",()=>{
  assert.match(css,/\.winui-thumb-item\[aria-selected="true"\]::before/);
  assert.match(css,/\.winui-thumb-item\[aria-selected="true"\]::after/);
  assert.doesNotMatch(css,/\.preview-thumb-wrap::(?:before|after)/);
  assert.match(css,/forced-colors:active/);
});
test("The Splitter thumbnail is a read-only preview, not a selectable card",()=>{
  assert.match(html,/class="preview-thumb-wrap winui-thumbnail-visual"/);
  assert.doesNotMatch(html,/id="preview-thumb"[^>]*aria-selected/);
  assert.match(app,/scan_package_resources/);
  assert.match(app,/merge_packages_with_manifest/);
});
