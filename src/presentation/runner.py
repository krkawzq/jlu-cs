"""Local presentation server and bounded adapter to the real Prim CLI."""
from __future__ import annotations

import argparse
from dataclasses import asdict
import hashlib
import json
import os
from pathlib import Path
import secrets
import selectors
import signal
import subprocess
import sys
import tempfile
import threading
import time


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
MAX_CODE = 32 * 1024
MAX_OUTPUT = 64 * 1024


class BusyError(Exception):
    pass


class Runner:
    def __init__(self, binary: Path | None, work_dir: Path, timeout: float = 6):
        self.binary = binary
        self.work_dir = work_dir
        self.timeout = timeout
        self.gate = threading.Lock()
        self.lock = threading.Lock()
        self.active: subprocess.Popen | None = None
        self.cancelled = False
        self.version = ''
        self.problem = ''
        if binary:
            try:
                version = subprocess.run([str(binary), '--version'], capture_output=True, text=True, timeout=2)
                if version.returncode:
                    self.problem = version.stderr.strip()[:1000] or 'Prim 执行程序无法启动'
                else:
                    self.version = version.stdout.strip()[:1000]
            except (OSError, subprocess.SubprocessError) as error:
                self.problem = str(error)
        else:
            self.problem = '尚未找到 Prim 执行程序；使用 --runtime 指定已有可执行文件。'

    def status(self):
        return {'available': bool(self.binary and not self.problem), 'version': self.version,
                'message': self.problem, 'busy': self.gate.locked(), 'timeout_seconds': self.timeout}

    @staticmethod
    def terminate(process):
        if process and process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def cancel(self):
        with self.lock:
            if not self.active:
                return False
            self.cancelled = True
            self.terminate(self.active)
            return True

    def run(self, code: str, mode: str) -> dict:
        if not self.status()['available']:
            raise ValueError(self.problem)
        if not self.gate.acquire(blocking=False):
            raise BusyError('已有示例正在运行，请稍后重试或停止当前运行。')
        started = time.monotonic()
        buffers = {'stdout': bytearray(), 'stderr': bytearray()}
        reason = ''
        process = None
        try:
            self.work_dir.mkdir(parents=True, exist_ok=True)
            with tempfile.TemporaryDirectory(prefix='demo-', dir=self.work_dir) as directory:
                source = Path(directory) / 'demo.prim'
                source.write_text(code, encoding='utf-8')
                flags = ['--check'] if mode == 'check' else ['--stats', '--fuel', '100000', '--jit-log']
                if mode == 'interpreter':
                    flags.append('--jit=off')
                command = [sys.executable, str(HERE / 'limits.py'), str(self.binary), *flags, str(source)]
                with self.lock:
                    self.cancelled = False
                    process = subprocess.Popen(command, cwd=directory, stdin=subprocess.DEVNULL,
                                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
                    self.active = process
                with selectors.DefaultSelector() as selector:
                    selector.register(process.stdout, selectors.EVENT_READ, 'stdout')
                    selector.register(process.stderr, selectors.EVENT_READ, 'stderr')
                    while selector.get_map():
                        if time.monotonic() - started > self.timeout:
                            reason = 'timeout'
                            self.terminate(process)
                            break
                        for key, _ in selector.select(.05):
                            chunk = os.read(key.fileobj.fileno(), 4096)
                            if not chunk:
                                selector.unregister(key.fileobj)
                                continue
                            space = MAX_OUTPUT - sum(len(v) for v in buffers.values())
                            buffers[key.data].extend(chunk[:space])
                            if len(chunk) > space:
                                reason = 'output_limit'
                                self.terminate(process)
                                break
                        if reason:
                            break
                process.wait(timeout=2)
                with self.lock:
                    if self.cancelled:
                        reason = 'cancelled'
                return {'stdout': buffers['stdout'].decode('utf-8', errors='replace'),
                        'stderr': buffers['stderr'].decode('utf-8', errors='replace').replace(str(source), '<demo>'),
                        'exit_code': process.returncode, 'reason': reason,
                        'elapsed_ms': round((time.monotonic() - started) * 1000, 1),
                        'mode': mode, 'runtime': self.version,
                        'command': ['prim', *flags, '<demo>']}
        finally:
            with self.lock:
                self.terminate(process)
                self.active = None
            if process:
                process.wait(timeout=2)
                process.stdout.close()
                process.stderr.close()
            self.gate.release()

