import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const THEME_KEY = "s3cc-splitter-theme";
const VALID_THEMES = new Set(["system","veiga-light","veiga-dark","mean-girls","ea-app","winui-light","winui-dark"]);
const VEIGA_ACCENT = "#7FAD2D";
const MEAN_GIRLS_ACCENT = "#F92F60";
const EA_APP_ACCENT = "#FF4747";
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
  if (choice==="ea-app") return accentRamp(EA_APP_ACCENT,"dark",false);
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
  // Manager parity: all theme-specific button foreground colors must come
  // from the shared Fluent tokens, never a Packer-only native accent heuristic.
  root.style.removeProperty("--fluent-accent-foreground");
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

/* Exact Manager theme flyout interaction, independent persistence per tool. */
const themeButton = document.getElementById("theme-button");
const themeFlyout = document.getElementById("theme-flyout");
function closeThemeFlyout() {
  themeFlyout?.classList.add("hidden");
  themeButton?.setAttribute("aria-expanded", "false");
}
themeButton?.addEventListener("click", (event) => {
  event.stopPropagation();
  if (!themeFlyout) return;
  const opening = themeFlyout.classList.contains("hidden");
  themeFlyout.classList.toggle("hidden", !opening);
  themeButton.setAttribute("aria-expanded", String(opening));
});
themeFlyout?.addEventListener("click", (event) => event.stopPropagation());
document.querySelectorAll(".theme-option").forEach((option) => {
  option.addEventListener("click", () => {
    applyTheme(option.dataset.theme || "system");
    closeThemeFlyout();
    themeButton?.focus();
  });
});
document.addEventListener("click", closeThemeFlyout);
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeThemeFlyout();
});

const localizedNames = {
  en: ["System","Veiga Light","Veiga Dark","Mean Girls","EA App","WinUI Light","WinUI Dark"],
  pt: ["Sistema","Veiga Claro","Veiga Escuro","Meninas Malvadas","EA App","WinUI Claro","WinUI Escuro"],
  es: ["Sistema","Veiga Claro","Veiga Oscuro","Chicas Malasis","EA App","WinUI Claro","WinUI Oscuro"]
};
const captionLabels = {
  en: {minimize:"Minimize", maximize:"Maximize", restore:"Restore down", close:"Close", windowControls:"Window controls", inputMode:"Input mode"},
  pt: {minimize:"Minimizar", maximize:"Maximizar", restore:"Restaurar tamanho", close:"Fechar", windowControls:"Controles da janela", inputMode:"Modo de entrada"},
  es: {minimize:"Minimizar", maximize:"Maximizar", restore:"Restaurar tamaño", close:"Cerrar", windowControls:"Controles de ventana", inputMode:"Modo de entrada"}
};
function currentInterfaceLanguage() {
  const code=document.getElementById("lang-current-code")?.textContent?.trim().toLowerCase() || "en";
  return code.startsWith("pt") ? "pt" : code.startsWith("es") ? "es" : "en";
}
function setCaptionLabel(id,label) {
  const control=document.getElementById(id);
  control?.setAttribute("aria-label",label);
  if (control) control.title=label;
}
function syncThemeLanguage() {
  const code = document.getElementById("lang-current-code")?.textContent?.trim()?.toLowerCase() || "en";
  const lang = currentInterfaceLanguage();
  document.documentElement.lang=lang;
  const captions=captionLabels[lang];
  setCaptionLabel("titlebar-minimize",captions.minimize);
  setCaptionLabel("titlebar-close",captions.close);
  const restoreShown=!document.querySelector("#titlebar-maximize .winui-restore-glyph")?.hasAttribute("hidden");
  setCaptionLabel("titlebar-maximize",restoreShown?captions.restore:captions.maximize);
  document.querySelector(".winui-caption-buttons")?.setAttribute("aria-label",captions.windowControls);
  document.querySelector(".topbar-mode-switcher")?.setAttribute("aria-label",captions.inputMode);
  setCaptionLabel("analysis-results-close",captions.close);
  const label = lang === "pt" ? "Tema" : lang === "es" ? "Tema" : "Theme";
  const footnote = {
    en: "System theme and accent are read from the operating system when supported.",
    pt: "O tema e a cor de destaque do sistema são utilizados quando disponíveis.",
    es: "Se utilizan el tema y el color de acento del sistema cuando están disponibles."
  };
  themeButton?.setAttribute("aria-label", label);
  themeButton?.setAttribute("title", label);
  const themeText = document.getElementById("theme-button-label");
  if (themeText) themeText.textContent = label;
  themeFlyout?.setAttribute("aria-label", label);
  const title = themeFlyout?.querySelector(".theme-flyout-title");
  if (title) title.textContent = label;
  themeFlyout?.querySelectorAll(".theme-option strong").forEach((element, i) => {
    element.textContent = localizedNames[lang][i];
  });
  const footer = themeFlyout?.querySelector(".theme-flyout-footnote");
  if (footer) footer.textContent = footnote[lang];
}
syncThemeLanguage();
const langIndicator = document.getElementById("lang-current-code");
if (langIndicator) {
  new MutationObserver(syncThemeLanguage).observe(langIndicator, {
    childList:true, characterData:true, subtree:true
  });
}

osDark.addEventListener?.("change", () => {
  if (document.documentElement.dataset.themeChoice === "system") applyTheme("system", {persist:false});
});
function updateSystemTheme() {
  if (["system","winui-light","winui-dark"].includes(document.documentElement.dataset.themeChoice)) void refreshNativeAppearance();
}
document.addEventListener("visibilitychange", () => { if (!document.hidden) updateSystemTheme(); });

const appWindow = getCurrentWindow();
const maximizeButton = document.getElementById("titlebar-maximize");
async function updateCaptionMaximize() {
  if (!maximizeButton) return;
  try {
    const maximized = await appWindow.isMaximized();
    maximizeButton.querySelector(".winui-maximize-glyph")?.toggleAttribute("hidden",maximized);
    maximizeButton.querySelector(".winui-restore-glyph")?.toggleAttribute("hidden",!maximized);
    const captions=captionLabels[currentInterfaceLanguage()];
    const text = maximized ? captions.restore : captions.maximize;
    maximizeButton.setAttribute("aria-label",text);
    maximizeButton.title = text;
  } catch { /* Browser preview: static glyph remains available. */ }
}
document.getElementById("titlebar-minimize")?.addEventListener("click", async () => {
  try { await appWindow.minimize(); } catch (error) { console.error("Minimize failed:", error); }
});
maximizeButton?.addEventListener("click", async () => {
  try { await appWindow.toggleMaximize(); await updateCaptionMaximize(); }
  catch (error) { console.error("Maximize failed:", error); }
});
document.getElementById("titlebar-close")?.addEventListener("click", async () => {
  try { await appWindow.close(); } catch (error) { console.error("Close failed:", error); }
});
appWindow.onResized(() => { void updateCaptionMaximize(); }).catch(() => {});
appWindow.onFocusChanged(({payload}) => {
  if (payload) { updateSystemTheme(); void updateCaptionMaximize(); }
}).catch(() => {});
void updateCaptionMaximize();
applyTheme(currentChoice(), {persist:false});
void refreshNativeAppearance();
