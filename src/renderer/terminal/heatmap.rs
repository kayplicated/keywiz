//! Heatmap color gradient for the terminal renderer.
//!
//! Input is a normalized heat value `0.0..=1.0` — the engine has
//! already queried stats and produced it. Output is a ratatui
//! color sampled from the cool-to-hot gradient.

use ratatui::style::Color;

/// Map a normalized heat level to a ratatui color, cool → hot.
pub fn color_for_heat(t: f32) -> Color {
    const STOPS: &[(u8, u8, u8)] = &[
        (120, 60, 200),
        (70, 100, 220),
        (60, 180, 210),
        (220, 200, 80),
        (240, 140, 50),
        (230, 50, 50),
    ];
    let t = t.clamp(0.0, 1.0) as f64;
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
