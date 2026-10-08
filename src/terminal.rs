//! Bounded startup negotiation. Only our replies are removed from user input.
use crate::graphics::PROBE_ID;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Capabilities {
    pub kitty_graphics: bool,
    pub synchronized_updates: bool,
    pub cell_width: u16,
    pub cell_height: u16,
    pub measured_cell: bool,
    pub foreground: Option<String>,
    pub background: Option<String>,
}

pub trait TerminalProbe {
    fn queries(&self) -> String;
    fn receive(&mut self, bytes: &[u8]);
    fn complete(&self) -> bool;
    fn capabilities(&self, fallback: (u16, u16)) -> Capabilities;
    fn take_input(&mut self) -> Vec<u8>;
}

#[derive(Default)]
pub struct KittyProbe {
    replies: Replies,
}
impl TerminalProbe for KittyProbe {
    fn queries(&self) -> String {
        format!(
            "\x1b]10;?\x07\x1b]11;?\x07\x1b[16t\x1b[?2026$p{}",
            crate::graphics::query()
        )
    }
    fn receive(&mut self, bytes: &[u8]) {
        self.replies.push(bytes);
    }
    fn complete(&self) -> bool {
        self.replies.complete()
    }
    fn capabilities(&self, fallback: (u16, u16)) -> Capabilities {
        let (w, h) = self.replies.cell.unwrap_or(fallback);
        Capabilities {
            kitty_graphics: self.replies.kitty == Some(true),
            synchronized_updates: self.replies.synchronized == Some(true),
            cell_width: w.max(1),
            cell_height: h.max(1),
            measured_cell: self.replies.cell.is_some(),
            foreground: self.replies.foreground.clone(),
            background: self.replies.background.clone(),
        }
    }
    fn take_input(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies.pending)
    }
}

#[derive(Default, Debug)]
pub struct Replies {
    pub cell: Option<(u16, u16)>,
    pub kitty: Option<bool>,
    pub synchronized: Option<bool>,
    pub pending: Vec<u8>,
    pub foreground: Option<String>,
    pub background: Option<String>,
}
impl Replies {
    pub fn push(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        let mut i = 0;
        while i < self.pending.len() {
            let tail = &self.pending[i..];
            let end = if tail.starts_with(b"\x1b_G") {
                tail.windows(2).position(|v| v == b"\x1b\\").map(|n| n + 2)
            } else if tail.starts_with(b"\x1b]") {
                tail.iter()
                    .position(|b| *b == 7)
                    .map(|n| n + 1)
                    .or_else(|| tail.windows(2).position(|v| v == b"\x1b\\").map(|n| n + 2))
            } else if tail.starts_with(b"\x1b[") {
                tail.iter()
                    .enumerate()
                    .skip(2)
                    .find(|(_, b)| (0x40..=0x7e).contains(*b))
                    .map(|(n, _)| n + 1)
            } else {
                None
            };
            let Some(end) = end else {
                i += 1;
                continue;
            };
            let reply = self.pending[i..i + end].to_vec();
            if self.consume(&reply) {
                self.pending.drain(i..i + end);
            } else {
                i += end;
            }
        }
    }
    fn consume(&mut self, bytes: &[u8]) -> bool {
        let Ok(s) = std::str::from_utf8(bytes) else {
            return false;
        };
        if let Some(body) = s
            .strip_prefix("\x1b]")
            .and_then(|s| s.strip_suffix('\x07').or_else(|| s.strip_suffix("\x1b\\")))
        {
            if let Some((code, value)) = body.split_once(';') {
                if let Some(color) = parse_color(value) {
                    match code {
                        "10" => {
                            self.foreground = Some(color);
                            return true;
                        }
                        "11" => {
                            self.background = Some(color);
                            return true;
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(body) = s
            .strip_prefix("\x1b_G")
            .and_then(|s| s.strip_suffix("\x1b\\"))
        {
            if let Some((params, response)) = body.split_once(';') {
                if params.split(',').any(|p| p == format!("i={PROBE_ID}")) {
                    self.kitty = Some(response == "OK");
                    return true;
                }
            }
        }
        if let Some(body) = s.strip_prefix("\x1b[6;").and_then(|s| s.strip_suffix('t')) {
            if let Some((h, w)) = body.split_once(';') {
                if let (Ok(h), Ok(w)) = (h.parse::<u16>(), w.parse::<u16>()) {
                    if h > 0 && w > 0 {
                        self.cell = Some((w, h));
                    }
                    return true;
                }
            }
        }
        if let Some(body) = s
            .strip_prefix("\x1b[?2026;")
            .and_then(|s| s.strip_suffix("$y"))
        {
            if let Ok(state @ 0..=4) = body.parse::<u8>() {
                self.synchronized = Some(matches!(state, 1..=3));
                return true;
            }
        }
        false
    }
    pub fn complete(&self) -> bool {
        self.cell.is_some()
            && self.kitty.is_some()
            && self.synchronized.is_some()
            && self.foreground.is_some()
            && self.background.is_some()
    }
}
fn parse_color(value: &str) -> Option<String> {
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(format!("#{hex}").to_lowercase());
        }
    }
    let channels: Vec<_> = value.strip_prefix("rgb:")?.split('/').collect();
    if channels.len() != 3 {
        return None;
    }
    let mut color = String::from("#");
    for channel in channels {
        if channel.is_empty() || channel.len() > 4 {
            return None;
        }
        let n = u32::from_str_radix(channel, 16).ok()?;
        let max = (1u32 << (channel.len() * 4)) - 1;
        color += &format!("{:02x}", (n * 255 + max / 2) / max);
    }
    Some(color)
}
pub fn name() -> String {
    for (key, name) in [
        ("KITTY_WINDOW_ID", "Kitty"),
        ("WEZTERM_PANE", "WezTerm"),
        ("KONSOLE_VERSION", "Konsole"),
    ] {
        if std::env::var_os(key).is_some() {
            return name.into();
        }
    }
    std::env::var("TERM_PROGRAM")
        .or_else(|_| std::env::var("TERM"))
        .unwrap_or_else(|_| "unknown".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_reordered_replies_preserve_input() {
        let stream = format!(
            "typed\x1b]10;rgb:ffff/ffff/ffff\x07\x1b]11;rgb:2828/2c2c/3434\x1b\\\x1b[?2026;2$y\x1b_Gi={PROBE_ID};OK\x1b\\\x1b[6;34;16t\x1b[A後\n"
        );
        for split in 0..=stream.len() {
            let mut r = Replies::default();
            r.push(&stream.as_bytes()[..split]);
            r.push(&stream.as_bytes()[split..]);
            assert!(r.complete());
            assert_eq!(r.cell, Some((16, 34)));
            assert_eq!(r.foreground.as_deref(), Some("#ffffff"));
            assert_eq!(r.background.as_deref(), Some("#282c34"));
            assert_eq!(r.kitty, Some(true));
            assert_eq!(r.synchronized, Some(true));
            assert_eq!(r.pending, "typed\x1b[A後\n".as_bytes());
        }
    }
    #[test]
    fn negative_unknown_and_malformed_replies() {
        let mut r = Replies::default();
        r.push(
            format!("\x1b_Gi={PROBE_ID};ENOTSUP: unsupported\x1b\\\x1b[?2026;0$y\x1b[6;0;0t")
                .as_bytes(),
        );
        assert_eq!(r.kitty, Some(false));
        assert_eq!(r.synchronized, Some(false));
        assert_eq!(r.cell, None);
        let other = b"\x1b_Gi=42;OK\x1b\\\x1b[6;bad;16t\x1b[?2026;99$y\x1b[1;2R";
        r.push(other);
        assert_eq!(r.pending, other);
    }
}
