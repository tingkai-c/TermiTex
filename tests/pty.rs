#![cfg(unix)]
mod support;
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn contains(output: &[u8], needle: &[u8]) -> bool {
    output.windows(needle.len()).any(|w| w == needle)
}
fn resize(fd: i32, rows: u16, cols: u16) {
    let size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: cols * 16,
        ws_ypixel: rows * 34,
    };
    assert_eq!(unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &size) }, 0);
}

// Run the test executable itself as the interactive child, avoiding Python or
// a production-only test switch. Parent advances each frame after observing its
// image placements, so slow CI does not race fixed sleeps.
#[test]
#[ignore = "fixture process, launched by the PTY integration test"]
fn terminal_fixture() {
    if std::env::var_os("TERMITEX_PTY_FIXTURE").is_none() {
        return;
    }
    for (hello, wrapped) in [(1, 3), (5, 7), (12, 14)] {
        print!(
            "\x1b[?1049h\x1b[?2026h\x1b[2J\x1b[{hello};1HHello \\(x^2\\)\x1b[{wrapped};1HHere \\(\\partial/\r\n  \\partial t\\) means time.\x1b[?2026l"
        );
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "next");
    }
    print!("\x1b[?1049l");
    std::io::stdout().flush().unwrap();
}

fn exercise(renderer: &str, compatibility: bool) {
    let (mut master, mut slave) = (-1, -1);
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    resize(slave.as_raw_fd(), 30, 100);
    // Neither endpoint may leak through exec and keep the PTY alive on failure.
    for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) },
            0
        );
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_termitex"));
    command
        .args([
            "--renderer",
            renderer,
            "--layout",
            if compatibility {
                "compatibility"
            } else {
                "compact"
            },
            "--",
        ])
        .arg(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "terminal_fixture", "--nocapture"])
        .env("TERMITEX_PTY_FIXTURE", "1")
        .env_remove("TERMITEX_ACTIVE")
        .env_remove("TERMITEX_STATS")
        .env("XDG_CONFIG_HOME", "/nonexistent/termitex-tests")
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave));
    if renderer == "ratex" {
        command.env("PATH", "/usr/bin:/bin");
    }
    let mut child = support::Guard(command.spawn().unwrap());
    let mut output = Vec::new();
    let mut probe = false;
    let mut stage = 0;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut status = None;
    while Instant::now() < deadline {
        let mut fd = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut fd, 1, 50) };
        if ready > 0 && fd.revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            let mut buf = [0; 65536];
            match master.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => panic!("PTY read: {e}"),
            }
            if !probe && contains(&output, b"\x1b[16t") {
                master
                    .write_all(b"\x1b[6;34;16t\x1b[?2026;2$y\x1b_Gi=1799999999;OK\x1b\\")
                    .unwrap();
                probe = true;
            }
            let expected: &[u8] = if compatibility {
                match stage {
                    0 => b"\x1b[1;7H\x1b_Ga=p,",
                    1 => b"\x1b[5;7H\x1b_Ga=p,",
                    _ => b"\x1b[12;7H\x1b_Ga=p,",
                }
            } else {
                match stage {
                    0 => b"\x1b[4;3H\x1b_Ga=p,",
                    1 => b"\x1b[8;3H\x1b_Ga=p,",
                    _ => b"\x1b[15;3H\x1b_Ga=p,",
                }
            };
            if stage < 3 && contains(&output, expected) {
                if stage == 1 {
                    resize(master.as_raw_fd(), 20, 80);
                    assert_eq!(
                        unsafe { libc::kill(child.0.id() as i32, libc::SIGWINCH) },
                        0
                    );
                }
                master.write_all(b"next\n").unwrap();
                stage += 1;
            }
        }
        status = child.0.try_wait().unwrap();
        if status.is_some() {
            break;
        }
    }
    // Bounded exit wait also handles platforms where a closed PTY returns EIO
    // before the process has become waitable.
    while status.is_none() && Instant::now() < deadline {
        status = child.0.try_wait().unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(status.expect("PTY process timed out").success());
    assert!(probe);
    assert_eq!(
        stage,
        3,
        "missing placement stage; output: {}",
        String::from_utf8_lossy(&output)
    );
    assert!(contains(&output, b"\x1b[5;7H\x1b_Ga=p,"));
    assert!(
        output
            .windows(b"\x1b_Ga=p,".len())
            .filter(|w| *w == b"\x1b_Ga=p,")
            .count()
            >= if compatibility { 3 } else { 4 }
    );
}
#[test]
fn native_pty_scroll_redraw_resize() {
    exercise("ratex", false);
}
#[test]
#[ignore = "requires Node.js and npm ci --ignore-scripts"]
fn mathjax_pty_scroll_redraw_resize() {
    exercise("mathjax", false);
}

#[test]
fn native_compatibility_pty_scroll_redraw_resize() {
    exercise("ratex", true);
}
#[test]
#[ignore = "requires Node.js and npm ci --ignore-scripts"]
fn mathjax_compatibility_pty_scroll_redraw_resize() {
    exercise("mathjax", true);
}
