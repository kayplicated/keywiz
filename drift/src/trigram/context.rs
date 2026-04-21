//! The context passed to every trigram rule.
//!
//! Precomputes common properties (same-hand runs, finger indices,
//! row pattern) so rules don't each re-derive them.

use crate::keyboard::{Finger, Key};

/// Per-trigram context. Built once per trigram and passed to every
/// rule in the pipeline.
#[derive(Debug)]
pub struct TrigramContext<'a> {
    pub chars: [char; 3],
    pub keys: [&'a Key; 3],
    pub freq: f64,

    /// Whether each consecutive pair is on the same hand.
    pub same_hand: [bool; 2],

    /// All three letters on the same hand.
    pub all_same_hand: bool,
}

impl<'a> TrigramContext<'a> {
    pub fn new(chars: [char; 3], keys: [&'a Key; 3], freq: f64) -> Self {
        let same_hand = [
            keys[0].finger.same_hand(keys[1].finger),
            keys[1].finger.same_hand(keys[2].finger),
        ];
        let all_same_hand = same_hand[0] && same_hand[1];
        Self {
            chars,
            keys,
            freq,
            same_hand,
            all_same_hand,
        }
    }

    /// 0=pinky .. 3=index index within hand. Useful for roll direction.
    pub fn finger_column(&self, i: usize) -> u8 {
        finger_column(self.keys[i].finger)
    }

    /// Row index (-1 top, 0 home, 1 bot).
    pub fn row(&self, i: usize) -> i32 {
        self.keys[i].row
    }

    /// True if the trigram is a strict roll in direction `dir`:
    /// all same hand, fingers strictly monotonic in the given
    /// direction, with adjacent columns each step.
    pub fn is_roll3(&self, dir: RollDir) -> bool {
        if !self.all_same_hand {
            return false;
        }
        let a = self.finger_column(0);
        let b = self.finger_column(1);
        let c = self.finger_column(2);
        match dir {
            RollDir::Inward => a + 1 == b && b + 1 == c,
            RollDir::Outward => a == b + 1 && b == c + 1,
        }
    }

    /// True if the trigram is an "inroll-skip": monotonic inward/
    /// outward with a single gap of two on one of the two steps.
    /// e.g. pinky -> ring -> index (skip middle), still feels rolly.
    pub fn is_roll3_skip(&self, dir: RollDir) -> bool {
        if !self.all_same_hand {
            return false;
        }
        let a = self.finger_column(0) as i8;
        let b = self.finger_column(1) as i8;
        let c = self.finger_column(2) as i8;
        let (d1, d2) = (b - a, c - b);
        match dir {
            RollDir::Inward => {
                d1 > 0 && d2 > 0 && (d1 + d2 <= 3) && (d1 == 2 || d2 == 2) && d1 != d2
            }
            RollDir::Outward => {
                d1 < 0 && d2 < 0 && (d1.abs() + d2.abs() <= 3) && (d1 == -2 || d2 == -2) && d1 != d2
            }
        }
    }

    /// Whether the trigram alternates hands: L-R-L or R-L-R.
    pub fn is_alternating(&self) -> bool {
        !self.same_hand[0] && !self.same_hand[1]
    }

    /// True if the trigram is same-hand but direction flips at the
    /// middle letter (outer→inner→outer, or inner→outer→inner).
    /// The middle key is also strictly farther (in column index)
    /// than both outer keys on one side.
    pub fn is_redirect(&self) -> bool {
        if !self.all_same_hand {
            return false;
        }
        let a = self.finger_column(0) as i8;
        let b = self.finger_column(1) as i8;
        let c = self.finger_column(2) as i8;
        // Direction flip: signs of (b-a) and (c-b) differ, neither zero.
        let d1 = b - a;
        let d2 = c - b;
        d1 != 0 && d2 != 0 && (d1.signum() != d2.signum())
    }

    /// Terminal finger of the trigram (finger of keys[2]).
    pub fn terminal_finger(&self) -> Finger {
        self.keys[2].finger
    }
}

/// Direction of a roll along finger-columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollDir {
    /// Toward thumb (pinky -> ring -> middle -> index).
    Inward,
    /// Toward edge (index -> middle -> ring -> pinky).
    Outward,
}

fn finger_column(f: Finger) -> u8 {
    use Finger::*;
    match f {
        LPinky | RPinky => 0,
        LRing | RRing => 1,
        LMiddle | RMiddle => 2,
        LIndex | RIndex => 3,
    }
}
