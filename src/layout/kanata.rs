use super::Layout;

/// Parse a kanata .kbd file and extract a named layer as a Layout.
///
/// Looks for `(deflayer <name> ...)` blocks and maps the key positions
/// to a standard ANSI keyboard layout.
pub fn parse_kanata(source: &str, layer_name: &str) -> Option<Layout> {
    let aliases = parse_aliases(source);
    let layer_content = extract_deflayer(source, layer_name)?;
    let tokens = tokenize(&layer_content);

    // A standard ANSI keyboard has these row lengths in a defsrc/deflayer:
    // Row 0 (esc + F-keys):  13 keys — skip
    // Row 1 (number row):    14 keys (grv 1-9 0 - = bspc)
    // Row 2 (top alpha):     14 keys (tab q w e r t y u i o p [ ] \)
    // Row 3 (home alpha):    13 keys (caps a s d f g h j k l ; ' ret)
    // Row 4 (bottom alpha):  12 keys (lsft z x c v b n m , . / rsft)
    // Row 5 (modifiers):     6 keys  — skip

    let row_lengths = [13, 14, 14, 13, 12, 6];

    if tokens.len() < row_lengths.iter().sum::<usize>() {
        return None;
    }

    let mut offset = row_lengths[0]; // skip F-key row

    // Number row: 14 tokens, we want positions 1..13 (skip grv at 0, bspc at 13)
    let num_tokens = &tokens[offset..offset + row_lengths[1]];
    let number_row = parse_number_row(num_tokens, &aliases);
    offset += row_lengths[1];

    // Top row: 14 tokens, skip tab at 0, keep positions 1..13
    let top_tokens = &tokens[offset..offset + row_lengths[2]];
    let top_row = parse_alpha_row(&top_tokens[1..], &aliases);
    offset += row_lengths[2];

    // Home row: 13 tokens, skip caps at 0, ret at 12, keep 1..12
    let home_tokens = &tokens[offset..offset + row_lengths[3]];
    let home_row = parse_alpha_row(&home_tokens[1..home_tokens.len() - 1], &aliases);
    offset += row_lengths[3];

    // Bottom row: 12 tokens, skip lsft at 0, rsft at 11, keep 1..11
    let bottom_tokens = &tokens[offset..offset + row_lengths[4]];
    let bottom_row = parse_alpha_row(&bottom_tokens[1..bottom_tokens.len() - 1], &aliases);

    Some(Layout::from_rows(
        layer_name,
        [number_row, top_row, home_row, bottom_row],
    ))
}

/// Extract the content of a `(deflayer name ...)` block.
/// Find the name of the first `deflayer` in a kanata config.
pub fn first_layer_name(source: &str) -> Option<String> {
    let pattern = "(deflayer ";
    let start = source.find(pattern)?;
    let after = &source[start + pattern.len()..];
    let name_end = after.find(|c: char| c.is_whitespace() || c == ')')?;
    Some(after[..name_end].to_string())
}

fn extract_deflayer(source: &str, name: &str) -> Option<String> {
    let pattern = format!("(deflayer {}", name);
    let start = source.find(&pattern)?;
    let after = &source[start + pattern.len()..];

    // Find matching closing paren
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

/// Split kanata content into whitespace-separated tokens, stripping comments.
fn tokenize(s: &str) -> Vec<String> {
    s.lines()
        .map(|line| {
            // strip comments
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

/// Resolve a kanata token to a character, using alias map for @-references.
fn resolve_key_with_aliases(
    token: &str,
    aliases: &std::collections::HashMap<String, char>,
) -> Option<char> {
    if token.starts_with('@') {
        aliases.get(&token[1..]).copied()
    } else if token.len() == 1 {
        Some(token.chars().next().unwrap())
    } else {
        // Named keys
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
}

/// Parse `(defalias ...)` blocks and extract the tap key from tap-hold aliases.
/// For other alias types (layer-switch etc.), we skip them since they don't produce typeable keys.
fn parse_aliases(source: &str) -> std::collections::HashMap<String, char> {
    use std::collections::HashMap;
    let mut aliases = HashMap::new();

    let mut search = source;
    while let Some(start) = search.find("(defalias") {
        let block_start = start + "(defalias".len();
        let after = &search[block_start..];

        // Find matching close paren for the defalias block
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

        // Parse each alias definition using regex-like line matching
        // Format: name (tap-hold timeout timeout tap-key hold-action)
        // or:     name (layer-switch ...)
        // or:     name simple-value
        for line in block.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(";;") {
                continue;
            }

            // Try to match: name (tap-hold N N key ...)
            if let Some(tap_key) = parse_tap_hold_alias(line) {
                aliases.insert(tap_key.0, tap_key.1);
            }
        }

        search = &search[block_start + end..];
    }

    aliases
}

/// Try to parse a line like `hqw (tap-hold 200 200 t (layer-while-held qwerty))`
/// Returns (alias_name, tap_char) if successful.
fn parse_tap_hold_alias(line: &str) -> Option<(String, char)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 5 {
        return None;
    }
    let name = tokens[0];
    // tokens[1] should be "(tap-hold"
    if tokens[1] != "(tap-hold" {
        return None;
    }
    // tokens[2] = tap-timeout, tokens[3] = hold-timeout, tokens[4] = tap-key
    let tap_key = resolve_key_simple(tokens[4])?;
    Some((name.to_string(), tap_key))
}

/// Simple key resolution without alias lookup (used during alias parsing).
fn resolve_key_simple(token: &str) -> Option<char> {
    if token.len() == 1 {
        Some(token.chars().next().unwrap())
    } else {
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
}

fn parse_number_row(
    tokens: &[String],
    aliases: &std::collections::HashMap<String, char>,
) -> Vec<(char, char)> {
    let shifted = ['~', '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '_', '+'];
    tokens[..13]
        .iter()
        .enumerate()
        .filter_map(|(i, tok)| {
            let lower = resolve_key_with_aliases(tok, aliases)?;
            let upper = shifted.get(i).copied().unwrap_or(lower.to_ascii_uppercase());
            Some((lower, upper))
        })
        .collect()
}

fn parse_alpha_row(
    tokens: &[String],
    aliases: &std::collections::HashMap<String, char>,
) -> Vec<(char, char)> {
    tokens
        .iter()
        .filter_map(|tok| {
            let lower = resolve_key_with_aliases(tok, aliases)?;
            let upper = if lower.is_alphabetic() {
                lower.to_ascii_uppercase()
            } else {
                match lower {
                    '[' => '{',
                    ']' => '}',
                    '\\' => '|',
                    ';' => ':',
                    '\'' => '"',
                    ',' => '<',
                    '.' => '>',
                    '/' => '?',
                    _ => lower,
                }
            };
            Some((lower, upper))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gallium_v2() {
        let source = std::fs::read_to_string("layouts/gallium_v2.kbd")
            .expect("layouts/gallium_v2.kbd should exist");
        let layout = parse_kanata(&source, "gallium_v2").expect("should parse gallium_v2 layer");

        assert_eq!(layout.name, "gallium_v2");

        // Home row should be: n r t s g y h a e i
        let home: Vec<char> = layout.rows[2].keys.iter().map(|k| k.lower).collect();
        assert_eq!(home, vec!['n', 'r', 't', 's', 'g', 'y', 'h', 'a', 'e', 'i', '\'']);
    }
}
