//! Kitty graphics transport shared by Ghostty, Kitty, WezTerm, iTerm2 and Konsole.
//! Keep protocol details out of screen reconciliation and math layout.
const Z: u32 = 20_270_001;
pub const PROBE_ID: u32 = 1_799_999_999;
pub fn query() -> String {
    format!("\x1b_Ga=q,t=d,f=24,s=1,v=1,i={PROBE_ID};AAAA\x1b\\")
}
#[derive(Clone, Copy, Debug)]
pub struct Rectangle {
    pub row: u16,
    pub col: u16,
    pub cols: u16,
    pub rows: u16,
}

/// Encodes graphics operations; never writes to the terminal itself. The session
/// owns output ordering so text and placements can be committed together.
pub trait GraphicsBackend {
    fn upload(&self, id: u32, png: &str) -> String;
    fn place(&self, id: u32, placement: u32, rect: Rectangle) -> String;
    fn remove(&self, id: u32, placement: u32) -> String;
    fn release(&self, id: u32) -> String;
    fn cleanup(&self) -> String;
}

pub struct KittyGraphics;
impl GraphicsBackend for KittyGraphics {
    fn cleanup(&self) -> String {
        format!("\x1b_Ga=d,d=Z,z={Z},q=2\x1b\\")
    }
    fn remove(&self, id: u32, placement: u32) -> String {
        format!("\x1b_Ga=d,d=i,i={id},p={placement},q=2\x1b\\")
    }
    fn release(&self, id: u32) -> String {
        format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\")
    }
    fn place(&self, id: u32, placement: u32, rect: Rectangle) -> String {
        let Rectangle {
            row,
            col,
            cols,
            rows,
        } = rect;
        format!(
            "\x1b[{};{}H\x1b_Ga=p,i={id},p={placement},q=2,c={cols},r={rows},C=1,z={Z}\x1b\\",
            row + 1,
            col + 1
        )
    }
    fn upload(&self, id: u32, png: &str) -> String {
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
}
