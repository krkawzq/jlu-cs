import sys
from export import launch_browser, preview_server, inspect_layout
from server import ROOT, default_runtime
from playwright.sync_api import sync_playwright
out=sys.argv[1]
with preview_server(ROOT/'docs/class-presentation.md',runtime=default_runtime()) as (url,app):
    with sync_playwright() as p, launch_browser(p) as browser:
        page=browser.new_page(viewport={'width':1600,'height':960},device_scale_factor=1)
        errs=[];page.on('pageerror',lambda e:errs.append(str(e)));page.on('console',lambda m:m.type=='error' and errs.append(m.text))
        page.goto(url);page.wait_for_function('() => window.PrimPresentation?.ready === true',timeout=60000)
        for s in app.deck()['slides']:
            page.evaluate('(id)=>window.PrimPresentation.show(id,{step:"all"})',s['id'])
            page.wait_for_timeout(500)
            pr=inspect_layout(page)
            if pr:print('LAYOUT',s['id'],pr)
            page.screenshot(path=f"{out}/s{s['id']}.png")
        page.evaluate('()=>window.PrimPresentation.show("10")')
        page.locator('.slide.active .demo-link').click()
        page.wait_for_function('() => !!window.PrimEditor.debug().client?.capabilities',timeout=30000)
        page.locator('#run').click();page.wait_for_timeout(2500)
        page.screenshot(path=f"{out}/lab10.png")
        page.evaluate('()=>window.PrimPresentation.show("13",{step:"all"})');page.wait_for_timeout(800)
        page.locator('#run').click();page.wait_for_timeout(2500)
        page.screenshot(path=f"{out}/lab13.png")
        page.evaluate('()=>window.PrimPresentation.show("02")');page.wait_for_timeout(600)
        page.screenshot(path=f"{out}/lab02-step0.png")
        print('errors',errs)
