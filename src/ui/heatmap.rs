//! Keyboard heatmap colorization.
//!
//! Drives the keyboard widget's per-key coloring from each key's accumulated
//! *heat* — an integer 0..=MAX_HEAT maintained by [`Stats`]. A key with no
//! heat (i.e. no wrong presses, or fully cooled) renders with its default
//! styling. Keys with rising heat fade through a cool-to-hot gradient:
//! violet → blue → yellow → orange → red. Green is intentionally absent —
//! a correctly-typed key should look calm, not "good."

use crate::stats::{Stats, MAX_HEAT};
use ratatui::style::Color;

/// Look up the heat color for a key, or `None` if the key has never
/// accumulated heat. `None` should render with the default (finger) color.
pub fn heat_color(stats: &Stats, ch: char) -> Option<Color> {
    let record = stats.get(ch)?;
    if record.heat == 0 {
        return None;
    }
    let t = record.heat as f64 / MAX_HEAT as f64;
    Some(color_for_heat(t.clamp(0.0, 1.0)))
}

/// Map a normalized heat level (0.0 = first warning, 1.0 = fully hot)
/// to a cool-to-hot gradient: violet → blue → cyan → yellow → orange → red.
///
/// The gradient passes through multiple hue stops so 20 heat steps produce
/// visibly distinct colors across the range, not just "more red."
fn color_for_heat(t: f64) -> Color {
    // Anchor colors, in order, from coolest to hottest.
    const STOPS: &[(u8, u8, u8)] = &[
        (120, 60, 200),  // violet — first sign of trouble
        (70, 100, 220),  // blue
        (60, 180, 210),  // cyan
        (220, 200, 80),  // yellow
        (240, 140, 50),  // orange
        (230, 50, 50),   // red — fully hot
    ];

    // Find which segment t falls into and interpolate within it.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untouched_key_returns_none() {
        let s = Stats::new();
        assert_eq!(heat_color(&s, 'z'), None);
    }

    #[test]
    fn fully_cooled_key_returns_none() {
        let mut s = Stats::new();
        s.record('a', true); // cold from the start
        assert_eq!(heat_color(&s, 'a'), None);
    }

    #[test]
    fn one_wrong_press_starts_cool() {
        let mut s = Stats::new();
        s.record('a', false);
        let Some(Color::Rgb(r, g, b)) = heat_color(&s, 'a') else {
            panic!("expected RGB color");
        };
        // First step = violet-ish: blue-dominant, not red.
        assert!(b > r, "expected cool color, got ({r},{g},{b})");
    }

    #[test]
    fn fully_hot_key_is_red_dominant() {
        let mut s = Stats::new();
        for _ in 0..MAX_HEAT {
            s.record('a', false);
        }
        let Some(Color::Rgb(r, g, b)) = heat_color(&s, 'a') else {
            panic!("expected RGB color");
        };
        assert!(r > g && r > b, "expected red-dominant, got ({r},{g},{b})");
    }

    #[test]
    fn heat_transitions_through_distinct_colors() {
        // Sample colors at several heat levels and assert they're all
        // distinguishable from each other — the whole point of the gradient.
        let mut seen = Vec::new();
        for heat in [1, 5, 10, 15, 20] {
            let mut s = Stats::new();
            for _ in 0..heat {
                s.record('a', false);
            }
            let Some(Color::Rgb(r, g, b)) = heat_color(&s, 'a') else {
                panic!("expected RGB color at heat={heat}");
            };
            seen.push((heat, r, g, b));
        }
        // Each pair of adjacent samples should differ by a meaningful amount.
        for pair in seen.windows(2) {
            let (_, r1, g1, b1) = pair[0];
            let (_, r2, g2, b2) = pair[1];
            let dist = (r1 as i32 - r2 as i32).abs()
                + (g1 as i32 - g2 as i32).abs()
                + (b1 as i32 - b2 as i32).abs();
            assert!(dist > 50, "adjacent heat steps too similar: {pair:?}");
        }
    }
}
