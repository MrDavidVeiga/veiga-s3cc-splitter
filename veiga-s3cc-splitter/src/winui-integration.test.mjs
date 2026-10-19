import test from "node:test";
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import vm from "node:vm";

const source=readFileSync(new URL("./app.js",import.meta.url),"utf8");
const markup=readFileSync(new URL("../index.html",import.meta.url),"utf8");
const css=readFileSync(new URL("./winui-splitter.css",import.meta.url),"utf8");
const foundation=readFileSync(new URL("./winui-foundation.css",import.meta.url),"utf8");
const engine=readFileSync(new URL("./winui-theme.js",import.meta.url),"utf8");
const backend=readFileSync(new URL("../src-tauri/src/lib.rs",import.meta.url),"utf8");
const begin=source.indexOf("const translations = {");
const end=source.indexOf("const state = {",begin);
assert.ok(begin>=0&&end>begin);
const literal=source.slice(begin,end).replace(/^const translations = /,"").trim().replace(/;\s*$/,"");
const locale=vm.runInNewContext("("+literal+")");

test("English Portuguese and Spanish contain identical translation keys",()=>{
 const keys=Object.keys(locale.en).sort();
 for(const lang of ["pt","es"]){
   assert.deepEqual(Object.keys(locale[lang]).sort(),keys,lang);
   assert.deepEqual(Object.keys(locale[lang].filterLabels).sort(),Object.keys(locale.en.filterLabels).sort(),lang);
 }
});
test("All three languages use consistent interaction terminology",()=>{
 assert.equal(locale.pt.modeMerger,"Mesclador");
 assert.equal(locale.es.modeMerger,"Fusionador");
 assert.equal(locale.es.selectNone,"Deseleccionar Todo");
 assert.equal(locale.pt.previewTitle,"Preview"); // Retained as familiar software UI terminology.
 assert.equal(locale.pt.clear,"Limpar Lista");
 assert.equal(locale.es.restoreOriginals,"Desfusionar");
 for(const language of ["en","pt","es"]){
   assert.ok(locale[language].dropHint && locale[language].mergerAddHint);
 }
});
test("Manager theme tokens, drag hints, titlebar and seven themes are in place",()=>{
 for(const name of ["system","veiga-light","veiga-dark","mean-girls","ea-app","winui-light","winui-dark"]){
   assert.ok(engine.includes('"'+name+'"'));
 }
 assert.match(engine,/removeProperty\("--fluent-accent-foreground"\)/);
 assert.doesNotMatch(engine,/setProperty\("--fluent-accent-foreground"/);
 assert.match(css,/min-height:52px/);
 assert.match(css,/var\(--fluent-accent-foreground\)/);
 assert.match(markup,/class="winui-app-icon"/);
 assert.match(markup,/id="package-drop-hint" class="drop-hint" role="note"/);
 assert.match(markup,/id="merger-add-hint" class="drop-hint" role="note"/);
});
test("Native Splitter Merger restore export and drag handlers remain present",()=>{
 for(const call of [
  "scan_package_resources","merge_packages_with_manifest",
  "collect_merge_package_files","restore_manifest_packages",
  "remove_selected_from_merged","export_selected_resources"
 ]){
   assert.ok(source.includes('"'+call+'"'),call);
 }
 assert.match(backend,/commands::zip_selected_resources/);
 assert.match(backend,/system_appearance::get_system_appearance/);
 assert.match(source,/bindDragAndDrop\(\)/);
 assert.match(source,/formatRemoveConfirm\(/);
});

test("Package drop hint matches accepted file types in all languages",()=>{
 const hints={
  en:"Select or drag a .package or .dbc here",
  pt:"Selecione ou arraste um .package ou .dbc aqui",
  es:"Selecciona o arrastra un .package o .dbc aquí"
 };
 for(const [lang,hint] of Object.entries(hints)){
  assert.equal(locale[lang].dropHint,hint,lang);
 }
});

test("Theme selector displays the localized label and matches Manager's EA App palette",()=>{
 assert.match(markup,/id="theme-button-label">Theme<\/span>/);
 assert.match(css,/Packer-compatible icon \+ localized label in theme button/);
 assert.match(css,/html\[data-theme-choice\] body\.release-v1 \.topbar \.theme-button\s*\{[\s\S]*?width:auto;[\s\S]*?display:inline-flex;[\s\S]*?gap:7px;/);
 assert.match(css,/html\[data-theme-choice\] body\.release-v1 \.topbar #theme-button-label\s*\{[\s\S]*?white-space:nowrap;/);
 assert.match(engine,/const label = lang === "pt" \? "Tema" : lang === "es" \? "Tema" : "Theme";/);
 assert.match(engine,/const EA_APP_ACCENT = "#276AFC";/);
 assert.match(engine,/if \(choice==="ea-app"\) return \{\s*\.\.\.accentRamp\(EA_APP_ACCENT,"dark",false\),\s*light1: "#3978FC",\s*dark1: "#215BD8"\s*\};/);
 assert.match(foundation,/\.ea-app-swatch\s*\{[^}]*#276AFC/s);
 assert.match(foundation,/\.theme-check\s*\{[^}]*color:var\(--accent\)/);
 assert.match(css,/html\[data-theme-choice="ea-app"\] body\.release-v1 \.theme-button\s*\{[^}]*color:#FFFFFF/s);
 assert.match(css,/html\[data-theme-choice="ea-app"\] body\.release-v1 \.theme-button:hover[\s\S]*?background:#343647/);
});
