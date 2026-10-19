"""Presentation HTTP service, LSP bridge, real terminal sessions, and evidence."""
from __future__ import annotations
import argparse
import asyncio
from dataclasses import asdict
import hashlib
import json
import mimetypes
import os
from pathlib import Path
import secrets
import socket
import threading
from urllib.parse import urlsplit,unquote
from aiohttp import web

from content import read_outline
from runner import Runner,BusyError,HERE,ROOT,MAX_CODE
from channels import language_channel,terminal_channel


class Application:
    def __init__(self,outline,runtime,presenter='',course='',date=''):
        self.outline=Path(outline)
        self.token=secrets.token_urlsafe(32)
        self.presenter={'name':presenter,'course':course,'date':date}
        self.runner=Runner(runtime,ROOT/'build/presentation/runs')
        self.lsp_connections=set()
        self.terminal_connections=set()
        self.benchmark_lock=asyncio.Lock()
        read_outline(self.outline)

    def deck(self):
        metadata,slides=read_outline(self.outline)
        demos=json.loads((HERE/'demos.json').read_text())
        for demo in demos:
            if 'file' in demo:demo['code']=(ROOT/demo['file']).read_text()
        return {'metadata':metadata,'slides':[asdict(slide) for slide in slides],
            'source_sha256':hashlib.sha256(self.outline.read_bytes()).hexdigest(),
            'presenter':self.presenter,'demos':demos,
            'workspace_uri':(ROOT/'build/presentation/workspace').as_uri(),
            'extension':{'name':'Prim Language','version':json.loads((ROOT/'src/prim-language-server/package.json').read_text())['version']}}

    def authorize(self,request):
        origin=request.headers.get('Origin')
        if origin and urlsplit(origin).netloc != request.host:raise web.HTTPForbidden(text='只接受同源请求')
        token=request.headers.get('X-Prim-Token') or request.query.get('token','')
        if not secrets.compare_digest(token,self.token):raise web.HTTPForbidden(text='会话已变更，请刷新页面后重试')

    def evidence(self):
        path=ROOT/'build/presentation/evidence/benchmarks.json'
        if not path.exists():return {'available':False,'message':'尚无已完成的测量；可在实战页运行测量。'}
        data=json.loads(path.read_text())
        data['available']=True
        checks={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (HERE/'scenarios/orders.csv',HERE/'scenarios/orders.pr',HERE/'scenarios/jit.pr')}
        data['source_matches']=checks==data.get('source_sha256')
        data['runtime_matches']=bool(self.runner.binary and hashlib.sha256(self.runner.binary.read_bytes()).hexdigest()==data.get('runtime_sha256'))
        return data

    def native_evidence(self):
        path=ROOT/'build/presentation/evidence/native.json'
        if not path.exists():return {'available':False,'message':'尚未通过 builtin 导出机器码。'}
        data=json.loads(path.read_text())
        data['source_matches']=hashlib.sha256((HERE/'scenarios/jit.pr').read_bytes()).hexdigest()==data['source_sha256']
        data['runtime_matches']=bool(self.runner.binary and hashlib.sha256(self.runner.binary.read_bytes()).hexdigest()==data['runtime_sha256'])
        return data

    async def handle(self,request):
        path=request.path
        if path=='/':
            html=(HERE/'static/index.html').read_text().replace('__TOKEN__',self.token)
            manifest=ROOT/'build/presentation-web/.vite/manifest.json'
            if manifest.exists():
                values=json.loads(manifest.read_text())
                css=values.get('web/editor.js',{}).get('css',[])
                html=html.replace('<!-- EDITOR_CSS -->',''.join(f'<link rel="stylesheet" href="/assets/{name}">' for name in css))
            return web.Response(text=html,content_type='text/html')
        if path=='/api/deck':return web.json_response(self.deck())
        if path=='/api/status':return web.json_response({**self.runner.status(),'lsp':bool(__import__('shutil').which('node')),'terminal_sessions':len(self.terminal_connections)})
        if path=='/api/evidence':return web.json_response(self.evidence())
        if path=='/api/native':return web.json_response(self.native_evidence())
        if path=='/api/extension':
            ext=ROOT/'src/prim-language-server'
            return web.json_response({'manifest':json.loads((ext/'package.json').read_text()),'configuration':json.loads((ext/'language-configuration.json').read_text()),
                'grammars':{name:json.loads((ext/'syntaxes'/name).read_text()) for name in ('prim.tmLanguage.json','pri.tmLanguage.json')}})
        if path=='/api/document':
            self.authorize(request)
            uri=request.query.get('uri','')
            parsed=urlsplit(uri)
            if parsed.scheme!='file' or parsed.netloc:raise web.HTTPForbidden(text='不允许的文档 URI')
            file=Path(unquote(parsed.path)).resolve()
            allowed=[(ROOT/'builtin').resolve(),(ROOT/'src/prim-language-server/builtin').resolve()]
            if not any(file.is_relative_to(base) for base in allowed) or file.suffix!='.pri':raise web.HTTPForbidden(text='只提供内置接口文档')
            return web.json_response({'uri':uri,'text':file.read_text(),'language':'prim-interface'})
        if path.startswith('/assets/'):
            base=(ROOT/'build/presentation-web').resolve()
            file=(base/path.removeprefix('/assets/')).resolve()
            if not file.is_relative_to(base) or not file.is_file():raise web.HTTPNotFound(text='前端资源缺失，请先执行 npm run build')
            return web.Response(body=file.read_bytes(),content_type=mimetypes.guess_type(str(file))[0] or 'application/octet-stream')
        if path=='/static/logo.svg':return web.Response(body=(ROOT/'icons/logo.svg').read_bytes(),content_type='image/svg+xml')
        if path.startswith('/static/'):
            base=(HERE/'static').resolve();file=(HERE/path.lstrip('/')).resolve()
            if not file.is_relative_to(base) or not file.is_file():raise web.HTTPNotFound()
            return web.Response(body=file.read_bytes(),content_type=mimetypes.guess_type(str(file))[0] or 'application/octet-stream')
        if path.startswith('/evidence/'):
            base=(ROOT/'build/presentation/evidence').resolve();file=(base/path.removeprefix('/evidence/')).resolve()
            if not file.is_relative_to(base) or file.suffix not in ('.png','.json') or not file.is_file():raise web.HTTPNotFound()
            return web.Response(body=file.read_bytes(),content_type=mimetypes.guess_type(str(file))[0] or 'application/octet-stream')
        raise web.HTTPNotFound(text='页面不存在')

    async def post(self,request):
        self.authorize(request)
        if request.path=='/api/stop':return web.json_response({'stopped':self.runner.cancel()})
        if request.path=='/api/native':
            if self.benchmark_lock.locked() or self.terminal_connections:raise web.HTTPConflict(text='请先结束其他实战会话')
            if not self.runner.status()['available']:raise ValueError(self.runner.problem)
            async with self.benchmark_lock:
                from native import capture_native
                result=await asyncio.to_thread(capture_native,self.runner.binary,ROOT/'build/presentation/evidence')
                return web.json_response(result)
        if request.path=='/api/benchmark':
            if self.benchmark_lock.locked() or self.terminal_connections:raise web.HTTPConflict(text='请先结束其他实战会话')
            if not self.runner.status()['available']:raise ValueError(self.runner.problem)
            async with self.benchmark_lock:
                from benchmark import measure
                result=await asyncio.to_thread(measure,self.runner.binary,ROOT/'build/presentation/evidence',True)
                return web.json_response(result)
        if request.path!='/api/run':raise web.HTTPNotFound()
        value=await request.json()
        if not isinstance(value,dict):raise ValueError('请求必须为 JSON 对象')
        code,mode=value.get('code'),value.get('mode','interpreter')
        if not isinstance(code,str) or not code.strip() or len(code.encode())>MAX_CODE:raise ValueError('代码不能为空，且不能超过 32 KiB')
        if mode not in ('interpreter','jit','check'):raise ValueError('未知执行模式')
        return web.json_response(await asyncio.to_thread(self.runner.run,code,mode))

    async def websocket(self,request):
        self.authorize(request)
        if request.path=='/ws/lsp':return await language_channel(request,self)
        return await terminal_channel(request,self)

    async def close(self):
        self.runner.cancel()
        for ws in list(self.lsp_connections|self.terminal_connections):await ws.close(code=1001,message=b'server shutdown')

    def aiohttp_app(self):
        @web.middleware
        async def errors(request,handler):
            try:response=await handler(request)
            except web.HTTPException as error:
                response=web.json_response({'error':error.text or error.reason},status=error.status)
            except BusyError as error:response=web.json_response({'error':str(error)},status=409)
            except (ValueError,OSError) as error:response=web.json_response({'error':str(error)},status=400)
            if not isinstance(response,web.WebSocketResponse):
                response.headers['Cache-Control']='no-store'
                response.headers['X-Content-Type-Options']='nosniff'
                response.headers['Content-Security-Policy']="default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; worker-src 'self' blob:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'"
            return response
        application=web.Application(middlewares=[errors],client_max_size=MAX_CODE*6+1024)
        application.router.add_get('/ws/lsp',self.websocket)
        application.router.add_get('/ws/terminal',self.websocket)
        application.router.add_post('/api/{name}',self.post)
        application.router.add_get('/{path:.*}',self.handle)
        application.on_shutdown.append(lambda _:self.close())
        return application


class PresentationServer:
    """Small lifecycle adapter shared by CLI, screenshot export, and tests."""
    def __init__(self,address,app):
        self.app=app
        self.socket=socket.socket()
        self.socket.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
        self.socket.bind(address);self.socket.listen(128);self.socket.setblocking(False)
        self.server_port=self.socket.getsockname()[1]
        self.loop=None;self.done=None;self.ready=threading.Event();self.closed=threading.Event()
    async def _serve(self):
        self.loop=asyncio.get_running_loop();self.done=asyncio.Event()
        runner=web.AppRunner(self.app.aiohttp_app(),access_log=None,shutdown_timeout=2)
        await runner.setup()
        await web.SockSite(runner,self.socket).start()
        self.ready.set()
        try:await self.done.wait()
        finally:await runner.cleanup()
    def serve_forever(self):
        try:asyncio.run(self._serve())
        except KeyboardInterrupt:pass
        finally:self.closed.set()
    def shutdown(self):
        if self.ready.wait(5) and self.loop and self.loop.is_running():self.loop.call_soon_threadsafe(self.done.set)
    def server_close(self):
        self.app.runner.cancel()
        if self.loop and self.loop.is_running():
            self.shutdown()
            self.closed.wait(5)
        self.socket.close()


def default_runtime():
    for path in (ROOT/'build/prim-rs/prim',ROOT/'target/release/prim',ROOT/'target/debug/prim'):
        if path.is_file() and os.access(path,os.X_OK):return path.resolve()
    return None


def main():
    parser=argparse.ArgumentParser(description='Prim 交互式课堂展示、语言服务与终端')
    parser.add_argument('--host',default='127.0.0.1');parser.add_argument('--port',type=int,default=8765)
    parser.add_argument('--outline',type=Path,default=ROOT/'docs/class-presentation.md')
    parser.add_argument('--runtime',type=Path,default=default_runtime());parser.add_argument('--no-runtime',action='store_true')
    for key in ('presenter','course','date'):parser.add_argument('--'+key,default='')
    args=parser.parse_args()
    runtime=args.runtime.resolve() if args.runtime and not args.no_runtime else None
    app=Application(args.outline.resolve(),runtime,args.presenter,args.course,args.date)
    server=PresentationServer((args.host,args.port),app)
    print(f'Prim 展示：http://{args.host}:{server.server_port}\n运行时：{app.runner.version or app.runner.problem}',flush=True)
    try:server.serve_forever()
    finally:server.server_close()


if __name__=='__main__':main()
