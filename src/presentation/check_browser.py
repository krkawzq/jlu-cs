"""Verify the actual extension bridge, PTY interaction and slide export layout."""
import hashlib
import json
from pathlib import Path
import time

from export import inspect_layout,launch_browser,preview_server
from playwright.sync_api import sync_playwright,expect
from server import ROOT,default_runtime


def main():
    output=ROOT/'build/presentation/browser-check';output.mkdir(parents=True,exist_ok=True)
    evidence=ROOT/'build/presentation/evidence';evidence.mkdir(parents=True,exist_ok=True)
    errors=[]
    with preview_server(ROOT/'docs/class-presentation.md',runtime=default_runtime()) as (url,app):
        with sync_playwright() as playwright,launch_browser(playwright) as browser:
            page=browser.new_page(viewport={'width':1600,'height':1000},reduced_motion='reduce')
            page.on('pageerror',lambda error:(print('BROWSER ERROR',error.stack,flush=True),errors.append(str(error))))
            page.goto(url)
            page.wait_for_function('() => window.PrimPresentation?.ready === true')
            assert page.locator('.code-panel').count()>=12
            page.keyboard.press('ArrowRight');expect(page.locator('.slide.active')).to_have_attribute('data-slide','02')
            # Stepped copy: the first press reveals the question, the second moves on.
            expect(page.locator('.slide.active .ask')).to_have_class('ask pending')
            page.keyboard.press('ArrowRight');expect(page.locator('.slide.active')).to_have_attribute('data-slide','02')
            expect(page.locator('.slide.active .ask')).not_to_have_class('ask pending')
            page.keyboard.press('ArrowRight');expect(page.locator('.slide.active')).to_have_attribute('data-slide','03')
            if app.runner.status()['available']:
                page.evaluate('window.PrimPresentation.show("13")')
                expect(page.locator('.slide.active [data-stage="机器码"] .hot').first).to_contain_text('imul')
            page.keyboard.press('o');page.locator('[data-page="10"]').click()
            page.locator('.slide.active .demo-link').click()
            expect(page.locator('#lab')).to_be_visible()
            expect(page.locator('.tab[aria-selected="true"]')).to_have_attribute('data-demo','accumulator')
            page.wait_for_function('() => !!window.PrimEditor.debug().client?.capabilities')
            print('LSP connected',flush=True)
            # Diagnostics and completion must come from the existing language server.
            page.evaluate('window.PrimEditor.setValue("let x = missing;")')
            page.wait_for_function('() => window.PrimEditor.debug().monaco.editor.getModelMarkers({owner:"prim-lsp"}).some(m => m.message.includes("missing"))')
            completions=page.evaluate('''async () => {
                const d=window.PrimEditor.debug(); d.editor.setValue('std.');
                return await d.client.request('textDocument/completion',{textDocument:{uri:d.editor.getModel().uri.toString()},position:{line:0,character:4}});
            }''')
            assert any(item['label']=='print' for item in completions['items'])
            page.evaluate('''() => {
                const d=window.PrimEditor.debug();d.editor.focus();
                d.editor.setPosition({lineNumber:1,column:5});
                d.editor.trigger('test','editor.action.triggerSuggest',{});
            }''')
            expect(page.locator('.suggest-widget.visible')).to_be_visible()
            page.keyboard.press('Escape')
            definition=page.evaluate('''async () => {
                const d=window.PrimEditor.debug();d.editor.setValue('std.print(1);');
                const v=await d.client.request('textDocument/definition',{textDocument:{uri:d.editor.getModel().uri.toString()},position:{line:0,character:6}});
                const model=await d.client.loadDocument(v.uri);
                return {uri:v.uri,text:model.getValue()};
            }''')
            assert definition['uri'].endswith('/std.pri') and 'std' in definition['text']
            renamed=page.evaluate('''async () => {
                const d=window.PrimEditor.debug();d.editor.setValue('let n = 1; std.print(n);');
                return await d.client.request('textDocument/rename',{textDocument:{uri:d.editor.getModel().uri.toString()},position:{line:0,character:4},newName:'amount'});
            }''')
            assert len(renamed['documentChanges'][0]['edits'])==2
            assert page.evaluate('document.fonts.check(\'16px "JetBrains Mono"\')')
            print('LSP diagnostics, completion, builtin navigation, rename and font: PASS',flush=True)
            if app.runner.status()['available']:
                for name,expected_output in [('closures','42\n3\nfunction\nobject'),('slots','2\n1\n2'),('capture','200\n300'),('accumulator','500'),('lifetime','200\n500'),('jit-shared','uncompiled\n500\nready\ntrue'),('pipeline','imul')]:
                    page.locator(f'.tab[data-demo="{name}"]').click();page.locator('#reset-demo').click();page.locator('#run').click()
                    page.wait_for_function('() => document.querySelector("#run-status").textContent.includes("退出 0")')
                    actual=page.evaluate('window.PrimEditor.getTerminalText()').replace('\r','')
                    assert expected_output.replace('\r','') in actual,(name,actual)
                print('Short real runtime cases: PASS',flush=True)
                for name in ('orders','orders-python','jit'):
                    page.locator(f'.tab[data-demo="{name}"]').click();page.locator('#reset-demo').click()
                    if name!='orders-python':page.locator('[data-mode="jit"]').click()
                    page.locator('#run').click()
                    page.wait_for_function('() => document.querySelector("#run-status").textContent.includes("退出 0")',timeout=20000)
                    terminal=page.evaluate('window.PrimEditor.getTerminalText()')
                    assert 'checksum=' in terminal,(name,terminal)
                    if name.startswith('orders'):assert f"total_cents={app.evidence()['expected']['total_cents']}" in terminal
                    if name=='orders':
                        expect(page.locator('#artifact')).to_be_visible()
                        page.screenshot(path=str(output/'orders.png'))
                    if name=='jit':assert 'native_calls=' in terminal
                print('Prim/Python orders and live JIT PTY: PASS',flush=True)
                page.locator('.tab[data-demo="snake"]').click();page.locator('#reset-demo').click();page.locator('#run').click()
                page.wait_for_function('() => window.PrimEditor.getTerminalText().includes("Prim Snake")',timeout=10000)
                page.evaluate('window.PrimEditor.debug().terminal.focus()')
                page.keyboard.press('p')
                page.wait_for_function('() => /PAUSED|Paused|paused/.test(window.PrimEditor.getTerminalText())')
                page.screenshot(path=str(output/'snake-workspace.png'))
                page.locator('#terminal').screenshot(path=str(evidence/'snake.png'))
                snapshot={'runtime':app.runner.version,'source_sha256':hashlib.sha256((ROOT/'src/prim-rs/examples/snake.pr').read_bytes()).hexdigest(),'record':'Real Prim process, PTY input p, xterm screenshot in paused state'}
                (evidence/'snake.json').write_text(json.dumps(snapshot,indent=2)+'\n')
                head_row=page.evaluate('window.PrimEditor.getTerminalText().split("\\n").findIndex(line=>line.includes("@@"))')
                page.keyboard.press('r')
                page.keyboard.press('ArrowUp')
                page.wait_for_function(r'''row => {
                    const current=window.PrimEditor.getTerminalText().split('\n').findIndex(line=>line.includes('@@'));
                    return current>=0 && current<row;
                }''',arg=head_row,timeout=3000)
                page.keyboard.press('p');page.keyboard.press('q')
                page.wait_for_function('() => document.querySelector("#run-status").textContent.includes("退出 0")',timeout=10000)
                page.locator('#run').click()
                page.wait_for_function('() => window.PrimEditor.getTerminalText().includes("Prim Snake")')
                page.locator('#stop').click()
                page.wait_for_function('() => document.querySelector("#run-status").textContent.includes("stopped")')
                page.wait_for_function('async () => (await fetch("/api/status").then(r=>r.json())).terminal_sessions === 0')
                print('Snake keyboard, pause, restart and exit: PASS',flush=True)
            page.set_viewport_size({'width':1600,'height':1000})
            for slide in app.deck()['slides']:
                page.evaluate('(id) => window.PrimPresentation.show(id)',slide['id'])
                problems=inspect_layout(page)
                assert not problems,('lab-open',slide['id'],problems)
            page.screenshot(path=str(output/'workbench-docked.png'))
            page.locator('#lab-close').click()
            expect(page.locator('#lab')).to_be_hidden()
            # Re-read the captured game record before exporting the slides.
            page.reload();page.wait_for_function('() => window.PrimPresentation?.ready === true')
            for width,height in ((1600,1000),(1280,800),(768,1024),(390,844)):
                page.set_viewport_size({'width':width,'height':height})
                for slide in app.deck()['slides']:
                    page.evaluate('(id) => window.PrimPresentation.show(id)',slide['id'])
                    problems=inspect_layout(page)
                    assert not problems,(width,slide['id'],problems)
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'),width
            page.set_viewport_size({'width':1600,'height':1000});page.evaluate('window.PrimPresentation.show("15")')
            page.screenshot(path=str(output/'benchmark-slide.png'))
            assert not errors,errors
    receipt={'slides':len(app.deck()['slides']),'viewports':[[1600,1000],[1280,800],[768,1024],[390,844]],'lsp':['diagnostics','completion','definition','rename'],'font':'JetBrains Mono loaded','pty':['Prim','CPython','snake'],'workbench':'docked right, tabbed','browser_errors':errors}
    (output/'result.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print('PASS:',json.dumps(receipt,ensure_ascii=False))


if __name__=='__main__':main()
