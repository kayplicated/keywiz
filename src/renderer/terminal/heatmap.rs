//! Color gradients for the terminal renderer.
//!
//! Two gradients, one per overlay mode:
//!
//! - [`color_for_heat`] — warm ramp (violet → blue → yellow →
//!   red) for error heat. "This key is hurting me."
//! - [`color_for_usage`] — cool ramp (gray → indigo → teal →
//!   green) for usage heat. "My fingers live here."
//!
//! Cool vs. warm split is deliberate: the two signals answer
//! different questions, so they shouldn't be mistaken for one
//! another at a glance. Both take a pre-normalized `0.0..=1.0`
//! input — scaling (linear vs. log) is the caller's problem.

use ratatui::style::Color;

/// Map a normalized heat level to a ratatui color, cool → hot.
/// For the F2 "heat-errors" overlay.
pub fn color_for_heat(t: f32) -> Color {
    const STOPS: &[(u8, u8, u8)] = &[
        (120, 60, 200),
        (70, 100, 220),
        (60, 180, 210),
        (220, 200, 80),
        (240, 140, 50),
        (230, 50, 50),
    ];
    gradient(t, STOPS)
}

/// Map a normalized usage level to a ratatui color, dim → vivid.
/// For the F2 "heat-usage" overlay. Stays entirely in the cool
/// hue range (blue → teal → green) so it reads as a different
/// axis from error heat at a glance.
pub fn color_for_usage(t: f32) -> Color {
    const STOPS: &[(u8, u8, u8)] = &[
        (70, 70, 90),    // dim slate — rarely used
        (60, 80, 160),   // deep indigo
        (50, 130, 200),  // blue
        (50, 180, 200),  // teal
        (80, 210, 170),  // cyan-green
        (120, 230, 120), // bright green — most-used
    ];
    gradient(t, STOPS)
}

/// Sample a piecewise-linear RGB gradient at `t ∈ [0, 1]`.
fn gradient(t: f32, stops: &[(u8, u8, u8)]) -> Color {
    let t = t.clamp(0.0, 1.0) as f64;
    let segments = stops.len() - 1;
    let scaled = t * segments as f64;
    let i = (scaled.floor() as usize).min(segments - 1);
    let local = scaled - i as f64;
    let (r, g, b) = lerp_rgb(stops[i], stops[i + 1], local);
    Color::Rgb(r, g, b)
}

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let lerp = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    (lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2))
}
