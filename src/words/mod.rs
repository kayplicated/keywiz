//! Word selection for typing modes.
//!
//! Wordlists live as `.txt` files under `words/` (one word per
//! line). Each file becomes one selectable list — mirrors how
//! `texts/` holds one passage per file. The title comes from the
//! file stem (`english.txt` → "English"), so lists dropped in
//! from external sources work without editing.
//!
//! The active wordlist is an *instance* on the words exercise, so
//! cycling with `Alt+←/→` walks between lists the same way it
//! walks between text passages.
//!
//! [`random_word`] takes a wordlist index and a heat map. When
//! the heat map has entries, words containing hot letters are
//! weighted higher so struggling keys get more practice without
//! any explicit "drill X" UI. An empty heat map falls through to
//! uniform random. A missing or empty `words/` directory falls
//! back to a tiny built-in list so the app still runs.

pub mod heated;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use rand::prelude::IndexedRandom;

/// Fallback used when `words/` is empty or unreadable. Keeps the
/// exercises functional enough to notice something's wrong.
const FALLBACK_WORDS: &[&str] = &[
    "the", "and", "for", "you", "with", "have", "this", "that", "from", "they",
];

/// One wordlist loaded from disk.
struct Wordlist {
    title: String,
    words: Vec<String>,
}

/// Cached wordlist catalog. Loaded on first use; stable thereafter
/// within a single run so the engine's instance-count bound matches
/// what exercises actually see.
static LISTS: OnceLock<Vec<Wordlist>> = OnceLock::new();

fn lists() -> &'static [Wordlist] {
    LISTS.get_or_init(load_lists)
}

fn load_lists() -> Vec<Wordlist> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir("words") else {
        eprintln!("keywiz: could not read words/ — using built-in fallback");
        return vec![fallback_list()];
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.extension().is_none_or(|e| e != "txt") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let words: Vec<String> = content
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();
        if words.is_empty() {
            continue;
        }
        let title = title_from_stem(&path);
        found.push(Wordlist { title, words });
    }
    if found.is_empty() {
        return vec![fallback_list()];
    }
    found
}

/// Humanize a file stem: `short_words` → "Short Words". Keeps
/// anything unusual (e.g. `top-5k`) readable without being fussy.
fn title_from_stem(path: &std::path::Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled");
    stem.split(['_', '-'])
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn fallback_list() -> Wordlist {
    Wordlist {
        title: "Fallback".to_string(),
        words: FALLBACK_WORDS.iter().map(|s| s.to_string()).collect(),
    }
}

/// Number of wordlists available on disk (minimum 1 — the fallback
/// counts when nothing loaded).
pub fn list_count() -> usize {
    lists().len()
}

/// Title of the wordlist at `index`, if any.
pub fn list_title(index: usize) -> Option<String> {
    lists().get(index).map(|l| l.title.clone())
}

/// Pick the next word to type from wordlist `index`. Heat-weighted
/// when `heat` has entries, uniform random otherwise. Out-of-range
/// indices clamp to the first list.
pub fn random_word(index: usize, heat: &HashMap<char, f32>) -> String {
    let all = lists();
    if all.is_empty() {
        return "the".to_string();
    }
    let list = &all[index.min(all.len() - 1)];
    let refs: Vec<&str> = list.words.iter().map(String::as_str).collect();
    heated::pick_weighted(&refs, heat)
        .or_else(|| refs.choose(&mut rand::rng()).copied())
        .unwrap_or("the")
        .to_string()
}
