import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
const css=readFileSync(new URL("./winui-splitter.css",import.meta.url),"utf8");
const tokens=readFileSync(new URL("./ui/fluent/tokens.css",import.meta.url),"utf8");
test("WinUI thumbnail cards preserve Fluent theme backgrounds and borders",()=>{
  assert.match(css,/var\(--winui-card-fill-alt\)/);
  assert.match(css,/var\(--winui-control-stroke\)/);
  assert.match(css,/var\(--winui-control-fill-hover\)/);
  assert.match(css,/var\(--accent-outline\)/);
  assert.match(css,/aspect-ratio:\s*1\s*\/\s*1/);
  assert.match(css,/object-fit:\s*cover/);
  for(const selector of ["preview-card","preview-thumb-wrap","type-pill","lang-menu-item.active"]) {
    assert.ok(css.includes(selector),selector);
  }
  assert.match(tokens,/--fluent-accent-foreground/);
});
test("Thumbnail parity overrides use live theme attributes, never hardcoded green",()=>{
  const parity=css.slice(css.lastIndexOf("/* Manager"));
  assert.ok(parity.includes("html[data-theme-choice]"));
  assert.doesNotMatch(parity, /#(?:9acd3f|7fad2d|618422|435c17)|rgba?\(\s*(?:96\s*,\s*132\s*,\s*34|122\s*,\s*166\s*,\s*43)/i);
});
