"""Split/Merge WinUI 3 smoke: static browser, no fake claims about native CC processing."""
from pathlib import Path
import re
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
html = (ROOT / "index.html").read_text(encoding="utf-8")

def css_inline(match):
    path = match.group(1).removeprefix("./")
    css = (ROOT / path).read_text(encoding="utf-8")
    if path.endswith("winui-foundation.css"):
        css = css.replace('@import url("./ui/fluent/tokens.css");',
            (ROOT / "src/ui/fluent/tokens.css").read_text(encoding="utf-8"))
    return "<style>\n" + css + "\n</style>"

html = re.sub(r'<link rel="stylesheet" href="([^"]+)"\s*/>', css_inline, html)

theme_js = (ROOT / "src/winui-theme.js").read_text(encoding="utf-8")
theme_js = re.sub(r'^import\s+.*?;\s*$', "", theme_js, flags=re.M)

VIEWPORTS = [(1280,800),(1100,720),(1024,768),(900,680),(820,620)]
THEMES = ["system","veiga-light","veiga-dark","mean-girls","ea-app","winui-light","winui-dark"]
FOREGROUNDS = {
    "system":"#ffffff", "winui-light":"#ffffff", "winui-dark":"#ffffff",
    "veiga-light":"#142007","veiga-dark":"#142007",
    "mean-girls":"#4d1332","ea-app":"#ffffff"
}

def run():
    image_dir = ROOT / "winui-smoke-previews"
    image_dir.mkdir(exist_ok=True)
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True, args=["--no-sandbox"])
        try:
            for width,height in VIEWPORTS:
                page=browser.new_page(viewport={"width":width,"height":height})
                errors=[]
                page.on("pageerror",lambda e: errors.append(str(e)))
                page.set_content(html,wait_until="domcontentloaded")
                page.evaluate("""() => {
                    const store = {};
                    Object.defineProperty(window,'localStorage', {configurable:true,value:{
                      getItem:k=>store[k]||null,setItem:(k,v)=>{store[k]=String(v)}
                    }});
                    window.matchMedia=()=>({matches:true,addEventListener:()=>{}});
                    window.invoke=async()=>({mode:'dark',accent:'#7FAD2D'});
                    window.getCurrentWindow=()=>({
                      isMaximized:async()=>false,toggleMaximize:async()=>{},
                      minimize:async()=>{},close:async()=>{},
                      onResized:()=>Promise.resolve(),
                      onFocusChanged:()=>Promise.resolve()
                    });
                }""")
                page.evaluate(theme_js)
                assert page.locator(".winui-app-icon").count()==1
                assert page.locator(".winui-social-footer .social-icons-list a").count()==5
                for theme in THEMES:
                    page.locator("#theme-button").click()
                    page.locator(f'.theme-option[data-theme="{theme}"]').click()
                    page.wait_for_timeout(130)
                    state=page.evaluate("""() => {
                      const root=document.documentElement;
                      const computed=getComputedStyle(root);
                      const sidebar=document.querySelector('.left-panel').getBoundingClientRect();
                      const cta=document.querySelector('#pick-package-btn').getBoundingClientRect();
                      const load=document.querySelector('#scan-btn').getBoundingClientRect();
                      const icon=getComputedStyle(document.querySelector('.winui-app-icon'));
                      const pane=getComputedStyle(document.querySelector('.right-panel'));
                      const left=getComputedStyle(document.querySelector('.left-panel'));
                      const hint=document.querySelector('#package-drop-hint');
                      const hs=getComputedStyle(hint),hr=hint.getBoundingClientRect();
                      const links=Array.from(document.querySelectorAll('.winui-social-footer a'));
                      return {
                        theme:root.dataset.themeChoice,
                        accent:root.style.getPropertyValue('--accent').trim(),
                        foreground:computed.getPropertyValue('--fluent-accent-foreground').trim(),
                        customForeground:root.style.getPropertyValue('--fluent-accent-foreground'),
                        sidebar:sidebar.width,
                        ctaWidth:cta.width,ctaHeight:cta.height,
                        loadWidth:load.width,loadHeight:load.height,
                        paneBackground:pane.backgroundColor,leftBackground:left.backgroundColor,
                        hintBorder:hs.borderTopStyle,hintBg:hs.backgroundColor,
                        hintHeight:hr.height,hintBelow:hr.top>=cta.bottom,
                        captionHeight:document.querySelector('.winui-titlebar').getBoundingClientRect().height,
                        navHeight:document.querySelector('.topbar').getBoundingClientRect().height,
                        iconWidth:document.querySelector('.winui-app-icon').getBoundingClientRect().width,
                        socialWidths:links.map(e=>e.getBoundingClientRect().width),
                        extraWidth:document.documentElement.scrollWidth>innerWidth+1,
                        buttonColor:getComputedStyle(document.querySelector('#pick-package-btn')).color,
                        themeLabelSize:getComputedStyle(document.querySelector('#theme-button-label')).fontSize,
                        languageCodeSize:getComputedStyle(document.querySelector('#lang-current-code')).fontSize,
                        samples:Array.from(document.querySelectorAll('.theme-option .theme-swatch'))
                          .map(s=>getComputedStyle(s).backgroundImage)
                      };
                    }""")
                    assert state["theme"]==theme,(width,theme,state)
                    assert state["themeLabelSize"]=="14px" and state["languageCodeSize"]=="14px",(width,theme,state)
                    assert len(state["samples"])==7 and all(s.startswith("linear-gradient(") for s in state["samples"]),(width,theme,state)
                    if width==1280 and theme in ("veiga-dark","mean-girls"):
                        page.locator("#theme-button").click()
                        page.screenshot(path=str(image_dir / f"theme-menu-{width}-{theme}.png"))
                        page.locator("#theme-button").click()
                    assert state["foreground"].lower()==FOREGROUNDS[theme],(width,theme,state)
                    assert state["customForeground"]=="",(width,theme,state)
                    for key in ["ctaHeight","loadHeight"]:
                        assert abs(state[key]-52)<1,(width,theme,state)
                    assert abs(state["ctaWidth"]-state["loadWidth"])<1,(width,theme,state)
                    assert abs(state["captionHeight"]-32)<1 and abs(state["navHeight"]-48)<1,(width,theme,state)
                    assert state["iconWidth"]==16 and not state["extraWidth"],(width,theme,state)
                    assert state["hintBelow"] and state["hintBorder"]=="dashed" and state["hintHeight"]>=90,(width,theme,state)
                    assert state["hintBg"] not in ("transparent","rgba(0, 0, 0, 0)"),(width,theme,state)
                    assert state["leftBackground"] in ("transparent","rgba(0, 0, 0, 0)"),(width,theme,state)
                    assert state["paneBackground"] not in ("transparent","rgba(0, 0, 0, 0)"),(width,theme,state)
                    assert max(state["socialWidths"])-min(state["socialWidths"])<1,(width,theme,state)
                    if theme=="veiga-dark":
                        page.screenshot(path=str(image_dir / f"splitter-{width}-{theme}.png"))
                    base=page.locator("#package-drop-hint").evaluate("e=>getComputedStyle(e).backgroundColor")
                    page.locator("#package-drop-zone").evaluate("e=>e.classList.add('drag-over')")
                    page.wait_for_timeout(110)
                    active=page.locator("#package-drop-hint").evaluate("e=>getComputedStyle(e).backgroundColor")
                    assert active!=base,(width,theme,base,active)
                    page.locator("#package-drop-zone").evaluate("e=>e.classList.remove('drag-over')")
                    # Merger: separate CTA/hint, matching geometry and accent states.
                    page.evaluate("""() => {
                      document.querySelector('#splitter-controls').classList.add('hidden');
                      document.querySelector('#merger-controls').classList.remove('hidden');
                      document.querySelector('#splitter-workspace').classList.add('hidden');
                      document.querySelector('#merger-workspace').classList.remove('hidden');
                    }""")
                    merged=page.evaluate("""() => {
                      const b=document.querySelector('#merger-add-packages-btn').getBoundingClientRect();
                      const h=document.querySelector('#merger-add-hint');
                      const rect=h.getBoundingClientRect();
                      return {w:b.width,h:b.height,hintBorder:getComputedStyle(h).borderTopStyle,
                        hintBelow:rect.top>=b.bottom};
                    }""")
                    assert abs(merged["h"]-52)<1 and abs(merged["w"]-state["ctaWidth"])<1,(width,theme,merged)
                    assert merged["hintBorder"]=="dashed" and merged["hintBelow"],(width,theme,merged)
                    if theme=="veiga-dark":
                        page.screenshot(path=str(image_dir / f"merger-{width}-{theme}.png"))
                    page.evaluate("""() => {
                      document.querySelector('#splitter-controls').classList.remove('hidden');
                      document.querySelector('#merger-controls').classList.add('hidden');
                      document.querySelector('#splitter-workspace').classList.remove('hidden');
                      document.querySelector('#merger-workspace').classList.add('hidden');
                    }""")
                    # Synthetic resources exercise the real CSS result layout
                    # without inventing or simulating Tauri native data.
                    page.evaluate("""() => {
                      document.getElementById('empty-state').classList.add('hidden');
                      document.getElementById('results-state').classList.remove('hidden');
                      const list=document.getElementById('resource-list');
                      list.replaceChildren();
                      for(let i=0;i<36;i++){
                        const row=document.createElement('div');
                        row.className='resource-row';
                        row.innerHTML='<div class="resource-check"><input type="checkbox"></div>'+
                          '<button class="resource-button">Resource '+i+'</button>'+
                          '<span class="type-pill">CASP</span>';
                        list.appendChild(row);
                      }
                    }""")
                    # A valid merged shows all four actions. Force only CSS
                    # visibility in this browser-only geometry fixture.
                    page.evaluate("""() => {
                      document.getElementById('remove-merged-btn').classList.remove('hidden');
                      document.getElementById('restore-manifest-btn').classList.remove('hidden');
                    }""")
                    controls=page.evaluate("""() => {
                      const rect=id=>document.getElementById(id).getBoundingClientRect();
                      const list=document.getElementById('resource-list');
                      const all=rect('select-all-btn'),none=rect('select-none-btn');
                      const filter=rect('content-filter-toggle-btn'),search=rect('search-input');
                      const previewCard=document.querySelector('#results-state .preview-card').getBoundingClientRect();
                      const listCard=document.querySelector('#results-state .list-card').getBoundingClientRect();
                      const footer=document.querySelector('#results-state .results-footer-actions').getBoundingClientRect();
                      const exportBtn=rect('export-btn'),clear=rect('clear-btn');
                      return {allH:all.height,noneH:none.height,allW:all.width,noneW:none.width,
                        allTop:all.top,noneTop:none.top,allLeft:all.left,allBottom:all.bottom,
                        filterTop:filter.top,filterH:filter.height,searchTop:search.top,searchH:search.height,
                        previewTop:previewCard.top,previewLeft:previewCard.left,
                        previewRight:previewCard.right,listBottom:listCard.bottom,
                        footerTop:footer.top,footerRight:footer.right,
                        exportH:exportBtn.height,clearH:clear.height,
                        exportW:exportBtn.width,clearW:clear.width,
                        scroll:list.scrollHeight>list.clientHeight,
                        listWidth:list.clientWidth,outerWidth:document.documentElement.scrollWidth};
                    }""")
                    assert controls["allH"]==36 and controls["noneH"]==36,(width,theme,controls)
                    assert abs(controls["allW"]-controls["noneW"])<1,(width,theme,controls)
                    assert controls["filterH"]==36 and controls["searchH"]==36,(width,theme,controls)
                    assert abs(controls["allTop"]-controls["filterTop"])<1,(width,theme,controls)
                    assert abs(controls["noneTop"]-controls["searchTop"])<1,(width,theme,controls)
                    assert controls["allBottom"]<=controls["previewTop"]+1,(width,theme,controls)
                    assert abs(controls["allLeft"]-controls["previewLeft"])<2,(width,theme,controls)
                    assert controls["footerTop"]>=controls["listBottom"]-1,(width,theme,controls)
                    assert controls["footerRight"]<=controls["previewLeft"],(width,theme,controls)
                    assert controls["exportH"]>=36 and controls["clearH"]>=36,(width,theme,controls)
                    assert abs(controls["exportW"]-controls["clearW"])<1,(width,theme,controls)
                    assert controls["listWidth"]>0 and controls["scroll"],(width,theme,controls)
                    assert controls["outerWidth"]<=width+1,(width,theme,controls)
                    # Screenshot the populated CC results, not just the empty
                    # welcome screen. Assert action alignment in the rendered UI.
                    positions=page.evaluate("""() => {
                      const rect=id=>document.getElementById(id).getBoundingClientRect();
                      const buttons=['export-btn','remove-merged-btn','restore-manifest-btn','clear-btn']
                        .filter(id=>getComputedStyle(document.getElementById(id)).display!=='none')
                        .map(id=>({id,top:rect(id).top,height:rect(id).height}));
                      const list=document.querySelector('.results-list-column').getBoundingClientRect();
                      const footer=document.querySelector('.results-footer-actions').getBoundingClientRect();
                      return {buttons,listWidth:list.width,footerWidth:footer.width,footerTop:footer.top};
                    }""")
                    assert len(positions["buttons"])==4,(width,theme,positions)
                    if positions["listWidth"]>=520:
                        assert max(b["top"] for b in positions["buttons"])-min(b["top"] for b in positions["buttons"])<2,(width,theme,positions)
                    assert positions["footerWidth"]<=positions["listWidth"]+1,(width,theme,positions)
                    if theme=="veiga-dark":
                        page.screenshot(path=str(image_dir / f"results-{width}-{theme}.png"))

                    # Actual busy state: centered, no card background, always moving.
                    page.evaluate("""() => {
                      document.getElementById('results-state').classList.add('hidden');
                      document.getElementById('busy-state').classList.remove('hidden');
                      document.getElementById('busy-title').textContent='Analisando dependências';
                      document.getElementById('busy-message').textContent='Verificando recursos';
                      document.getElementById('progress-bar').style.width='35%';
                    }""")
                    page.wait_for_timeout(120)
                    b=page.evaluate("""() => {
                      const work=document.getElementById('splitter-workspace').getBoundingClientRect();
                      const card=document.querySelector('#busy-state .progress-card');
                      const r=card.getBoundingClientRect(),s=getComputedStyle(card);
                      const bar=document.getElementById('progress-bar');
                      return {dx:Math.abs((r.left+r.width/2)-(work.left+work.width/2)),
                        dy:Math.abs((r.top+r.height/2)-(work.top+work.height/2)),
                        bg:s.backgroundColor,border:s.borderTopWidth,shadow:s.boxShadow,
                        name:getComputedStyle(bar).animationName,x:bar.getBoundingClientRect().x};
                    }""")
                    assert b["dx"]<3 and b["dy"]<3,(width,theme,b)
                    assert b["bg"] in ("transparent","rgba(0, 0, 0, 0)") and b["border"]=="0px" and b["shadow"]=="none",(width,theme,b)
                    assert b["name"]=="splitter-busy-marquee",(width,theme,b)
                    page.wait_for_timeout(350)
                    x=page.evaluate("document.getElementById('progress-bar').getBoundingClientRect().x")
                    assert abs(x-b["x"])>3,(width,theme,b,x)
                    if theme=="veiga-dark" and width==1280:
                        page.screenshot(path=str(image_dir / f"busy-1280-{theme}.png"))
                    font=page.evaluate("""() => ({
                      tema:getComputedStyle(document.getElementById('theme-button-label')).fontSize,
                      idioma:getComputedStyle(document.getElementById('lang-label')).fontSize,
                      code:getComputedStyle(document.getElementById('lang-current-code')).fontSize,
                      themeWeight:getComputedStyle(document.getElementById('theme-button-label')).fontWeight,
                      langWeight:getComputedStyle(document.getElementById('lang-label')).fontWeight
                    })""")
                    assert font["tema"]==font["idioma"]==font["code"]=="14px",(width,theme,font)
                    assert font["themeWeight"]!=font["langWeight"],(width,theme,font)
                    page.evaluate("""() => {
                      document.getElementById('busy-state').classList.add('hidden');
                      document.getElementById('results-state').classList.remove('hidden');
                    }""")
                    # All modals keep theme surface/text and remain in viewport.
                    modal=page.evaluate("""() => {
                      const overlay=document.getElementById('app-modal');
                      overlay.classList.remove('hidden');
                      overlay.setAttribute('aria-hidden','false');
                      const dialog=document.getElementById('app-modal-panel');
                      const s=getComputedStyle(dialog),r=dialog.getBoundingClientRect();
                      const root=getComputedStyle(document.documentElement);
                      return {background:s.backgroundColor,color:s.color,
                        token:root.getPropertyValue('--winui-text-primary').trim(),
                        x:r.x,right:r.right,y:r.y,bottom:r.bottom};
                    }""")
                    assert modal["background"] not in ("transparent","rgba(0, 0, 0, 0)"),(width,theme,modal)
                    assert modal["x"]>=0 and modal["right"]<=width+1,(width,theme,modal)
                    assert modal["y"]>=0 and modal["bottom"]<=height+1,(width,theme,modal)
                    # Cancel and Remove: equal, narrow and centered.
                    pair=page.evaluate("""() => {
                      const c=document.getElementById('app-modal-cancel'),
                        p=document.getElementById('app-modal-primary');
                      c.classList.remove('hidden');c.textContent='Cancelar';p.textContent='Remover';
                      const a=document.querySelector('.app-modal-actions').getBoundingClientRect(),
                        cr=c.getBoundingClientRect(),pr=p.getBoundingClientRect();
                      return {cw:cr.width,pw:pr.width,center:(cr.left+pr.right)/2,
                        target:a.left+a.width/2,dy:Math.abs(cr.top-pr.top)};
                    }""")
                    assert abs(pair["cw"]-120)<1 and abs(pair["pw"]-120)<1,(width,theme,pair)
                    assert abs(pair["center"]-pair["target"])<2 and pair["dy"]<2,(width,theme,pair)
                    if width==1280 and theme=="veiga-dark":
                        page.screenshot(path=str(image_dir / f"modal-pair-1280-{theme}.png"))
                    ok=page.evaluate("""() => {
                      document.getElementById('app-modal-cancel').classList.add('hidden');
                      const p=document.getElementById('app-modal-primary');p.textContent='OK';
                      const pr=p.getBoundingClientRect(),a=document.querySelector('.app-modal-actions').getBoundingClientRect();
                      return {width:pr.width,offset:Math.abs(pr.left+pr.width/2-a.left-a.width/2)};
                    }""")
                    assert abs(ok["width"]-120)<1 and ok["offset"]<2,(width,theme,ok)
                    if width==1280 and theme=="veiga-dark":
                        page.screenshot(path=str(image_dir / f"modal-ok-1280-{theme}.png"))
                    page.evaluate("""() => {
                      document.getElementById('app-modal').classList.add('hidden');
                      document.getElementById('results-state').classList.add('hidden');
                      document.getElementById('empty-state').classList.remove('hidden');
                    }""")
                    print("PASS",width,height,theme,flush=True)
                assert not errors,(width,errors)
                page.close()
        finally:
            browser.close()

if __name__=="__main__":
    run()
