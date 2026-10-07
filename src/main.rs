mod detect;
mod engine;
mod frame;
use engine::{Engine, Request, Response};
use frame::FrameGate;
use std::{
    ffi::CString,
    io::{self, BufRead, BufReader, Read, Write},
    os::fd::FromRawFd,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
struct TerminalGuard(libc::termios);
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.0);
        }
        let _ = io::stdout().write_all(format!("{}\x1b[?2026l", Engine::cleanup()).as_bytes());
    }
}
struct WorkerGuard(std::process::Child);
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct ChildGuard(libc::pid_t);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0 > 0 {
            unsafe {
                libc::kill(self.0, libc::SIGHUP);
                libc::waitpid(self.0, std::ptr::null_mut(), 0);
            }
        }
    }
}
fn window() -> libc::winsize {
    let mut w: libc::winsize = unsafe { std::mem::zeroed() };
    unsafe {
        libc::ioctl(1, libc::TIOCGWINSZ, &mut w);
    }
    if w.ws_row == 0 {
        w.ws_row = 24;
    }
    if w.ws_col == 0 {
        w.ws_col = 80;
    }
    w
}
fn cell(w: &libc::winsize) -> (u16, u16) {
    (
        if w.ws_xpixel > 0 {
            w.ws_xpixel / w.ws_col
        } else {
            16
        },
        if w.ws_ypixel > 0 {
            w.ws_ypixel / w.ws_row
        } else {
            34
        },
    )
}
fn poll_fd(fd: i32, timeout: i32) -> bool {
    let mut p = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    unsafe { libc::poll(&mut p, 1, timeout) > 0 }
}
fn probe(default: (u16, u16)) -> io::Result<((u16, u16), Vec<u8>)> {
    io::stdout().write_all(b"\x1b[16t")?;
    io::stdout().flush()?;
    let mut data = Vec::new();
    let start = Instant::now();
    let mut b = [0u8; 1024];
    while start.elapsed() < Duration::from_millis(180) {
        if poll_fd(0, 10) {
            let n = unsafe { libc::read(0, b.as_mut_ptr().cast(), b.len()) };
            if n > 0 {
                data.extend_from_slice(&b[..n as usize]);
                if let Some(a) = data.windows(4).position(|v| v == b"\x1b[6;") {
                    if let Some(end) = data[a..].iter().position(|v| *v == b't') {
                        let text = String::from_utf8_lossy(&data[a + 4..a + end]);
                        let values: Vec<_> = text
                            .split(';')
                            .filter_map(|s| s.parse::<u16>().ok())
                            .collect();
                        if values.len() == 2 && values[0] > 0 && values[1] > 0 {
                            let size = (values[1], values[0]);
                            data.drain(a..=a + end);
                            return Ok((size, data));
                        }
                    }
                }
            }
        }
    }
    Ok((default, data))
}
fn main() {
    if let Err(e) = run() {
        eprintln!("termitex: {e}");
        std::process::exit(1)
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--help" || a == "-h") {
        println!(
            "termitex [--] command [args...]\nExperimental Ghostty PTY math renderer.\nUse: termitex\nTERMITEX_STATS=/path.json writes timing counters at exit.\nTERMITEX_FG / TERMITEX_BG override default colors.\nNo stock Codex modifications."
        );
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "--") {
        args.remove(0);
    }
    if args.is_empty() {
        args = vec![
            "codex".into(),
            "-c".into(),
            "tui.rendering.math=false".into(),
        ];
    }
    if unsafe { libc::isatty(0) } != 1 || unsafe { libc::isatty(1) } != 1 {
        return Err("run from an interactive Ghostty terminal".into());
    }
    if std::env::var_os("TERMITEX_ACTIVE").is_some() {
        return Err("nested termitex sessions are unsupported".into());
    }
    // No threads exist yet; the child inherits the nesting guard.
    unsafe {
        std::env::set_var("TERMITEX_ACTIVE", "1");
    }
    let argv: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.as_str()))
        .collect::<Result<_, _>>()?;
    let mut ptrs: Vec<_> = argv.iter().map(|a| a.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    let mut original: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(0, &mut original) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    let mut raw = original;
    unsafe {
        libc::cfmakeraw(&mut raw);
        libc::tcsetattr(0, libc::TCSANOW, &raw);
    }
    let _tty = TerminalGuard(original);
    let mut w = window();
    let (metrics, pending) = probe(cell(&w))?;
    let mut master = -1;
    let pid = unsafe { libc::forkpty(&mut master, std::ptr::null_mut(), &mut original, &mut w) };
    if pid < 0 {
        return Err(io::Error::last_os_error().into());
    }
    if pid == 0 {
        unsafe {
            libc::execvp(ptrs[0], ptrs.as_ptr());
            libc::_exit(127);
        }
    }
    let mut child = ChildGuard(pid);
    let mut pty = unsafe { std::fs::File::from_raw_fd(master) };
    // PTY input writes are small; only reads are attempted after readiness polling.
    if !pending.is_empty() {
        pty.write_all(&pending)?;
    }
    let worker_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("worker/render.mjs");
    let mut worker = WorkerGuard(
        Command::new("node")
            .arg(worker_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let mut worker_in = worker.0.stdin.take().unwrap();
    let worker_out = worker.0.stdout.take().unwrap();
    let (tx, requests) = mpsc::sync_channel::<Request>(1);
    let (responses, rx) = mpsc::channel::<Response>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(worker_out);
        for req in requests {
            let key = req.key.clone();
            let result = (|| -> io::Result<Response> {
                serde_json::to_writer(&mut worker_in, &req)?;
                worker_in.write_all(b"\n")?;
                worker_in.flush()?;
                let mut line = String::new();
                reader.by_ref().take(17_000_000).read_line(&mut line)?;
                serde_json::from_str(&line).map_err(io::Error::other)
            })();
            let r = result.unwrap_or_else(|e| Response {
                key,
                png: String::new(),
                error: Some(e.to_string()),
            });
            if responses.send(r).is_err() {
                break;
            }
        }
    });
    let stop = Arc::new(AtomicBool::new(false));
    let resize = Arc::new(AtomicBool::new(false));
    for s in [libc::SIGTERM, libc::SIGHUP] {
        signal_hook::flag::register(s, stop.clone())?;
    }
    signal_hook::flag::register(libc::SIGWINCH, resize.clone())?;
    let mut engine = Engine::new(w.ws_row, w.ws_col, metrics, tx, rx);
    engine.fg = std::env::var("TERMITEX_FG").unwrap_or(engine.fg);
    engine.bg = std::env::var("TERMITEX_BG").unwrap_or(engine.bg);
    let mut gate = FrameGate::new();
    let mut held_since = Instant::now();
    let mut output = io::stdout();
    let mut buf = [0u8; 65536];
    let mut status = 0;
    output.write_all(Engine::cleanup().as_bytes())?;
    output.flush()?;
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if resize.swap(false, Ordering::Relaxed) {
            w = window();
            unsafe {
                libc::ioctl(master, libc::TIOCSWINSZ, &w);
            }
            engine.resize(w.ws_row, w.ws_col);
            output.write_all(Engine::cleanup().as_bytes())?;
            if w.ws_xpixel > 0 && w.ws_ypixel > 0 {
                engine.cell = cell(&w);
            }
        }
        let mut fds = [
            libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: master,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        unsafe {
            libc::poll(fds.as_mut_ptr(), 2, 4);
        }
        if fds[0].revents & libc::POLLIN != 0 {
            let n = unsafe { libc::read(0, buf.as_mut_ptr().cast(), buf.len()) };
            if n > 0 {
                pty.write_all(&buf[..n as usize])?;
            }
        }
        if fds[1].revents & libc::POLLIN != 0 {
            let n = pty.read(&mut buf);
            match n {
                Ok(0) => break,
                Ok(n) => {
                    if !gate.has_pending() {
                        held_since = Instant::now();
                    }
                    if let Some(batch) = gate.push(&buf[..n]) {
                        emit(&mut output, &mut engine, batch)?;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                Err(e) => return Err(e.into()),
            }
        }
        if gate.has_pending() && held_since.elapsed() > Duration::from_millis(100) {
            if let Some(batch) = gate.flush() {
                emit(&mut output, &mut engine, batch)?;
                held_since = Instant::now();
            }
        }
        if !gate.has_pending() {
            let graphics = engine.poll();
            if !graphics.is_empty() {
                output.write_all(b"\x1b[?2026h")?;
                output.write_all(graphics.as_bytes())?;
                output.write_all(b"\x1b[?2026l")?;
                output.flush()?;
            }
        }
        if engine.worker_stalled() {
            engine.enabled = false;
            let _ = worker.0.kill();
            output.write_all(Engine::cleanup().as_bytes())?;
        }
        let exited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if exited == pid {
            child.0 = 0;
            if !poll_fd(master, 0) {
                break;
            }
        }
        if fds[1].revents & libc::POLLHUP != 0 && fds[1].revents & libc::POLLIN == 0 {
            break;
        }
    }
    if let Some(batch) = gate.flush() {
        emit(&mut output, &mut engine, batch)?;
    }
    let _ = worker.0.kill();
    let _ = worker.0.wait();
    if let Ok(path) = std::env::var("TERMITEX_STATS") {
        std::fs::write(path, serde_json::to_string_pretty(&engine.stats)?)?;
    }
    Ok(())
}
fn emit(out: &mut impl Write, e: &mut Engine, b: frame::Batch) -> io::Result<()> {
    let graphics = e.accept(&b.bytes, b.invalidate, b.moved);
    // Keep child frame-closing controls until after cached placements, so Ghostty
    // commits text and equations together. A protocol-aware gate bounds buffering.
    out.write_all(b"\x1b[?2026h")?;
    let close = b"\x1b[?2026l";
    let mut start = 0;
    for (i, v) in b.bytes.windows(close.len()).enumerate() {
        if v == close {
            out.write_all(&b.bytes[start..i])?;
            start = i + close.len();
        }
    }
    out.write_all(&b.bytes[start..])?;
    out.write_all(graphics.as_bytes())?;
    out.write_all(b"\x1b[?2026l")?;
    out.flush()
}
