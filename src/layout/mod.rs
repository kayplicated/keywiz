pub mod kanata;

/// Standard US QWERTY layout.
pub fn qwerty() -> Layout {
    Layout::from_rows("qwerty", [
        vec![('`','~'),('1','!'),('2','@'),('3','#'),('4','$'),('5','%'),('6','^'),('7','&'),('8','*'),('9','('),('0',')'),('-','_'),('=','+')],
        vec![('q','Q'),('w','W'),('e','E'),('r','R'),('t','T'),('y','Y'),('u','U'),('i','I'),('o','O'),('p','P'),('[','{'),('\\','|'),(']','}')],
        vec![('a','A'),('s','S'),('d','D'),('f','F'),('g','G'),('h','H'),('j','J'),('k','K'),('l','L'),(';',':'),('\'','"')],
        vec![('z','Z'),('x','X'),('c','C'),('v','V'),('b','B'),('n','N'),('m','M'),(',','<'),('.','>'),('/','?')],
    ])
}

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

/// Finger assignment for columnar stagger keyboards.
/// Each finger gets one column except index fingers which get two.
/// Left: pinky=0, ring=1, middle=2, index=3-4 | Right: index=5-6, middle=7, ring=8, pinky=9+
fn finger_for_col_colstag(col: usize) -> Finger {
    match col {
        0 => Finger::LPinky,
        1 => Finger::LRing,
        2 => Finger::LMiddle,
        3 | 4 => Finger::LIndex,
        5 | 6 => Finger::RIndex,
        7 => Finger::RMiddle,
        8 => Finger::RRing,
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

    /// Reassign finger mappings for colstag or rowstag layout.
    /// Only reassigns alpha rows (top, home, bottom) — number row keeps standard mapping.
    pub fn set_colstag(&mut self, colstag: bool) {
        let finger_fn = if colstag { finger_for_col_colstag } else { finger_for_col };
        for row in &mut self.rows[1..] {
            for (col, key) in row.keys.iter_mut().enumerate() {
                key.finger = finger_fn(col);
            }
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
    #[allow(dead_code)]
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

    /// Build a translation map from another layout to this one by physical position.
    /// e.g. if `from` is QWERTY and `self` is Gallium, then pressing QWERTY 'j'
    /// maps to whatever Gallium has at that position ('h').
    pub fn translation_from(&self, from: &Layout) -> std::collections::HashMap<char, char> {
        let mut map = std::collections::HashMap::new();
        for (row_idx, from_row) in from.rows.iter().enumerate() {
            if let Some(to_row) = self.rows.get(row_idx) {
                for (col_idx, from_key) in from_row.keys.iter().enumerate() {
                    if let Some(to_key) = to_row.keys.get(col_idx) {
                        map.insert(from_key.lower, to_key.lower);
                        map.insert(from_key.upper, to_key.upper);
                    }
                }
            }
        }
        map
    }
}
