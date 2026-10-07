//! Complete VT controls are never split. Synchronized redraws become atomic
//! batches, but malformed/unterminated frames have bounded size and time.
use vte::{Params, Perform};
#[derive(Default)]
struct Controls {
    sync: bool,
    in_control: bool,
    epoch: u64,
    moved: bool,
}
impl Perform for Controls {
    fn csi_dispatch(&mut self, p: &Params, i: &[u8], _: bool, a: char) {
        self.in_control = false;
        if i == b"?" && (a == 'h' || a == 'l') {
            for v in p {
                if v[0] == 2026 {
                    self.sync = a == 'h';
                }
                if [47, 1047, 1049].contains(&v[0]) {
                    self.epoch += 1;
                    self.moved = true;
                }
            }
        }
        if a == 'J' && p.iter().any(|v| v[0] == 2 || v[0] == 3) {
            self.epoch += 1;
        }
        if "STLM".contains(a) {
            self.moved = true;
        }
    }
    fn esc_dispatch(&mut self, _: &[u8], _: bool, b: u8) {
        self.in_control = false;
        if b == b'c' {
            self.epoch += 1;
        }
        if b"DEM".contains(&b) {
            self.moved = true;
        }
    }
    fn osc_dispatch(&mut self, _: &[&[u8]], _: bool) {
        self.in_control = false;
    }
    fn unhook(&mut self) {
        self.in_control = false;
    }
    fn print(&mut self, _: char) {
        self.in_control = false;
    }
}
pub struct FrameGate {
    parser: vte::Parser,
    controls: Controls,
    pending: Vec<u8>,
}
pub struct Batch {
    pub bytes: Vec<u8>,
    pub invalidate: bool,
    pub moved: bool,
}
impl FrameGate {
    pub fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            controls: Controls::default(),
            pending: Vec::new(),
        }
    }
    pub fn push(&mut self, bytes: &[u8]) -> Option<Batch> {
        for b in bytes {
            if *b == 0x1b {
                self.controls.in_control = true;
            }
            self.parser
                .advance(&mut self.controls, std::slice::from_ref(b));
        }
        self.pending.extend_from_slice(bytes);
        if (!self.controls.sync && !self.controls.in_control) || self.pending.len() > 1_048_576 {
            self.flush()
        } else {
            None
        }
    }
    pub fn flush(&mut self) -> Option<Batch> {
        if self.pending.is_empty() {
            return None;
        }
        let batch = Batch {
            bytes: std::mem::take(&mut self.pending),
            invalidate: self.controls.epoch > 0,
            moved: self.controls.moved,
        };
        self.controls.epoch = 0;
        self.controls.moved = false;
        Some(batch)
    }
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_frame() {
        let mut f = FrameGate::new();
        assert!(f.push(b"\x1b[?2026hhello").is_none());
        assert!(f.push(b"\x1b[?202").is_none());
        let b = f.push(b"6l").unwrap();
        assert!(b.bytes.ends_with(b"2026l"));
    }
    #[test]
    fn ordinary() {
        assert_eq!(FrameGate::new().push(b"hello").unwrap().bytes, b"hello");
    }
}
