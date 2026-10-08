//! Deterministic content for real-emulator screenshots. No shell/user paths shown.
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
fn paint(scrolled: bool) {
    print!(
        "{}",
        concat!(
            "\x1b[?2026h\x1b[2J",
            "\x1b[1;3H\x1b[1;32mTermiTex | terminal compatibility check\x1b[0m",
            "\x1b[3;3HInline: \\(x^2 + y^2 = z^2\\) and \\(e^{i\\pi}+1=0\\).",
            "\x1b[5;3HDisplay: quadratic formula",
            "\x1b[6;3H\\[\r\n  x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}\r\n\r\n  \\]",
            "\x1b[11;3HIntegral: \\(\\int_0^1 x^2\\,dx = \\frac{1}{3}\\)",
            "\x1b[13;3HSymbol                          Meaning",
            "\x1b[14;3H\\(\\nabla\\cdot\\)                Divergence",
            "\x1b[15;3H\\(\\epsilon_0,\\mu_0\\)           Constants",
            "\x1b[17;3HWrapped: \\(\\partial/\r\n  \\partial t\\) measures change over time.",
            "\x1b[20;3H```tex\r\n  \\frac{source}{code}\r\n  ```",
            "\x1b[24;3H› Ask Codex to do anything: \\(this stays source\\)"
        )
    );
    if scrolled {
        print!("\x1b[1;1H\x1b[2M");
    }
    print!("\x1b[?2026l");
    io::stdout().flush().unwrap();
}
fn main() {
    let dir = PathBuf::from(std::env::args_os().nth(1).expect("control directory"));
    let resized = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(libc::SIGWINCH, resized.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    print!("\x1b]0;TermiTex visual check\x07\x1b[?1049h\x1b[?25l");
    io::stdout().flush().unwrap();
    while !dir.join("start").exists() {
        assert!(Instant::now() < deadline, "no controller start signal");
        std::thread::sleep(Duration::from_millis(20));
    }
    paint(false);
    std::fs::write(dir.join("ready"), "ready").unwrap();
    let mut moved = false;
    while Instant::now() < deadline && !dir.join("exit").exists() {
        if !moved && dir.join("scroll").exists() {
            // Application-driven scroll: move source cells two rows up.
            print!("\x1b[?2026h\x1b[1;1H\x1b[2M\x1b[?2026l");
            io::stdout().flush().unwrap();
            std::fs::write(dir.join("scrolled"), "scrolled").unwrap();
            moved = true;
        }
        if resized.swap(false, Ordering::Relaxed) {
            paint(moved);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    print!("\x1b[?25h\x1b[?1049l");
    io::stdout().flush().unwrap();
}
