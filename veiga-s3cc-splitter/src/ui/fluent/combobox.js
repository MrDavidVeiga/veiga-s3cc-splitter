/**
 * Fluent ComboBox progressive enhancement.
 *
 * Keep each native <select> in the DOM as the authoritative value source.
 * Existing .value / .options / .disabled and change listeners continue to work.
 * A button with role="combobox" and a single body-level listbox replace the
 * platform-rendered popup. Long lists use fixed-height virtualization.
 *
 * No dependency on Tauri or app.js; source module can load ahead of app.js.
 */
const ITEM_HEIGHT = 34;
const OVERSCAN = 6;
const all = new Set();
let opened = null;
let keyboardQuery = "";
let keyboardQueryAt = 0;

const flyout = document.createElement("div");
flyout.id = "fluent-combobox-flyout";
flyout.className = "fluent-combobox-flyout";
flyout.setAttribute("role", "listbox");
flyout.setAttribute("aria-label", "Options");
flyout.hidden = true;
document.body.append(flyout);

const escId = value => String(value).replace(/[^a-zA-Z0-9_-]/g, "_");
const entryOptions = select => Array.from(select.options).map((option, index) => ({
  index,
  value: option.value,
  label: option.label || option.textContent.trim(),
  group: option.parentElement?.tagName === "OPTGROUP" ? option.parentElement.label : "",
  disabled: option.disabled || !!option.parentElement?.disabled,
}));

function displayedLabel(select) {
  // selectedIndex and value are the authoritative state. Some WebView/DOM
  // implementations may lag when updating the selectedOptions collection.
  const selected = select.options[select.selectedIndex];
  return selected?.label || selected?.textContent?.trim() ||
    select.options[0]?.label || "—";
}

function updateButton(item) {
  const {select, trigger, caption} = item;
  caption.textContent = displayedLabel(select);
  const disabled = select.disabled || select.options.length === 0;
  trigger.disabled = disabled;
  trigger.setAttribute("aria-disabled", String(disabled));
  if (disabled && opened?.item === item) closeFlyout({restoreFocus:false});
  if (opened?.item === item && !flyout.hidden) {
    const priorValue = opened.entries[opened.active]?.value;
    opened.entries = entryOptions(select);
    const selected = opened.entries.findIndex(e=>e.index === select.selectedIndex);
    opened.active = priorValue != null
      ? Math.max(0,opened.entries.findIndex(e=>e.value === priorValue))
      : Math.max(0,selected);
    renderFlyout();
  }
}

function closeFlyout({restoreFocus=false}={}) {
  if (!opened) return;
  const item = opened.item;
  flyout.hidden = true;
  flyout.replaceChildren();
  item.trigger.setAttribute("aria-expanded","false");
  item.trigger.removeAttribute("aria-activedescendant");
  item.wrapper.classList.remove("open");
  opened = null;
  keyboardQuery = "";
  if (restoreFocus && !item.trigger.disabled) item.trigger.focus({preventScroll:true});
}

function keepVisible(index) {
  const current = opened;
  if (!current) return;
  const top = index * ITEM_HEIGHT;
  if (top < flyout.scrollTop) flyout.scrollTop = top;
  else if (top + ITEM_HEIGHT > flyout.scrollTop + flyout.clientHeight) {
    flyout.scrollTop = top + ITEM_HEIGHT - flyout.clientHeight;
  }
}

function focusActive(index) {
  if (!opened || index < 0 || index >= opened.entries.length) return;
  if (opened.entries[index].disabled) return;
  opened.active = index;
  keepVisible(index);
  renderFlyout();
}

function moveActive(direction) {
  if (!opened) return;
  const count = opened.entries.length;
  if (!count) return;
  for (let offset=1;offset<=count;offset++) {
    const idx=(opened.active + direction*offset + count*1000) % count;
    if (!opened.entries[idx].disabled) {focusActive(idx);return;}
  }
}

// Closed ComboBoxes behave like native WinUI selection fields: arrows change
// the value without expanding the list and preserve existing change listeners.
function changeClosedSelection(item, direction) {
  const select = item.select;
  if (select.disabled) return;
  const options = entryOptions(select);
  const start = select.selectedIndex < 0
    ? (direction > 0 ? -1 : options.length)
    : select.selectedIndex;
  for (let index = start + direction; index >= 0 && index < options.length; index += direction) {
    if (options[index].disabled) continue;
    if (index === select.selectedIndex) return;
    select.selectedIndex = index;
    select.dispatchEvent(new Event("input", {bubbles: true}));
    select.dispatchEvent(new Event("change", {bubbles: true}));
    updateButton(item);
    return;
  }
}

function pick(index) {
  const current = opened;
  if (!current) return;
  const option = current.entries[index];
  if (!option || option.disabled) return;
  const select = current.item.select;
  const changed=select.selectedIndex !== option.index;
  closeFlyout({restoreFocus:true});
  if (!changed) return;
  select.selectedIndex=option.index;
  select.dispatchEvent(new Event("input",{bubbles:true}));
  select.dispatchEvent(new Event("change",{bubbles:true}));
  updateButton(current.item);
}

function renderFlyout() {
  if (!opened || flyout.hidden) return;
  const {entries,active,item}=opened;
  const count=entries.length;
  const visible=Math.max(1,Math.ceil((flyout.clientHeight || 288) / ITEM_HEIGHT));
  const first=Math.max(0,Math.floor(flyout.scrollTop/ITEM_HEIGHT)-OVERSCAN);
  const last=Math.min(count,first+visible+OVERSCAN*2);
  const children=[];
  const before=document.createElement("div");
  before.className="fluent-combobox-spacer";
  before.style.height=`${first*ITEM_HEIGHT}px`;
  before.setAttribute("aria-hidden","true");
  children.push(before);
  for(let idx=first;idx<last;idx++){
    const entry=entries[idx];
    const element=document.createElement("div");
    element.className="fluent-combobox-option";
    element.id=`${item.id}-option-${idx}`;
    element.setAttribute("role","option");
    element.setAttribute("aria-selected",String(entry.index===item.select.selectedIndex));
    element.setAttribute("aria-disabled",String(entry.disabled));
    element.setAttribute("aria-posinset",String(idx+1));
    element.setAttribute("aria-setsize",String(count));
    if(idx===active) element.classList.add("active");
    if(entry.index===item.select.selectedIndex)element.classList.add("selected");
    if(entry.disabled)element.classList.add("disabled");
    const check=document.createElement("span");
    check.className="fluent-combobox-check";
    check.setAttribute("aria-hidden","true");
    check.textContent=entry.index===item.select.selectedIndex ? "✓":"";
    const text=document.createElement("span");
    text.className="fluent-combobox-option-label";
    text.textContent=entry.label;
    if(entry.group) text.title=entry.group;
    element.append(check,text);
    element.addEventListener("pointerdown",e=>{if(!entry.disabled)e.preventDefault();});
    element.addEventListener("pointerenter",()=>{
      if (!opened || opened.item !== item || entry.disabled) return;
      opened.active = idx;
      // Preserve the hovered node; rebuilding it loses :hover in WebView2.
      for (const sibling of flyout.querySelectorAll(".fluent-combobox-option.active")) {
        sibling.classList.remove("active");
      }
      element.classList.add("active");
      item.trigger.setAttribute("aria-activedescendant", element.id);
    });
    element.addEventListener("click",()=>pick(idx));
    children.push(element);
  }
  const after=document.createElement("div");
  after.className="fluent-combobox-spacer";
  after.style.height=`${Math.max(0,count-last)*ITEM_HEIGHT}px`;
  after.setAttribute("aria-hidden","true");
  children.push(after);
  flyout.replaceChildren(...children);
  if(active>=first && active<last)item.trigger.setAttribute("aria-activedescendant",`${item.id}-option-${active}`);
  else item.trigger.removeAttribute("aria-activedescendant");
}

function positionFlyout() {
  if (!opened) return;
  const rect=opened.item.trigger.getBoundingClientRect();
  const margin=8;
  const width=Math.min(window.innerWidth-margin*2,Math.max(rect.width,200));
  const left=Math.max(margin,Math.min(rect.left,window.innerWidth-margin-width));
  const below=window.innerHeight-rect.bottom-margin;
  const above=rect.top-margin;
  const maxHeight=Math.max(ITEM_HEIGHT,Math.min(320,Math.max(below,above)));
  const aboveTrigger=below<Math.min(180,maxHeight) && above>below;
  flyout.style.width=`${width}px`;
  flyout.style.maxHeight=`${maxHeight}px`;
  flyout.style.left=`${left}px`;
  flyout.style.top=aboveTrigger ? "auto":`${rect.bottom+3}px`;
  flyout.style.bottom=aboveTrigger ? `${window.innerHeight-rect.top+3}px`:"auto";
}

function openFlyout(item) {
  if (item.select.disabled || item.select.options.length===0) return;
  if (opened?.item===item){closeFlyout();return;}
  closeFlyout();
  opened={item,entries:entryOptions(item.select),active:Math.max(0,item.select.selectedIndex)};
  if (opened.entries[opened.active]?.disabled) {
    opened.active=Math.max(0,opened.entries.findIndex(e=>!e.disabled));
  }
  flyout.hidden=false;
  flyout.setAttribute("aria-label",item.trigger.getAttribute("aria-label") || "Options");
  item.wrapper.classList.add("open");
  item.trigger.setAttribute("aria-expanded","true");
  positionFlyout();
  renderFlyout();
  // The list must have content before scrollTop can reach a selected item.
  flyout.scrollTop=Math.max(0,opened.active*ITEM_HEIGHT-ITEM_HEIGHT*2);
  renderFlyout();
}

function typeahead(key,item) {
  const timestamp=Date.now();
  keyboardQuery=timestamp-keyboardQueryAt>850 ? key.toLocaleLowerCase() : keyboardQuery+key.toLocaleLowerCase();
  keyboardQueryAt=timestamp;
  if (!opened) openFlyout(item);
  if (!opened) return;
  const options=opened.entries;
  const start=opened.active+1;
  let match=-1;
  for(let offset=0;offset<options.length;offset++){
    const pos=(start+offset)%options.length;
    if(!options[pos].disabled && options[pos].label.toLocaleLowerCase().startsWith(keyboardQuery)){match=pos;break;}
  }
  if(match>=0)focusActive(match);
}

function handleKey(event,item) {
  const key=event.key;
  if(key==="Tab") {if(opened?.item===item)closeFlyout();return;}
  if(key==="Escape" && opened?.item===item){
    event.preventDefault();event.stopPropagation();closeFlyout({restoreFocus:true});return;
  }
  if(key==="Enter" || key===" "){
    event.preventDefault();
    event.stopPropagation();
    if(opened?.item===item)pick(opened.active);
    else openFlyout(item);
    return;
  }
  if(key==="ArrowDown" || key==="ArrowUp"){
    event.preventDefault();event.stopPropagation();
    const direction = key==="ArrowDown" ? 1 : -1;
    if(opened?.item!==item) {
      if(event.altKey) openFlyout(item);
      else changeClosedSelection(item,direction);
    } else moveActive(direction);
    return;
  }
  if(key==="Home" || key==="End"){
    event.preventDefault();
    if(opened?.item!==item)openFlyout(item);
    if(!opened) return;
    const step=key==="Home"?1:-1;
    for(let i=key==="Home"?0:opened.entries.length-1;i>=0&&i<opened.entries.length;i+=step){
      if(!opened.entries[i].disabled){focusActive(i);break;}
    }
    return;
  }
  if(key.length===1 && !event.ctrlKey && !event.metaKey && !event.altKey){
    event.preventDefault();
    typeahead(key,item);
  }
}

function enhanceSelect(select,sequence) {
  if(select.dataset.fluentCombobox==="true")return;
  const wrapper=document.createElement("span");
  wrapper.className="fluent-combobox";
  const id=`fluent-combobox-${escId(select.id || sequence)}`;
  const trigger=document.createElement("button");
  trigger.type="button";
  trigger.id=id+"-trigger";
  trigger.className="fluent-combobox-trigger";
  trigger.setAttribute("role","combobox");
  trigger.setAttribute("aria-haspopup","listbox");
  trigger.setAttribute("aria-expanded","false");
  trigger.setAttribute("aria-controls",flyout.id);
  trigger.setAttribute("aria-autocomplete","none");
  const associated=select.labels?.[0];
  const label=(associated?.textContent || select.getAttribute("aria-label") || select.id || "Options").trim();
  trigger.setAttribute("aria-label",label);
  const caption=document.createElement("span");
  caption.className="fluent-combobox-caption";
  const arrow=document.createElement("span");
  arrow.className="fluent-combobox-chevron";
  arrow.setAttribute("aria-hidden","true");
  trigger.append(caption,arrow);
  select.before(wrapper);
  wrapper.append(select,trigger);
  select.dataset.fluentCombobox="true";
  select.classList.add("fluent-native-select");
  select.tabIndex=-1;
  select.setAttribute("aria-hidden","true");
  const item={select,wrapper,trigger,caption,id};
  all.add(item);
  trigger.addEventListener("click",()=>openFlyout(item));
  trigger.addEventListener("keydown",e=>handleKey(e,item));
  select.addEventListener("change",()=>updateButton(item));
  select.addEventListener("input",()=>updateButton(item));
  select.addEventListener("focus",()=>trigger.focus({preventScroll:true}));
  const observer=new MutationObserver(()=>{
    if(item.queued)return;
    item.queued=true;
    queueMicrotask(()=>{item.queued=false;updateButton(item);});
  });
  observer.observe(select,{subtree:true,childList:true,attributes:true,characterData:true,attributeFilter:["value","label","selected","disabled"]});
  item.observer=observer;
  updateButton(item);
}

document.querySelectorAll("select").forEach((select,index)=>enhanceSelect(select,index));
flyout.addEventListener("scroll",()=>{if(opened)renderFlyout();},{passive:true});
document.addEventListener("pointerdown",event=>{
  if(opened && !flyout.contains(event.target) && !opened.item.wrapper.contains(event.target))closeFlyout();
},true);
document.addEventListener("keydown",event=>{
  if(event.key==="Escape"&&opened){
    event.preventDefault();event.stopPropagation();closeFlyout({restoreFocus:true});
  }
},true);
window.addEventListener("resize",()=>{if(opened)positionFlyout();});
document.addEventListener("scroll",event=>{
  if(opened && !flyout.contains(event.target))closeFlyout();
},true);

export function closeFluentComboBox() {closeFlyout();}
export function refreshFluentComboBoxes() {for(const item of all)updateButton(item);}
