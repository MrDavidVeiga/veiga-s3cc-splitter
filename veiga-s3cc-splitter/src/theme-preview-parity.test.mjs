import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
const html=readFileSync(new URL("../index.html",import.meta.url),"utf8");
const theme=readFileSync(new URL("./winui-theme.js",import.meta.url),"utf8");
const app=readFileSync(new URL("./app.js",import.meta.url),"utf8");
const css=readFileSync(new URL("./winui-splitter.css",import.meta.url),"utf8");

test("Manager theme swatches show two tones for all seven options",()=>{
 const names=["system","veiga-light","veiga-dark","mean-girls","ea-app","winui-light","winui-dark"];
 assert.match(theme,/function applyThemeSwatches\(/);
 assert.match(theme,/linear-gradient\(135deg,/);
 for(const id of names){
   const row=html.split("\n").find(s=>s.includes('data-theme="'+id+'"'));
   assert.ok(row?.includes(id+"-swatch"),id);
 }
 assert.match(theme,/applyAccent\(resolved\);\s*applyThemeSwatches\(\)/);
});
test("Language abbreviation and Tema have identical 14px typography",()=>{
 assert.match(css,/#theme-button-label,\s*html\[data-theme-choice\] body\.release-v1 \.topbar #lang-current-code/);
 assert.match(css,/font-size:14px;\s*line-height:20px;\s*font-weight:600/);
});
test("Only a real resource thumbnail is displayed; no stock placeholder",()=>{
 assert.doesNotMatch(app,/noPreviewImage/);
 assert.match(app,/previewUnavailable/);
 assert.match(app,/previewThumb\.removeAttribute\("src"\)/);
 assert.match(app,/thumbBase64/);
});
