//! Trigram redirect analyzer — same-hand direction flip.
//!
//! Fires on same-hand trigrams whose finger direction reverses at
//! the middle key. Splits into `redirect` (some finger in the
//! trigram is in the configured anchor set) and `bad_redirect` (no
//! anchor present — more punishing because the hand has no strong
//! pivot).
//!
//! The anchor set is a config list of finger names. Default is
//! `["l_index", "r_index"]`. Users whose stable pivot is another
//! finger (e.g. middle on wide-stagger boards) can change that
//! without editing code.

use anyhow::Result;
use drift_analyzer::{f64_or, strings_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Finger, Hit, Scope, Window};

use crate::finger_util::parse_finger_name;
use crate::trigram_util::is_redirect;

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "redirect",
        build: |cfg| Ok(Box::new(Redirect::from_config(cfg)?)),
    });
    registry.register(AnalyzerEntry {
        name: "bad_redirect",
        build: |cfg| Ok(Box::new(BadRedirect::from_config(cfg)?)),
    });
}

fn read_anchors(cfg: Option<&dyn ConfigValue>) -> Vec<Finger> {
    let names = strings_or(cfg, "anchor_fingers", &["l_index", "r_index"]);
    names.iter().filter_map(|n| parse_finger_name(n)).collect()
}

fn has_anchor(window: &Window, anchors: &[Finger]) -> bool {
    window.keys.iter().any(|k| anchors.contains(&k.finger))
}

pub struct Redirect {
    pub weight: f64,
    pub anchors: Vec<Finger>,
}

impl Redirect {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight: f64_or(cfg, "weight", -3.0),
            anchors: read_anchors(cfg),
        })
    }
}

impl Analyzer for Redirect {
    fn name(&self) -> &'static str {
        "redirect"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        if !is_redirect(window.props) {
            return Vec::new();
        }
        if !has_anchor(window, &self.anchors) {
            return Vec::new();
        }
        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "redirect",
            label: format!("{a}{b}{c}"),
            cost: window.freq * self.weight,
        }]
    }
}

pub struct BadRedirect {
    pub weight: f64,
    pub anchors: Vec<Finger>,
}

impl BadRedirect {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight: f64_or(cfg, "weight", -5.0),
            anchors: read_anchors(cfg),
        })
    }
}

impl Analyzer for BadRedirect {
    fn name(&self) -> &'static str {
        "bad_redirect"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        if !is_redirect(window.props) {
            return Vec::new();
        }
        if has_anchor(window, &self.anchors) {
            return Vec::new();
        }
        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "bad_redirect",
            label: format!("{a}{b}{c}"),
            cost: window.freq * self.weight,
        }]
    }
}
