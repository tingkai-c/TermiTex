//! Visible grid structure, independent of math syntax. A horizontal rule and
//! its junctions establish columns; rules divide logical rows. No geometry is
//! inferred from vertical bars in ordinary text or from off-screen history.
use vt100::Screen;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub left: u16,
    pub right: u16,
    pub top: u16,
    pub bottom: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn screen(text: &str) -> vt100::Parser {
        let mut parser = vt100::Parser::new(20, 80, 0);
        parser.process(text.as_bytes());
        parser
    }
    #[test]
    fn rules_define_columns_and_separate_records() {
        let p = screen(
            "┌──────┬─────┐\r\n│ 中文 │ x   │\r\n│      │ y   │\r\n├──────┼─────┤\r\n│ next │ z   │\r\n└──────┴─────┘",
        );
        let t = Tables::detect(p.screen(), 20);
        assert_eq!(
            t.cell(1, 9),
            Some(Cell {
                left: 8,
                right: 13,
                top: 1,
                bottom: 3
            })
        );
        assert_eq!(t.cell(1, 9), t.cell(2, 9));
        assert_ne!(t.cell(2, 9), t.cell(4, 9));
        assert!(t.cell(3, 9).is_none());
        assert_eq!(t.boundaries(1), &[0, 7, 13]);
    }
    #[test]
    fn viewport_can_use_a_visible_bottom_rule_but_not_bars_alone() {
        let p = screen("│ x │ y │\r\n│ z │ w │\r\n└───┴───┘");
        assert!(Tables::detect(p.screen(), 20).cell(0, 2).is_some());
        assert!(Tables::detect(p.screen(), 2).cell(0, 2).is_none());
        for text in [
            "│ x │ y │\r\n│ z │ w │",
            "│ x │ y │\r\n└───┴────┘",
            "│ x │ y │\r\n└──x┴───┘",
            "│ x │ y │\r\n┌───┬───┐",
        ] {
            let p = screen(text);
            assert!(
                Tables::detect(p.screen(), 20).cell(0, 2).is_none(),
                "{text}"
            );
        }
    }
    #[test]
    fn supports_grid_styles_without_treating_content_bars_as_columns() {
        for (top, data, bottom) in [
            ("+-------+---+", "| a|b   | z |", "+-------+---+"),
            ("┏━━━━━━━┳━━━┓", "┃ a│b   ┃ z ┃", "┗━━━━━━━┻━━━┛"),
            ("╔═══════╦═══╗", "║ a|b   ║ z ║", "╚═══════╩═══╝"),
        ] {
            let p = screen(&format!("  {top}\r\n  {data}\r\n  {bottom}"));
            let t = Tables::detect(p.screen(), 20);
            assert_eq!(t.boundaries(1), &[2, 10, 14]);
            assert_eq!(t.cell(1, 4), t.cell(1, 8));
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Band {
    columns: Vec<u16>,
    top: u16,
    bottom: u16,
}
struct Rule {
    columns: Vec<u16>,
    vertical: char,
    up: bool,
    down: bool,
}
pub struct Tables {
    rows: Vec<Option<Band>>,
}
fn glyph(screen: &Screen, row: u16, col: u16) -> char {
    screen
        .cell(row, col)
        .and_then(|c| c.contents().chars().next())
        .unwrap_or(' ')
}
fn rule(screen: &Screen, row: u16) -> Option<Rule> {
    let width = screen.size().1;
    let left = (0..width).find(|&c| glyph(screen, row, c) != ' ')?;
    let right = (left..width)
        .rev()
        .find(|&c| glyph(screen, row, c) != ' ')?;
    let (middle, end, horizontal, vertical, up, down) = match glyph(screen, row, left) {
        '┌' => ('┬', '┐', '─', '│', false, true),
        '╭' => ('┬', '╮', '─', '│', false, true),
        '├' => ('┼', '┤', '─', '│', true, true),
        '└' => ('┴', '┘', '─', '│', true, false),
        '╰' => ('┴', '╯', '─', '│', true, false),
        '┏' => ('┳', '┓', '━', '┃', false, true),
        '┣' => ('╋', '┫', '━', '┃', true, true),
        '┗' => ('┻', '┛', '━', '┃', true, false),
        '╔' => ('╦', '╗', '═', '║', false, true),
        '╠' => ('╬', '╣', '═', '║', true, true),
        '╚' => ('╩', '╝', '═', '║', true, false),
        '+' => ('+', '+', '-', '|', true, true),
        _ => return None,
    };
    if glyph(screen, row, right) != end || screen.row_wrapped(row) {
        return None;
    }
    let mut columns = vec![left];
    let mut segment = 0;
    for col in left + 1..right {
        match glyph(screen, row, col) {
            c if c == horizontal => segment += 1,
            c if c == middle && segment > 0 => {
                columns.push(col);
                segment = 0;
            }
            _ => return None,
        }
    }
    if segment == 0 || columns.len() < 2 {
        return None;
    }
    columns.push(right);
    Some(Rule {
        columns,
        vertical,
        up,
        down,
    })
}
fn content(screen: &Screen, row: u16, rule: &Rule) -> bool {
    let left = rule.columns[0];
    let right = *rule.columns.last().unwrap();
    !screen.row_wrapped(row)
        && rule
            .columns
            .iter()
            .all(|&c| glyph(screen, row, c) == rule.vertical)
        && (0..left)
            .chain(right + 1..screen.size().1)
            .all(|c| glyph(screen, row, c) == ' ')
}
impl Tables {
    pub fn detect(screen: &Screen, end: u16) -> Self {
        let end = end.min(screen.size().0);
        let mut rows = vec![None; end as usize];
        let mut ambiguous = vec![false; end as usize];
        for row in 0..end {
            let Some(rule) = rule(screen, row) else {
                continue;
            };
            for direction in [-1i32, 1] {
                if (direction < 0 && !rule.up) || (direction > 0 && !rule.down) {
                    continue;
                }
                let mut next = row as i32 + direction;
                let mut members = Vec::new();
                while next >= 0 && next < end as i32 && content(screen, next as u16, &rule) {
                    members.push(next as u16);
                    next += direction;
                }
                let (Some(&top), Some(&bottom)) = (members.iter().min(), members.iter().max())
                else {
                    continue;
                };
                let band = Band {
                    columns: rule.columns.clone(),
                    top,
                    bottom: bottom + 1,
                };
                for member in members {
                    let index = member as usize;
                    if rows[index].as_ref().is_some_and(|old| old != &band) {
                        ambiguous[index] = true;
                    }
                    rows[index] = Some(band.clone());
                }
            }
        }
        for (row, ambiguous) in rows.iter_mut().zip(ambiguous) {
            if ambiguous {
                *row = None;
            }
        }
        Self { rows }
    }
    pub fn cell(&self, row: u16, col: u16) -> Option<Cell> {
        let band = self.rows.get(row as usize)?.as_ref()?;
        let pair = band.columns.windows(2).find(|p| p[0] < col && col < p[1])?;
        Some(Cell {
            left: pair[0] + 1,
            right: pair[1],
            top: band.top,
            bottom: band.bottom,
        })
    }
    pub fn boundaries(&self, row: u16) -> &[u16] {
        self.rows
            .get(row as usize)
            .and_then(Option::as_ref)
            .map(|band| band.columns.as_slice())
            .unwrap_or(&[])
    }
}
