#![cfg(unix)]
mod support;
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    process::{Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

// A headless terminal peer: replies can arrive fragmented with real keystrokes.
// These tests exercise negotiation, not any particular emulator's renderer.
fn run(args: &[&str], replies: &[u8]) -> (Vec<u8>, ExitStatus) {
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
    let mut child = support::Guard(
        Command::new(env!("CARGO_BIN_EXE_termitex"))
            .args(args)
            .env_remove("TERMITEX_ACTIVE")
            .env_remove("TERMITEX_STATS")
            .env_remove("TERMITEX_RENDERER")
            .env_remove("TERMITEX_LAYOUT")
            .env("XDG_CONFIG_HOME", "/nonexistent/termitex-tests")
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave.try_clone().unwrap()))
            .spawn()
            .unwrap(),
    );
    let mut output = Vec::new();
    let mut replied = false;
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut status = None;
    while Instant::now() < deadline {
        let mut fd = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut fd, 1, 10) } > 0 {
            let mut buf = [0; 8192];
            match master.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
                Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                Err(e) => panic!("{e}"),
            }
            if !replied && output.windows(5).any(|w| w == b"\x1b[16t") {
                for fragment in replies.chunks(3) {
                    master.write_all(fragment).unwrap();
                }
                replied = true;
            }
        }
        if status.is_none() {
            status = child.0.try_wait().unwrap();
        }
        if status.is_some() {
            break;
        }
    }
    while status.is_none() && Instant::now() < deadline {
        status = child.0.try_wait().unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    let status = status.expect("session timed out");
    // The slave clone permits checking the wrapper restored the original mode.
    let mut attrs: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut attrs) }, 0);
    assert_ne!(attrs.c_lflag & libc::ICANON, 0);
    (output, status)
}

#[test]
fn doctor_reports_probed_capabilities_without_starting_child() {
    for (sync, expected) in [(2, true), (0, false)] {
        let reply = format!(
            "\x1b]10;rgb:bbbb/bbbb/bbbb\x07\x1b]11;rgb:0000/0000/0000\x07\x1b_Gi=1799999999;OK\x1b\\\x1b[6;22;11t\x1b[?2026;{sync}$y"
        );
        let (output, status) = run(&["doctor"], reply.as_bytes());
        assert!(status.success());
        let text = String::from_utf8_lossy(&output);
        let start = text.find('{').unwrap();
        let report: serde_json::Value = serde_json::from_str(&text[start..]).unwrap();
        assert_eq!(report["capabilities"]["kitty_graphics"], true);
        assert_eq!(report["capabilities"]["cell_width"], 11);
        assert_eq!(report["capabilities"]["foreground"], "#bbbbbb");
        assert_eq!(report["capabilities"]["background"], "#000000");
        assert_eq!(report["capabilities"]["cell_height"], 22);
        assert_eq!(report["capabilities"]["synchronized_updates"], expected);
        assert!(!text.contains("a=p,"));
    }
}
#[test]
fn rejected_graphics_preserves_input_source_and_child_exit_status() {
    let (output, status) = run(
        &[
            "--",
            "/bin/sh",
            "-c",
            "read -r line; printf 'INPUT:%s\\n' \"$line\"; printf '%s\\n' '\\(x^2\\)'; exit 23",
        ],
        b"hello\n\x1b[6;34;16t\x1b_Gi=1799999999;ENOTSUP\x1b\\\x1b[?2026;0$y",
    );
    let text = String::from_utf8_lossy(&output);
    assert_eq!(status.code(), Some(23), "{text}");
    assert!(text.contains("INPUT:hello"), "{text}");
    assert!(text.contains("\\(x^2\\)"));
    assert!(!text.contains("ENOTSUP"));
    assert!(!text.contains("a=t,") && !text.contains("a=p,") && !text.contains("a=d,"));
}
#[test]
fn no_reply_times_out_to_plain_text() {
    let (output, status) = run(
        &[
            "--renderer",
            "mathjax",
            "--",
            "/bin/sh",
            "-c",
            "printf 'SOURCE'; exit 7",
        ],
        b"",
    );
    assert_eq!(status.code(), Some(7));
    assert!(String::from_utf8_lossy(&output).contains("SOURCE"));
    assert!(!String::from_utf8_lossy(&output).contains("a=t,"));
}
#[test]
fn explicit_off_does_not_probe_or_modify_output() {
    let (output, status) = run(
        &[
            "--graphics=off",
            "--",
            "/bin/sh",
            "-c",
            "printf 'SOURCE'; exit 9",
        ],
        b"",
    );
    assert_eq!(status.code(), Some(9));
    assert_eq!(output, b"SOURCE");
}
