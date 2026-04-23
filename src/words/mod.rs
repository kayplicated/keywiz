//! Word selection for typing modes.
//!
//! The public API is [`random_word`], which returns the next
//! word to type. When passed a non-empty heat map, words
//! containing hot letters become more likely to appear, so
//! struggling keys get more practice without any explicit
//! "drill X" UI. With an empty map (or nothing hot) the
//! selection is uniform random over the word list.
//!
//! The list is read at runtime from `words.txt` at the project
//! root (same spot as `texts/`), so users can edit it without
//! rebuilding. Loaded once on first use, then cached. A missing
//! or unreadable file falls back to a tiny built-in list so the
//! app still runs.

pub mod heated;

use std::collections::HashMap;
use std::sync::OnceLock;

use rand::prelude::IndexedRandom;

/// Fallback when `words.txt` is missing or unreadable. Keeps the
/// exercises functional enough to notice something's wrong.
const FALLBACK_WORDS: &[&str] = &[
    "the", "and", "for", "you", "with", "have", "this", "that", "from", "they",
];

static WORDS: OnceLock<Vec<String>> = OnceLock::new();

fn load_words() -> Vec<String> {
    match std::fs::read_to_string("words.txt") {
        Ok(content) => {
            let list: Vec<String> = content
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            if list.is_empty() {
                fallback()
            } else {
                list
            }
        }
        Err(e) => {
            eprintln!("keywiz: could not read words.txt: {e} — using built-in fallback");
            fallback()
        }
    }
}

fn fallback() -> Vec<String> {
    FALLBACK_WORDS.iter().map(|s| s.to_string()).collect()
}

fn all_words() -> &'static [String] {
    WORDS.get_or_init(load_words)
}

/// Pick the next word to type. Uses heat-weighted selection when
/// `heat` has entries, falls back to uniform random otherwise.
pub fn random_word(heat: &HashMap<char, f32>) -> String {
    let words = all_words();
    let refs: Vec<&str> = words.iter().map(String::as_str).collect();
    heated::pick_weighted(&refs, heat)
        .or_else(|| refs.choose(&mut rand::rng()).copied())
        .unwrap_or("the")
        .to_string()
}
