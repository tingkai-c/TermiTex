//! Isolated RaTeX worker: the PTY process remains responsive if rendering fails.
use crate::engine::{Request, Response};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parser::parse;
use ratex_render::{RenderOptions, render_to_png};
use ratex_types::{color::Color, display_item::DisplayItem, math_style::MathStyle};
use std::io::{self, BufRead, Read, Write};
fn color(text: &str) -> Result<Color, String> {
    let hex = text
        .strip_prefix('#')
        .ok_or("expected #RGB or #RRGGBB color")?;
    let owned;
    if hex.len() == 3 {
        owned = hex.chars().flat_map(|c| [c, c]).collect::<String>();
    } else {
        owned = hex.to_owned();
    }
    if owned.len() != 6 || !owned.is_ascii() {
        return Err("invalid color".into());
    }
    let channel = |start| {
        u8::from_str_radix(&owned[start..start + 2], 16)
            .map(|x| x as f32 / 255.0)
            .map_err(|_| "invalid color".to_string())
    };
    Ok(Color {
        r: channel(0)?,
        g: channel(2)?,
        b: channel(4)?,
        a: 1.0,
    })
}
pub fn render(req: &Request) -> Result<(Vec<u8>, u16), String> {
    let f = &req.formula;
    if f.latex.len() > 20000 {
        return Err("formula too long".into());
    }
    let width = f.cols as u32 * req.cell_width as u32;
    let height = f.rows as u32 * req.cell_height as u32;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("formula canvas exceeds raster limit".into());
    }
    let fg = color(&f.fg)?;
    let bg = color(&f.bg)?;
    let ast = parse(&f.latex).map_err(|e| e.to_string())?;
    let options = LayoutOptions::default()
        .with_style(if f.display {
            MathStyle::Display
        } else {
            MathStyle::Text
        })
        .with_color(fg);
    let mut dl = to_display_list(&layout(&ast, &options));
    if !dl.width.is_finite()
        || !dl.total_height().is_finite()
        || dl.width <= 0.0
        || dl.total_height() <= 0.0
    {
        return Err("empty or invalid formula layout".into());
    }
    let base = req.cell_height as f64 * 0.9;
    let px = if f.display {
        req.cell_width as f64
    } else {
        1.0
    };
    let py = if f.display {
        0.0
    } else {
        (req.cell_height as f64 * 0.08).max(1.0)
    };
    let fit = 1f64
        .min((width as f64 - 2.0 * px).max(1.0) / (dl.width * base))
        .min((height as f64 - 2.0 * py).max(1.0) / (dl.total_height() * base));
    if fit < 0.4 {
        return Err("formula needs more space".into());
    }
    let font_size = (base * fit) as f32;
    let scale = font_size as f64;
    let columns = if !f.display && f.rows == 1 {
        (((px + dl.width * scale) / req.cell_width as f64).ceil() as u16).clamp(1, f.cols)
    } else {
        f.cols
    };
    let canvas_width = columns as f64 * req.cell_width as f64;
    let x = if f.display {
        ((canvas_width - dl.width * scale) / 2.0).max(0.0)
    } else {
        px
    };
    let natural_height = dl.total_height() * scale;
    let y = if f.display {
        ((height as f64 - natural_height) / 2.0).max(0.0)
    } else {
        (req.cell_height as f64 * 0.78 - dl.height * scale)
            .clamp(0.0, (height as f64 - natural_height).max(0.0))
    };
    for item in &mut dl.items {
        let (ix, iy) = match item {
            DisplayItem::GlyphPath { x, y, .. }
            | DisplayItem::Line { x, y, .. }
            | DisplayItem::Rect { x, y, .. }
            | DisplayItem::Path { x, y, .. } => (x, y),
        };
        *ix += x / scale;
        *iy += y / scale;
    }
    // RaTeX's API uses ceil() after f32 multiplication. A subpixel epsilon
    // prevents rounding up into an extra terminal pixel, without scaling glyphs.
    dl.width = (canvas_width - 0.001) / scale;
    dl.height = (height as f64 - 0.001) / scale - dl.depth;
    let png = render_to_png(
        &dl,
        &RenderOptions {
            font_size,
            padding: 0.0,
            background_color: bg,
            device_pixel_ratio: 1.0,
            ..Default::default()
        },
    )?;
    if png.len() > 12 * 1024 * 1024 {
        return Err("formula PNG exceeds limit".into());
    }
    Ok((png, columns))
}
pub fn worker() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    loop {
        let mut line = String::new();
        let n = input.by_ref().take(131073).read_line(&mut line)?;
        if n == 0 {
            break;
        }
        if n > 131072 {
            return Err("oversized worker request".into());
        }
        let req: Request = serde_json::from_str(&line)?;
        let result = std::panic::catch_unwind(|| render(&req))
            .unwrap_or_else(|_| Err("native renderer failed".into()));
        let response = match result {
            Ok((png, columns)) => Response {
                key: req.key,
                png: STANDARD.encode(png),
                columns,
                error: None,
            },
            Err(error) => Response {
                key: req.key,
                png: String::new(),
                columns: 0,
                error: Some(error),
            },
        };
        serde_json::to_writer(&mut output, &response)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validate_colors() {
        assert!(color("#fff").is_ok());
        assert!(color("#12abEF").is_ok());
        for bad in ["red", "#gggggg", "#a", "#ééé"] {
            assert!(color(bad).is_err());
        }
    }
}
