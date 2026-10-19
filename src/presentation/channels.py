"""WebSocket transports for the existing Prim LSP and real PTY processes."""
from __future__ import annotations
import asyncio
import contextlib
import fcntl
import json
import os
from pathlib import Path
import pty
import shutil
import signal
import struct
import sys
import tempfile
import termios
import time

from aiohttp import web, WSMsgType

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


async def stop_process(process):
    if process.returncode is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            with contextlib.suppress(ProcessLookupError):
                process.terminate()
        try:
            await asyncio.wait_for(process.wait(), .7)
        except asyncio.TimeoutError:
            with contextlib.suppress(ProcessLookupError):
                os.killpg(process.pid, signal.SIGKILL)
            with contextlib.suppress(ProcessLookupError):
                process.kill()
            await process.wait()


async def language_channel(request, app):
    if len(app.lsp_connections) >= 4:
        raise web.HTTPTooManyRequests(text='语言服务连接数达到上限')
    ws = web.WebSocketResponse(max_msg_size=1024**2, heartbeat=25)
    await ws.prepare(request)
    app.lsp_connections.add(ws)
    process = None
    tasks = []
    try:
        node = shutil.which('node')
        if not node:
            await ws.send_json({'jsonrpc':'2.0','method':'window/showMessage','params':{'type':1,'message':'Node.js 不可用'}})
            return ws
        process = await asyncio.create_subprocess_exec(node, str(ROOT/'src/prim-language-server/server/server.js'), '--stdio',
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE,
            limit=2*1024**2, start_new_session=True, cwd=ROOT)

        async def incoming():
            async for message in ws:
                if message.type != WSMsgType.TEXT:
                    if message.type in (WSMsgType.CLOSE,WSMsgType.ERROR): break
                    continue
                parsed = json.loads(message.data)
                if not isinstance(parsed, dict) or parsed.get('jsonrpc') != '2.0':
                    continue
                data = json.dumps(parsed,ensure_ascii=False).encode()
                process.stdin.write(f'Content-Length: {len(data)}\r\n\r\n'.encode()+data)
                await process.stdin.drain()

        async def outgoing():
            while True:
                header = await process.stdout.readuntil(b'\r\n\r\n')
                length = next((int(line.split(b':',1)[1]) for line in header.split(b'\r\n') if line.lower().startswith(b'content-length:')),0)
                if not 0 < length <= 1024**2: raise ValueError('LSP 消息过大')
                body = await process.stdout.readexactly(length)
                await ws.send_str(body.decode())

        async def diagnostics():
            while line := await process.stderr.readline():
                await ws.send_json({'jsonrpc':'2.0','method':'window/logMessage','params':{'type':2,'message':line[:4000].decode(errors='replace')}})

        tasks = [asyncio.create_task(f()) for f in (incoming,outgoing,diagnostics)]
        done,_ = await asyncio.wait(tasks,return_when=asyncio.FIRST_COMPLETED)
        for task in done:
            if not task.cancelled() and task.exception() and not isinstance(task.exception(),asyncio.IncompleteReadError):
                raise task.exception()
    except (OSError, ValueError, ConnectionError) as error:
        if not ws.closed:
            await ws.send_json({'jsonrpc':'2.0','method':'window/showMessage','params':{'type':1,'message':str(error)}})
    finally:
        for task in tasks: task.cancel()
        await asyncio.gather(*tasks,return_exceptions=True)
        if process: await stop_process(process)
        app.lsp_connections.discard(ws)
        await ws.close()
    return ws


async def terminal_channel(request, app):
    if not app.runner.status()['available']:
        raise web.HTTPServiceUnavailable(text=app.runner.problem)
    if len(app.terminal_connections) >= 2:
        raise web.HTTPTooManyRequests(text='已有两个交互会话，请先关闭其中一个')
    ws = web.WebSocketResponse(max_msg_size=128*1024,heartbeat=20)
    await ws.prepare(request)
    app.terminal_connections.add(ws)
    process = None
    master = slave = None
    tasks = []
    loop = asyncio.get_running_loop()
    reason = 'exited'
    try:
        first = await asyncio.wait_for(ws.receive_json(),10)
        if not isinstance(first,dict): raise ValueError('会话启动参数无效')
        mode = first.get('mode','jit')
        if mode not in ('jit','interpreter'): raise ValueError('交互模式只支持解释或 JIT')
        scenario = first.get('scenario','code')
        code = first.get('code','')
        if not isinstance(code,str) or len(code.encode()) > 64*1024: raise ValueError('代码超出 64 KiB 限制')
        if not code.strip(): raise ValueError('代码不能为空')
        cols, rows = int(first.get('cols',100)), int(first.get('rows',30))
        cols, rows = max(20,min(240,cols)),max(8,min(80,rows))
        app.runner.work_dir.mkdir(parents=True,exist_ok=True)
        with tempfile.TemporaryDirectory(prefix='terminal-',dir=app.runner.work_dir) as directory:
            source = Path(directory)/'demo.pr'
            source.write_text(code)
            if scenario == 'orders':
                shutil.copyfile(HERE/'scenarios/orders.csv',Path(directory)/'orders.csv')
            flags = ['--jit-log','--stats']
            # --fuel disables the JIT scalar path; the PTY session has its own time and output limits.
            if scenario not in ('snake','jit','jit-native'): flags += ['--fuel','50000000']
            if mode == 'interpreter': flags += ['--jit=off']
            executable = str(app.runner.binary)
            runtime_label = app.runner.version
            master,slave = pty.openpty()
            fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',rows,cols,0,0))
            script_args=['--','--inspect'] if scenario=='jit-native' else []
            process = await asyncio.create_subprocess_exec(sys.executable,str(HERE/'terminal_child.py'),executable,*flags,str(source),*script_args,
                stdin=slave,stdout=slave,stderr=slave,cwd=directory,env={**os.environ,'TERM':'xterm-256color','COLORTERM':'truecolor'})
            os.close(slave); slave=None
            os.set_blocking(master,False)
            queue = asyncio.Queue(maxsize=16)
            paused = False
            ended = False
            pending = 0
            acknowledged = asyncio.Event()
            acknowledged.set()
            total = 0
            def readable():
                nonlocal paused,ended,total
                if queue.full():
                    loop.remove_reader(master); paused=True; return
                try: chunk=os.read(master,16384)
                except BlockingIOError:return
                except OSError:chunk=b''
                if not chunk:
                    ended=True; loop.remove_reader(master); queue.put_nowait(None); return
                total += len(chunk)
                queue.put_nowait(chunk)
            loop.add_reader(master,readable)
            await ws.send_json({'kind':'started','runtime':runtime_label,'scenario':scenario,'timeout_seconds':180})
            async def output():
                nonlocal paused,pending
                while True:
                    chunk=await queue.get()
                    if paused and not ended:
                        loop.add_reader(master,readable); paused=False
                    if chunk is None: return
                    if total > 16*1024**2: raise ValueError('交互输出达到 16 MiB 上限')
                    await ws.send_bytes(chunk)
                    pending += len(chunk)
                    while pending > 64*1024:
                        acknowledged.clear()
                        await asyncio.wait_for(acknowledged.wait(),10)
            async def incoming():
                nonlocal pending,reason
                async for msg in ws:
                    if msg.type != WSMsgType.TEXT: continue
                    value=json.loads(msg.data)
                    if value.get('kind') == 'ack':
                        pending=max(0,pending-max(0,min(65536,int(value.get('bytes',0)))))
                        acknowledged.set()
                    elif value.get('kind') == 'input':
                        payload=value.get('data','')
                        if isinstance(payload,str) and len(payload.encode()) <= 4096:
                            with contextlib.suppress(BlockingIOError,OSError):os.write(master,payload.encode())
                    elif value.get('kind') == 'resize':
                        c=max(20,min(240,int(value.get('cols',80))));r=max(8,min(80,int(value.get('rows',24))))
                        fcntl.ioctl(master,termios.TIOCSWINSZ,struct.pack('HHHH',r,c,0,0))
                    elif value.get('kind') == 'stop':
                        reason='stopped';return
                reason='disconnected'
            output_task=asyncio.create_task(output());input_task=asyncio.create_task(incoming());wait_task=asyncio.create_task(process.wait())
            tasks=[output_task,input_task,wait_task]
            done,_=await asyncio.wait(tasks,timeout=180,return_when=asyncio.FIRST_COMPLETED)
            if not done: reason='timeout'
            if wait_task in done:
                with contextlib.suppress(asyncio.TimeoutError):await asyncio.wait_for(asyncio.shield(output_task),1)
            for task in done:
                if task.exception():raise task.exception()
            await stop_process(process)
            report = Path(directory)/'order-report.txt'
            if not ws.closed and report.is_file() and report.stat().st_size <= 65536:
                await ws.send_json({'kind':'artifact','name':report.name,'text':report.read_text()})
            if not ws.closed:await ws.send_json({'kind':'exit','code':process.returncode,'reason':reason})
    except (ValueError,TypeError,OSError,ConnectionError,asyncio.TimeoutError) as error:
        if not ws.closed:
            with contextlib.suppress(ConnectionError):await ws.send_json({'kind':'error','message':str(error) or '会话超时'})
    finally:
        if master is not None:loop.remove_reader(master)
        for task in tasks:task.cancel()
        await asyncio.gather(*tasks,return_exceptions=True)
        if process:await stop_process(process)
        for fd in (master,slave):
            if fd is not None:
                with contextlib.suppress(OSError):os.close(fd)
        app.terminal_connections.discard(ws)
        await ws.close()
    return ws
