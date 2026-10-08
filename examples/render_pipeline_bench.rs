//! Run: cargo run --release --example render_pipeline_bench
//! Isolated pipeline latency, not terminal frame rate or CPU time.
#![allow(dead_code)]
#[path = "../src/app.rs"]
mod app;
#[path = "../src/config.rs"]
mod config;
#[path = "../src/detect.rs"]
mod detect;
#[path = "../src/native.rs"]
mod native;
#[path = "../src/renderer.rs"]
mod renderer;
#[path = "../src/table.rs"]
mod table;
use renderer::{MathRenderer, RenderPool, Request};
use std::time::{Duration, Instant};
fn request(i: usize) -> Request {
    Request {
        key: i.to_string(),
        compatibility: false,
        cell_width: 16,
        cell_height: 34,
        formula: detect::Formula {
            sources: vec![],
            latex: format!(r"\frac{{x^{{{}}}+1}}{{\sqrt{{y+2}}}}", i + 1),
            row: 0,
            col: 0,
            rows: 3,
            cols: 80,
            display: true,
            fg: "#ffffff".into(),
            bg: "#282c34".into(),
        },
    }
}
fn batch(pool: &mut RenderPool, polling: bool, offset: usize) -> f64 {
    let started = Instant::now();
    let (mut sent, mut received) = (0, 0);
    let mut seen = std::collections::HashSet::new();
    while received < 24 {
        while sent < 24 && pool.submit(request(offset + sent)) {
            sent += 1;
        }
        if polling {
            std::thread::sleep(Duration::from_millis(4));
        } else {
            let mut fd = libc::pollfd {
                fd: pool.completion_fd().unwrap(),
                events: libc::POLLIN,
                revents: 0,
            };
            assert!(
                unsafe { libc::poll(&mut fd, 1, 5000) } > 0,
                "worker did not wake poll"
            );
        }
        pool.clear_notification();
        while let Some(r) = pool.poll() {
            assert!(r.error.is_none(), "{:?}", r.error);
            assert!(seen.insert(r.key));
            assert!(!r.png.is_empty());
            received += 1;
        }
    }
    started.elapsed().as_secs_f64() * 1000.0
}
fn main() {
    if std::env::args().any(|a| a == "--internal-ratex-worker") {
        native::worker().unwrap();
        return;
    }
    for (workers, polling) in [(1, true), (1, false), (2, false)] {
        let mut timings = Vec::new();
        for _ in 0..7 {
            let mut pool = RenderPool::with_size(config::Renderer::Ratex, workers).unwrap();
            batch(&mut pool, false, 0); // Warm fonts and worker initialization.
            timings.push(batch(&mut pool, polling, 24));
        }
        timings.sort_by(f64::total_cmp);
        println!(
            "workers={workers} polling={polling}: median {:.2} ms / 24 uncached equations (7 trials)",
            timings[3]
        );
    }
}
