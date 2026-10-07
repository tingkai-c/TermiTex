//! Headless CLI comparison; emulates graphics acknowledgments, not Ghostty frames.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const FORMS: [&str; 12] = [
    r"x^2+y^2=z^2",
    r"e^{i\pi}+1=0",
    r"\frac{1}{2}",
    r"\int_0^1 x^2\,dx=\frac{1}{3}",
    r"\sum_{k=1}^{n} k=\frac{n(n+1)}{2}",
    r"\frac{-b\pm\sqrt{b^2-4ac}}{2a}",
    r"P(A\mid B)=\frac{P(A\cap B)}{P(B)}",
    r"\nabla\cdot\mathbf{E}=\frac{\rho}{\epsilon_0}",
    r"\nabla\cdot\mathbf{B}=0",
    r"\nabla\times\mathbf{E}=-\frac{\partial\mathbf{B}}{\partial t}",
    r"c=\frac{1}{\sqrt{\mu_0\epsilon_0}}",
    r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
];
const CASES: usize = FORMS.len();
fn fixture() {
    for i in 0..CASES * 2 {
        let latex = FORMS[i % CASES];
        print!("\x1b[?1049h\x1b[?2026h\x1b[2J\x1b[HCASE{i:03}\r\n\\[\r\n{latex}\r\n\\]\x1b[?2026l");
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "next");
    }
    print!("\x1b[?1049l");
    std::io::stdout().flush().unwrap();
}
struct Guard(std::process::Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn mem_sample(root: u32) -> u64 {
    let out = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,rss="])
        .output()
        .unwrap();
    assert!(out.status.success(), "ps access is required");
    let rows: Vec<(u32, u32, u64)> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter_map(|l| {
            let v: Vec<_> = l.split_whitespace().collect();
            Some((
                v.first()?.parse().ok()?,
                v.get(1)?.parse().ok()?,
                v.get(2)?.parse().ok()?,
            ))
        })
        .collect();
    let mut ids = HashSet::from([root]);
    loop {
        let old = ids.len();
        for (pid, ppid, _) in &rows {
            if ids.contains(ppid) {
                ids.insert(*pid);
            }
        }
        if ids.len() == old {
            break;
        }
    }
    rows.iter()
        .filter(|(pid, _, _)| ids.contains(pid))
        .map(|(_, _, rss)| *rss)
        .sum()
}
fn validate(png: &[u8]) {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert!(info.width > 0 && info.width <= 1600 && info.height > 0 && info.height <= 1020);
    let channels = info.color_type.samples();
    assert!(
        buf[..info.buffer_size()]
            .chunks_exact(channels)
            .any(|p| p[0] > 150 && p[1] > 150 && p[2] > 150 && (channels != 4 || p[3] > 0)),
        "blank raster"
    );
}
fn trial(name: &str, argv: &[String], dir: &std::path::Path, trial: usize) -> serde_json::Value {
    let cache = dir.join(format!("cache-{name}-{trial}"));
    assert!(
        !cache.exists(),
        "use a fresh output directory for cold application-cache trials"
    );
    std::fs::create_dir_all(&cache).unwrap();
    let (mut master, mut slave) = (-1, -1);
    let mut w = libc::winsize {
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
                &mut w,
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
        assert_eq!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) },
            0
        );
    }
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .arg(std::env::current_exe().unwrap())
        .arg("--fixture")
        .env("TERM", "xterm-ghostty")
        .env("TERM_PROGRAM", "ghostty")
        .env("TFORMULA_CACHE_DIR", &cache)
        .env("XDG_CONFIG_HOME", &cache)
        .env("XDG_DATA_HOME", &cache)
        .env("TERMITEX_FG", "#ffffff")
        .env("TERMITEX_BG", "#282c34")
        .env_remove("TERMITEX_ACTIVE")
        .env_remove("TERMITEX_STATS")
        .env_remove("TMUX")
        .env_remove("STY")
        .env_remove("ZELLIJ")
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY")
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave));
    let start = Instant::now();
    let mut child = Guard(command.spawn().unwrap());
    let running = Arc::new(AtomicBool::new(true));
    let active = running.clone();
    let pid = child.0.id();
    let sampler = std::thread::spawn(move || {
        let mut peak = 0;
        while active.load(Ordering::Relaxed) {
            peak = peak.max(mem_sample(pid));
            std::thread::sleep(Duration::from_millis(20));
        }
        peak
    });
    let mut pending = Vec::new();
    let mut log = Vec::new();
    let mut payload = Vec::new();
    let mut upload_id = String::new();
    let mut upload_quiet = false;
    let mut valid = HashSet::new();
    let mut placed = Vec::new();
    let mut source_at = None;
    let mut next_at = None;
    let mut scan_marker = String::new();
    let mut first = None;
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if next_at.is_some_and(|t| Instant::now() >= t) {
            master.write_all(b"next\n").unwrap();
            next_at = None;
        }
        let mut fd = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut fd, 1, 2) } > 0 {
            let mut buf = [0; 65536];
            match master.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    log.extend_from_slice(&buf[..n]);
                }
                Err(e) if e.raw_os_error() == Some(libc::EIO) => break,
                Err(e) => panic!("{e}"),
            };
        }
        loop {
            if pending.is_empty() {
                break;
            }
            if pending[0] != 27 {
                let c = pending.remove(0);
                scan_marker.push(c as char);
                if scan_marker.ends_with(&format!("CASE{:03}", placed.len())) {
                    source_at = Some(Instant::now());
                }
                if scan_marker.len() > 32 {
                    scan_marker.drain(..16);
                }
                continue;
            }
            if pending.len() < 2 {
                break;
            }
            let end = match pending[1] {
                b'[' => pending[2..]
                    .iter()
                    .position(|c| (0x40..=0x7e).contains(c))
                    .map(|i| i + 3),
                b'_' | b']' => pending
                    .windows(2)
                    .position(|w| w == b"\x1b\\")
                    .map(|i| i + 2),
                _ => Some(2),
            };
            let Some(end) = end else {
                break;
            };
            let control = String::from_utf8(pending.drain(..end).collect()).unwrap();
            let reply = match control.as_str() {
                "\x1b[16t" => Some("\x1b[6;34;16t"),
                "\x1b[14t" => Some("\x1b[4;1020;1600t"),
                "\x1b[c" => Some("\x1b[?62;4;22c"),
                "\x1b]10;?\x1b\\" => Some("\x1b]10;rgb:ffff/ffff/ffff\x1b\\"),
                "\x1b]11;?\x1b\\" => Some("\x1b]11;rgb:2828/2c2c/3434\x1b\\"),
                _ => None,
            };
            if let Some(reply) = reply {
                master.write_all(reply.as_bytes()).unwrap();
            }
            if let Some(body) = control.strip_prefix("\x1b_G") {
                let body = body.strip_suffix("\x1b\\").unwrap();
                let (head, data) = body.split_once(';').unwrap_or((body, ""));
                let fields: HashMap<_, _> =
                    head.split(',').filter_map(|s| s.split_once('=')).collect();
                let action = fields.get("a").copied().unwrap_or("");
                let id = fields.get("i").copied().unwrap_or("");
                if action == "q" {
                    master
                        .write_all(format!("\x1b_Gi={id};OK\x1b\\").as_bytes())
                        .unwrap();
                }
                if action == "t" || action == "T" {
                    upload_id = id.into();
                    upload_quiet = fields.get("q") == Some(&"2");
                    payload.clear();
                }
                if fields.get("t") == Some(&"t") {
                    let path = String::from_utf8(STANDARD.decode(data).unwrap()).unwrap();
                    let png = std::fs::read(&path).unwrap();
                    validate(&png);
                    valid.insert(id.to_string());
                    std::fs::write(dir.join(format!("{name}-{trial}-{id}.png")), png).unwrap();
                    master
                        .write_all(format!("\x1b_Gi={id};OK\x1b\\").as_bytes())
                        .unwrap();
                } else if !data.is_empty() && action != "q" {
                    payload.extend(STANDARD.decode(data).unwrap());
                    if fields.get("m") != Some(&"1") {
                        validate(&payload);
                        valid.insert(upload_id.clone());
                        std::fs::write(
                            dir.join(format!("{name}-{trial}-{upload_id}.png")),
                            &payload,
                        )
                        .unwrap();
                        if !upload_quiet {
                            master
                                .write_all(format!("\x1b_Gi={upload_id};OK\x1b\\").as_bytes())
                                .unwrap();
                        }
                    }
                }
                if action == "p" || action == "T" {
                    assert!(valid.contains(id), "placement without validated image {id}");
                    if let Some(t) = source_at.take() {
                        first.get_or_insert(start.elapsed().as_secs_f64() * 1000.);
                        placed.push(t.elapsed().as_secs_f64() * 1000.);
                        next_at = Some(Instant::now() + Duration::from_millis(80));
                    }
                    if fields.get("q") != Some(&"2") {
                        let p = fields.get("p").copied().unwrap_or("0");
                        master
                            .write_all(format!("\x1b_Gi={id},p={p};OK\x1b\\").as_bytes())
                            .unwrap();
                    }
                }
            }
        }
        if child.0.try_wait().unwrap().is_some() {
            break;
        }
    }
    running.store(false, Ordering::Relaxed);
    let rss = sampler.join().unwrap();
    std::fs::write(dir.join(format!("{name}-{trial}.ansi")), log).unwrap();
    assert_eq!(
        placed.len(),
        CASES * 2,
        "{name}: missing renders; see captured ANSI"
    );
    json!({"tool":name,"trial":trial,"launch_to_first_placement_ms":first,"source_to_placement_ms":placed,"peak_sampled_tree_rss_kib":rss,"unique_images":valid.len(),"rendered_cases":placed.len()})
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--fixture") {
        fixture();
        return;
    }
    assert!(
        args.len() >= 4,
        "compare_wrappers TERMITEX NODE TFORMULA_CLI OUTPUT_DIR [TRIALS]"
    );
    let dir = std::path::Path::new(&args[3]);
    std::fs::create_dir_all(dir).unwrap();
    let trials = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(5);
    for i in 0..trials {
        let mut tools = vec![
            (
                "termitex",
                vec![
                    args[0].clone(),
                    "--renderer".into(),
                    "ratex".into(),
                    "--layout".into(),
                    "compatibility".into(),
                    "--".into(),
                ],
            ),
            (
                "tformula",
                vec![args[1].clone(), args[2].clone(), "--".into()],
            ),
        ];
        if i % 2 == 1 {
            tools.reverse();
        }
        for (name, argv) in tools {
            println!("{}", trial(name, &argv, dir, i));
        }
    }
}
