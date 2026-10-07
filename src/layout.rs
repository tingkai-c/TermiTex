//! Project compact inline math into terminal cells, retaining native prose.
use vt100::{Cell, Color, Screen};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Style {
    fg: Color,
    bg: Color,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
}
impl Style {
    fn of(c: &Cell) -> Self {
        Self {
            fg: c.fgcolor(),
            bg: c.bgcolor(),
            bold: c.bold(),
            dim: c.dim(),
            italic: c.italic(),
            underline: c.underline(),
            inverse: c.inverse(),
        }
    }
    fn plain() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }
    fn ansi(&self) -> String {
        let mut codes = vec!["0".to_owned()];
        for (on, code) in [
            (self.bold, "1"),
            (self.dim, "2"),
            (self.italic, "3"),
            (self.underline, "4"),
            (self.inverse, "7"),
        ] {
            if on {
                codes.push(code.into());
            }
        }
        for (c, base) in [(self.fg, 38), (self.bg, 48)] {
            match c {
                Color::Default => {}
                Color::Idx(i) => codes.push(format!("{base};5;{i}")),
                Color::Rgb(r, g, b) => codes.push(format!("{base};2;{r};{g};{b}")),
            }
        }
        format!("\x1b[{}m", codes.join(";"))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Slot {
    text: String,
    continuation: bool,
    style: Style,
}
impl Slot {
    fn of(c: &Cell) -> Self {
        Self {
            text: if c.contents().is_empty() {
                " ".into()
            } else {
                c.contents().into()
            },
            continuation: c.is_wide_continuation(),
            style: Style::of(c),
        }
    }
    fn blank(style: Style) -> Self {
        Self {
            text: " ".into(),
            continuation: false,
            style,
        }
    }
}
/// Source coordinates, original source width, and rendered width, in cells.
pub type Span = (u16, u16, u16);
pub fn shifted(col: u16, spans: &[Span]) -> u16 {
    col - spans
        .iter()
        .filter(|(start, _, _)| *start < col)
        .map(|(_, old, new)| old - new)
        .sum::<u16>()
}
pub fn paint_row(source: &Screen, physical: &Screen, row: u16, spans: &[Span]) -> String {
    let cols = source.size().1;
    let mut desired = Vec::with_capacity(cols as usize);
    let mut c = 0;
    while c < cols {
        if let Some(&(_, old, new)) = spans.iter().find(|(start, _, _)| *start == c) {
            let style = Style::of(source.cell(row, c).unwrap());
            desired.extend((0..new).map(|_| Slot::blank(style.clone())));
            c += old;
        } else {
            desired.push(Slot::of(source.cell(row, c).unwrap()));
            c += 1;
        }
    }
    desired.resize_with(cols as usize, || Slot::blank(Style::plain()));
    if desired
        .iter()
        .enumerate()
        .all(|(c, s)| *s == Slot::of(physical.cell(row, c as u16).unwrap()))
    {
        return String::new();
    }
    let mut out = format!("\x1b[{};1H", row + 1);
    let mut previous = None;
    for slot in desired {
        if slot.continuation {
            continue;
        }
        if previous.as_ref() != Some(&slot.style) {
            out += &slot.style.ansi();
            previous = Some(slot.style);
        }
        out += &slot.text;
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_native_prose_preserves_styles_and_unicode_and_restores() {
        let mut source = vt100::Parser::new(4, 80, 0);
        source.process(
            "Here \\(\\rho\\) is \x1b[1;38;5;12mcharge\x1b[0m 中文 and \\(B\\).".as_bytes(),
        );
        let mut physical = vt100::Parser::new(4, 80, 0);
        physical.process(&source.screen().contents_formatted());
        let spans = [(5, 8, 2), (33, 5, 2)];
        // Locate the second expression from the source to avoid byte/cell confusion.
        let second = (0..80)
            .find(|&c| source.screen().cell(0, c).unwrap().contents() == "B")
            .unwrap()
            - 2;
        let spans = [spans[0], (second, 5, 2)];
        let paint = paint_row(source.screen(), physical.screen(), 0, &spans);
        physical.process(paint.as_bytes());
        assert!(
            physical
                .screen()
                .contents()
                .starts_with("Here    is charge 中文 and   ."),
            "{}",
            physical.screen().contents()
        );
        let charge = (0..80)
            .find(|&c| physical.screen().cell(0, c).unwrap().contents() == "c")
            .unwrap();
        assert!(physical.screen().cell(0, charge).unwrap().bold());
        assert_eq!(
            physical.screen().cell(0, charge).unwrap().fgcolor(),
            Color::Idx(12)
        );
        assert!(paint_row(source.screen(), physical.screen(), 0, &spans).is_empty());
        physical.process(paint_row(source.screen(), physical.screen(), 0, &[]).as_bytes());
        assert_eq!(
            physical.screen().contents().trim_end(),
            source.screen().contents().trim_end()
        );
        assert_eq!(shifted(second, &spans), second - 6);
    }
}
