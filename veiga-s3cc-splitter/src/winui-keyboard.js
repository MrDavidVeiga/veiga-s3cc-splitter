/*
 * Fluent keyboard bridge for existing Tauri DOM controls.
 * Keeps the apps' original click, filter, selection and render code authoritative.
 * Menu: ArrowUp/Down, Home/End, Escape, Enter and Space.
 * Tabs: Left/Right, Home/End.
 * Lists: Up/Down, Home/End, PageUp/PageDown and native Space checkbox toggle.
 */
const visible = el => !!(el && !el.disabled && !el.hidden && !el.classList.contains("hidden") && el.getClientRects().length);
function moveIndex(items,index,key,step=1) {
  if (!items.length) return -1;
  if (key === "Home") return 0;
  if (key === "End") return items.length - 1;
  const delta = key === "ArrowUp" || key === "ArrowLeft" ? -step : step;
  return Math.min(items.length - 1,Math.max(0,index + delta));
}
for(const [triggerId,menuId,selector] of [
  ["lang-toggle-btn","lang-menu","button"],
  ["content-filter-toggle-btn","content-filter-menu","button"],
  ["pick-sims3pack-btn","sims3pack-menu",'input[type="checkbox"]']
]) {
  const trigger=document.getElementById(triggerId),menu=document.getElementById(menuId);
  if (!trigger || !menu) continue;
  const options=()=>Array.from(menu.querySelectorAll(selector)).filter(visible);
  const focusOption=(key,source)=>{
    const items=options();
    if (!items.length) return;
    const i=items.indexOf(source);
    const next=moveIndex(items,i,key);
    if (next>=0) { items[next].focus(); items[next].scrollIntoView({block:"nearest"}); }
  };
  trigger.addEventListener("keydown",event=>{
    if (!["ArrowUp","ArrowDown","Home","End","Escape"].includes(event.key)) return;
    if (event.key==="Escape") { if (trigger.getAttribute("aria-expanded")==="true"){event.preventDefault();trigger.click();} return; }
    event.preventDefault();
    const wasClosed=trigger.getAttribute("aria-expanded")!=="true";
    if (wasClosed) trigger.click(); // delegate open-state to the original app
    queueMicrotask(()=>focusOption(event.key==="ArrowUp"?"End":event.key==="ArrowDown"?"Home":event.key, null));
  });
  menu.addEventListener("keydown",event=>{
    if (event.key==="Escape") {
      event.preventDefault(); event.stopPropagation();
      if(trigger.getAttribute("aria-expanded")==="true")trigger.click();
      trigger.focus();return;
    }
    if (!["ArrowUp","ArrowDown","Home","End"].includes(event.key)) return;
    const source=event.target.closest(selector);
    if (!source || !menu.contains(source)) return;
    event.preventDefault(); event.stopPropagation();
    focusOption(event.key,source);
  });
}
for(const tabList of document.querySelectorAll(".topbar-mode-switcher,[role=tablist].library-subtabs")) {
  const tabs=()=>Array.from(tabList.querySelectorAll("button")).filter(visible);
  const syncTabAttributes=()=>{
    for(const button of tabList.querySelectorAll("button")){
      const active=button.classList.contains("active");
      button.setAttribute("role","tab");
      button.setAttribute("aria-selected",String(active));
      button.tabIndex=active?0:-1;
    }
  };
  syncTabAttributes();
  const observer=new MutationObserver(syncTabAttributes);
  observer.observe(tabList,{subtree:true,attributes:true,attributeFilter:["class"]});
  tabList.addEventListener("keydown",e=>{
    if (!["ArrowLeft","ArrowRight","Home","End"].includes(e.key))return;
    const items=tabs(),index=items.indexOf(e.target.closest("button"));
    if (index<0||!items.length)return;
    e.preventDefault();
    const next=moveIndex(items,index,e.key);
    if(next>=0){items[next].tabIndex=0;items[next].focus();items[next].click();}
  });
}
for(const list of document.querySelectorAll(".package-list,.resource-list")) {
  const rows=()=>Array.from(list.querySelectorAll(":scope > .package-row, :scope > .resource-row"));
  list.addEventListener("keydown",event=>{
    if (!["ArrowUp","ArrowDown","Home","End","PageUp","PageDown"].includes(event.key))return;
    if(event.altKey||event.ctrlKey||event.metaKey)return;
    const row=event.target.closest(".package-row,.resource-row");
    if (!row||!list.contains(row))return;
    const all=rows(),i=all.indexOf(row);
    if(i<0)return;
    event.preventDefault();
    const next=moveIndex(all,i,event.key,event.key.startsWith("Page")?10:1);
    const selector=event.target.matches('input[type="checkbox"]')?'input[type="checkbox"]':".package-main,.resource-button";
    const focusable=all[next]?.querySelector(selector);
    focusable?.focus();
    focusable?.scrollIntoView({block:"nearest"});
  });
  // App render() reconstructs rows when a checkbox is selected. Keep keyboard focus.
  list.addEventListener("click",event=>{
    if(!event.target.matches('input[type="checkbox"]'))return;
    const row=event.target.closest(".package-row,.resource-row");
    const index=rows().indexOf(row);
    if(index<0)return;
    queueMicrotask(()=>{
      const check=rows()[index]?.querySelector('input[type="checkbox"]');
      if(check){check.focus({preventScroll:true});}
    });
  },true);
}
