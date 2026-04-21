//! Human-readable report printer.
//!
//! Takes a [`ScoreReport`] and prints a structured, colorized
//! breakdown. Kept separate from scoring logic so other frontends
//! (JSON, web) can share the score pipeline.

use owo_colors::OwoColorize;

use crate::keyboard::Finger;
use crate::score::{BigramDetail, ScoreReport, TrigramCategory, TrigramDetail};

/// Print a single score report in the default text format.
pub fn print(report: &ScoreReport) {
    println!(
        "{} {}",
        "Layout:".bold(),
        report.layout_name.bright_cyan().bold()
    );
    println!("{} {}", "Board: ".bold(), report.keyboard_name);
    println!("{} {}", "Corpus:".bold(), report.corpus_name);
    println!();

    print_row_distribution(report);
    println!();

    print_finger_load(report);
    println!();

    print_motion_breakdown(report);
    println!();

    print_top_details("Top SFBs (same-finger bigrams)", &report.top_sfbs);
    print_top_details("Top cross-row (scissor-like)", &report.top_scissors);
    print_top_details("Top rolls", &report.top_rolls);

    print_trigram_breakdown(&report.trigram_categories, report.trigram_cost);
    print_top_trigrams(&report.top_trigrams);

    println!(
        "{} {:.3}",
        "Overall score:".bold(),
        report.total_score.bright_green().bold()
    );
}

fn print_row_distribution(report: &ScoreReport) {
    println!("{}", "Row distribution:".bold());
    print_bar("Top ", report.row_pct.top);
    print_bar("Home", report.row_pct.home);
    print_bar("Bot ", report.row_pct.bot);
}

fn print_bar(label: &str, pct: f64) {
    let width = (pct * 0.5).round() as usize;
    let bar: String = "█".repeat(width);
    println!("  {} {:>5.1}%  {}", label, pct, bar.bright_blue());
}

fn print_finger_load(report: &ScoreReport) {
    println!("{}", "Finger load (strength-weighted):".bold());
    let order = [
        Finger::LPinky,
        Finger::LRing,
        Finger::LMiddle,
        Finger::LIndex,
        Finger::RIndex,
        Finger::RMiddle,
        Finger::RRing,
        Finger::RPinky,
    ];
    for f in order {
        let pct = report.finger_pct.get(&f).copied().unwrap_or(0.0);
        let load = report.finger_load.get(&f).copied().unwrap_or(0.0);
        println!("  {:<10} {:>5.1}%   load-score: {:>5.2}", f.to_string(), pct, load);
    }
    println!(
        "  {} {:+.3}",
        "Finger overload cost:".bold(),
        report.finger_overload_cost
    );
}

fn print_motion_breakdown(report: &ScoreReport) {
    let m = &report.motions;
    println!("{}", "Motion breakdown:".bold());
    println!("  {:<26} {:>6.2}%", "Alternate (diff hand):", m.alternate_pct);
    println!("  {:<26} {:>6.2}%", "Same-key repeat:", m.same_key_pct);
    println!("  {:<26} {:>6.2}%  cost {:+.3}", "SFB (same finger):", m.sfb_pct, m.sfb_cost);
    println!("  {:<26} {:>6.2}%", "Roll inward:", m.roll_inward_pct);
    println!("  {:<26} {:>6.2}%", "Roll outward:", m.roll_outward_pct);
    println!(
        "  {:<26} {:>6.3}",
        "Roll bonus total:",
        m.roll_bonus
    );
    println!("  {:<26} {:>6.2}%", "Same-row skip:", m.same_row_skip_pct);
    println!("  {:<26} {:>6.2}%", "Cross-row flexion:", m.cross_row_flexion_pct);
    println!("  {:<26} {:>6.2}%", "Cross-row extension:", m.cross_row_extension_pct);
    println!("  {:<26} {:>6.2}%", "Cross-row full:", m.cross_row_full_pct);
    println!("  {:<26} {:>6.2}%  (asym-rule exempt)", "Cross-row exempt:", m.cross_row_exempt_pct);
    println!(
        "  {:<26} {:>+6.3}",
        "Scissor cost total:",
        m.scissor_cost
    );
    println!("  {:<26} {:>6.2}%  cost {:+.3}", "Stretch:", m.stretch_pct, m.stretch_cost);
}

fn print_top_details(title: &str, details: &[BigramDetail]) {
    if details.is_empty() {
        return;
    }
    println!("{}", title.bold());
    for d in details {
        println!(
            "  {} → {:.3}% of typing, contribution {:+.3}",
            d.label.bright_yellow(),
            d.freq,
            d.contribution
        );
    }
    println!();
}

fn print_trigram_breakdown(categories: &[TrigramCategory], total: f64) {
    if categories.is_empty() {
        return;
    }
    println!("{}", "Trigram breakdown (by rule category):".bold());
    for cat in categories {
        println!(
            "  {:<20} {:>6.2}%  cost {:+.3}",
            cat.name,
            cat.trigram_pct,
            cat.total_cost
        );
    }
    println!(
        "  {:<20} {:+.3}",
        "Trigram cost total:".bold(),
        total
    );
    println!();
}

fn print_top_trigrams(details: &[TrigramDetail]) {
    if details.is_empty() {
        return;
    }
    println!("{}", "Top trigram contributions:".bold());
    for d in details.iter().take(12) {
        println!(
            "  [{}] {} → {:.3}% × {:+.3}",
            d.category.bright_blue(),
            d.label.bright_yellow(),
            d.freq,
            d.contribution
        );
    }
    println!();
}

/// Print a side-by-side `compare` view of two reports.
pub fn print_compare(a: &ScoreReport, b: &ScoreReport) {
    let w = 35;
    let header = format!("{:<w$}{}", &a.layout_name, &b.layout_name, w = w);
    println!("{}", header.bold());
    println!();

    row("Board", &a.keyboard_name, &b.keyboard_name, w);
    row("Corpus", &a.corpus_name, &b.corpus_name, w);
    println!();

    row_pct("Top row", a.row_pct.top, b.row_pct.top, w);
    row_pct("Home row", a.row_pct.home, b.row_pct.home, w);
    row_pct("Bottom row", a.row_pct.bot, b.row_pct.bot, w);
    println!();

    row_pct("SFB", a.motions.sfb_pct, b.motions.sfb_pct, w);
    row_pct("Scissor (flex)", a.motions.cross_row_flexion_pct, b.motions.cross_row_flexion_pct, w);
    row_pct("Scissor (ext)", a.motions.cross_row_extension_pct, b.motions.cross_row_extension_pct, w);
    row_pct("Roll inward", a.motions.roll_inward_pct, b.motions.roll_inward_pct, w);
    row_pct("Roll outward", a.motions.roll_outward_pct, b.motions.roll_outward_pct, w);
    println!();

    row_num("Score", a.total_score, b.total_score, w);
}

fn row(label: &str, a: &str, b: &str, w: usize) {
    println!("  {:<15} {:<w$}{}", label, a, b, w = w);
}

fn row_pct(label: &str, a: f64, b: f64, w: usize) {
    let a_str = format!("{:.2}%", a);
    let b_str = format!("{:.2}%", b);
    println!("  {:<15} {:<w$}{}", label, a_str, b_str, w = w);
}

fn row_num(label: &str, a: f64, b: f64, w: usize) {
    let a_str = format!("{:.3}", a);
    let b_str = format!("{:.3}", b);
    let better = if a > b { &a_str } else { &b_str };
    let (a_disp, b_disp) = if a > b {
        (a_str.bright_green().to_string(), b_str.dimmed().to_string())
    } else if b > a {
        (a_str.dimmed().to_string(), b_str.bright_green().to_string())
    } else {
        (a_str.clone(), b_str.clone())
    };
    let _ = better; // unused, for clarity
    println!("  {:<15} {:<w$}{}", label.bold(), a_disp, b_disp, w = w);
}
