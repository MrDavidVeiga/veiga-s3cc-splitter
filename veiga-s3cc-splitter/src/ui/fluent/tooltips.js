/* Delegated Fluent ToolTip for existing title attributes.
 * No per-row listeners, no changes to dynamically virtualized list items.
 */
const tooltip=document.createElement("div");
tooltip.id="fluent-shared-tooltip";
tooltip.className="fluent-tooltip";
tooltip.setAttribute("role","tooltip");
tooltip.hidden=true;
document.body.appendChild(tooltip);
let current=null;
let originalDescription=null;
let fromKeyboard=false;
function closeTooltip() {
  if (!current) return;
  if(!current.hasAttribute("title") && current.dataset.fluentTooltipTitle!==undefined){
    current.setAttribute("title",current.dataset.fluentTooltipTitle);
  }
  delete current.dataset.fluentTooltipTitle;
  if(originalDescription===null)current.removeAttribute("aria-describedby");
  else current.setAttribute("aria-describedby",originalDescription);
  current=null; originalDescription=null;
  tooltip.removeAttribute("data-visible");
  tooltip.hidden=true;
}
function showTooltip(node,keyboard=false) {
  if(!node || !(node instanceof Element))return;
  if(node===current)return;
  closeTooltip();
  const title=node.getAttribute("title");
  if(!title)return;
  current=node;fromKeyboard=keyboard;
  originalDescription=node.getAttribute("aria-describedby");
  node.dataset.fluentTooltipTitle=title;
  node.removeAttribute("title");
  tooltip.textContent=title;
  node.setAttribute("aria-describedby",
    [originalDescription,"fluent-shared-tooltip"].filter(Boolean).join(" "));
  tooltip.hidden=false;
  const rect=node.getBoundingClientRect();
  const tip=tooltip.getBoundingClientRect();
  const left=Math.min(Math.max(8,rect.left+(rect.width-tip.width)/2),Math.max(8,window.innerWidth-tip.width-8));
  const below=rect.bottom+8;
  const top=below+tip.height<=window.innerHeight-8 ? below : Math.max(8,rect.top-tip.height-8);
  tooltip.style.left=`${left}px`;tooltip.style.top=`${top}px`;
  tooltip.dataset.visible="true";
}
document.addEventListener("pointerover",event=>{
  const target=event.target.closest?.("[title]");
  if(target && !target.closest(".fluent-tooltip"))showTooltip(target);
});
document.addEventListener("pointerout",event=>{
  if(current && event.target instanceof Element && current.contains(event.target) &&
     !current.contains(event.relatedTarget))closeTooltip();
});
document.addEventListener("focusin",event=>{
  const target=event.target.closest?.("[title]");
  if(target)showTooltip(target,true);
});
document.addEventListener("focusout",event=>{
  if(current && current.contains(event.target) &&
     !current.contains(event.relatedTarget))closeTooltip();
});
document.addEventListener("keydown",event=>{if(event.key==="Escape")closeTooltip();});
window.addEventListener("scroll",closeTooltip,true);
window.addEventListener("blur",closeTooltip);
export { showTooltip, closeTooltip };
