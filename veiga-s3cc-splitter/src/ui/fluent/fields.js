/* Progressive SearchBox enhancer. Existing search inputs remain authoritative. */
const targets = ["results-search-input", "library-search", "save-search", "aux-results-search", "cc-links-search"];
const labels = {en:"Clear search",pt:"Limpar pesquisa",es:"Borrar búsqueda"};
function currentLabel() {
  const lang = document.getElementById("lang-current-code")?.textContent?.trim().toLowerCase() || document.documentElement.lang;
  return labels[lang] || labels.en;
}
function enhanceSearch(input) {
  if (!input || input.closest(".fluent-search-box")) return;
  const wrap = document.createElement("div");
  wrap.className = "fluent-search-box";
  input.parentNode.insertBefore(wrap, input);
  wrap.appendChild(input);
  const button = document.createElement("button");
  button.type = "button";
  button.className = "fluent-search-clear";
  button.setAttribute("aria-label", currentLabel());
  button.title = currentLabel();
  button.innerHTML = '<svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 2.5 9.5 9.5M9.5 2.5 2.5 9.5"/></svg>';
  wrap.appendChild(button);
  function refresh() {
    button.hidden = !input.value || input.disabled || input.readOnly;
    wrap.classList.toggle("fluent-search-filled",!!input.value);
  }
  button.addEventListener("pointerdown",event=>event.preventDefault());
  button.addEventListener("click", () => {
    if (input.disabled || input.readOnly) return;
    input.value = "";
    input.dispatchEvent(new Event("input",{bubbles:true}));
    input.dispatchEvent(new Event("change",{bubbles:true}));
    refresh();
    input.focus({preventScroll:true});
  });
  input.addEventListener("input",refresh);
  input.addEventListener("change",refresh);
  new MutationObserver(refresh).observe(input,{attributes:true,attributeFilter:["disabled","readonly"]});
  window.addEventListener("s3cc-language-changed",()=>{
    button.setAttribute("aria-label",currentLabel());
    button.title = currentLabel();
    refresh();
  });
  refresh();
}
for (const id of targets) enhanceSearch(document.getElementById(id));
export { enhanceSearch };

const fluentLanguageCode = document.getElementById("lang-current-code");
if(fluentLanguageCode) new MutationObserver(()=>window.dispatchEvent(new Event("s3cc-language-changed"))).observe(fluentLanguageCode,{subtree:true,childList:true,characterData:true});
