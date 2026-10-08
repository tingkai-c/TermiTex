//! Unix PTY lifecycle and ordered terminal I/O. Math and graphics stay behind interfaces.
use crate::{
    config,
    engine::Engine,
    frame::{self, FrameGate},
    graphics::{GraphicsBackend, KittyGraphics},
    renderer::WorkerRenderer,
    terminal::{self, Capabilities, KittyProbe, TerminalProbe},
};
use std::{
    ffi::CString,
    io::{self, Read, Write},
    os::fd::FromRawFd,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
struct TerminalGuard {
    original: libc::termios,
    cleanup: Option<String>,
    synchronized: bool,
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.original);
        }
        if let Some(cleanup) = &self.cleanup {
            let _ = io::stdout().write_all(cleanup.as_bytes());
        }
        if self.synchronized {
            let _ = io::stdout().write_all(b"\x1b[?2026l");
        }
        let _ = io::stdout().flush();
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
            (w.ws_xpixel / w.ws_col).max(1)
        } else {
            16
        },
        if w.ws_ypixel > 0 {
            (w.ws_ypixel / w.ws_row).max(1)
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
fn probe(
    probe: &mut dyn TerminalProbe,
    fallback: (u16, u16),
) -> io::Result<(Capabilities, Vec<u8>)> {
    io::stdout().write_all(probe.queries().as_bytes())?;
    io::stdout().flush()?;
    let deadline = Instant::now() + Duration::from_millis(500);
    let mut received = 0;
    let mut buf = [0; 1024];
    while Instant::now() < deadline && received < 65536 {
        if poll_fd(0, 10) {
            let n = unsafe { libc::read(0, buf.as_mut_ptr().cast(), buf.len()) };
            if n > 0 {
                received += n as usize;
                probe.receive(&buf[..n as usize]);
                if probe.complete() {
                    break;
                }
            }
        }
    }
    Ok((probe.capabilities(fallback), probe.take_input()))
}

pub fn run(options: config::Options, doctor: bool) -> Result<i32, Box<dyn std::error::Error>> {
    let renderer = options.renderer;
    let args = options.command;
    if unsafe { libc::isatty(0) } != 1 || unsafe { libc::isatty(1) } != 1 {
        return Err("run from an interactive terminal".into());
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
        if libc::tcsetattr(0, libc::TCSANOW, &raw) != 0 {
            return Err(io::Error::last_os_error().into());
        }
    }
    let mut tty = TerminalGuard {
        original,
        cleanup: None,
        synchronized: false,
    };
    let mut w = window();
    let (capabilities, pending) = if options.graphics == config::Graphics::Off && !doctor {
        let (width, height) = cell(&w);
        (
            Capabilities {
                kitty_graphics: false,
                synchronized_updates: false,
                cell_width: width,
                cell_height: height,
                measured_cell: false,
                foreground: None,
                background: None,
            },
            Vec::new(),
        )
    } else {
        probe(&mut KittyProbe::default(), cell(&w))?
    };
    if doctor {
        drop(tty);
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "termitex": env!("CARGO_PKG_VERSION"),
                "terminal": terminal::name(),
                "capabilities": capabilities,
                "multiplexer": std::env::var_os("TMUX").is_some() || std::env::var_os("STY").is_some(),
                "hint": "WezTerm requires enable_kitty_graphics = true. A successful probe checks transport, not visual correctness."
            }))?
        );
        return Ok(0);
    }
    let graphics = match options.graphics {
        config::Graphics::Auto => capabilities.kitty_graphics,
        config::Graphics::Kitty => true,
        config::Graphics::Off => false,
    };
    let synchronized = graphics && capabilities.synchronized_updates;
    let backend: Option<Box<dyn GraphicsBackend>> =
        graphics.then(|| Box::new(KittyGraphics) as Box<dyn GraphicsBackend>);
    tty.cleanup = backend.as_ref().map(|backend| backend.cleanup());
    tty.synchronized = synchronized;
    let metrics = (capabilities.cell_width, capabilities.cell_height);
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
    // Renderer execs must not inherit the application's PTY master.
    if unsafe { libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        unsafe {
            libc::close(master);
        }
        return Err(io::Error::last_os_error().into());
    }
    let mut pty = unsafe { std::fs::File::from_raw_fd(master) };
    // PTY input writes are small; only reads are attempted after readiness polling.
    if !pending.is_empty() {
        pty.write_all(&pending)?;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let resize = Arc::new(AtomicBool::new(false));
    for s in [libc::SIGTERM, libc::SIGHUP] {
        signal_hook::flag::register(s, stop.clone())?;
    }
    signal_hook::flag::register(libc::SIGWINCH, resize.clone())?;
    let mut engine = if let Some(backend) = backend {
        let mut e = Engine::with_backends(
            w.ws_row,
            w.ws_col,
            metrics,
            Box::new(WorkerRenderer::spawn(renderer)?),
            backend,
        );
        e.renderer = renderer.name().into();
        e.compatibility = options.layout == config::Layout::Compatibility;
        e.fg = std::env::var("TERMITEX_FG")
            .ok()
            .or(capabilities.foreground)
            .unwrap_or(e.fg);
        e.bg = std::env::var("TERMITEX_BG")
            .ok()
            .or(capabilities.background)
            .unwrap_or(e.bg);
        Some(e)
    } else {
        None
    };
    let mut gate = FrameGate::new();
    let mut held_since = Instant::now();
    let mut output = io::stdout();
    let mut buf = [0u8; 65536];
    let mut status = 0;
    if let Some(e) = &engine {
        output.write_all(e.cleanup().as_bytes())?;
    }
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
            if let Some(e) = &mut engine {
                e.resize(w.ws_row, w.ws_col);
                output.write_all(e.cleanup().as_bytes())?;
                if w.ws_xpixel > 0 && w.ws_ypixel > 0 {
                    e.cell = cell(&w);
                }
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
                    if engine.is_none() {
                        output.write_all(&buf[..n])?;
                        output.flush()?;
                    } else if !gate.has_pending() {
                        held_since = Instant::now();
                    }
                    if engine.is_some()
                        && let Some(batch) = gate.push(&buf[..n])
                    {
                        emit(&mut output, engine.as_mut().unwrap(), batch, synchronized)?;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                Err(e) => return Err(e.into()),
            }
        }
        if gate.has_pending() && held_since.elapsed() > Duration::from_millis(100) {
            if let Some(batch) = gate.flush() {
                emit(&mut output, engine.as_mut().unwrap(), batch, synchronized)?;
                held_since = Instant::now();
            }
        }
        if let Some(e) = &mut engine {
            if !gate.has_pending() {
                let graphics = e.poll();
                if !graphics.is_empty() {
                    if synchronized {
                        output.write_all(b"\x1b[?2026h")?;
                    }
                    output.write_all(graphics.as_bytes())?;
                    if synchronized {
                        output.write_all(b"\x1b[?2026l")?;
                    }
                    output.flush()?;
                }
            }
            if e.worker_stalled() {
                e.stop_renderer();
                output.write_all(e.cleanup().as_bytes())?;
            }
        }
        let exited = if child.0 > 0 {
            unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) }
        } else {
            0
        };
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
        emit(&mut output, engine.as_mut().unwrap(), batch, synchronized)?;
    }
    if let Some(e) = &engine {
        if let Ok(path) = std::env::var("TERMITEX_STATS") {
            std::fs::write(path, serde_json::to_string_pretty(&e.stats)?)?;
        }
    }
    if child.0 > 0 {
        // Reap the child after draining its PTY, retaining its exit status.
        if stop.load(Ordering::Relaxed) {
            unsafe {
                libc::kill(pid, libc::SIGHUP);
            }
        }
        loop {
            let result = unsafe { libc::waitpid(pid, &mut status, 0) };
            if result == pid {
                child.0 = 0;
                break;
            }
            if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                break;
            }
        }
    }
    Ok(if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        128 + libc::WTERMSIG(status)
    } else {
        1
    })
}

fn emit(
    out: &mut impl Write,
    e: &mut Engine,
    b: frame::Batch,
    synchronized: bool,
) -> io::Result<()> {
    let graphics = e.accept(&b.bytes, b.invalidate, b.moved);
    // Keep child frame-closing controls until after cached placements, so Ghostty
    // commits text and equations together. A protocol-aware gate bounds buffering.
    if !synchronized {
        out.write_all(&b.bytes)?;
        out.write_all(graphics.as_bytes())?;
        return out.flush();
    }
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
