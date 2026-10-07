use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Formula {
    pub latex: String,
    pub row: u16,
    pub col: u16,
    pub rows: u16,
    pub cols: u16,
    pub display: bool,
    pub fg: String,
    pub bg: String,
}
fn color(c: vt100::Color, default: &str) -> String {
    match c {
        vt100::Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => default.to_string(),
    }
}
fn line(s: &vt100::Screen, r: u16) -> (String, Vec<u16>) {
    let mut text = String::new();
    let mut map = Vec::new();
    for c in 0..s.size().1 {
        if let Some(cell) = s.cell(r, c) {
            if cell.is_wide_continuation() {
                continue;
            }
            let v = cell.contents();
            let v = if v.is_empty() { " " } else { v };
            map.extend(std::iter::repeat_n(c, v.len()));
            text.push_str(v);
        }
    }
    map.push(s.size().1);
    (text, map)
}
pub fn detect(s: &vt100::Screen, fg: &str, bg: &str) -> Vec<Formula> {
    let (rows, cols) = s.size();
    let lines: Vec<_> = (0..rows).map(|r| line(s, r)).collect();
    let mut end = rows;
    for r in (0..rows).rev() {
        let t = lines[r as usize].0.trim_start();
        if t.starts_with("› ") || t.starts_with("❯ ") {
            end = r;
            break;
        }
        if t.starts_with("• ") || t.starts_with("● ") {
            break;
        }
    }
    let mut result = Vec::new();
    let mut r = 0;
    let mut fenced = false;
    while r < end {
        let (text, map) = &lines[r as usize];
        let trim = text.trim();
        if trim.starts_with("```") || trim.starts_with("~~~") {
            fenced = !fenced;
            r += 1;
            continue;
        }
        if fenced {
            r += 1;
            continue;
        }
        if trim == "\\[" || trim == "$$" {
            let close = if trim == "\\[" { "\\]" } else { "$$" };
            let mut body = String::new();
            let mut stop = r + 1;
            while stop < end && stop - r <= 32 {
                let t = lines[stop as usize].0.trim();
                if t == close {
                    break;
                }
                body.push_str(t);
                body.push('\n');
                stop += 1;
            }
            if stop < end && lines[stop as usize].0.trim() == close && !body.trim().is_empty() {
                // Mask only the display block; never include surrounding prose.
                result.push(Formula {
                    latex: body.trim().into(),
                    row: r,
                    col: 0,
                    rows: stop - r + 1,
                    cols,
                    display: true,
                    fg: fg.into(),
                    bg: bg.into(),
                });
                r = stop + 1;
                continue;
            }
        }
        let mut offset = 0;
        let mut code = false;
        while offset < text.len() {
            let rest = &text[offset..];
            let ch = rest.chars().next().unwrap();
            if ch == '`' {
                code = !code;
                offset += 1;
                continue;
            }
            let pair = if !code && rest.starts_with("\\(") {
                Some(("\\(", "\\)", false))
            } else if !code && rest.starts_with("\\[") {
                Some(("\\[", "\\]", true))
            } else if !code && rest.starts_with("$$") {
                Some(("$$", "$$", true))
            } else if !code && rest.starts_with('$') {
                Some(("$", "$", false))
            } else {
                None
            };
            if let Some((open, close, display)) = pair {
                if let Some(n) = text[offset + open.len()..].find(close) {
                    let finish = offset + open.len() + n + close.len();
                    let body = &text[offset + open.len()..offset + open.len() + n];
                    if !body.trim().is_empty()
                        && (open != "$" || body.contains(['\\', '^', '_', '=', '+']))
                        && body.len() < 20000
                    {
                        let c = map[offset];
                        let cell = s.cell(r, c).unwrap();
                        result.push(Formula {
                            latex: body.into(),
                            row: r,
                            col: c,
                            rows: 1,
                            cols: map[finish] - c,
                            display,
                            fg: color(cell.fgcolor(), fg),
                            bg: color(cell.bgcolor(), bg),
                        });
                    }
                    offset = finish;
                    continue;
                }
            }
            offset += ch.len_utf8();
        }
        r += 1;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_math_and_not_composer() {
        let mut p = vt100::Parser::new(20, 100, 0);
        p.process(
            "• Here \\(J\\) is current density.\r\n\r\n\\[\r\n\\frac{1}{2}\r\n\\]\r\n› \\(input\\)"
                .as_bytes(),
        );
        let f = detect(p.screen(), "#fff", "#000");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].latex, "J");
        assert_eq!(f[0].cols, 5);
        assert_eq!(f[1].rows, 3);
    }
    #[test]
    fn code_and_unicode() {
        let mut p = vt100::Parser::new(10, 100, 0);
        p.process("• 中文 \\(x^2\\) and `\\(code\\)`".as_bytes());
        let f = detect(p.screen(), "#fff", "#000");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].col, 7);
    }
}
