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
fn escaped(text: &str, offset: usize) -> bool {
    text.as_bytes()[..offset]
        .iter()
        .rev()
        .take_while(|&&b| b == b'\\')
        .count()
        % 2
        == 1
}
fn dollar_run(text: &str, offset: usize, len: usize) -> bool {
    (offset == 0 || text.as_bytes()[offset - 1] != b'$')
        && text.as_bytes().get(offset + len) != Some(&b'$')
}
// Pandoc-style dollar boundaries: no space after an opener or before a
// closer, and no digit immediately after a closer. Escapes apply to both.
// https://pandoc.org/MANUAL.html#math
fn dollar_open(text: &str, offset: usize) -> bool {
    dollar_run(text, offset, 1)
        && text[offset + 1..]
            .chars()
            .next()
            .is_some_and(|c| !c.is_whitespace())
}
fn closing(text: &str, start: usize, close: &str) -> Option<usize> {
    text[start..].match_indices(close).find_map(|(n, _)| {
        let offset = start + n;
        if escaped(text, offset)
            || (close.starts_with('$') && !dollar_run(text, offset, close.len()))
        {
            return None;
        }
        if close == "$"
            && (text[..offset]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace)
                || text[offset + 1..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit()))
        {
            return None;
        }
        Some(offset)
    })
}
#[cfg(test)]
pub fn detect(s: &vt100::Screen, fg: &str, bg: &str) -> Vec<Formula> {
    use crate::app::InputPolicy;
    detect_output(s, fg, bg, crate::app::App::Codex.output_end(s))
}
pub fn detect_output(s: &vt100::Screen, fg: &str, bg: &str, end: u16) -> Vec<Formula> {
    let (rows, cols) = s.size();
    let lines: Vec<_> = (0..rows).map(|r| line(s, r)).collect();
    let mut result = Vec::new();
    let mut consumed = std::collections::HashMap::<u16, usize>::new();
    let mut r = 0;
    let mut fenced: Option<(u8, usize)> = None;
    while r < end {
        let (text, map) = &lines[r as usize];
        let trim = text.trim();
        let marker = trim.as_bytes().first().copied().unwrap_or(b' ');
        let run = trim.bytes().take_while(|&b| b == marker).count();
        if let Some((fence_marker, fence_len)) = fenced {
            if marker == fence_marker && run >= fence_len && trim[run..].trim().is_empty() {
                fenced = None;
            }
            r += 1;
            continue;
        }
        if matches!(marker, b'`' | b'~') && run >= 3 {
            fenced = Some((marker, run));
            r += 1;
            continue;
        }
        // Codex may put its response marker on the opening delimiter's row.
        // Only accept a marker-only prefix, never arbitrary prose or code.
        let display_open = ["• ", "● "]
            .iter()
            .find_map(|prefix| trim.strip_prefix(prefix))
            .map(str::trim_start)
            .unwrap_or(trim);
        if display_open == "\\[" || display_open == "$$" {
            let close = if display_open == "\\[" { "\\]" } else { "$$" };
            let display_col = if display_open == trim {
                0
            } else {
                map[text.find(display_open).unwrap()]
            };
            let mut body = String::new();
            let mut stop = r + 1;
            while stop < end && stop - r <= 32 {
                let t = lines[stop as usize].0.trim();
                if t == close || (display_open == "$$" && t.is_empty()) {
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
                    col: display_col,
                    rows: stop - r + 1,
                    cols: cols - display_col,
                    display: true,
                    fg: fg.into(),
                    bg: bg.into(),
                });
                r = stop + 1;
                continue;
            }
        }
        let mut offset = consumed.get(&r).copied().unwrap_or(0);
        // Include a soft-wrapped successor for delimiter lookahead (notably
        // a digit after a closing dollar at the right edge of the screen).
        let scan_text = if s.row_wrapped(r) && r + 1 < end {
            format!("{text}{}", lines[r as usize + 1].0)
        } else {
            text.clone()
        };
        let mut code = None;
        while offset < text.len() {
            let rest = &text[offset..];
            let ch = rest.chars().next().unwrap();
            if ch == '`' && (code.is_some() || !escaped(text, offset)) {
                let run = rest.bytes().take_while(|&b| b == b'`').count();
                if code == Some(run) {
                    code = None;
                } else if code.is_none() {
                    code = Some(run);
                }
                offset += run;
                continue;
            }
            let eligible = code.is_none() && !escaped(text, offset);
            let pair = if eligible && rest.starts_with("\\(") {
                Some(("\\(", "\\)", false))
            } else if eligible && rest.starts_with("\\[") {
                Some(("\\[", "\\]", true))
            } else if eligible && rest.starts_with("$$") && dollar_run(&scan_text, offset, 2) {
                Some(("$$", "$$", true))
            } else if eligible && rest.starts_with('$') && dollar_open(&scan_text, offset) {
                Some(("$", "$", false))
            } else {
                None
            };
            if let Some((open, close, display)) = pair {
                if let Some(n) = closing(&scan_text, offset + open.len(), close)
                    .filter(|&n| n + close.len() <= text.len())
                {
                    let finish = n + close.len();
                    let body = &text[offset + open.len()..n];
                    if !body.trim().is_empty() && body.len() < 20000 {
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
    let first_end = if s.row_wrapped(row) {
        first.0.len()
    } else {
        first.0.trim_end().len()
    };
    let mut body = first.0[start + open.len()..first_end].to_string();
    let mut sources = vec![SourceSpan {
        row,
        col: first.1[start],
        cols: first.1[first_end] - first.1[start],
    }];
    let mut ends = Vec::new();
    for next in row + 1..end.min(row.saturating_add(9)) {
        // A dollar in shell/code output is ambiguous. Only join its rows when
        // the terminal confirms a wrap; explicit bracket math retains support
        // for coding CLIs that wrap using cursor positioning.
        if open == "$" && !s.row_wrapped(next - 1) {
            return None;
        }
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
        let last = if s.row_wrapped(next) {
            text.len()
        } else {
            text.trim_end().len()
        };
        let part = &text[begin..last];
        if !s.row_wrapped(next - 1) {
            body.push(' ');
        }
        let previous_len = body.len();
        body.push_str(part);
        // Scan the joined text so escape parity and the character before a
        // closing dollar remain correct at a terminal row boundary.
        let scan_body = if s.row_wrapped(next) && next + 1 < end {
            format!("{body}{}", lines[next as usize + 1].0)
        } else {
            body.clone()
        };
        let closing = closing(&scan_body, previous_len, close)
            .filter(|&n| n + close.len() <= body.len())
            .map(|n| n - previous_len);
        if open != close
            && part
                .find(open)
                .is_some_and(|n| closing.is_none_or(|c| n < c))
        {
            return None;
        }
        if let Some(n) = closing {
            body.truncate(previous_len + n);
            if body.len() > 20000 || body.trim().is_empty() {
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
    fn formulas(input: &str) -> Vec<Formula> {
        let mut p = vt100::Parser::new(20, 100, 0);
        p.process(input.as_bytes());
        detect(p.screen(), "#fff", "#000")
    }
    #[test]
    fn pandoc_dollar_boundaries() {
        for input in ["$x$", "$2$", "$(x+y)^2$", "// $x$", "149 + // $x$"] {
            assert_eq!(formulas(input).len(), 1, "{input}");
        }
        for input in [
            "$ x$",
            "$x $",
            "$ x $",
            "$x$2",
            "$5 + $10",
            "$20,000 and $30,000",
            "$\u{2003}x$",
            "$x\u{2003}$",
            "$$x^2$",
            "$x^2$$",
            "$$$x^2$$$",
        ] {
            assert!(formulas(input).is_empty(), "{input}");
        }
        let f = formulas("中文 $x$ and $2$, then $$ x^2 $$");
        assert_eq!(
            f.iter().map(|f| f.latex.as_str()).collect::<Vec<_>>(),
            ["x", "2", " x^2 "]
        );
        assert_eq!((f[0].col, f[0].cols), (5, 3));
        assert!(f[2].display);
    }
    #[test]
    fn escaped_delimiters() {
        for input in [r"\$x^2\$", r"\\(x^2\\)", r"\$$x^2\$$"] {
            assert!(formulas(input).is_empty(), "{input}");
        }
        assert_eq!(formulas(r"$x+\$5+y$")[0].latex, r"x+\$5+y");
        assert_eq!(formulas(r"\\$x$")[0].latex, "x");
        assert_eq!(formulas(r"\(x+\\)+y\)")[0].latex, r"x+\\)+y");
    }
    #[test]
    fn javascript_diff_and_unrelated_rows_stay_text() {
        let input = "149 - $('math-codex').hidden = key !== 'codex';\r\n150 - $('math-claude').hidden = key !== 'claude';\r\n149 + $('math-codex').toggleAttribute('hidden', key !== 'codex');";
        assert!(formulas(input).is_empty());
        assert!(formulas("$x^2\r\nlog: completed\r\nnext $").is_empty());
        assert!(formulas("$$\r\nx^2\r\n\r\n$$").is_empty());
    }
    #[test]
    fn dollar_softwrap_preserves_escapes_and_boundaries() {
        for (input, expected) in [
            ("123456789 $x+y+z$", Some("x+y+z")),
            ("123456789 $x+\\$5+y$", Some(r"x+\$5+y")),
            ("123456789 $x  $", None),
            ("123456789 $xx$2", None),
            ("123456789 $xxxxxxxxxxxxxxx$2", None),
            ("123456789012$x$", Some("x")),
        ] {
            let mut p = vt100::Parser::new(8, 13, 0);
            p.process(input.as_bytes());
            assert!(p.screen().row_wrapped(0));
            let f = detect(p.screen(), "#fff", "#000");
            assert_eq!(f.first().map(|f| f.latex.as_str()), expected, "{input}");
        }
    }
    #[test]
    fn matching_code_delimiters() {
        for input in [
            "``$x^2$``",
            "``a ` $x$``",
            "```js\r\n~~~\r\n$x$\r\n```",
            "````js\r\n```\r\n$x$\r\n````",
        ] {
            assert!(formulas(input).is_empty(), "{input}");
        }
        assert_eq!(formulas("``$x$`` and $y$")[0].latex, "y");
        assert_eq!(formulas("```js\r\n$x$\r\n```\r\n$y$")[0].latex, "y");
    }
    #[test]
    fn response_marker_before_display_block() {
        let latex = r"c=(\mu_0\varepsilon_0)^{-1/2}\approx3.00\times10^8\,\mathrm{m/s}";
        for marker in ["•", "●"] {
            for (open, close) in [(r"\[", r"\]"), ("$$", "$$")] {
                let mut p = vt100::Parser::new(10, 100, 0);
                p.process(
                    format!("{marker} {open}\r\n  {latex}\r\n  {close}\r\n› input").as_bytes(),
                );
                let f = detect(p.screen(), "#fff", "#000");
                assert_eq!(f.len(), 1);
                assert_eq!(f[0].latex, latex);
                assert!(f[0].display);
                assert_eq!((f[0].row, f[0].col, f[0].rows, f[0].cols), (0, 2, 3, 98));
            }
        }
        for prefix in ["• prose ", "› ", "`", "```\r\n• "] {
            let mut p = vt100::Parser::new(10, 100, 0);
            p.process(format!("{prefix}\\[\r\n  {latex}\r\n  \\]").as_bytes());
            assert!(detect(p.screen(), "#fff", "#000").is_empty());
        }
    }
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
