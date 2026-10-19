use std::{
    collections::VecDeque,
    ffi::{c_int, c_short, c_ulong, c_void},
    io::{self, Write},
    sync::atomic::{AtomicBool, AtomicI32, Ordering},
    time::{Duration, Instant},
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Termios {
    iflag: u32,
    oflag: u32,
    cflag: u32,
    lflag: u32,
    line: u8,
    cc: [u8; 32],
    ispeed: u32,
    ospeed: u32,
}
#[repr(C)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}
#[repr(C)]
#[derive(Default)]
struct WinSize {
    rows: u16,
    cols: u16,
    xpixel: u16,
    ypixel: u16,
}
unsafe extern "C" {
    fn isatty(fd: c_int) -> c_int;
    fn tcgetattr(fd: c_int, term: *mut Termios) -> c_int;
    fn tcsetattr(fd: c_int, action: c_int, term: *const Termios) -> c_int;
    fn cfmakeraw(term: *mut Termios);
    fn poll(fds: *mut PollFd, count: c_ulong, timeout: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    fn signal(number: c_int, handler: usize) -> usize;
}
static ACTIVE: AtomicBool = AtomicBool::new(false);
static INTERRUPTED: AtomicI32 = AtomicI32::new(0);
extern "C" fn on_signal(number: c_int) {
    INTERRUPTED.store(number, Ordering::Relaxed);
}

pub(crate) struct Terminal {
    original: Option<Termios>,
    handlers: [usize; 2],
    pending: VecDeque<u8>,
}
impl Terminal {
    pub fn new() -> Self {
        Self {
            original: None,
            handlers: [0; 2],
            pending: VecDeque::new(),
        }
    }
    pub fn active(&self) -> bool {
        self.original.is_some()
    }
    pub fn interrupted(&self) -> bool {
        self.active() && INTERRUPTED.load(Ordering::Relaxed) != 0
    }
    pub fn is_tty() -> bool {
        unsafe { isatty(0) == 1 && isatty(1) == 1 }
    }
    pub fn size() -> (i64, i64) {
        let mut size = WinSize::default();
        if unsafe { ioctl(1, 0x5413, &mut size) } == 0 && size.rows > 0 && size.cols > 0 {
            (i64::from(size.cols), i64::from(size.rows))
        } else {
            (80, 24)
        }
    }
    pub fn enter(&mut self) -> io::Result<()> {
        if self.active() {
            return Ok(());
        }
        if !Self::is_tty() {
            return Err(io::Error::other(
                "terminal mode requires TTY stdin and stdout",
            ));
        }
        if ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return Err(io::Error::other("another runtime owns the terminal"));
        }
        let mut original = Termios::default();
        if unsafe { tcgetattr(0, &mut original) } != 0 {
            ACTIVE.store(false, Ordering::Release);
            return Err(io::Error::last_os_error());
        }
        INTERRUPTED.store(0, Ordering::Relaxed);
        for (index, number) in [2, 15].into_iter().enumerate() {
            let previous = unsafe { signal(number, on_signal as *const () as usize) };
            if previous == usize::MAX {
                let error = io::Error::last_os_error();
                if index == 1 {
                    unsafe {
                        signal(2, self.handlers[0]);
                    }
                }
                ACTIVE.store(false, Ordering::Release);
                return Err(error);
            }
            self.handlers[index] = previous;
        }
        let mut raw = original;
        unsafe {
            cfmakeraw(&mut raw);
        }
        // Keep signal delivery and output newline processing while disabling canonical input and echo.
        raw.lflag |= 1;
        raw.oflag = original.oflag;
        if unsafe { tcsetattr(0, 0, &raw) } != 0 {
            let error = io::Error::last_os_error();
            self.restore_handlers();
            ACTIVE.store(false, Ordering::Release);
            return Err(error);
        }
        self.original = Some(original);
        if let Err(error) = write_terminal("\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H") {
            let _ = self.leave();
            return Err(error);
        }
        Ok(())
    }
    fn restore_handlers(&self) {
        unsafe {
            signal(2, self.handlers[0]);
            signal(15, self.handlers[1]);
        }
    }
    pub fn leave(&mut self) -> io::Result<()> {
        let Some(original) = self.original.take() else {
            return Ok(());
        };
        let restored = unsafe { tcsetattr(0, 0, &original) };
        let error = if restored != 0 {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        let output = write_terminal("\x1b[0m\x1b[?25h\x1b[?1049l");
        self.restore_handlers();
        INTERRUPTED.store(0, Ordering::Relaxed);
        ACTIVE.store(false, Ordering::Release);
        if let Some(error) = error {
            Err(error)
        } else {
            output
        }
    }
    fn byte(&mut self, timeout: i64) -> io::Result<ReadByte> {
        if let Some(b) = self.pending.pop_front() {
            return Ok(ReadByte::Byte(b));
        }
        let started = Instant::now();
        loop {
            if self.interrupted() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "terminal interrupted",
                ));
            }
            let elapsed = started.elapsed().as_millis() as i64;
            let wait = if timeout < 0 {
                50
            } else {
                (timeout - elapsed).clamp(0, 50) as c_int
            };
            let mut fd = PollFd {
                fd: 0,
                events: 1,
                revents: 0,
            };
            let ready = unsafe { poll(&mut fd, 1, wait) };
            if ready < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if ready > 0 {
                if fd.revents & 0x20 != 0 {
                    return Err(io::Error::other("stdin is not a valid descriptor"));
                }
                let mut byte = 0u8;
                let count = unsafe { read(0, (&mut byte as *mut u8).cast(), 1) };
                if count == 1 {
                    return Ok(ReadByte::Byte(byte));
                }
                if count == 0 {
                    return Ok(ReadByte::Eof);
                }
                let e = io::Error::last_os_error();
                if e.kind() != io::ErrorKind::Interrupted {
                    return Err(e);
                }
            }
            if timeout >= 0 && started.elapsed().as_millis() >= timeout as u128 {
                return Ok(ReadByte::Timeout);
            }
        }
    }
    pub fn read_line(&mut self) -> io::Result<Option<String>> {
        if self.active() {
            return Err(io::Error::other(
                "leave terminal mode before reading a line",
            ));
        }
        let mut bytes = vec![];
        loop {
            match self.byte(-1)? {
                ReadByte::Byte(b'\n') => break,
                ReadByte::Byte(b) => {
                    bytes.push(b);
                    if bytes.len() > 16 * 1024 * 1024 {
                        return Err(io::Error::other("input line exceeds 16 MiB"));
                    }
                }
                ReadByte::Eof => {
                    if bytes.is_empty() {
                        return Ok(None);
                    }
                    break;
                }
                ReadByte::Timeout => unreachable!(),
            }
        }
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "stdin is not valid UTF-8"))
    }
    pub fn poll_key(&mut self, timeout: i64) -> io::Result<Option<String>> {
        if !self.active() {
            return Err(io::Error::other("call std.term.enter() before poll_key"));
        }
        let first = match self.byte(timeout)? {
            ReadByte::Timeout => return Ok(None),
            ReadByte::Eof => return Ok(Some("eof".into())),
            ReadByte::Byte(b) => b,
        };
        let key = match first {
            27 => match self.byte(35)? {
                ReadByte::Byte(prefix @ (b'[' | b'O')) => {
                    let mut sequence = vec![prefix];
                    loop {
                        match self.byte(35)? {
                            ReadByte::Byte(b) => {
                                sequence.push(b);
                                if (0x40..=0x7e).contains(&b) || sequence.len() >= 16 {
                                    break;
                                }
                            }
                            _ => break,
                        }
                    }
                    match sequence.as_slice() {
                        b"[A" | b"OA" => "up",
                        b"[B" | b"OB" => "down",
                        b"[C" | b"OC" => "right",
                        b"[D" | b"OD" => "left",
                        b"[H" | b"OH" => "home",
                        b"[F" | b"OF" => "end",
                        b"[3~" => "delete",
                        _ => "unknown",
                    }
                    .into()
                }
                ReadByte::Byte(b) => {
                    self.pending.push_front(b);
                    "escape".into()
                }
                _ => "escape".into(),
            },
            b'\r' | b'\n' => "enter".into(),
            9 => "tab".into(),
            127 | 8 => "backspace".into(),
            3 => "ctrl_c".into(),
            4 => "ctrl_d".into(),
            0..=31 => "unknown".into(),
            32..=126 => (first as char).to_string(),
            _ => {
                let length = match first {
                    0xc2..=0xdf => 2,
                    0xe0..=0xef => 3,
                    0xf0..=0xf4 => 4,
                    _ => return Err(io::Error::other("invalid UTF-8 key")),
                };
                let mut bytes = vec![first];
                for _ in 1..length {
                    match self.byte(100)? {
                        ReadByte::Byte(b) => bytes.push(b),
                        _ => return Err(io::Error::other("incomplete UTF-8 key")),
                    }
                }
                String::from_utf8(bytes).map_err(|_| io::Error::other("invalid UTF-8 key"))?
            }
        };
        Ok(Some(key))
    }
    pub fn sleep(&self, millis: u64) -> io::Result<()> {
        let until = Instant::now() + Duration::from_millis(millis);
        loop {
            if self.interrupted() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "terminal interrupted",
                ));
            }
            let Some(remaining) = until.checked_duration_since(Instant::now()) else {
                return Ok(());
            };
            std::thread::sleep(remaining.min(Duration::from_millis(25)));
        }
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}
enum ReadByte {
    Byte(u8),
    Timeout,
    Eof,
}
fn write_terminal(text: &str) -> io::Result<()> {
    let mut output = io::stdout().lock();
    output.write_all(text.as_bytes())?;
    output.flush()
}
