//! Heatmap colorization — moved from the old `ui/heatmap.rs`.
//!
//! Drives per-key coloring from accumulated heat in `Stats`.

use crate::stats::{Stats, MAX_HEAT};
use ratatui::style::Color;

pub fn heat_color(stats: &Stats, ch: char) -> Option<Color> {
    let record = stats.get(ch)?;
    if record.heat == 0 {
        return None;
    }
    let t = record.heat as f64 / MAX_HEAT as f64;
    Some(color_for_heat(t.clamp(0.0, 1.0)))
}

fn color_for_heat(t: f64) -> Color {
    const STOPS: &[(u8, u8, u8)] = &[
        (120, 60, 200),
        (70, 100, 220),
        (60, 180, 210),
        (220, 200, 80),
        (240, 140, 50),
        (230, 50, 50),
    ];
    let segments = STOPS.len() - 1;
    let scaled = t * segments as f64;
    let i = (scaled.floor() as usize).min(segments - 1);
    let local = scaled - i as f64;
    let (r, g, b) = lerp_rgb(STOPS[i], STOPS[i + 1], local);
    Color::Rgb(r, g, b)
}

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let lerp = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    (lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2))
}
