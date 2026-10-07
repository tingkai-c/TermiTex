use crate::detect::{Formula, detect};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::mpsc::{Receiver, SyncSender},
    time::Instant,
};
const Z: u32 = 20_270_001;
#[derive(Clone, Serialize)]
pub struct Request {
    pub key: String,
    pub formula: Formula,
    pub cell_width: u16,
    pub cell_height: u16,
}
#[derive(Deserialize)]
pub struct Response {
    pub key: String,
    #[serde(default)]
    pub png: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub columns: u16,
}
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
    inflight: Option<(String, Instant)>,
    tx: SyncSender<Request>,
    rx: Receiver<Response>,
    next_id: u32,
    next_pin: u32,
    tick: u64,
    pub cell: (u16, u16),
    pub fg: String,
    pub bg: String,
    pub enabled: bool,
}
impl Engine {
    pub fn new(
        rows: u16,
        cols: u16,
        cell: (u16, u16),
        tx: SyncSender<Request>,
        rx: Receiver<Response>,
    ) -> Self {
        Self {
            parser: vt100::Parser::new(rows, cols, 0),
            physical: vt100::Parser::new(rows, cols, 0),
            has_projection: false,
            stats: Stats::default(),
            cache: HashMap::new(),
            pins: HashMap::new(),
            failed: HashSet::new(),
            inflight: None,
            tx,
            rx,
            next_id: 1_800_000_000,
            next_pin: 1,
            tick: 0,
            cell,
            fg: "#ffffff".into(),
            bg: "#282c34".into(),
            enabled: true,
        }
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
                out += &delete_pin(p);
            }
            self.pins.clear();
        }
        out + &self.reconcile()
    }
    pub fn poll(&mut self) -> String {
        let mut changed = false;
        while let Ok(r) = self.rx.try_recv() {
            self.inflight = None;
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
        if changed {
            self.reconcile()
        } else {
            String::new()
        }
    }
    pub fn worker_stalled(&self) -> bool {
        self.inflight
            .as_ref()
            .is_some_and(|(_, t)| t.elapsed().as_secs() > 20)
    }
    pub fn reconcile(&mut self) -> String {
        if !self.enabled {
            return String::new();
        }
        let t = Instant::now();
        self.tick += 1;
        self.stats.scans += 1;
        let formulas = detect(self.parser.screen(), &self.fg, &self.bg);
        let mut compact: HashMap<u16, Vec<crate::layout::Span>> = HashMap::new();
        for f in &formulas {
            if f.display || f.rows != 1 {
                continue;
            }
            let mut geometry = f.clone();
            geometry.row = 0;
            geometry.col = 0;
            if let Some(im) = self.cache.get(&key(&geometry, self.cell)) {
                if im.columns > 0 && im.columns <= f.cols {
                    compact
                        .entry(f.row)
                        .or_default()
                        .push((f.col, f.cols, im.columns));
                }
            }
        }
        let mut desired = HashMap::new();
        let mut out = String::new();
        let mut request = None;
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
            let key = key(&geometry, self.cell);
            let spans = compact.get(&f.row).map(Vec::as_slice).unwrap_or(&[]);
            let col = crate::layout::shifted(f.col, spans);
            let width = spans
                .iter()
                .find(|(start, _, _)| *start == f.col)
                .map(|(_, _, new)| *new)
                .unwrap_or(f.cols);
            let pos = (f.row, col, f.rows, width);
            if let Some(im) = self.cache.get_mut(&key) {
                im.used = self.tick;
                if !im.uploaded {
                    out += &upload(im.id, &im.data);
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
                out += &format!(
                    "\x1b[{};{}H\x1b_Ga=p,i={},p={},q=2,c={},r={},C=1,z={}\x1b\\",
                    f.row + 1,
                    col + 1,
                    p.id,
                    p.placement,
                    width,
                    f.rows,
                    Z
                );
                self.stats.placements += 1;
            } else if request.is_none() && !self.failed.contains(&key) {
                request = Some(Request {
                    key,
                    formula: f,
                    cell_width: self.cell.0,
                    cell_height: self.cell.1,
                });
            }
        }
        // Remove obsolete pins, not shared uploaded rasters.
        for (pos, pin) in &self.pins {
            if desired.get(pos) != Some(pin) {
                out += &delete_pin(pin);
            }
        }
        self.pins = desired;
        if self.inflight.is_none() {
            if let Some(r) = request {
                let key = r.key.clone();
                if self.tx.try_send(r).is_ok() {
                    self.inflight = Some((key, Instant::now()));
                    self.stats.requests += 1;
                }
            }
        }
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
                out += &format!("\x1b_Ga=d,d=I,i={},q=2\x1b\\", i.id);
            }
        }
        if !out.is_empty() {
            out += &String::from_utf8(self.parser.screen().attributes_formatted()).unwrap();
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
    pub fn cleanup() -> String {
        format!("\x1b_Ga=d,d=Z,z={Z},q=2\x1b\\")
    }
}
fn delete_pin(p: &Pin) -> String {
    format!("\x1b_Ga=d,d=i,i={},p={},q=2\x1b\\", p.id, p.placement)
}
fn key(f: &Formula, cell: (u16, u16)) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    f.hash(&mut h);
    cell.hash(&mut h);
    format!("{:016x}", h.finish())
}
fn upload(id: u32, png: &str) -> String {
    let mut s = String::new();
    let chunks: Vec<_> = png.as_bytes().chunks(4096).collect();
    for (i, c) in chunks.iter().enumerate() {
        let more = usize::from(i + 1 < chunks.len());
        if i == 0 {
            s += &format!("\x1b_Ga=t,f=100,i={id},q=2,m={more};");
        } else {
            s += &format!("\x1b_Gm={more};");
        }
        s += std::str::from_utf8(c).unwrap();
        s += "\x1b\\";
    }
    s
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
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
        let t = Instant::now();
        for _ in 0..1000 {
            e.accept(b"\x1b[25;1Hstatus", false, false);
        }
        assert!(t.elapsed().as_secs() < 2);
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
