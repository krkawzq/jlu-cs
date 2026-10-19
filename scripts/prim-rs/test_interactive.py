#!/usr/bin/env -S uv run --script
"""Exercise the Prim CLI through pipes and a real Linux pseudo-terminal."""
from __future__ import annotations

import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time

ROOT = Path(__file__).resolve().parents[2]
BINARY = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else ROOT / "build/prim-rs/prim"


class Session:
    def __init__(self, args: list[str]):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        self.original = termios.tcgetattr(self.slave)
        self.output = bytearray()

        def setup():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        self.process = subprocess.Popen(
            [str(BINARY), *args], stdin=self.slave, stdout=self.slave,
            stderr=self.slave, preexec_fn=setup, close_fds=True,
        )

    def pump(self, timeout: float = 0.05):
        ready, _, _ = select.select([self.master], [], [], timeout)
        if ready:
            try:
                data = os.read(self.master, 65536)
            except OSError:
                return
            self.output.extend(data)
            if len(self.output) > 2_000_000:
                raise AssertionError("unexpectedly unbounded terminal output")

    def wait_text(self, text: str, start: int = 0, timeout: float = 8.0):
        until = time.monotonic() + timeout
        expected = text.encode()
        while expected not in self.output[start:]:
            self.pump()
            if time.monotonic() > until or self.process.poll() is not None:
                raise AssertionError(f"missing {text!r}: {self.output[-3000:].decode(errors='replace')}")

    def send(self, data: bytes):
        begin = len(self.output)
        os.write(self.master, data)
        return begin

    def finish(self, expected: int):
        until = time.monotonic() + 8
        while self.process.poll() is None and time.monotonic() < until:
            self.pump()
        if self.process.poll() is None:
            raise AssertionError("terminal process did not exit")
        self.pump(0.02)
        assert self.process.returncode == expected, self.output.decode(errors="replace")
        assert termios.tcgetattr(self.slave) == self.original, "terminal attributes were not restored"
        assert b"\x1b[?25h" in self.output and b"\x1b[?1049l" in self.output, "cursor/screen not restored"

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait(timeout=3)
        termios.tcsetattr(self.slave, termios.TCSANOW, self.original)
        os.close(self.master)
        os.close(self.slave)


def terminal_case(args, callback):
    session = Session(args)
    try:
        callback(session)
    finally:
        session.close()


def main():
    line_source = 'let name = std.input("name> "); std.print(name); std.print(std.parse_int(std.read_line())); std.assert(std.read_line() == ());'
    result = subprocess.run([str(BINARY), "-e", line_source], input="小明\r\n42\n", text=True, capture_output=True, timeout=8)
    assert result.returncode == 0, result.stderr
    assert result.stdout == "name> 小明\n42\n", repr(result.stdout)
    print("PASS line input, prompt flushing, CRLF, Unicode, numeric parsing and EOF")

    key_source = '''
std.term.enter();
let read_keys = @jit $[]() {
    std.print("READY"); std.flush();
    std.assert(std.term.poll_key(0) == ());
    let i = 0;
    loop {
        if i == 4 { break; };
        let key = std.term.poll_key(-1);
        std.print("KEY=" + key); std.flush();
        i = i + 1;
    };
}; read_keys();
'''
    def keys(s):
        s.wait_text("READY")
        # Allow the initial nonblocking timeout check to finish before providing input.
        time.sleep(0.05)
        start=s.send(b"\x1b["); time.sleep(0.01); s.send(b"A")
        s.wait_text("KEY=up", start)
        start=s.send("é".encode()); s.wait_text("KEY=é", start)
        start=s.send(b"\x1b"); s.wait_text("KEY=escape", start)
        start=s.send(b"\r"); s.wait_text("KEY=enter", start)
        s.finish(0)
    terminal_case(["-e", key_source], keys)
    print("PASS nonblocking poll, fragmented arrow key, Unicode, Escape, Enter and normal cleanup")

    error_source='let run = @jit $[]() { std.term.enter(); std.print("READY"); std.flush(); std.term.poll_key(-1); 1 / 0 }; run();'
    def error(s):
        s.wait_text("READY"); s.send(b"x"); s.wait_text("DivisionByZero"); s.finish(1)
    terminal_case(["-e",error_source],error)
    print("PASS JIT runtime-error terminal cleanup")

    wait_source='std.term.enter(); std.print("READY"); std.flush(); std.term.poll_key(-1);'
    def interrupt(s):
        s.wait_text("READY"); s.send(b"\x03"); s.wait_text("Interrupted"); s.finish(1)
    terminal_case(["-e",wait_source],interrupt)
    print("PASS Ctrl+C terminal cleanup")
    def terminate(s):
        s.wait_text("READY"); s.process.send_signal(signal.SIGTERM); s.wait_text("Interrupted"); s.finish(1)
    terminal_case(["-e",wait_source],terminate)
    print("PASS SIGTERM terminal cleanup")

    scalar_source = '''
std.term.enter();
let spin = @jit $[]() { let i=0; loop { i=i+1; if i==1000000 { i=0; }; } };
std.inspect.compile(spin);
std.assert(std.inspect.jit(spin).lowering == "local-scalars");
std.print("READY"); std.flush(); spin();
'''
    terminal_case(["-O2", "-e", scalar_source], terminate)
    print("PASS scalar JIT loop safepoint interruption and terminal cleanup")

    def snake(s):
        s.wait_text("Prim Snake")
        start=s.send(b"p"); s.wait_text("PAUSED",start)
        start=s.send(b"\x1b[A"); s.wait_text("PAUSED",start)
        s.send(b"p"); time.sleep(0.25)
        start=s.send(b"p"); s.wait_text("PAUSED",start)
        frame=s.output[start:].decode(errors="replace")
        rows=[line for line in frame.splitlines() if line.startswith("|")]
        assert len(rows)>=14, frame
        head=[i for i,row in enumerate(rows[-14:]) if "@@" in row]
        assert head and head[0]<7, "arrow key did not move the snake upward"
        start=s.send(b"r"); s.wait_text("score: 0",start)
        # Resize while playing, then restore the canvas and exit.
        fcntl.ioctl(s.slave,termios.TIOCSWINSZ,struct.pack("HHHH",10,30,0,0))
        s.wait_text("Resize terminal",len(s.output))
        fcntl.ioctl(s.slave,termios.TIOCSWINSZ,struct.pack("HHHH",24,80,0,0))
        s.wait_text("Prim Snake",len(s.output))
        s.send(b"q"); s.wait_text("Snake finished. Score:"); s.finish(0)
    terminal_case([str(ROOT/"src/prim-rs/examples/snake.pr")],snake)
    print("PASS playable Snake: render, arrows, pause/resume, restart, resize and quit")
    print("All interactive checks passed.")


if __name__ == "__main__":
    main()
