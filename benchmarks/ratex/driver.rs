use base64::{engine::general_purpose::STANDARD, Engine};
use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};
use ratex_types::{color::Color, math_style::MathStyle};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
fn render(r: &Value) -> Result<Value, String> {
    let f = &r["formula"];
    let latex = f["latex"].as_str().unwrap();
    let display = f["display"].as_bool().unwrap();
    let cols = f["cols"].as_u64().unwrap() as f32;
    let rows = f["rows"].as_u64().unwrap() as f32;
    let cw = r["cell_width"].as_u64().unwrap() as f32;
    let ch = r["cell_height"].as_u64().unwrap() as f32;
    let color = |key: &str| {
        let hex = f[key].as_str().unwrap().trim_start_matches('#');
        Color {
            r: u8::from_str_radix(&hex[0..2], 16).unwrap() as f32 / 255.0,
            g: u8::from_str_radix(&hex[2..4], 16).unwrap() as f32 / 255.0,
            b: u8::from_str_radix(&hex[4..6], 16).unwrap() as f32 / 255.0,
            a: 1.0,
        }
    };
    let ast = parse(latex).map_err(|e| e.to_string())?;
    let opts = LayoutOptions::default()
        .with_style(if display {
            MathStyle::Display
        } else {
            MathStyle::Text
        })
        .with_color(color("fg"));
    let mut dl = to_display_list(&layout(&ast, &opts));
    let base = ch * 0.9;
    let fit = 1f32.min((cols * cw - 2.0) / (dl.width as f32 * base)).min(
        (rows * ch - if display { 0.0 } else { ch * 0.16 }) / (dl.total_height() as f32 * base),
    );
    let font_size = base * fit;
    // Equal canvas dimensions, not a terminal layout implementation.
    let width = r["target_width"].as_u64().unwrap() as f64;
    let height = r["target_height"].as_u64().unwrap() as f64;
    let dx = if display {
        ((width / font_size as f64 - dl.width) / 2.0).max(0.0)
    } else {
        1.0 / font_size as f64
    };
    let dy = if display {
        ((height / font_size as f64 - dl.total_height()) / 2.0).max(0.0)
    } else {
        ((ch as f64 * 0.78 / font_size as f64) - dl.height).max(1.0 / font_size as f64)
    };
    use ratex_types::display_item::DisplayItem;
    for item in &mut dl.items {
        let (x, y) = match item {
            DisplayItem::GlyphPath { x, y, .. }
            | DisplayItem::Line { x, y, .. }
            | DisplayItem::Rect { x, y, .. }
            | DisplayItem::Path { x, y, .. } => (x, y),
        };
        *x += dx;
        *y += dy;
    }
    dl.width = (width - 0.001) / font_size as f64;
    dl.height = (height - 0.001) / font_size as f64 - dl.depth;
    let png = render_to_png(
        &dl,
        &RenderOptions {
            font_size,
            padding: 0.0,
            background_color: color("bg"),
            device_pixel_ratio: 1.0,
            ..Default::default()
        },
    )?;
    Ok(json!({"key":r["key"],"png":STANDARD.encode(png)}))
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let r: Value = serde_json::from_str(&line).unwrap();
        let result = render(&r).unwrap_or_else(|e| json!({"error":e}));
        println!("{}", result);
        io::stdout().flush().unwrap();
    }
}
