# Drift modularity audit

**Goal of this document:** identify everywhere drift's current
structure blocks adding new kinds of checks — and propose a refactor
path that makes the scorer open to arbitrary pluggable analyses
(e.g. cross-hand async movement detection, n-gram rhythm, finger
travel distance, whatever someone wants to score) without touching
the core.

**Companion to:** `DRIFT_AUDIT.md`. That doc is about *bias*: which
claims are baked in vs. configurable. This one is about *structure*:
which extensions require editing core code vs. dropping in a file.
Fix either without the other and you still lose.

---

## TL;DR

Trigram scoring is already modular: rules are a trait
(`TrigramRule`), they live in `rules/*.rs`, they self-register via a
`[trigram].rules` list. Adding a new trigram rule is one file plus
one line in `registry::construct_rule`. That seam works.

Everything else — bigram scoring, finger-load analysis, row
distribution, any "new dimension" like async cross-hand movement — is
hardcoded in `score.rs` and duplicated in `delta.rs`. There is no
trait, no registry, no extension point. Adding a new kind of check
means editing both files *and* extending `MotionTally` / `ScoreReport`
/ the `apply_motion` match.

The path forward is to promote the trigram pattern to a universal
scoring abstraction: **every scoring axis is a module**, bigrams and
trigrams included. The config-driven pipeline becomes the single
entry point; new checks drop in beside existing ones.

The refactor is ~2–3 days of focused work. Output below.

**Note on the current report layer.** The existing `ScoreReport`,
`MotionTally`, and `report.rs` are placeholder output — "there to see
something is working," not a stable surface that needs to survive the
refactor. Blockers #3 and #4 below describe real structural problems,
but they should be fixed by replacing the report layer wholesale, not
by preserving its current shape. The refactor plan below does this
from day one.

---

## Current state — what's modular vs. what's not

### Modular and clean

**Trigram rules.**

- `TrigramRule` trait in `src/trigram/rule.rs` (33 lines).
- 9 concrete rules under `src/trigram/rules/`, each its own file.
- Config-driven dispatch: `[trigram].rules = [...]` in `drift.toml`.
- `TrigramPipeline` holds `Vec<Box<dyn TrigramRule>>`, ordered by
  config.
- Precomputed `TrigramContext` passes shared geometric queries
  (`is_roll3`, `is_redirect`, row, finger column) to rules so they
  don't each re-derive them.
- `RuleHit` is the uniform return type: `{ category, label, cost }`.
  Scorer aggregates by category for reporting.

This is good architecture. Adding a new trigram rule is ~40 lines in
one file plus one match arm in `registry.rs`.

**Corpus loading and blending.**

- `corpus.rs` reads the oxey-compatible JSON, blends multiple corpora
  with weights. Extensible to new formats by adding a variant, but
  format-agnostic consumers.

**Keyboard loading.**

- `keyboard.rs` loads keywiz JSON5. Clean data model (`Key`, `Keyboard`).
- `Key::is_alpha_core()` method is there as a seam for non-alpha
  scoring, even though unused today.

**Sign-convention scoring.**

- Every rule contributes a signed `cost` summed into `total_score`.
  Order-independent and composable. This is the right invariant.

### Not modular — the seams that block extensibility

These are the blockers for "drop in a new check without touching core."

**1. Bigram scoring is a fixed `match` on a fixed `Motion` enum.**

`src/motion.rs` defines `Motion` with 8 hardcoded variants (Alternate,
SameKey, Sfb, Roll, SameRowSkip, CrossRow, AdjacentForwardOk, Stretch).
`src/score.rs::apply_motion` is a fixed `match` on that enum, each arm
hardcoded to apply a specific weight from config and update a specific
field in `MotionTally`.

Adding a new bigram classification (say, "lateral-stretch" or
"cross-row-aligned-fingers") requires:

- adding a variant to `Motion`
- adding a field to `MotionTally`
- adding a match arm in `apply_motion`
- adding a field to `BigramWeights` in config
- replicating the same change in `delta.rs::bigram_contribution`
- adding a display row in `report.rs::print_motion_breakdown`

That's 6 touch points for one new check. Compare to trigrams: 1 new
file + 1 line in registry.

**2. Bigram scoring has no trait.**

There is no `BigramRule` trait analogous to `TrigramRule`. `motion.rs`
does classification (one-of-N variant); `score.rs` does cost lookup
(fixed match); they're two halves of what should be a rule.

The trigram design split classification (derived properties on
`TrigramContext`) from rule evaluation (`TrigramRule::evaluate`). The
bigram design didn't.

**3. `MotionTally` is a fixed struct.**

`src/score.rs` lines 110–127. Every bigram-aggregate metric (alternate
pct, sfb pct, roll inward pct, cross-row flexion pct, etc.) is a named
field. A new bigram metric has to be added as a new field; the struct
doesn't grow via extension.

**4. `ScoreReport` is equally fixed.**

`src/score.rs` lines 42–81. All rollups (row distribution, finger
load, motion breakdown, top SFBs, top scissors, top rolls, trigram
categories) are named fields with specific types. A new rollup means a
new field and every `print_*` function in `report.rs` has to be
touched.

**5. `apply_motion` and `bigram_contribution` are duplicated.**

`score.rs::apply_motion` (for full scoring with detail collection) and
`delta.rs::bigram_contribution` (for fast SA delta) are two copies of
the same match. Any new bigram classification has to be wired into
both, or scoring and generation diverge.

Same issue with the trigram loop: `score.rs` has a trigram-loop with
category aggregation, `delta.rs::trigram_contribution` has a minimal
version. When a rule's cost computation changes they have to match.

**6. There is no cross-hand or temporal analysis surface.**

All current analyses take `(Key, Key)` (bigram) or `(Key, Key, Key)`
(trigram). There is no way to ask questions like:

- "in a 4-gram, does the left hand drift from home to top while the
  right hand drifts from home to bottom?" (your async example)
- "does the user land on the pinky while the other hand is still
  recovering from a row-change?"
- "sustained row-skew bias over a window" — any n-gram with n > 3.

These need an n-gram-of-arbitrary-length primitive plus state that
tracks "which hand did what, and when." The current scoring loop
doesn't expose that state. `TrigramContext` is hardcoded to 3 keys.

**7. Motion classification state is minimal.**

`classify(a, b, asym)` takes only two keys and the asymmetric rules.
It returns a single `Motion`. It can't emit multiple classifications
("this is both a roll AND a stretch-like thing for finger-x"), can't
flag secondary properties, and can't be extended without changing the
enum.

Trigram rules can independently fire on the same trigram (multiple
rules evaluate the same context, each emits `Option<RuleHit>`). Bigram
classification can't.

**8. `ScoreMode` is a two-valued enum that controls two things.**

`ScoreMode::Full` vs. `FastTotalOnly` gates (a) whether details are
collected for reporting, and (b) whether low-frequency n-grams are
pruned. These should be independent axes. A user might want "full
detail on all n-grams including rare ones" for a report, or "no detail,
no pruning" for batch analytics. The coupling means there are 4
possible states and only 2 are reachable.

**9. Low-frequency thresholds are hardcoded constants.**

`MIN_BIGRAM_FREQ = 0.001` and `MIN_TRIGRAM_FREQ = 0.01` live as
`const` at the top of `score.rs` and *are duplicated* as `const` at
the top of `delta.rs`. Changing them requires editing both. See
category 2.4 of `DRIFT_AUDIT.md`.

**10. Per-char analysis is also hardcoded.**

Row distribution (top/home/bot %) and finger load are computed in a
fixed loop at the top of `score()`. A user wanting "fingers-used-per-
word distribution" or "column-X-load" has no extension point; they'd
edit `score.rs`.

**11. `TrigramContext` has no upward extensibility.**

Rules can't add their own precomputed properties to the context. If
a new rule needs, say, "is this trigram a palindrome in column space,"
it either recomputes on every hit or someone edits `context.rs` to
add a method. The latter works but means rules aren't fully
self-contained.

**12. Trigram rules are hardcoded to the trigram scope.**

`TrigramRule::evaluate(&TrigramContext)` — the signature is
trigram-specific. A rule that wants to look at a 4-gram or a 5-gram
can't exist today. Arbitrary-n context needs a generalization.

**13. Registry uses a hand-rolled match for trigram rule construction.**

`registry::construct_rule` is a `match name { ... }` with one arm per
rule. Adding a rule still needs a code edit — the registry isn't
*fully* data-driven. This is a minor issue, but for a system that
wants to be "drop in a file and it works" it matters. Workaround: an
inventory-style static registration (`inventory` crate or equivalent)
lets rules self-register at link time.

**14. No versioning on config sections.**

`drift.toml`'s schema is implicit. If a rule's config shape changes,
old configs silently get defaults. Not a blocker for modularity but
will bite as rules proliferate.

---

## Proposed architecture

Unify the scoring model around a single abstraction: **analyzers**.
One trait, one registry, one pipeline, one uniform return type. Bigram
and trigram scoring become specializations; n-gram scoring and
stateful analyses (async hand movement, travel distance, etc.) are
first-class.

### The trait

```rust
/// A pluggable analysis over some window of the corpus.
pub trait Analyzer: Send + Sync {
    /// Module name, matches `[analyzers]` entries.
    fn name(&self) -> &'static str;

    /// What this analyzer consumes. Drives dispatch.
    fn scope(&self) -> Scope;

    /// Evaluate one window. Return 0+ hits; empty = rule didn't apply.
    /// The same analyzer can emit multiple hits per window for
    /// separate sub-patterns.
    fn evaluate(&self, window: &Window) -> Vec<Hit>;
}

/// What shape of input an analyzer needs.
pub enum Scope {
    /// One char at a time with frequency. Row/finger-load analyzers.
    Unigram,
    /// Two chars. Current "Motion" classifications.
    Bigram,
    /// Three chars. Current trigram rules.
    Trigram,
    /// Any fixed window length. Async hand movement, rhythm, n-gram.
    Ngram(usize),
    /// Whole-corpus aggregation; runs once, after all n-gram passes.
    /// For things like variance across fingers, overload cost, etc.
    Aggregate,
}

/// Everything an analyzer might need about its window.
/// Extensible via the `props` bag so analyzers can precompute shared
/// derivations (same_hand flags, roll direction, hand-trajectory).
pub struct Window<'a> {
    pub chars: &'a [char],
    pub keys: &'a [&'a Key],
    pub freq: f64,
    pub props: WindowProps,
}

pub struct Hit {
    pub category: &'static str,
    pub label: String,
    pub cost: f64,
}
```

Trigram rules become a degenerate case of `Analyzer` with
`Scope::Trigram`. Existing motion classifications become bigram
analyzers. Row-distribution and finger-load become aggregate or
unigram analyzers.

### The pipeline

```toml
[analyzers]
# Ordered list; each entry names a module under src/analyzers/.
# Unknown names are a hard error. Remove from list to disable.
enabled = [
    "row_distribution",
    "finger_load",
    "sfb",
    "scissor",
    "roll",
    "stretch",
    "inward_roll",
    "outward_roll",
    "onehand",
    "redirect",
    "hand_territory",
    "async_hand_drift",   # new analyzer, just works
]

[analyzers.sfb]
penalty = -7.0

[analyzers.async_hand_drift]
# hypothetical: reward or penalize when both hands change rows in
# opposite directions within a 4-char window
window = 4
opposite_penalty = -2.0
same_direction_reward = 0.5
```

One entry point in code: `pipeline.evaluate(corpus, layout)` runs
every active analyzer in scope order (unigram pass, bigram pass,
trigram pass, n-gram pass per unique window length, aggregate).
Returns `Vec<Hit>` plus `Vec<AggregateResult>` for reporting.

Every analyzer is a file under `src/analyzers/`. Adding an analyzer
is **one file, zero core changes** if the registry is inventory-based
(see below).

### The registry

Two options:

**A. Static match (current pattern, one extra line per rule):**

```rust
fn construct(name: &str, sub: Option<&Value>) -> Result<Box<dyn Analyzer>> {
    match name {
        "sfb" => Ok(Box::new(analyzers::sfb::Sfb::from_config(sub)?)),
        "async_hand_drift" => Ok(Box::new(
            analyzers::async_hand_drift::AsyncHandDrift::from_config(sub)?
        )),
        // ...
        other => Err(anyhow!("unknown analyzer: {other}")),
    }
}
```

Simple, explicit, grep-able. Adding an analyzer = 1 file + 1 match
arm. This is what trigram rules do today and it works fine at ~10
rules. Probably fine up to 40–50.

**B. Inventory-based self-registration (truly drop-in):**

```rust
// In each analyzer file:
inventory::submit! {
    AnalyzerEntry {
        name: "async_hand_drift",
        build: |cfg| Ok(Box::new(AsyncHandDrift::from_config(cfg)?)),
    }
}
```

Zero core-code edits. Pay a dependency (`inventory` crate), gain
truly open extensibility. Recommended once the rule count gets large
enough that the registry match starts feeling like boilerplate.

Either works; start with (A) and upgrade to (B) when the list passes
~20 entries.

### The window-prop bag

`TrigramContext` has precomputed `same_hand`, `is_roll3`, etc. Its
problem is they're hardcoded methods on a trigram-specific struct.
Replace with a computed properties bag:

```rust
pub struct WindowProps {
    pub same_hand_pairs: Vec<bool>,   // same_hand[i] = pair (i, i+1)
    pub roll_direction: Option<RollDir>,
    pub row_pattern: Vec<i32>,
    pub finger_columns: Vec<u8>,
    // Optional: analyzer-attached properties cache
    pub extras: HashMap<&'static str, Box<dyn Any>>,
}
```

Analyzers that need expensive derivations can cache into `extras`;
later analyzers in the same window read the cached value. This is
how async hand drift would precompute "left hand row deltas" once
and share it.

### What this unlocks

- **Async hand movement** is a `Scope::Ngram(4)` analyzer that walks
  a 4-char window, extracts the per-hand row trajectory, and fires if
  left and right hands change rows in opposite directions across the
  window. ~60 lines in one file.

- **Finger travel distance** is a `Scope::Bigram` analyzer that reads
  `(dx, dy)` on the window's two keys and accumulates Euclidean
  travel per hand. ~40 lines.

- **Sustained SFB-adjacent pressure** is a `Scope::Ngram(N)` analyzer
  that detects when a finger is used non-trivially across a window of
  N characters. ~50 lines.

- **Column load** (as distinct from finger load) is a `Scope::Unigram`
  or `Scope::Aggregate` analyzer reading `key.col`. ~30 lines.

- **Row transition rate** per sentence/word boundary is a
  `Scope::Aggregate` analyzer that sums row-changes over the corpus
  and normalizes to typing time. ~30 lines.

None of these require touching `score.rs`, `motion.rs`, `delta.rs`,
`report.rs`, or `config.rs`. They're pure drop-ins.

---

## Refactor path

Since the current report layer is a placeholder, this plan replaces
`ScoreReport`/`MotionTally`/`report.rs` in one go instead of keeping
them alive through intermediate states. That simplifies the sequencing.

### Phase 1 — introduce the Analyzer trait and unified pipeline (~1 day)

1. Create `src/analyzers/` with the `Analyzer` trait, `Scope` enum,
   `Window`, `WindowProps`, and `Hit` types.
2. Create `src/analyzers/registry.rs` and `src/analyzers/pipeline.rs`.
   Pipeline runs one pass per scope (Unigram → Bigram → Trigram →
   each Ngram(n) → Aggregate) and returns `Vec<Hit>` plus per-scope
   rollups.
3. Port existing bigram motion classifications (sfb, roll, scissor,
   stretch) as `Scope::Bigram` analyzers. Keep `motion::classify` as a
   free-function primitive the analyzers call; delete the `Motion`
   enum match.
4. Port trigram rules from `src/trigram/rules/*.rs` to
   `src/analyzers/*.rs` as `Scope::Trigram` analyzers. Delete the
   `src/trigram/` tree.
5. Port row-distribution and finger-load as `Scope::Unigram` +
   `Scope::Aggregate` analyzers. Delete the per-char loop at the top
   of `score()`.
6. Replace `ScoreReport` with a generic result type. Replace
   `report.rs` with a generic renderer that walks the analyzer
   outputs. No more fixed fields.
7. Rebuild `delta.rs` around the same pipeline. `bigram_contribution`
   and `trigram_contribution` become "call analyzer pipeline for this
   window." The duplication goes away.

End of phase 1: every existing score axis runs through the unified
analyzer pipeline. No `MotionTally`, no fixed `ScoreReport`, no dual
bigram/trigram codepaths, no duplicated contribution functions.

### Phase 2 — generalize to N-grams (~half a day)

8. Add `Scope::Ngram(usize)` dispatch to the pipeline. Collect each
   unique window length requested by enabled analyzers; do one pass
   per length.
9. Corpus gains `window_iter(n)` returning (char-sequence, frequency)
   pairs for n > 3. If the corpus file doesn't supply n-gram data for
   the requested length, derive an approximation from bigram chains
   (imperfect but usable) or error clearly.

### Phase 3 — ship first N-gram analyzer (~2 hours)

10. Write `analyzers::async_hand_drift` as `Scope::Ngram(N)` with N
    configurable. The motivating example: "left hand drifts top while
    right hand drifts bottom within an N-char window."
11. Add defaults to `drift.toml`; wire into the registry.

### Phase 4 — documentation and presets (~few hours)

12. Write `docs/WRITING_ANALYZERS.md` covering the trait, the `Scope`
    variants, `WindowProps` + the `extras` cache pattern, and the
    hit-contract expected by the renderer.
13. Ship `presets/neutral.toml` (all opinion-bearing analyzers at
    weight 0 or disabled) and `presets/drifter.toml` (current values
    under their new names).
14. Update `drift.toml` to use `[analyzers]` as the single dispatch
    section. Old section names stop being consulted; document in the
    header that the shape changed.

### Optional phase 5 — inventory-based self-registration

If the analyzer count crosses ~20, switch registry from match-based
to `inventory::submit!`. At that point the registry becomes a
`for entry in inventory::iter::<AnalyzerEntry>()` loop and adding an
analyzer is truly one file, zero core edits.

**Total estimated effort:** 2–3 focused days. Phase 1 is the big one;
phases 2–4 are each small.

---

## Structural invariants worth keeping

Not everything is wrong. Preserve these in the refactor.

- **Signed-cost aggregation.** Every contribution is a signed float
  summed into total. Order-independent, composable, easy to debug.
  Keep.
- **Config-driven rule activation.** `[trigram].rules = [...]` lets
  you disable any rule without rebuilding. Expand to
  `[analyzers].enabled` but keep the mechanism.
- **Hit category + label structure.** `RuleHit { category, label,
  cost }` is the right contract between an analyzer and whatever
  consumes its output (renderer, JSON exporter, future web UI). Keep,
  as `Hit` under the unified analyzer trait.
- **Separation of classification and scoring.** `TrigramContext`
  precomputes geometric queries; `TrigramRule` only decides cost.
  Generalize this into `WindowProps`.
- **Delta scoring capability.** `ScoreAccumulator` enables SA hot
  loops. The refactor has to keep delta scoring *correct*, not
  preserve its current shape — the goal is that a swap only re-runs
  analyzers whose windows include either of the swapped chars.
  Easiest way: let each `Hit` carry (or the pipeline track) the set
  of chars it depended on; delta re-runs analyzers whose dependency
  sets intersect the swap.

---

## Ordering vs. the bias audit

`DRIFT_AUDIT.md` proposed P0 = ship neutral baseline, P1 = add mirror
rules. That work is partly *shape* work (adding rules) and partly
*preset* work (new config file). The shape work is trivial once the
analyzer abstraction exists — every new mirror rule is a drop-in
file.

**Recommended sequence:**

1. Phase 1 of this audit (~1 day): Analyzer trait, unified pipeline,
   port all existing axes (bigram, trigram, unigram, aggregate),
   replace placeholder report layer.
2. P0 of bias audit (~half day): ship `presets/neutral.toml`.
3. P1 of bias audit (~1 day): add mirror rules as analyzers in the
   unified pipeline.
4. Phase 2+3 of this audit (~half day + 2 hours): ngram scope +
   first n-gram analyzer (async hand drift).
5. Phase 4 of this audit (~few hours): writing-analyzers doc,
   finalize preset files.

Total: ~3–4 days of focused work turns drift from "trigram-modular
with hardcoded bigram core" into "fully-modular analyzer framework
where every claim is expressible as a pluggable module." At that
point the 2× gap between Gallium and Drifter can be re-examined on
provably-neutral defaults, and new analyses (async movement, travel,
rhythm) land without core edits.

---

## What's landed

The refactor described above has been done. Quick status of each phase:

**Phase 1 — Analyzer trait + unified pipeline: ✓ done.**
- 12 crates in a workspace under `drift/crates/`, compiler-enforced
  dependency graph.
- `drift-core` holds shared types (Finger, Key, Row, Keyboard, Layout,
  Hit, Scope, Window, WindowProps) with `#[non_exhaustive]` on the
  variant-bearing enums for future-proof additions.
- `drift-analyzer` holds the `Analyzer` trait, `Registry`, `Pipeline`,
  `PipelineBuilder`, `AggregateContext`, and a format-agnostic
  `ConfigValue` trait.
- `drift-analyzers` contains 20 stock analyzer modules; each registers
  itself via a per-module `register(&mut Registry)` function.
- `drift-motion` holds classification primitives (`Geometry`,
  `cross_row_kind`, `is_forward_exempt`, `roll_direction`).
- `drift-score` is a ~200-line scope-driven executor; there is no
  longer a fixed `ScoreReport`, `MotionTally`, or per-scope match.
- `drift-delta` generic delta scoring with a regression test that
  verifies bit-level agreement with full rescoring on both presets.

**Phase 2 — Scope beyond bigram/trigram: ✓ partial.**
- `Scope::Ngram(usize)` variant exists and the pipeline dispatches
  it. No analyzer uses it yet; the oxey corpus format doesn't
  supply 4-grams and loader-side derivation from trigrams isn't
  implemented. When someone needs genuine 4-grams+, implement
  derivation in drift-corpus and add an `Scope::Ngram(n)` analyzer.
- `Scope::Skipgram(gap)` variant added and dispatched. The oxey
  corpus supplies skipgrams at gaps 1/2/3; `MemoryCorpus::skipgrams`
  stores them and `CorpusSource::iter_skipgrams(gap)` exposes
  them. Blend propagates skipgrams. `drift-delta` builds
  skipgram-scope indexes and re-uses the generic window-reevaluation
  path. Ships a first analyzer (`sfs`, same-finger skipgram).

**Phase 3 — Async-hand-drift analyzer: ✓ done (at trigram scope).**
- `async_hand_drift` analyzer registered in `drift-analyzers`.
- Partitions each trigram's chars by hand, computes mean row offset
  per hand, penalizes windows where the hands' mean offsets have
  strictly opposite signs.
- Drifter preset enables at `weight = -2.0`; neutral at `0.0`.
- Verified: on the drifter preset, Gallium racks up 3525 async-drift
  hits at -5.706 cost vs Drifter's 2783 / -4.359 — a 27% higher hit
  rate and 31% worse contribution for Gallium, matching the
  row-territory thesis the analyzer is meant to capture.
- See also `drift-delta/tests/delta_matches_full.rs` — the analyzer
  is covered by the generic bit-exact delta regression test across
  all three presets.

**Phase 4 — Docs and presets: ✓ done.**
- Three presets ship: `neutral`, `drifter`, `extension`.
- `drift-config` loads presets by name; `--preset <name>` in the CLI.
- `docs/WRITING_ANALYZERS.md` covers the trait, scopes, window props,
  config helpers, registration, delta correctness, and a worked
  example.

**Additions beyond the original plan:**
- Layout writer in `drift-keyboard::writer` — `generate` can output
  a valid keywiz JSON5 file.
- Reproducibility fix: all summation paths sort their inputs so
  seeded SA runs produce identical results across invocations.
- Mirror analyzers (`extension_cascade`, per-finger `terminal_penalty`,
  parameterized `flexion_cascade` row set) landed alongside the
  refactor rather than as follow-ups — the analyzer trait made them
  drop-in additions.

**Validation that the refactor preserved behavior:**
- Drifter scores +23.989 on the drifter preset (matches pre-refactor).
- Gallium scores +11.544 on the drifter preset (matches pre-refactor).
- `drift-delta` regression tests pass bit-exact.
- 200k-iteration SA run completed with zero accumulator drift vs.
  full rescore of the best layout.
