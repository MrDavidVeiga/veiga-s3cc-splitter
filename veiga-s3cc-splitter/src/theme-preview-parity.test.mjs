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
test("Idioma, PT/EN/ES match only Tema SIZE; original weights are unchanged",()=>{
 assert.ok(css.includes("#theme-button-label {"));
 assert.ok(css.includes("#lang-label,"));
 const start=css.lastIndexOf("body.release-v1 .topbar #lang-label,");
 const end=css.indexOf("}",start);
 const rule=css.slice(start,end);
 assert.ok(rule.includes("font-size:14px;"));
 assert.doesNotMatch(rule,/font-weight/);
});

test("Original llama stays as fallback while real package thumbnails win",()=>{
 assert.ok(app.includes('import noPreviewImage from "./assets/no-preview.png"'));
 assert.ok(app.includes('if (state.preview.thumbBase64)'));
 assert.ok(app.includes('data:image/png;base64'));
 assert.ok(app.includes('el.previewThumb.src = noPreviewImage;'));
 assert.equal(app.split('el.previewThumb.src = noPreviewImage;').length-1,2);
});


test("Centered cardless busy status and animated indeterminate progress",()=>{
 assert.ok(css.includes("#splitter-workspace #busy-state:not(.hidden)"));
 assert.ok(css.includes("align-items:center; justify-content:center"));
 assert.ok(css.includes("#splitter-workspace #busy-state .progress-card"));
 assert.ok(css.includes("background:transparent; backdrop-filter:none; text-align:center;"));
 assert.ok(css.includes("animation:splitter-busy-marquee"));
 assert.ok(css.includes("@keyframes splitter-busy-marquee"));
 assert.ok(html.includes('role="progressbar"'));
});
test("Equal compact modal buttons for OK, Cancel and Remove",()=>{
 assert.ok(css.includes("#app-modal .app-modal-actions"));
 assert.ok(css.includes("flex:0 0 120px"));
 assert.ok(css.includes("justify-content:center"));
 assert.ok(html.includes('id="app-modal-cancel"')&&html.includes('id="app-modal-primary"'));
});
