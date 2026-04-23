# Writing a drift analyzer

A drift analyzer is a small module that reads a slice of the corpus
(a window), consults the resolved keyboard layout, and emits zero or
more signed contributions to a layout's total score. Analyzers are
the extension point — whenever you want drift to measure a new
ergonomic property, write one.

This doc walks through what you need to know to ship an analyzer,
with a worked example and callouts to the cases that trip people up.

## The trait

```rust
pub trait Analyzer: Send + Sync {
    fn name(&self) -> &'static str;
    fn scope(&self) -> Scope;

    fn evaluate(&self, window: &Window) -> Vec<Hit> { /* default empty */ }
    fn evaluate_aggregate(&self, ctx: &AggregateContext) -> Vec<Hit> { /* default empty */ }

    fn dependencies(&self, window: &Window) -> Vec<char> { /* default: all chars */ }
}
```

Four things to implement:

- `name` — stable identifier, used in config (`[analyzers.<name>]`),
  reports, and the registry. Must match the string users write in
  preset files. Typically a `&'static str` literal.
- `scope` — what window shape you consume (see next section).
- Either `evaluate` (for window-scoped analyzers) *or*
  `evaluate_aggregate` (for aggregate analyzers), not both.
- `dependencies` is optional; default "all chars in the window" is
  safe and correct for almost every analyzer. Only override when you
  can prove your output depends on a narrower subset — see the
  "delta correctness" section below.

## Choose a scope

`Scope` controls which pass of the pipeline calls your analyzer:

- **`Scope::Unigram`** — one character at a time, paired with its
  corpus frequency. Use for pure per-char analysis (is this letter
  on the row I want? on the finger I want?). Rarely needed; most
  per-char analysis happens in aggregate scope where you see the
  whole-corpus rollup.
- **`Scope::Bigram`** — adjacent pairs `(a, b)` with frequency.
  Most common for hand-motion analyzers (SFB, roll, scissor).
- **`Scope::Trigram`** — adjacent triples. Used for rhythm patterns
  (inward/outward rolls, redirects, cascades, row-synchrony).
- **`Scope::Ngram(usize)`** — arbitrary fixed-length windows. Use
  for patterns that need more than 3 keys of context (async
  hand-drift across 4 chars, travel distance over 5, sustained
  finger load). The pipeline does one pass per distinct n
  requested.
- **`Scope::Aggregate`** — runs once, at the end, over whole-corpus
  rollups (`char_load`, `finger_load`). Use for anything that
  depends on totals, not individual windows — row distribution,
  finger overload, entropy measures.

Pick the smallest scope that captures what you need. Smaller scopes
run faster and delta-score more efficiently.

## What the window gives you

For non-aggregate analyzers, `Window` is the input:

```rust
pub struct Window<'a> {
    pub chars: &'a [char],
    pub keys: &'a [&'a Key],
    pub freq: f64,
    pub props: &'a WindowProps,
}
```

`chars[i]` is the character in corpus order. `keys[i]` is its
resolved position on the layout. `freq` is the corpus frequency of
this exact sequence as a percentage (0..=100, so a 1% bigram reads
as `1.0` not `0.01`).

`props` is precomputed once per window and shared across all
analyzers at that scope:

```rust
pub struct WindowProps {
    pub same_hand_pairs: Vec<bool>, // length chars.len()-1
    pub all_same_hand: bool,
    pub finger_columns: Vec<u8>,    // 0=pinky..3=index, length chars.len()
    pub rows: Vec<Row>,             // length chars.len()
}
```

Read from `props` whenever you can — it's cheaper than deriving
the same info yourself, and consistent across analyzers.

## What the aggregate context gives you

For aggregate analyzers:

```rust
pub struct AggregateContext<'a> {
    pub layout: &'a Layout,
    pub corpus_name: &'a str,
    pub char_load: &'a HashMap<char, f64>,
    pub finger_load: &'a HashMap<Finger, f64>,
}
```

`char_load` and `finger_load` are percentages already filtered to
characters present on the layout — no need to re-check.

## Reading config

Analyzer constructors take `Option<&dyn ConfigValue>` — `None` if
the user didn't provide a subtree for you, `Some` otherwise. Use
the helpers in `drift_analyzer`:

```rust
use drift_analyzer::{f64_or, bool_or, strings_or};

pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
    Ok(Self {
        weight: f64_or(cfg, "weight", 1.0),
        enabled: bool_or(cfg, "enabled", true),
        fingers: strings_or(cfg, "fingers", &["l_index", "r_index"]),
    })
}
```

Each helper reads a named field, falling back to the given default
if the field is missing or of the wrong type. If you need more than
scalars and lists of strings, implement `ConfigValue::get` yourself
— it returns a boxed owned adapter.

For finger names use `crate::finger_util::parse_finger_name`. For
row names use `crate::row_util::parse_row_name`. Both are
case-insensitive and accept common variants (`l_pinky`, `LPinky`,
`l-pinky`).

## Shared helpers

Several utilities live in `drift-analyzers` for analyzer reuse:

- `trigram_util::is_roll3_inward / is_roll3_outward /
  is_roll3_inward_skip / is_roll3_outward_skip / is_alternating /
  is_redirect` — pure predicates on `WindowProps`.
- `row_util::row_index(Row) -> i32` — integer mapping for row
  arithmetic (Top=-1, Home=0, Bottom=1, Extra(n)=2+n).
- `row_util::parse_row_name(&str) -> Option<Row>`.
- `finger_util::parse_finger_name(&str) -> Option<Finger>`.

For bigram geometry use `drift-motion`:

- `drift_motion::geometry(a, b) -> Geometry` — struct with
  same_hand, same_finger, finger_gap, dx, dy, row_delta.
- `drift_motion::roll_direction(a, b) -> Option<RollDirection>`
  (Inward/Outward, `None` for cross-hand or same-finger).
- `drift_motion::cross_row_kind(row_a, row_b) -> CrossRowKind`
  (Flexion/Extension/FullCross/Other).
- `drift_motion::is_forward_exempt(a, b, &rules)` — asymmetric-
  forward exemption for col-stag keyboards.

## The Hit you emit

```rust
pub struct Hit {
    pub category: &'static str, // stable id for report rollups
    pub label: String,          // human-readable per-hit text
    pub cost: f64,              // signed — positive reward, negative penalty
}
```

A few rules:

- **Category must be a compile-time string literal.** Reports group
  by category; keep it stable across the analyzer's lifetime. If you
  need dynamic per-hit categorization, put the distinguishing info
  in `label`.
- **Return `Vec::new()` for non-matches.** Don't emit a zero-cost hit
  to signal "I saw this window but chose not to score it." Empty Vec
  is the canonical no-op.
- **One analyzer can emit multiple hits per window.** Rare, but
  supported (e.g. `row_distribution` emits one hit per row from a
  single aggregate call).

## Registration

Each analyzer file exposes a `register` function:

```rust
use drift_analyzer::{AnalyzerEntry, Registry};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "your_analyzer",
        build: |cfg| Ok(Box::new(YourAnalyzer::from_config(cfg)?)),
    });
}
```

Then add your module to `drift-analyzers/src/lib.rs` and wire
it into `register_all`:

```rust
pub mod your_analyzer;
// ...

pub fn register_all(registry: &mut Registry) {
    // ...
    your_analyzer::register(registry);
}
```

That's the only code change outside your new file.

## Adding to a preset

Config files list analyzers by name and supply per-analyzer
parameters:

```toml
[analyzers]
enabled = [
    # ...existing analyzers
    "your_analyzer",
]

[analyzers.your_analyzer]
weight = 1.0
fingers = ["l_index", "r_index"]
```

If you want the analyzer off by default in a preset, either omit
it from `enabled` or include it with weights set to 0. Including-
with-weight-0 makes the preset's position explicit — future-you
or a reader can tell the rule was considered and deliberately
disabled, rather than forgotten.

Drift ships three reference presets:

- `neutral.toml` — all opinion-bearing analyzers at weight 0.
  The baseline when you want claims to stand on structural merit.
- `drifter.toml` — flexion-favoring, hand-territory-aware.
- `extension.toml` — mirror of drifter for reaching-preferring
  philosophies.

When adding a new analyzer, ship it in **all three** presets with
weights that fit each preset's philosophy. At minimum: `neutral`
gets weight 0, the two opinionated presets get whatever makes sense.

## Delta correctness

Drift's SA generator uses incremental scoring to avoid rescoring
the full corpus on every candidate swap. This works by asking each
analyzer for its window-level dependency set:

```rust
fn dependencies(&self, window: &Window) -> Vec<char> {
    window.chars.to_vec() // default
}
```

The default "every char in the window" is always correct. Override
it only when you're certain your analyzer's output for a given
window depends on a strict subset — for example, if your analyzer
reads only `window.chars[0]` and ignores the rest, you could
return `vec![window.chars[0]]` and enable finer-grained delta
re-evaluation.

Getting this wrong produces silent correctness bugs: the
accumulator's total will drift from the true score over many
swaps. The regression test in
`drift-delta/tests/delta_matches_full.rs` catches this for the
stock analyzer set. If you override `dependencies`, extend that
test to include your analyzer in a non-default preset.

## A worked example — finger travel

Let's write an analyzer that penalizes bigrams by the Euclidean
travel distance between the two keys on the same hand. Zero for
alternation, penalty proportional to `sqrt(dx² + dy²)` for
same-hand motions. This isn't in the stock set — it's an example.

```rust
//! Finger travel distance — penalty for long same-hand motions.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "finger_travel",
        build: |cfg| Ok(Box::new(FingerTravel::from_config(cfg)?)),
    });
}

pub struct FingerTravel {
    pub weight_per_unit: f64,
}

impl FingerTravel {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight_per_unit: f64_or(cfg, "weight_per_unit", -0.1),
        })
    }
}

impl Analyzer for FingerTravel {
    fn name(&self) -> &'static str { "finger_travel" }
    fn scope(&self) -> Scope { Scope::Bigram }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let a = window.keys[0];
        let b = window.keys[1];
        if !a.finger.same_hand(b.finger) {
            return Vec::new();
        }
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance == 0.0 {
            return Vec::new();
        }
        vec![Hit {
            category: "finger_travel",
            label: format!("{}{} {:.2}u", window.chars[0], window.chars[1], distance),
            cost: window.freq * distance * self.weight_per_unit,
        }]
    }
}
```

Fewer than 40 lines. To ship this you:

1. Save as `drift-analyzers/src/finger_travel.rs`.
2. Add `pub mod finger_travel;` + `finger_travel::register(registry);`
   in `lib.rs`.
3. Add `"finger_travel"` to each preset's `[analyzers].enabled` list
   with a `[analyzers.finger_travel]` subtree (neutral: weight 0;
   opinionated presets: whatever fits).

No changes to drift-score, drift-delta, drift-generate, drift-
config, or drift-report. Your analyzer appears in reports
automatically because the text renderer groups by category.

## Gotchas

- **Don't mutate state in `evaluate`.** Analyzers must be stateless
  so the SA hot loop can call them thousands of times per second
  without interference. All state lives in the constructor-captured
  config.
- **Cost scales with `freq`.** The pipeline passes the corpus
  frequency in `window.freq`; you almost always want to multiply by
  it so rare bigrams contribute less than common ones. Forgetting
  this is the most common analyzer bug.
- **`all_same_hand` for bigram scope** — just check
  `props.same_hand_pairs[0]` or `a.finger.same_hand(b.finger)`.
  `all_same_hand` is convenient but always equivalent for length-2
  windows.
- **Avoid hardcoded per-finger / per-row magic.** If your analyzer
  singles out a finger or row, accept that as config so users whose
  hand shape differs can re-point it. Look at `terminal_penalty`
  and `redirect` for examples.
- **Report the `label` informatively.** Users scan the "top
  contributions per category" output to understand what's driving
  their score. A label like `"{a}{b}{c}"` beats `"hit"` by a lot.
