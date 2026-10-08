import { invoke } from "@tauri-apps/api/core";

const THEME_KEY = "s3cc-splitter-theme";
const VALID_THEMES = new Set(["system","veiga-light","veiga-dark","mean-girls","ea-app","winui-light","winui-dark"]);
const VEIGA_ACCENT = "#7FAD2D";
const MEAN_GIRLS_ACCENT = "#F92F60";
const EA_APP_ACCENT = "#276AFC";
const WINUI_ACCENT_FALLBACK = "#0078D4";
const osDark = window.matchMedia("(prefers-color-scheme: dark)");

let nativeAppearance = {
  mode: null, accent: null,
  accentLight1: null, accentLight2: null, accentLight3: null,
  accentDark1: null, accentDark2: null, accentDark3: null,
  source: "fallback"
};

function parseRgb(color) {
  const value=String(color||"").trim();
  const rgb=value.match(/rgba?\(\s*(\d+)\D+(\d+)\D+(\d+)/i);
  if (rgb) return [Number(rgb[1]),Number(rgb[2]),Number(rgb[3])];
  const hex=value.replace("#","");
  if (/^[0-9a-f]{6}$/i.test(hex)) return [parseInt(hex.slice(0,2),16),parseInt(hex.slice(2,4),16),parseInt(hex.slice(4,6),16)];
  return null;
}
function toRgb(v) { return `rgb(${v.map(n=>Math.max(0,Math.min(255,Math.round(n)))).join(", ")})`; }
function mix(color,target,amount) {
  const a=parseRgb(color), b=parseRgb(target);
  if (!a || !b) return color;
  return toRgb(a.map((v,i)=>v+(b[i]-v)*amount));
}
function alpha(color,a) {
  const v=parseRgb(color);
  return v ? `rgba(${v[0]}, ${v[1]}, ${v[2]}, ${a})` : `rgba(0,120,212,${a})`;
}
function cssAccentProbe() {
  const probe=document.createElement("span");
  probe.style.cssText="position:fixed;left:-9999px;top:-9999px;color:#010203;color:AccentColor;";
  document.body.appendChild(probe);
  const value=getComputedStyle(probe).color;
  probe.remove();
  const rgb=parseRgb(value);
  if (!rgb || (rgb[0]===1 && rgb[1]===2 && rgb[2]===3)) return null;
  return toRgb(rgb);
}
function systemMode() {
  return nativeAppearance.mode==="light" || nativeAppearance.mode==="dark" ? nativeAppearance.mode : (osDark.matches ? "dark" : "light");
}
function systemAccent() { return nativeAppearance.accent || cssAccentProbe() || WINUI_ACCENT_FALLBACK; }
function accentRamp(base,mode,useNative) {
  return {
    base, mode,
    light1:useNative && nativeAppearance.accentLight1 ? nativeAppearance.accentLight1 : mix(base,"#ffffff",.12),
    light2:useNative && nativeAppearance.accentLight2 ? nativeAppearance.accentLight2 : mix(base,"#ffffff",.24),
    light3:useNative && nativeAppearance.accentLight3 ? nativeAppearance.accentLight3 : mix(base,"#ffffff",.42),
    dark1:useNative && nativeAppearance.accentDark1 ? nativeAppearance.accentDark1 : mix(base,"#000000",.12),
    dark2:useNative && nativeAppearance.accentDark2 ? nativeAppearance.accentDark2 : mix(base,"#000000",.24),
    dark3:useNative && nativeAppearance.accentDark3 ? nativeAppearance.accentDark3 : mix(base,"#000000",.38),
  };
}
function resolvedTheme(choice) {
  if (choice==="veiga-light") return accentRamp(VEIGA_ACCENT,"light",false);
  if (choice==="veiga-dark") return accentRamp(VEIGA_ACCENT,"dark",false);
  if (choice==="mean-girls") return accentRamp(MEAN_GIRLS_ACCENT,"light",false);
  if (choice==="ea-app") return {
    ...accentRamp(EA_APP_ACCENT,"dark",false),
    light1: "#3978FC",
    dark1: "#215BD8"
  };
  if (choice==="winui-light") return accentRamp(systemAccent(),"light",true);
  if (choice==="winui-dark") return accentRamp(systemAccent(),"dark",true);
  return accentRamp(systemAccent(),systemMode(),true);
}
function applyAccent(ramp) {
  const root=document.documentElement;
  root.style.setProperty("--accent",ramp.base);
  // RGB channel tokens preserve the original opacity of each UI affordance
  // while following the chosen theme (including the Windows system accent).
  root.style.setProperty("--accent-rgb",(parseRgb(ramp.base) || [127,173,45]).join(", "));
  root.style.setProperty("--accent-dark-rgb",(parseRgb(ramp.dark3) || [67,92,23]).join(", "));
  root.style.setProperty("--accent-light-1",ramp.light1);
  root.style.setProperty("--accent-light-2",ramp.light2);
  root.style.setProperty("--accent-light-3",ramp.light3);
  root.style.setProperty("--accent-dark-1",ramp.dark1);
  root.style.setProperty("--accent-dark-2",ramp.dark2);
  root.style.setProperty("--accent-dark-3",ramp.dark3);
  root.style.setProperty("--accent-low",alpha(ramp.base,ramp.mode==="dark"?.12:.09));
  root.style.setProperty("--accent-outline",alpha(ramp.base,ramp.mode==="dark"?.44:.34));
}
function applyTheme(choice,{persist=true}={}) {
  if (!VALID_THEMES.has(choice)) choice="system";
  const resolved=resolvedTheme(choice);
  const root=document.documentElement;
  root.dataset.themeChoice=choice;
  root.dataset.resolvedMode=resolved.mode;
  applyAccent(resolved);
  document.querySelectorAll(".theme-option").forEach(option=>option.setAttribute("aria-checked",String(option.dataset.theme===choice)));
  document.querySelectorAll(".system-swatch").forEach(swatch=>{ swatch.style.background=systemAccent(); });
  if (persist) try { localStorage.setItem(THEME_KEY,choice); } catch {}
}
function currentChoice() {
  try {
    const saved=localStorage.getItem(THEME_KEY);
    return VALID_THEMES.has(saved) ? saved : "system";
  } catch { return "system"; }
}

async function refreshNativeAppearance() {
  // Manager's native command may not yet exist in the destination. Fallback
  // still probes Windows CSS AccentColor and prefers-color-scheme.
  try {
    const data = await invoke("get_system_appearance");
    if (data && typeof data === "object") nativeAppearance = { ...nativeAppearance, ...data };
  } catch {
    nativeAppearance = { ...nativeAppearance, mode: null, accent: null, source: "css-accent-fallback" };
  }
  applyTheme(document.documentElement.dataset.themeChoice || currentChoice(), { persist: false });
}
const labels = {
  en: {label:"Theme", options:["System","Veiga Light","Veiga Dark","WinUI Light","WinUI Dark","Mean Girls","EA App"]},
  pt: {label:"Tema", options:["Sistema","Veiga Claro","Veiga Escuro","WinUI Claro","WinUI Escuro","Mean Girls","EA App"]},
  es: {label:"Tema", options:["Sistema","Veiga Claro","Veiga Oscuro","WinUI Claro","WinUI Oscuro","Mean Girls","EA App"]}
};
const choices = ["system","veiga-light","veiga-dark","winui-light","winui-dark","mean-girls","ea-app"];
const group = document.createElement("div");
group.className = "winui-theme-picker";
const label = document.createElement("label");
label.htmlFor = "winui-theme-select";
const select = document.createElement("select");
select.id = "winui-theme-select";
select.setAttribute("aria-label", "Theme");
for (const choice of choices) {
  const option = document.createElement("option");
  option.value = choice;
  select.appendChild(option);
}
group.append(label, select);
const topbar = document.querySelector(".topbar");
topbar?.insertBefore(group, topbar.querySelector(".language-switcher"));
function syncLabels() {
  const langCode = document.getElementById("lang-current-code")?.textContent?.trim().toLowerCase();
  const lang = langCode?.startsWith("pt") ? "pt" : langCode?.startsWith("es") ? "es" : "en";
  label.textContent = labels[lang].label;
  select.setAttribute("aria-label", labels[lang].label);
  [...select.options].forEach((option,i) => { option.textContent = labels[lang].options[i]; });
}
select.addEventListener("change", () => applyTheme(select.value));
const saved = currentChoice();
select.value = saved;
applyTheme(saved, { persist: false });
syncLabels();
const languageIndicator = document.getElementById("lang-current-code");
if (languageIndicator) new MutationObserver(syncLabels).observe(languageIndicator, { childList: true, characterData: true, subtree: true });
osDark.addEventListener?.("change", () => {
  if (document.documentElement.dataset.themeChoice === "system") applyTheme("system", { persist: false });
});
document.addEventListener("visibilitychange", () => {
  if (!document.hidden && ["system","winui-light","winui-dark"].includes(document.documentElement.dataset.themeChoice)) void refreshNativeAppearance();
});
void refreshNativeAppearance();
