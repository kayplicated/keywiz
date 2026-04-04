pub mod kanata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Finger {
    LPinky,
    LRing,
    LMiddle,
    LIndex,
    RIndex,
    RMiddle,
    RRing,
    RPinky,
}

impl Finger {
    pub fn color(self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Finger::LPinky => Color::Red,
            Finger::LRing => Color::Yellow,
            Finger::LMiddle => Color::Green,
            Finger::LIndex => Color::Cyan,
            Finger::RIndex => Color::Blue,
            Finger::RMiddle => Color::Magenta,
            Finger::RRing => Color::Yellow,
            Finger::RPinky => Color::Red,
        }
    }
}

/// Finger assignment by column index within a row.
/// Standard touch-typing: columns 0-1 pinky, 2 ring, 3 middle, 4-5 index | 6-7 index, 8 middle, 9 ring, 10+ pinky
fn finger_for_col(col: usize) -> Finger {
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

#[derive(Debug, Clone)]
pub struct Key {
    pub lower: char,
    pub upper: char,
    pub finger: Finger,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub keys: Vec<Key>,
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    /// [number_row, top_row, home_row, bottom_row]
    pub rows: [Row; 4],
}

impl Layout {
    pub fn from_rows(name: impl Into<String>, rows: [Vec<(char, char)>; 4]) -> Self {
        let make_row = |pairs: Vec<(char, char)>| Row {
            keys: pairs
                .into_iter()
                .enumerate()
                .map(|(col, (lower, upper))| Key {
                    lower,
                    upper,
                    finger: finger_for_col(col),
                })
                .collect(),
        };
        Layout {
            name: name.into(),
            rows: [
                make_row(rows[0].clone()),
                make_row(rows[1].clone()),
                make_row(rows[2].clone()),
                make_row(rows[3].clone()),
            ],
        }
    }

    /// All typeable characters in this layout (lowercase).
    pub fn all_chars(&self) -> Vec<char> {
        self.rows
            .iter()
            .flat_map(|row| row.keys.iter().map(|k| k.lower))
            .filter(|c| c.is_alphabetic())
            .collect()
    }

    /// Home row characters (lowercase).
    pub fn home_row_chars(&self) -> Vec<char> {
        self.rows[2]
            .keys
            .iter()
            .map(|k| k.lower)
            .filter(|c| c.is_alphabetic())
            .collect()
    }

    /// Find which key produces this character and return its finger.
    pub fn finger_for_char(&self, ch: char) -> Option<Finger> {
        let lower = ch.to_ascii_lowercase();
        for row in &self.rows {
            for key in &row.keys {
                if key.lower == lower {
                    return Some(key.finger);
                }
            }
        }
        None
    }
}
