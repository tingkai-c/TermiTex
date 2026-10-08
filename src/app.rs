//! App-specific editable-region policy, independent of graphics and layout.
use std::path::Path;
pub trait InputPolicy {
    /// First row excluded from math detection; remaining rows are editable/UI.
    fn output_end(&self, screen: &vt100::Screen) -> u16;
}
#[derive(Clone, Copy)]
pub enum App {
    Generic,
    Codex,
    Claude,
}
impl App {
    pub fn for_command(command: &str) -> Self {
        match Path::new(command).file_name().and_then(|s| s.to_str()) {
            Some("codex") => Self::Codex,
            Some("claude") => Self::Claude,
            _ => Self::Generic,
        }
    }
}
fn row(screen: &vt100::Screen, r: u16) -> String {
    (0..screen.size().1)
        .filter_map(|c| screen.cell(r, c))
        .map(|c| {
            let s = c.contents();
            if s.is_empty() { " " } else { s }
        })
        .collect()
}
fn border(text: &str) -> bool {
    let text = text.trim();
    text.chars().count() >= 8 && text.chars().all(|c| matches!(c, '─' | '━' | '-'))
}
fn prompt(text: &str, marker: char) -> bool {
    text.strip_prefix(marker)
        .is_some_and(|rest| rest.chars().next().is_none_or(char::is_whitespace))
}
impl InputPolicy for App {
    fn output_end(&self, screen: &vt100::Screen) -> u16 {
        let rows = screen.size().0;
        if matches!(self, Self::Generic) {
            return rows;
        }
        if matches!(self, Self::Claude) {
            // Locate the bounded composer first: draft content may itself
            // contain response bullets, which must not terminate this search.
            let borders: Vec<_> = (0..rows).filter(|&r| border(&row(screen, r))).collect();
            if borders.len() >= 2 {
                let bottom = borders[borders.len() - 1];
                let top = borders[borders.len() - 2];
                if top + 1 < bottom {
                    let first = row(screen, top + 1);
                    let text = first.trim_start();
                    if prompt(text, '❯') || prompt(text, '>') {
                        return top + 1;
                    }
                }
            }
        }
        for r in (0..rows).rev() {
            let text = row(screen, r);
            let t = text.trim_start();
            let prompt = match self {
                Self::Codex => prompt(t, '›') || prompt(t, '❯'),
                Self::Claude => {
                    prompt(t, '❯')
                        || (prompt(t, '>') && r > 0 && {
                            let previous = row(screen, r - 1);
                            border(&previous)
                        })
                }
                Self::Generic => false,
            };
            if prompt {
                return r;
            }
            // A response below an old prompt belongs to output, not the editor.
            if t.starts_with("• ")
                || t.starts_with("● ")
                || (matches!(self, Self::Claude) && t.starts_with("⏺ "))
            {
                break;
            }
        }
        rows
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claude_scrolled_history_with_nbsp_composer_keeps_output_visible() {
        for whitespace in [" ", "\u{a0}", "\u{2003}", ""] {
            let mut p = vt100::Parser::new(12, 80, 0);
            p.process(format!("❯ Explain Maxwell's equations\r\n\\[\r\n\\nabla\\cdot B=0\r\n\\]\r\nCurrent \\(J\\)\x1b[8;1H─────────────────\r\n❯{whitespace}\r\n─────────────────\r\nauto mode on").as_bytes());
            let end = App::Claude.output_end(p.screen());
            assert_eq!(end, 8);
            assert_eq!(
                crate::detect::detect_output(p.screen(), "#fff", "#000", end).len(),
                2
            );
        }
        assert!(!prompt("❯not a prompt", '❯'));
    }
    #[test]
    fn claude_draft_is_not_detected_as_math() {
        let mut p = vt100::Parser::new(12, 80, 0);
        p.process("⏺ Result \\(x^2\\)\r\n────────────────\r\n❯ \\(draft\\)\r\n  • \\(continued\\)\r\n────────────────".as_bytes());
        let formulas = crate::detect::detect_output(
            p.screen(),
            "#fff",
            "#000",
            App::Claude.output_end(p.screen()),
        );
        assert_eq!(formulas.len(), 1);
        assert_eq!(formulas[0].latex, "x^2");
    }
    #[test]
    fn claude_multiline_composer_and_history() {
        for marker in ["❯", ">"] {
            let mut p = vt100::Parser::new(12, 80, 0);
            p.process(format!("⏺ Answer \\(x^2\\)\r\n─────────────────\r\n{marker} \\(draft\\)\r\n  • \\(continued\\)\r\n─────────────────\r\n? for shortcuts").as_bytes());
            assert_eq!(App::Claude.output_end(p.screen()), 2);
            assert_eq!(App::Generic.output_end(p.screen()), 12);
        }
        let mut p = vt100::Parser::new(12, 80, 0);
        p.process("❯ old prompt\r\n⏺ Answer \\(x\\)\r\n> quoted text".as_bytes());
        assert_eq!(App::Claude.output_end(p.screen()), 12);
    }
}
