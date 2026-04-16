//! Word list loading for typing modes.

use rand::prelude::IndexedRandom;

const WORDS: &str = include_str!("words.txt");

/// Pick a random word from the embedded word list.
pub fn random_word() -> String {
    let all_words: Vec<&str> = WORDS.lines().filter(|l| !l.is_empty()).collect();
    all_words
        .choose(&mut rand::rng())
        .unwrap_or(&"the")
        .to_string()
}
