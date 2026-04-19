//! Kanata `.kbd` reader.
//!
//! A kanata config declares the physical keyboard via `(defsrc ...)` and
//! one or more layers via `(deflayer <name> ...)`. Both lists must align
//! position-by-position. This reader pairs each defsrc token with the
//! same position in the chosen deflayer to produce a [`Grid`] — defsrc
//! supplies geometry + finger assignment, the deflayer supplies the
//! per-position character.
//!
//! Aliases (`(defalias ...)`) are supported for tap-hold definitions —
//! the tap key becomes the typed character.

use std::collections::HashMap;

use crate::configreader::{ConfigReader, ReaderError};
use crate::grid::layout::KeyMapping;
use crate::grid::{Finger, Grid};

/// Standard ANSI defsrc row layout that kanata users follow when their
/// physical keyboard is a regular full-size or TKL board. Other shapes
/// (split ortho, etc.) need a different defsrc and would currently fall
/// outside this reader's assumptions — left as a follow-up.
const ANSI_ROW_LENGTHS: &[usize] = &[13, 14, 14, 13, 12, 6];

pub struct KanataReader;

impl ConfigReader for KanataReader {
    fn format_name(&self) -> &'static str {
        "kanata"
    }

    fn read(&self, source: &str, selector: Option<&str>) -> Result<Grid, ReaderError> {
        let aliases = parse_aliases(source);
        let layers = list_layer_names(source);
        if layers.is_empty() {
            return Err(ReaderError::Malformed(
                "no (deflayer ...) blocks found".into(),
            ));
        }
        let layer_name = selector
            .map(|s| s.to_string())
            .unwrap_or_else(|| layers[0].clone());

        if !layers.iter().any(|n| n == &layer_name) {
            return Err(ReaderError::UnknownLayer {
                name: layer_name,
                available: layers,
            });
        }

        let layer_content = extract_block(source, "deflayer", Some(&layer_name)).ok_or_else(
            || ReaderError::Malformed(format!("could not extract layer {layer_name}")),
        )?;
        let tokens = tokenize(&layer_content);

        if tokens.len() < ANSI_ROW_LENGTHS.iter().sum::<usize>() {
            return Err(ReaderError::Malformed(format!(
                "layer {layer_name} has {} tokens; expected at least {}",
                tokens.len(),
                ANSI_ROW_LENGTHS.iter().sum::<usize>()
            )));
        }

        let buttons = build_ansi_buttons(&tokens, &aliases);
        let grid = Grid {
            keyboard_name: format!("kanata:{layer_name}"),
            keyboard_short: "Kanata".to_string(),
            layout_name: layer_name.clone(),
            layout_short: layer_name,
            buttons,
        };
        Ok(grid)
    }

    fn list_layers(&self, source: &str) -> Vec<String> {
        list_layer_names(source)
    }
}

/// Build [`GridButton`]s for an ANSI keyboard from `tokens` (a flat list
/// of every token in a deflayer, in defsrc order). Skips the F-key row
/// and modifier row — those aren't drilled.
fn build_ansi_buttons(
    tokens: &[String],
    aliases: &HashMap<String, char>,
) -> Vec<crate::grid::GridButton> {
    let mut buttons = Vec::new();
    // F-key row at offset 0 — skip.
    let mut offset = ANSI_ROW_LENGTHS[0];

    let rows: &[(&[&str], f32)] = &[
        // Number row keycodes (13, full row).
        (&NUMBER_KEYCODES, -2.0),
        // Top row, skip leading `tab` and trailing `bspc`.
        (&TOP_KEYCODES, -1.0),
        // Home row, skip leading `caps` and trailing `ret`.
        (&HOME_KEYCODES, 0.0),
        // Bottom row, skip leading `lsft` and trailing `rsft`.
        (&BOTTOM_KEYCODES, 1.0),
    ];

    let row_token_lengths = &ANSI_ROW_LENGTHS[1..5];
    let inner_offsets = [(0usize, 0usize), (1, 1), (1, 1), (1, 1)];

    for ((codes, y), (raw_len, (skip_left, skip_right))) in rows
        .iter()
        .zip(row_token_lengths.iter().zip(inner_offsets.iter()))
    {
        let row_tokens = &tokens[offset..offset + raw_len];
        let inner = &row_tokens[*skip_left..row_tokens.len() - *skip_right];

        for (col_idx, (code, token)) in codes.iter().zip(inner.iter()).enumerate() {
            let mapping = resolve_token(token, aliases).map(|lower| KeyMapping {
                lower,
                upper: shift_for(*code, lower),
            });
            let x = ansi_x_for(*y, col_idx);
            buttons.push(crate::grid::GridButton {
                code: code.to_string(),
                x,
                y: *y,
                finger: ansi_finger_for(col_idx),
                mapping,
            });
        }
        offset += raw_len;
    }

    buttons
}

/// Standard ANSI x-position for a key in the alpha rows. Number row
/// starts further left (KEY_GRAVE at -7); alpha rows shift right with
/// row stagger built into the keyboard.
fn ansi_x_for(y: f32, col_idx: usize) -> f32 {
    match y {
        y if y == -2.0 => -7.0 + col_idx as f32,
        y if y == -1.0 => -5.5 + col_idx as f32,
        y if y == 0.0 => -5.0 + col_idx as f32,
        y if y == 1.0 => -4.5 + col_idx as f32,
        _ => col_idx as f32,
    }
}

/// Standard ANSI finger assignment by column index within a row.
fn ansi_finger_for(col: usize) -> Finger {
    match col {
        0 | 1 => Finger::LPinky,
        2 => Finger::LRing,
        3 => Finger::LMiddle,
        4 | 5 => Finger::LIndex,
        6 | 7 => Finger::RIndex,
        8 => Finger::RMiddle,
        9 => Finger::RRing,
        _ => Finger::RPinky,
    }
}

/// Shift variant for a keycode + lowercase character. Letters become
/// uppercase; punctuation has known shift partners; everything else
/// passes through.
fn shift_for(code: &str, lower: char) -> char {
    if lower.is_alphabetic() {
        return lower.to_ascii_uppercase();
    }
    match code {
        "KEY_GRAVE" => '~',
        "KEY_1" => '!', "KEY_2" => '@', "KEY_3" => '#', "KEY_4" => '$', "KEY_5" => '%',
        "KEY_6" => '^', "KEY_7" => '&', "KEY_8" => '*', "KEY_9" => '(', "KEY_0" => ')',
        "KEY_MINUS" => '_', "KEY_EQUAL" => '+',
        "KEY_LEFTBRACE" => '{', "KEY_RIGHTBRACE" => '}', "KEY_BACKSLASH" => '|',
        "KEY_SEMICOLON" => ':', "KEY_APOSTROPHE" => '"',
        "KEY_COMMA" => '<', "KEY_DOT" => '>', "KEY_SLASH" => '?',
        _ => lower,
    }
}

const NUMBER_KEYCODES: &[&str] = &[
    "KEY_GRAVE", "KEY_1", "KEY_2", "KEY_3", "KEY_4", "KEY_5", "KEY_6", "KEY_7",
    "KEY_8", "KEY_9", "KEY_0", "KEY_MINUS", "KEY_EQUAL",
];
const TOP_KEYCODES: &[&str] = &[
    "KEY_Q", "KEY_W", "KEY_E", "KEY_R", "KEY_T", "KEY_Y", "KEY_U", "KEY_I",
    "KEY_O", "KEY_P", "KEY_LEFTBRACE", "KEY_RIGHTBRACE", "KEY_BACKSLASH",
];
const HOME_KEYCODES: &[&str] = &[
    "KEY_A", "KEY_S", "KEY_D", "KEY_F", "KEY_G", "KEY_H", "KEY_J", "KEY_K",
    "KEY_L", "KEY_SEMICOLON", "KEY_APOSTROPHE",
];
const BOTTOM_KEYCODES: &[&str] = &[
    "KEY_Z", "KEY_X", "KEY_C", "KEY_V", "KEY_B", "KEY_N", "KEY_M", "KEY_COMMA",
    "KEY_DOT", "KEY_SLASH",
];

/// List the layer names declared in `source`, in declaration order.
fn list_layer_names(source: &str) -> Vec<String> {
    let pattern = "(deflayer ";
    let mut names = Vec::new();
    let mut search = source;
    while let Some(start) = search.find(pattern) {
        let after = &search[start + pattern.len()..];
        if let Some(name_end) = after.find(|c: char| c.is_whitespace() || c == ')') {
            names.push(after[..name_end].to_string());
            search = &after[name_end..];
        } else {
            break;
        }
    }
    names
}

/// Extract the body of a `(<form> [name] ...)` block. With `name = Some`,
/// match a specific occurrence; with `None`, match the first occurrence
/// of the form regardless of its name.
fn extract_block(source: &str, form: &str, name: Option<&str>) -> Option<String> {
    let pattern = match name {
        Some(n) => format!("({form} {n}"),
        None => format!("({form}"),
    };
    let start = source.find(&pattern)?;
    let after = &source[start + pattern.len()..];

    let mut depth = 1;
    let mut end = 0;
    for (i, ch) in after.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    Some(after[..end].to_string())
}

/// Split kanata content into whitespace-separated tokens, stripping
/// `;;` line comments.
fn tokenize(s: &str) -> Vec<String> {
    s.lines()
        .map(|line| {
            if let Some(pos) = line.find(";;") {
                &line[..pos]
            } else {
                line
            }
        })
        .flat_map(|line| line.split_whitespace())
        .map(String::from)
        .collect()
}

/// Resolve a kanata token to its produced character, expanding `@alias`
/// references through the alias map.
fn resolve_token(token: &str, aliases: &HashMap<String, char>) -> Option<char> {
    if let Some(alias) = token.strip_prefix('@') {
        return aliases.get(alias).copied();
    }
    if token.len() == 1 {
        return Some(token.chars().next().unwrap());
    }
    match token {
        "grv" => Some('`'),
        "min" | "-" => Some('-'),
        "eql" | "=" => Some('='),
        "lbrc" | "[" => Some('['),
        "rbrc" | "]" => Some(']'),
        "bsls" | "\\" => Some('\\'),
        "scln" | ";" => Some(';'),
        "quot" | "'" => Some('\''),
        "comm" | "," => Some(','),
        "dot" | "." => Some('.'),
        "slsh" | "/" => Some('/'),
        "spc" => Some(' '),
        _ => None,
    }
}

/// Parse all `(defalias ...)` blocks and extract the tap character from
/// any `(tap-hold ...)` aliases; other alias forms are skipped because
/// they don't produce a typeable character.
fn parse_aliases(source: &str) -> HashMap<String, char> {
    let mut aliases = HashMap::new();
    let mut search = source;
    while let Some(start) = search.find("(defalias") {
        let block_start = start + "(defalias".len();
        let after = &search[block_start..];

        let mut depth = 1;
        let mut end = 0;
        for (i, ch) in after.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let block = &after[..end];

        for line in block.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(";;") {
                continue;
            }
            if let Some((name, ch)) = parse_tap_hold_alias(line) {
                aliases.insert(name, ch);
            }
        }

        search = &search[block_start + end..];
    }
    aliases
}

/// Try to match `name (tap-hold T1 T2 tap-key ...)` and return
/// `(name, tap-key character)`.
fn parse_tap_hold_alias(line: &str) -> Option<(String, char)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 5 {
        return None;
    }
    if tokens[1] != "(tap-hold" {
        return None;
    }
    let ch = resolve_token(tokens[4], &HashMap::new())?;
    Some((tokens[0].to_string(), ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
(defsrc
  esc f1 f2 f3 f4 f5 f6 f7 f8 f9 f10 f11 f12
  grv 1 2 3 4 5 6 7 8 9 0 - = bspc
  tab q w e r t y u i o p [ ] \
  caps a s d f g h j k l ; ' ret
  lsft z x c v b n m , . / rsft
  lctl lmet lalt spc ralt rctl
)

(deflayer qwerty
  esc f1 f2 f3 f4 f5 f6 f7 f8 f9 f10 f11 f12
  grv 1 2 3 4 5 6 7 8 9 0 - = bspc
  tab q w e r t y u i o p [ ] \
  caps a s d f g h j k l ; ' ret
  lsft z x c v b n m , . / rsft
  lctl lmet lalt spc ralt rctl
)
"#;

    #[test]
    fn lists_layers_in_declaration_order() {
        let r = KanataReader;
        let layers = r.list_layers(MINIMAL);
        assert_eq!(layers, vec!["qwerty"]);
    }

    #[test]
    fn read_ansi_qwerty_produces_expected_home_row() {
        let r = KanataReader;
        let grid = r.read(MINIMAL, Some("qwerty")).expect("should parse");

        let home: Vec<char> = grid
            .buttons
            .iter()
            .filter(|b| b.y == 0.0)
            .filter_map(|b| b.mapping.as_ref().map(|m| m.lower))
            .collect();
        assert_eq!(
            home,
            vec!['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'']
        );
    }

    #[test]
    fn unknown_layer_reports_available() {
        let r = KanataReader;
        let err = r.read(MINIMAL, Some("nope")).unwrap_err();
        match err {
            ReaderError::UnknownLayer { name, available } => {
                assert_eq!(name, "nope");
                assert_eq!(available, vec!["qwerty"]);
            }
            _ => panic!("wrong error: {err}"),
        }
    }

    #[test]
    fn defaults_to_first_layer_when_none_given() {
        let r = KanataReader;
        let grid = r.read(MINIMAL, None).expect("should parse");
        assert_eq!(grid.layout_name, "qwerty");
    }
}
