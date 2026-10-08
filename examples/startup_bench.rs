//! Headless launch-to-first-child-output benchmark; not Codex UI readiness.
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn trial(binary: &str, mode: &str) -> f64 {
    let (mut master, mut slave) = (-1, -1);
    let mut size = libc::winsize {
        ws_row: 30,
        ws_col: 100,
        ws_xpixel: 1600,
        ws_ypixel: 1020,
    };
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
        unsafe {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
        }
    }
    let mut command = if mode == "direct" {
        Command::new("/usr/bin/printf")
    } else {
        let mut c = Command::new(binary);
        if mode == "off" {
            c.arg("--graphics=off");
        }
        c.args(["--", "/usr/bin/printf"]);
        c
    };
    command
        .arg("READY")
        .env_remove("TERMITEX_ACTIVE")
        .env_remove("TERMITEX_RENDERER")
        .env_remove("TERMITEX_LAYOUT")
        .env_remove("TERMITEX_STATS")
        .env("XDG_CONFIG_HOME", "/nonexistent/termitex-bench")
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave));
    let start = Instant::now();
    let mut child = command.spawn().unwrap();
    let mut output = Vec::new();
    let mut replied = false;
    let elapsed = loop {
        assert!(start.elapsed() < Duration::from_secs(5));
        let mut fd = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut fd, 1, 100) } <= 0 {
            continue;
        }
        let mut buf = [0; 8192];
        let n = master.read(&mut buf).unwrap();
        output.extend_from_slice(&buf[..n]);
        if output.windows(5).any(|v| v == b"READY") {
            break start.elapsed().as_secs_f64() * 1000.0;
        }
        if !replied && output.windows(5).any(|v| v == b"\x1b[16t") {
            let base = b"\x1b_Gi=1799999999;OK\x1b\\\x1b[6;34;16t\x1b[?2026;2$y";
            master.write_all(base).unwrap();
            if mode == "complete" {
                master
                    .write_all(b"\x1b]10;rgb:ffff/ffff/ffff\x07\x1b]11;rgb:2828/2c2c/3434\x07")
                    .unwrap();
            }
            replied = true;
        }
    };
    assert!(child.wait().unwrap().success());
    elapsed
}
fn main() {
    let binary = std::env::args().nth(1).expect("path to termitex binary");
    for mode in ["direct", "off", "complete", "missing-colors"] {
        let mut times: Vec<_> = (0..9).map(|_| trial(&binary, mode)).collect();
        times.sort_by(f64::total_cmp);
        println!(
            "{mode}: median {:.2} ms, range {:.2}–{:.2} ms (9 launches)",
            times[4], times[0], times[8]
        );
    }
}
