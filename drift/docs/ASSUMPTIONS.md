# Drift — structural assumptions

This document lists the structural preconditions drift's code
relies on. These aren't bugs or biases in the configurable sense
(see `DRIFT_AUDIT.md` for those) — they're baked into the type
system, the keyboard schema, or the analyzer logic in ways that
aren't easy to parameterize. If your use case hits one of them,
drift's output will be wrong or nonsensical for reasons the config
can't fix.

Read this before you adapt drift to an unusual keyboard, an
unusual language, or an unusual scoring model.

---

## Finger model: four fingers per hand

**Where it lives:** `drift_core::Finger` variants.

```rust
pub enum Finger {
    LPinky, LRing, LMiddle, LIndex,
    RIndex, RMiddle, RRing, RPinky,
}
```

Eight alpha fingers total. No variant for thumbs; drift treats
thumb keys as not-scored. `Finger::column()` returns 0..=3
(pinky..index), which every analyzer that counts "finger
distance" assumes.

**What this breaks:**

- Keyboards where thumbs type alpha characters (e.g. splitography,
  dedicated-thumb chording schemes). Drift's loader drops thumb
  keys silently. Treat any thumb-alpha design as out of scope.
- Modified fingering schemes that reassign columns in
  non-traditional ways (e.g. "LIndex handles column 5") still
  work geometrically, but analyzers that compare `Finger` values
  interpret them as the standard fingering.

**What's actually configurable:** the `finger` string in each key
of the keyboard JSON picks from the eight variants. You can
assign any column to any finger — that's the schema's escape
hatch. But you can't add a ninth finger.

## Index finger owns two sub-columns

**Where it lives:** `drift_core::FingerColumn` with variants
`Outer` and `Inner`; `Key::finger_column`.

Index fingers on most keyboards cover two alpha columns — the
outer "home" column and an inner column reached across the
central split or toward the thumb. Drift models this explicitly
so analyzers that care about true vertical SFBs can distinguish
them from lateral column-crossings (`ck`, `wh`).

**What this breaks:**

- Keyboards where some non-index finger owns two columns
  (uncommon but theoretically possible). There's no way to
  represent "ring finger with two sub-columns" — the
  `FingerColumn` enum variant list would need extending.
- Keyboards where the index owns only one column (e.g. a
  hypothetical 8-column alpha grid). Works fine — every key
  just defaults to `Outer`.

**What's actually configurable:** the `finger_column` field in
each raw key in the keyboard JSON. Leave it out and the loader
infers from `|col|` (index with `|col|=1` is `Inner`, everything
else is `Outer`).

## Three alpha rows

**Where it lives:** `drift_core::Row` variants `Top`, `Home`,
`Bottom` (plus `Number` and `Extra(n)` for out-of-alpha).

Analyzers like `flexion_cascade`, `extension_cascade`,
`row_cascade`, and the scissor classifier all assume three alpha
rows. A row-cascade rule that "fires when a same-hand trigram
visits all three rows" is hardcoded to mean top+home+bottom.

**What this breaks:**

- 4-row alpha layouts (very rare). The `Row::Extra(n)` variant
  exists as an escape hatch, but no current analyzer scores it.
  If you assign a common letter to `Row::Extra(0)`, drift will
  see it as "on some row we don't know what to do with" and it
  won't participate in any row-based scoring.
- Smaller alpha grids (e.g. 2-row ortho) still work — missing
  rows just don't get populated — but row-cascade rules will
  never fire.

**What's actually configurable:** the `r` integer in each raw
key. The loader maps `-2` → `Number`, `-1` → `Top`, `0` → `Home`,
`1` → `Bottom`, anything else → `Extra(n)`.

## Y-coordinate convention: larger = toward user

**Where it lives:** `drift-motion::asymmetric::is_forward_exempt`.

The asymmetric-forward scissor exemption checks
`outer.y > inner.y` where "outer" is the pinky-ward finger. The
semantic assumption is: **larger y = physically closer to the
user**. On col-stag boards with the standard splay, the pinky
home row sits at larger y (pulled back toward user) than the
middle home row (pushed forward away from user).

**What this breaks:**

- Boards where the y-convention is reversed (larger y = away
  from user). The asymmetric-forward exemption would fire in
  reverse — exempting real scissors, treating natural splay as
  a scissor.
- Flat-splay keyboards (no col-stag) where all keys sit at the
  same y. The exemption never fires, which is correct — every
  adjacent cross-row motion on a flat board is a real scissor.
- Inverted-splay keyboards (where middle is pulled back and
  pinky is pushed forward — unusual but exists on some
  ergonomic designs). The current rule won't exempt their
  natural rest shape.

**What's actually configurable:** per-pair toggles
(`index_middle_forward_ok`, `middle_ring_forward_ok`,
`ring_pinky_forward_ok`) and a `forward_threshold`. The
direction of the rule (outer-forward = natural) is hardcoded in
drift-motion. Inverting it requires a code change, not a config
change. See `DRIFT_AUDIT.md` item 1.6.

## Cross-row kind mapping

**Where it lives:** `drift-motion::cross_row::cross_row_kind`.

The mapping of `(Row, Row)` pairs to `CrossRowKind` hardcodes:
- `(Home, Bottom)` / `(Bottom, Home)` → `Flexion`
- `(Home, Top)` / `(Top, Home)` → `Extension`
- `(Top, Bottom)` / `(Bottom, Top)` → `FullCross`
- everything else → `Other`

"Flexion" means curling the finger down from home; "extension"
means reaching up. This is the biomechanically standard
convention for row-stag and col-stag boards with a horizontal
baseline.

**What this breaks:**

- Boards with heavily tilted or tented geometries where "down"
  no longer means "curl." Rare.
- Boards with more than three alpha rows — the kind classifier
  returns `Other` for any pair involving `Extra(n)`, and
  scissor analyzers skip those.

**What's actually configurable:** the individual direction
weights (`flexion`, `extension`, `full_cross`) on the
`scissor` analyzer. The mapping itself is not.

## Anglo-centric corpus assumptions

**Where it lives:** preset defaults; some analyzer heuristics.

Several analyzer weights reflect English-typing patterns:
- `inward_roll` defaults to higher weight than `outward_roll`
  (3.0 vs 2.5 in the Drifter preset). This reflects inward
  rolls being more common and more valuable in English.
- The redirect rule's anchor-finger default is `[l_index,
  r_index]`. On English keyboards the index tends to be the
  strongest and most central pivot, but this is a biomechanical
  default, not a linguistic one.

**What this breaks:** languages with very different bigram/
trigram distributions may want different weights. None of the
analyzer logic assumes English; it's all corpus-driven. But the
preset defaults do.

**What's actually configurable:** every weight in the presets.
Drift ships a `drifter.toml`, `neutral.toml`, `extension.toml`,
and `oxey_mimic.toml` as reference points. Building a preset for
German, Dutch, French, etc. is a matter of tuning weights and
swapping the corpus — no code changes.

## Keyboard-native layout schema

**Where it lives:** `drift-keyboard::keyboard` and
`drift-keyboard::layout` loaders; drift-keyboard's JSON5
parsing.

Drift reads the keywiz keyboard + layout schemas. Other tools'
layouts (oxey's `.dof`, QMK `keymap.c`, kanata, etc.) can't be
loaded without a converter.

**What this breaks:** anyone wanting to score a non-keywiz
layout must first convert it. No drift crate currently provides
such a converter.

**What's actually configurable:** the schema is stable and
documented in the keywiz project. Third parties could write
converters that produce keywiz JSON as output.

## Corpus schema: oxeylyzer-compatible JSON

**Where it lives:** `drift-corpus::oxey`.

The only corpus format drift loads is the oxeylyzer-style JSON
with `chars`, `bigrams`, `trigrams`, and (optionally)
`skipgrams`, `skipgrams2`, `skipgrams3` fields. Frequencies are
percentages (0..=100), not counts.

**What this breaks:** custom corpus formats. Raw-text-corpus
ingestion (read a file, count n-grams, produce a
`MemoryCorpus`) is not implemented; it would need to be written.

**What's actually configurable:** drift defines a
`CorpusSource` trait. Any type implementing it can be consumed
by the scoring pipeline. A raw-text or custom-format loader
would implement this trait.

## N-gram derivation

**Where it lives:** `drift-corpus::MemoryCorpus::iter_ngrams`.

For n ≤ 3, ngrams come from the corpus's `chars`, `bigrams`,
and `trigrams` tables directly. For n ≥ 4, ngrams come only
from the corpus's `ngrams` map — which is populated when the
caller pre-loads them, and empty otherwise.

Drift has **no automatic n-gram derivation**. An analyzer of
scope `Ngram(4)` will see an empty iterator unless the loader
supplies data.

**What this breaks:** any analyzer at `Scope::Ngram(4)` or
higher returns zero hits on a standard oxeylyzer corpus file.
The `async_hand_drift` analyzer was originally intended as
`Ngram(4)` but was downgraded to `Trigram` scope for this
reason.

**What's actually configurable:** nothing at runtime. A future
loader that derives higher-n ngrams from trigram chains (or
from raw text) would lift this restriction.

---

## Summary

Drift makes seven structural assumptions: 4 fingers per hand,
index has 2 sub-columns, 3 alpha rows, y-up = toward-user,
standard flexion/extension direction mapping, keywiz schemas for
keyboards and layouts, oxey schema for corpora. Six of those
reflect mainstream keyboard conventions and are fine for ≥99% of
use cases. The seventh (no n-gram derivation for n ≥ 4) is a
gap that limits what analyzers can be written today.

If you're hitting a case that doesn't fit, the path forward is
usually to extend the type system (add a new `Row` variant, a
new `FingerColumn` variant) or add a loader, rather than work
around the assumption in config.
