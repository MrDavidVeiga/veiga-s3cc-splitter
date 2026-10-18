import { getCurrentWindow } from "@tauri-apps/api/window";
/* WinUI 3 keyboard affordances. No native processing changes. */
const focusAt = (elements,index) => {
  const element=elements[(index+elements.length)%elements.length];
  if (!element) return;
  element.focus();
  element.scrollIntoView({block:"nearest",inline:"nearest"});
};
document.addEventListener("keydown",(event)=>{
  const target=event.target;
  if (!(target instanceof HTMLElement)) return;
  if (target.matches('.topbar-mode-switcher [role="tab"]')) {
    const tabs=[...document.querySelectorAll('.topbar-mode-switcher [role="tab"]:not(:disabled)')];
    const current=tabs.indexOf(target);
    let next=null;
    if(event.key==="ArrowRight")next=current+1;
    else if(event.key==="ArrowLeft")next=current-1;
    else if(event.key==="Home")next=0;
    else if(event.key==="End")next=tabs.length-1;
    if(next!==null && tabs.length){
      event.preventDefault();focusAt(tabs,next);tabs[(next+tabs.length)%tabs.length].click();
    }
    return;
  }
  const resourceButton=target.closest("#resource-list .resource-button");
  if(resourceButton && (event.key==="ArrowUp"||event.key==="ArrowDown")){
    const buttons=[...document.querySelectorAll("#resource-list .resource-button")];
    event.preventDefault();
    focusAt(buttons,buttons.indexOf(resourceButton)+(event.key==="ArrowUp"?-1:1));
    return;
  }
  const menu=target.closest(".lang-menu:not(.hidden),.theme-flyout:not(.hidden)");
  if(menu){
    const options=[...menu.querySelectorAll('button:not(:disabled):not(.hidden)')];
    const index=options.indexOf(target);
    if(index<0)return;
    let next=null;
    if(event.key==="ArrowDown")next=index+1;
    else if(event.key==="ArrowUp")next=index-1;
    else if(event.key==="Home")next=0;
    else if(event.key==="End")next=options.length-1;
    if(next!==null&&options.length){event.preventDefault();focusAt(options,next);}
  }
});

const legacyWindow = getCurrentWindow();
const minimizeControl = document.getElementById("titlebar-minimize");
const maximizeControl = document.getElementById("titlebar-maximize");
const closeControl = document.getElementById("titlebar-close");
const updateCaption = async () => {
  if (!maximizeControl) return;
  try {
    const isMax = await legacyWindow.isMaximized();
    const maxGlyph = maximizeControl.querySelector(".winui-maximize-glyph");
    const restoreGlyph = maximizeControl.querySelector(".winui-restore-glyph");
    if (maxGlyph) maxGlyph.hidden = isMax;
    if (restoreGlyph) restoreGlyph.hidden = !isMax;
    maximizeControl.title = isMax ? "Restore" : "Maximize";
    maximizeControl.setAttribute("aria-label", maximizeControl.title);
  } catch (error) { console.warn("Cannot check maximized state", error); }
};
minimizeControl?.addEventListener("click", () => legacyWindow.minimize().catch(console.error));
maximizeControl?.addEventListener("click", async () => {
  try {
    if (await legacyWindow.isMaximized()) await legacyWindow.unmaximize();
    else await legacyWindow.maximize();
    await updateCaption();
  } catch (error) { console.error(error); }
});
closeControl?.addEventListener("click", () => legacyWindow.close().catch(console.error));
void legacyWindow.onResized(updateCaption);
void updateCaption();
