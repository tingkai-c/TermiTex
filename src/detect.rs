use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceSpan {
    pub row: u16,
    pub col: u16,
    pub cols: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Formula {
    #[serde(default)]
    pub sources: Vec<SourceSpan>,
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
    let mut consumed = std::collections::HashMap::<u16, usize>::new();
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
                    sources: Vec::new(),
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
        let mut offset = consumed.get(&r).copied().unwrap_or(0);
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
                            sources: Vec::new(),
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
                } else if !display {
                    if let Some((formula, ends)) =
                        wrapped(s, &lines, r, offset, open, close, end, fg, bg)
                    {
                        for (row, finish) in ends {
                            consumed.insert(row, finish);
                        }
                        result.push(formula);
                        break;
                    }
                }
            }
            offset += ch.len_utf8();
        }
        r += 1;
    }
    result
}
// Codex can wrap with explicit cursor positioning, so terminal soft-wrap flags
// alone are insufficient. Join only a bounded, fully visible delimited span.
#[allow(clippy::too_many_arguments)]
fn wrapped(
    s: &vt100::Screen,
    lines: &[(String, Vec<u16>)],
    row: u16,
    start: usize,
    open: &str,
    close: &str,
    end: u16,
    fg: &str,
    bg: &str,
) -> Option<(Formula, Vec<(u16, usize)>)> {
    let first = &lines[row as usize];
    let first_end = first.0.trim_end().len();
    let mut body = first.0[start + open.len()..first_end].to_string();
    let mut sources = vec![SourceSpan {
        row,
        col: first.1[start],
        cols: first.1[first_end] - first.1[start],
    }];
    let mut ends = Vec::new();
    for next in row + 1..end.min(row.saturating_add(9)) {
        let (text, map) = &lines[next as usize];
        let trim = text.trim();
        if trim.is_empty()
            || ["› ", "❯ ", "• ", "● ", "#", "```", "~~~", "\\[", "\\]"]
                .iter()
                .any(|v| trim.starts_with(v))
        {
            return None;
        }
        let begin = if s.row_wrapped(next - 1) {
            0
        } else {
            text.len() - text.trim_start().len()
        };
        let last = text.trim_end().len();
        let part = &text[begin..last];
        let closing = part.find(close);
        if open != close
            && part
                .find(open)
                .is_some_and(|n| closing.is_none_or(|c| n < c))
        {
            return None;
        }
        if !s.row_wrapped(next - 1) {
            body.push(' ');
        }
        if let Some(n) = closing {
            body.push_str(&part[..n]);
            if body.len() > 20000 || (open == "$" && !body.contains(['\\', '^', '_', '=', '+'])) {
                return None;
            }
            let finish = begin + n + close.len();
            sources.push(SourceSpan {
                row: next,
                col: map[begin],
                cols: map[finish] - map[begin],
            });
            ends.push((next, finish));
            // Keep the equation intact on the source row with the most room.
            // Other source fragments disappear; surrounding prose remains native.
            let target = sources.iter().max_by_key(|span| span.cols)?;
            let cell = s.cell(row, first.1[start])?;
            let formula = Formula {
                latex: body,
                row: target.row,
                col: target.col,
                rows: 1,
                cols: target.cols,
                display: false,
                fg: color(cell.fgcolor(), fg),
                bg: color(cell.bgcolor(), bg),
                sources,
            };
            return Some((formula, ends));
        }
        body.push_str(part);
        sources.push(SourceSpan {
            row: next,
            col: map[begin],
            cols: map[last] - map[begin],
        });
        ends.push((next, last));
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapped_inline_explicit_rows_and_following_formula() {
        let mut p = vt100::Parser::new(8, 70, 0);
        p.process(b"Here \\(\\partial/\r\n  \\partial t\\) means time; \\(B\\) too.");
        let f = detect(p.screen(), "#fff", "#000");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].latex, r"\partial/ \partial t");
        assert_eq!(f[0].sources.len(), 2);
        assert_eq!(f[0].sources[1].col, 2);
        assert_eq!(f[1].latex, "B");
    }
    #[test]
    fn wrapped_command_softwrap_and_boundaries() {
        let mut p = vt100::Parser::new(8, 16, 0);
        p.process(b"0123456789 \\(\\partial t\\)");
        assert!(p.screen().row_wrapped(0));
        let f = detect(p.screen(), "#fff", "#000");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].latex, r"\partial t");
        for separator in ["\r\n\r\n", "\r\n› ", "\r\n```\r\n", "\r\n• "] {
            let mut p = vt100::Parser::new(8, 70, 0);
            p.process(format!("open \\(x+{separator}y\\)").as_bytes());
            assert!(detect(p.screen(), "#fff", "#000").is_empty());
        }
    }
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
