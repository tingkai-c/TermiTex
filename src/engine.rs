use crate::app::{App, InputPolicy};
use crate::detect::{Formula, detect_output};
#[cfg(test)]
use crate::renderer::Response;
use crate::{
    graphics::{GraphicsBackend, Rectangle},
    renderer::{MathRenderer, Request},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    time::Instant,
};
struct Image {
    columns: u16,
    data: String,
    id: u32,
    uploaded: bool,
    used: u64,
}
#[derive(Clone, PartialEq, Eq)]
struct Pin {
    key: String,
    id: u32,
    placement: u32,
}
#[derive(Default, Debug, Serialize)]
pub struct Stats {
    pub batches: u64,
    pub scans: u64,
    pub uploads: u64,
    pub placements: u64,
    pub requests: u64,
    pub failures: u64,
    pub max_reconcile_us: u128,
}
pub struct Engine {
    pub parser: vt100::Parser,
    physical: vt100::Parser,
    has_projection: bool,
    pub stats: Stats,
    cache: HashMap<String, Image>,
    pins: HashMap<(u16, u16, u16, u16), Pin>,
    failed: HashSet<String>,
    inflight: HashMap<String, Instant>,
    renderer_backend: Box<dyn MathRenderer>,
    graphics: Box<dyn GraphicsBackend>,
    next_id: u32,
    next_pin: u32,
    tick: u64,
    pub cell: (u16, u16),
    pub fg: String,
    pub bg: String,
    pub enabled: bool,
    pub renderer: String,
    pub compatibility: bool,
    pub app: App,
}
impl Engine {
    pub fn with_backends(
        rows: u16,
        cols: u16,
        cell: (u16, u16),
        renderer_backend: Box<dyn MathRenderer>,
        graphics: Box<dyn GraphicsBackend>,
    ) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
            physical: vt100::Parser::new(rows, cols, 0),
            has_projection: false,
            stats: Stats::default(),
            cache: HashMap::new(),
            pins: HashMap::new(),
            failed: HashSet::new(),
            inflight: HashMap::new(),
            renderer_backend,
            graphics,
            next_id: 1_800_000_000,
            next_pin: 1,
            tick: 0,
            cell,
            fg: "#ffffff".into(),
            bg: "#282c34".into(),
            enabled: true,
            renderer: "ratex".into(),
            compatibility: false,
            app: App::Codex,
        }
    }
    pub fn stop_renderer(&mut self) {
        self.enabled = false;
        self.renderer_backend.stop();
        self.inflight.clear();
    }
    #[cfg(test)]
    fn new(
        rows: u16,
        cols: u16,
        cell: (u16, u16),
        tx: std::sync::mpsc::SyncSender<Request>,
        rx: std::sync::mpsc::Receiver<Response>,
    ) -> Self {
        Self::with_backends(
            rows,
            cols,
            cell,
            Box::new(crate::renderer::ChannelRenderer { tx, rx }),
            Box::new(crate::graphics::KittyGraphics),
        )
    }
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows, cols);
        self.physical.screen_mut().set_size(rows, cols);
        self.pins.clear();
        for i in self.cache.values_mut() {
            i.uploaded = false;
        }
    }
    pub fn accept(&mut self, bytes: &[u8], invalid: bool, moved: bool) -> String {
        self.collect_completions();
        self.parser.process(bytes);
        self.physical.process(bytes);
        self.stats.batches += 1;
        let mut out = String::new();
        if invalid {
            self.pins.clear();
            for i in self.cache.values_mut() {
                i.uploaded = false;
            }
        } else if moved {
            for p in self.pins.values() {
                out += &self.graphics.remove(p.id, p.placement);
            }
            self.pins.clear();
        }
        out + &self.reconcile()
    }
    pub fn completion_fd(&self) -> Option<std::os::fd::RawFd> {
        if self.enabled {
            self.renderer_backend.completion_fd()
        } else {
            None
        }
    }
    fn collect_completions(&mut self) -> bool {
        // Clear before draining responses so a concurrent completion cannot
        // lose its wakeup. Drain every ready result before a single reconcile.
        self.renderer_backend.clear_notification();
        let mut changed = false;
        while let Some(r) = self.renderer_backend.poll() {
            self.inflight.remove(&r.key);
            if r.error.is_some() || r.png.len() > 16_777_216 || STANDARD.decode(&r.png).is_err() {
                self.failed.insert(r.key);
                self.stats.failures += 1;
            } else {
                self.next_id += 1;
                self.cache.insert(
                    r.key,
                    Image {
                        data: r.png,
                        columns: r.columns,
                        id: self.next_id,
                        uploaded: false,
                        used: self.tick,
                    },
                );
            }
            changed = true;
        }
        changed
    }
    pub fn poll(&mut self) -> String {
        if self.collect_completions() {
            self.reconcile()
        } else {
            String::new()
        }
    }
    pub fn worker_stalled(&self) -> bool {
        self.inflight.values().any(|t| t.elapsed().as_secs() > 20)
    }
    pub fn reconcile(&mut self) -> String {
        if !self.enabled {
            return String::new();
        }
        let t = Instant::now();
        self.tick += 1;
        self.stats.scans += 1;
        let formulas: Vec<_> = detect_output(
            self.parser.screen(),
            &self.fg,
            &self.bg,
            self.app.output_end(self.parser.screen()),
        )
        .into_iter()
        // A wrapped inline formula has disjoint source spans. A rectangular
        // overlay could hide neighboring prose; keep those as source here.
        .filter(|f| !self.compatibility || f.sources.is_empty())
        .collect();
        let mut compact: HashMap<u16, Vec<crate::layout::Span>> = HashMap::new();
        for f in &formulas {
            if self.compatibility || f.display || f.rows != 1 {
                continue;
            }
            let mut geometry = f.clone();
            geometry.row = 0;
            geometry.col = 0;
            geometry.sources.clear();
            if let Some(im) = self.cache.get(&key(
                &geometry,
                self.cell,
                &self.renderer,
                self.compatibility,
            )) {
                if im.columns > 0 && im.columns <= f.cols {
                    if f.sources.is_empty() {
                        compact
                            .entry(f.row)
                            .or_default()
                            .push((f.col, f.cols, im.columns));
                    } else {
                        for span in &f.sources {
                            let width = if span.row == f.row && span.col == f.col {
                                im.columns
                            } else {
                                0
                            };
                            compact
                                .entry(span.row)
                                .or_default()
                                .push((span.col, span.cols, width));
                        }
                    }
                }
            }
        }
        let mut desired = HashMap::new();
        let mut out = String::new();
        let mut accepting = true;
        if self.has_projection || !compact.is_empty() {
            for row in 0..self.parser.screen().size().0 {
                let spans = compact.get(&row).map(Vec::as_slice).unwrap_or(&[]);
                out += &crate::layout::paint_row(
                    self.parser.screen(),
                    self.physical.screen(),
                    row,
                    spans,
                );
            }
        }
        self.has_projection = !compact.is_empty();
        for f in formulas {
            let mut geometry = f.clone();
            geometry.row = 0;
            geometry.col = 0;
            geometry.sources.clear();
            let key = key(&geometry, self.cell, &self.renderer, self.compatibility);
            let spans = compact.get(&f.row).map(Vec::as_slice).unwrap_or(&[]);
            let col = crate::layout::columns(self.parser.screen(), f.row, spans)[f.col as usize];
            let width = spans
                .iter()
                .find(|(start, _, _)| *start == f.col)
                .map(|(_, _, new)| *new)
                .unwrap_or(f.cols);
            let pos = (f.row, col, f.rows, width);
            if let Some(im) = self.cache.get_mut(&key) {
                im.used = self.tick;
                if !im.uploaded {
                    out += &self.graphics.upload(im.id, &im.data);
                    im.uploaded = true;
                    self.stats.uploads += 1;
                }
                if let Some(old) = self.pins.get(&pos).filter(|p| p.key == key) {
                    desired.insert(pos, old.clone());
                    continue;
                }
                self.next_pin += 1;
                let p = Pin {
                    key,
                    id: im.id,
                    placement: self.next_pin,
                };
                desired.insert(pos, p.clone());
                out += &self.graphics.place(
                    p.id,
                    p.placement,
                    Rectangle {
                        row: f.row,
                        col,
                        cols: width,
                        rows: f.rows,
                    },
                );
                self.stats.placements += 1;
            } else if accepting
                && self.inflight.len() < self.renderer_backend.capacity()
                && !self.failed.contains(&key)
                && !self.inflight.contains_key(&key)
            {
                let request = Request {
                    key: key.clone(),
                    formula: f,
                    compatibility: self.compatibility,
                    cell_width: self.cell.0,
                    cell_height: self.cell.1,
                };
                if self.renderer_backend.submit(request) {
                    self.inflight.insert(key, Instant::now());
                    self.stats.requests += 1;
                } else {
                    accepting = false;
                }
            }
        }
        // Remove obsolete pins, not shared uploaded rasters.
        for (pos, pin) in &self.pins {
            if desired.get(pos) != Some(pin) {
                out += &self.graphics.remove(pin.id, pin.placement);
            }
        }
        self.pins = desired;
        let mut bytes: usize = self.cache.values().map(|i| i.data.len()).sum();
        while self.cache.len() > 128 || bytes > 64 * 1024 * 1024 {
            let victim = self
                .cache
                .iter()
                .filter(|(k, _)| !self.pins.values().any(|p| &p.key == *k))
                .min_by_key(|(_, v)| v.used)
                .map(|(k, _)| k.clone());
            let Some(k) = victim else { break };
            if let Some(i) = self.cache.remove(&k) {
                bytes -= i.data.len();
                out += &self.graphics.release(i.id);
            }
        }
        if !out.is_empty() {
            if !self.compatibility {
                out += &String::from_utf8(self.parser.screen().attributes_formatted()).unwrap();
            }
            let (r, c) = self.parser.screen().cursor_position();
            out += &format!(
                "\x1b[{};{}H",
                r + 1,
                c.min(self.parser.screen().size().1 - 1) + 1
            );
        }
        self.physical.process(out.as_bytes());
        self.stats.max_reconcile_us = self.stats.max_reconcile_us.max(t.elapsed().as_micros());
        out
    }
    pub fn cleanup(&self) -> String {
        self.graphics.cleanup()
    }
}
fn key(f: &Formula, cell: (u16, u16), renderer: &str, compatibility: bool) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    renderer.hash(&mut h);
    compatibility.hash(&mut h);
    f.hash(&mut h);
    cell.hash(&mut h);
    format!("{:016x}", h.finish())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    #[test]
    fn parallel_results_are_batched_deduplicated_and_placed_at_current_rows() {
        struct Pair(crate::renderer::ChannelRenderer);
        impl MathRenderer for Pair {
            fn submit(&mut self, r: Request) -> bool {
                self.0.submit(r)
            }
            fn poll(&mut self) -> Option<Response> {
                self.0.poll()
            }
            fn stop(&mut self) {}
            fn capacity(&self) -> usize {
                2
            }
        }
        let (tx, requests) = mpsc::sync_channel(2);
        let (responses, rx) = mpsc::channel();
        let mut e = Engine::with_backends(
            20,
            80,
            (16, 34),
            Box::new(Pair(crate::renderer::ChannelRenderer { tx, rx })),
            Box::new(crate::graphics::KittyGraphics),
        );
        e.accept(b"\\(a\\) and \\(a\\)\r\n\\(b\\)\r\n\\(c\\)", false, false);
        let a = requests.try_recv().unwrap();
        let b = requests.try_recv().unwrap();
        assert_ne!(a.key, b.key);
        assert!(requests.try_recv().is_err());
        assert_eq!(e.inflight.len(), 2);
        // Complete in reverse order, with both results ready in one event turn.
        for req in [&b, &a] {
            responses
                .send(Response {
                    key: req.key.clone(),
                    png: STANDARD.encode([1, 2, 3]),
                    columns: 1,
                    error: None,
                })
                .unwrap();
        }
        let scans = e.stats.scans;
        // Child movement and completions share one layout pass.
        e.accept(b"\x1b[1;1H\x1b[2L", false, true);
        assert_eq!(e.stats.scans, scans + 1);
        assert_eq!(e.stats.uploads, 2);
        assert_eq!(e.stats.placements, 3);
        assert!(e.pins.keys().all(|(row, _, _, _)| *row >= 2));
        assert_eq!(e.inflight.len(), 1);
        assert_eq!(requests.try_recv().unwrap().formula.latex, "c");
        assert!(requests.try_recv().is_err());
        assert!(e.poll().is_empty());
        assert_eq!(e.stats.scans, scans + 1);
    }
    #[test]
    fn compatibility_preserves_source_cells_and_skips_wrapped_inline() {
        let (tx, requests) = mpsc::sync_channel(1);
        let (responses, rx) = mpsc::channel();
        let mut e = Engine::new(20, 80, (16, 34), tx, rx);
        e.compatibility = true;
        e.accept(
            b"\x1b[?1049hHere \\(\\rho\\) is \x1b[1mcharge\x1b[0m.",
            false,
            false,
        );
        let req = requests.recv().unwrap();
        assert!(req.compatibility);
        responses
            .send(Response {
                key: req.key,
                png: STANDARD.encode([1, 2, 3]),
                columns: req.formula.cols,
                error: None,
            })
            .unwrap();
        let out = e.poll();
        assert!(out.contains("c=8,r=1"));
        assert!(!out.contains("\x1b[0")); // No attribute resets or row repaint.
        assert_eq!(
            e.parser.screen().contents_formatted(),
            e.physical.screen().contents_formatted()
        );
        assert!(!e.has_projection);
        e.accept(b"\x1b[1;1H\x1b[2L", false, true);
        assert_eq!(
            e.parser.screen().contents_formatted(),
            e.physical.screen().contents_formatted()
        );
        assert!(
            e.pins
                .keys()
                .any(|&(r, c, _, w)| r == 2 && c == 5 && w == 8)
        );
        assert_eq!(e.stats.requests, 1);
        e.accept(
            b"\x1b[5;1HHere \\(\\partial/\r\n  \\partial t\\) means time.",
            false,
            false,
        );
        assert!(requests.try_recv().is_err());
        assert_eq!(
            e.parser.screen().contents_formatted(),
            e.physical.screen().contents_formatted()
        );
    }
    #[test]
    fn wrapped_source_fragments_are_removed_and_scroll_as_one_equation() {
        let (tx, requests) = mpsc::sync_channel(1);
        let (responses, rx) = mpsc::channel();
        let mut e = Engine::new(15, 70, (16, 34), tx, rx);
        e.accept(
            b"\x1b[?1049hHere \\(\\partial/\r\n  \\partial t\\) means time.",
            false,
            false,
        );
        let req = requests.recv().unwrap();
        assert_eq!(req.formula.sources.len(), 2);
        responses
            .send(Response {
                key: req.key,
                png: STANDARD.encode([1, 2, 3]),
                columns: 5,
                error: None,
            })
            .unwrap();
        e.poll();
        let text = e.physical.screen().contents();
        assert!(!text.contains("partial"));
        assert!(text.contains("means time."));
        assert_eq!(e.pins.len(), 1);
        e.accept(b"\x1b[1;1H\x1b[2L", false, true);
        assert_eq!(e.stats.requests, 1);
        assert_eq!(e.stats.uploads, 1);
        assert_eq!(e.pins.len(), 1);
        assert!(!e.physical.screen().contents().contains("partial"));
        // If the closing delimiter leaves the viewport, restore readable source.
        e.accept(b"\x1b[4;1H\x1b[2K", false, false);
        assert!(e.pins.is_empty());
        assert!(e.physical.screen().contents().contains("partial/"));
    }
    #[test]
    fn table_padding_keeps_columns_and_images_aligned_after_scroll() {
        let (tx, requests) = mpsc::sync_channel(1);
        let (responses, rx) = mpsc::channel();
        let mut e = Engine::new(20, 100, (16, 34), tx, rx);
        let left = r"\(\nabla\cdot\) - divergence";
        let table = format!(
            "\x1b[?1049h{:<40}Meaning\r\n{:<40}How \\(E\\) flows\r\n{:40}a point",
            "Symbol", left, ""
        );
        e.accept(table.as_bytes(), false, false);
        for _ in 0..2 {
            let req = requests.recv().unwrap();
            responses
                .send(Response {
                    key: req.key,
                    png: STANDARD.encode([1, 2, 3]),
                    columns: 2,
                    error: None,
                })
                .unwrap();
            e.poll();
        }
        for (row, text) in [(0, "Meaning"), (1, "How    flows"), (2, "a point")] {
            let line = e.physical.screen().rows(0, 100).nth(row).unwrap();
            assert!(line[40..].starts_with(text), "{line:?}");
        }
        assert!(e.pins.keys().any(|&(r, c, _, _)| r == 1 && c == 44));
        assert!(
            e.physical
                .screen()
                .rows(0, 100)
                .nth(1)
                .unwrap()
                .starts_with("   - divergence")
        );
        e.accept(b"\x1b[1;1H\x1b[2L", false, true);
        let line = e.physical.screen().rows(0, 100).nth(3).unwrap();
        assert!(line[40..].starts_with("How    flows"));
        assert!(e.pins.keys().any(|&(r, c, _, _)| r == 3 && c == 44));
        assert_eq!(e.stats.requests, 2);
        assert_eq!(e.stats.uploads, 2);
        e.accept(b"\x1b[4;41HNow", false, false);
        let line = e.physical.screen().rows(0, 100).nth(3).unwrap();
        assert!(line[40..].starts_with("Now    flows"));
    }
    #[test]
    fn compact_prose_survives_partial_updates_and_scroll_without_rerender() {
        let (tx, requests) = mpsc::sync_channel(1);
        let (responses, rx) = mpsc::channel();
        let mut e = Engine::new(30, 100, (16, 34), tx, rx);
        e.accept(b"\x1b[?1049hHere \\(E\\) tells \\(B\\) more", false, false);
        for _ in 0..2 {
            let req = requests.recv().unwrap();
            responses
                .send(Response {
                    key: req.key,
                    png: STANDARD.encode([1, 2, 3]),
                    columns: 2,
                    error: None,
                })
                .unwrap();
            e.poll();
        }
        assert!(
            e.physical
                .screen()
                .contents()
                .starts_with("Here    tells    more")
        );
        assert!(
            e.parser
                .screen()
                .contents()
                .starts_with("Here \\(E\\) tells \\(B\\) more")
        );
        let initial = e.stats.placements;
        // A partial redraw writes to the child's original source coordinates.
        e.accept(b"\x1b[1;24Hnext", false, false);
        assert!(e.physical.screen().contents().contains("tells    next"));
        let started = Instant::now();
        for _ in 0..1000 {
            assert!(e.accept(b"\x1b[25;1Hstatus", false, false).is_empty());
        }
        eprintln!(
            "1000 cached inline reconciliations: {:?}",
            started.elapsed()
        );
        assert_eq!(e.stats.placements, initial);
        assert_eq!(e.stats.requests, 2);
        // Scroll moves existing compact text too; projection must follow it.
        e.accept(b"\x1b[1;1H\x1b[2L", false, true);
        assert!(
            e.physical
                .screen()
                .rows(0, 100)
                .nth(2)
                .unwrap()
                .starts_with("Here    tells    next")
        );
        assert_eq!(e.stats.requests, 2);
        assert_eq!(e.stats.uploads, 2);
        // Replacing the formula row restores normal native text, without ghosts.
        e.accept(b"\x1b[3;1H\x1b[2Kplain text", false, false);
        assert_eq!(
            e.physical.screen().rows(0, 100).nth(2).unwrap().trim_end(),
            "plain text"
        );
    }
    #[test]
    fn slow_worker_does_not_block_and_stale_results_use_current_position() {
        let (tx, requests) = mpsc::sync_channel(1);
        let (responses, rx) = mpsc::channel();
        let mut e = Engine::new(30, 100, (16, 34), tx, rx);
        e.accept(b"\x1b[?1049h\\[\r\nx^2\r\n\\]", false, false);
        let req = requests.recv().unwrap();
        // No response is sent until after these updates: progress must not
        // depend on the worker. Throughput belongs in benchmarks, not a wall
        // clock assertion that varies with shared CI runner load.
        for _ in 0..32 {
            e.accept(b"\x1b[25;1Hstatus", false, false);
        }
        assert!(!e.inflight.is_empty());
        assert!(requests.try_recv().is_err());
        assert_eq!(e.stats.requests, 1);
        e.accept(b"\x1b[2J\x1b[5;1H\\[\r\nx^2\r\n\\]", true, false);
        responses
            .send(Response {
                key: req.key,
                png: STANDARD.encode([1, 2, 3]),
                columns: 0,
                error: None,
            })
            .unwrap();
        let out = e.poll();
        assert!(out.contains("\x1b[5;1H"));
        assert_eq!(e.stats.uploads, 1);
        let initial = e.stats.placements;
        for _ in 0..100 {
            e.accept(b"\x1b[25;1Hstatus", false, false);
        }
        assert_eq!(e.stats.placements, initial);
        e.accept(b"\x1b[5;1H\x1b[3M\x1b[2;1H\\[\r\nx^2\r\n\\]", false, true);
        assert_eq!(e.stats.uploads, 1);
        assert!(e.stats.placements > initial);
    }
}
