# Drift — aspirational specs

What drift could be. Not a committed plan, not a to-do list — a
statement of shapes we'd like drift to grow into, with enough
detail that the intent is clear and the gap between now and there
is visible.

Current state: the foundation is stable. The audit docs
(`DRIFT_AUDIT.md`, `MODULARITY_AUDIT.md`, `ASSUMPTIONS.md`) track
what's been built; this doc tracks what we'd still like to build.

Specs below are grouped by certainty, not by priority. A Tier 1
item is one where "what done looks like" is already clear. A Tier
2 item needs design thinking first. A Tier 3 item is a direction,
not a destination.

---

## Tier 1 — shapes with clear definitions

*All previously-listed Tier 1 items are complete (2026-04-22) —
see the "Completed" section. This section is intentionally left
open; new items with clear definitions land here.*

---

## Tier 2 — useful but needs design work

### Smarter generator

**What:** The current `drift-generate` is plain simulated
annealing with random single-char swaps and a linear cooling
schedule. It works and produces the results we've seen, but it's
the simplest possible SA variant.

**What could change:**

- **Swap modes.** Currently only 2-char swaps. Adding 3-char
  rotations, row-swaps, column-swaps, or "swap this whole finger
  between two layouts" would let the search escape local minima
  that single-char swaps can't.
- **Restart.** When the search plateaus, kick temperature back up
  or re-seed from the best-known-so-far.
- **Multi-seed.** Start from several different initial layouts
  in parallel and keep the best.
- **Constraint pinning by region.** Currently pin is per-char.
  "Pin the entire home row," "pin the right-hand vowel cluster,"
  "pin finger assignments but not row," etc.
- **Early stopping.** If the last N iterations produced no
  improvement, stop rather than running to the configured
  iteration count.

**Why:** The current generator produces reasonable output but
clearly isn't finding global optima — we've seen it plateau
around a local best in long runs. Better search would probably
produce better layouts.

**Why this is Tier 2:** "Better" isn't measurable without a
benchmark. The right design probably depends on what pain points
show up when people actually use the generator — right now
that's a sample size of one (Kay on Drifter, and you're not
using the generator for anything serious yet). Would benefit
from real usage before committing to an upgrade shape.

### Rank-correlation analysis

**What:** Score all of oxey's ~100 library layouts in drift,
alongside their oxey scores. Compute rank correlation (Spearman,
or Kendall's tau) between drift's presets and oxey. Answer:
"when drift and oxey disagree, is it a systematic difference or
noise?"

**Why:** We hand-verified this for Drifter vs Gallium — drift
ranks them one way, oxey ranks them the other, and we know why.
Scaling that verification to all ~100 layouts would tell us
whether drift's ordering is *broadly* consistent with oxey
(implying drift's unique analyzers add signal but don't flip
the ranking of well-tuned layouts) or *systematically different*
(implying the philosophy choice materially affects which layouts
you'd recommend).

**Why Tier 2:** Requires `.dof` import first (Tier 1). And the
output is a research artifact, not a feature users consume
directly — its value is in answering the question "is drift
internally consistent with external reality?"

**Shape:** A CLI command `drift rank <glob>` that scores every
matching layout and emits a sorted table with per-preset ranks.
Optional `--correlate <external.csv>` to compute rank-correlation
with an external ranking (oxey's, or hand-supplied).

### Non-monotonic trigram rolls

**What:** Trigram patterns like `middle→index→middle` ("rocking"
on two adjacent fingers) currently score nothing. The bias audit
flagged this; we scoped the bigram version (`same_row_skip`) but
not the trigram version because the definition gets messy.

**Why Tier 2:** The definition is the hard part. Is
`middle→index→middle` a roll? A finger-return? An awkward
twitch? Depends on the specific fingers and the specific
intermediate letter. Whether to reward or penalize it depends
on a kinesthetic judgment that varies by hand. We'd need to
decide whether this is a universal analyzer with direction
weights, or another opinionated `_fingerpair` variant.

**What the resolution might look like:**

Option A — one opinionated analyzer, like
`same_row_skip_fingerpair` but for trigrams. Per-pattern
weights. Explodes the config surface.

Option B — a rule based on finger-column sequence signature.
`same finger pattern[0] == pattern[2]` with adjacent-finger
middle. Single weight. Simple but doesn't distinguish "feels
good" from "feels bad" rocking patterns.

Option C — skip it. The bigram-scoped analyzer captures the
motion; trigram scoring of the same pattern is probably
redundant.

Pick once someone has a real use case.

---

## Tier 3 — directions

### Corpus format expansion

Raw-text ingestion (read a file, count n-grams, emit
`MemoryCorpus`). Would let users score on their own writing
rather than oxey's pre-computed corpora. Moderate plumbing,
high value for personalization.

### Language-specific presets

German, Dutch, French, Spanish presets with language-tuned
analyzer weights and appropriate corpus defaults. Mostly config
work, not code. Value scales with drift's audience.

### Code-corpus support

Programming languages have radically different n-gram
distributions (high `==`, `=>`, `{}`, etc). Scoring a layout
for code-typing would mean ingesting code corpora — maybe
scraped from GitHub or a user's own repos. Probably overlaps
with raw-text ingestion.

### Benchmark suite

We've added 20 analyzers, 4 presets, a full workspace, delta
scoring, SA generation. There's no perf baseline. Would be
useful to know whether drift-generate runs faster or slower
than oxey's generator, and whether any specific analyzer is a
bottleneck. `criterion`-based benches.

### Drift-as-library

Currently drift is a CLI that ships a workspace of crates. The
crates are usable as a library (drift-score and drift-analyzer
expose clean APIs), but there's no documentation pointing at
them as a library use case. A guide for "use drift as a Rust
dep to score layouts from your own code" would open it up for
other tools.

### Web UI / visualization

A rendered heatmap of analyzer contributions per key. Or a
"what-if" UI where you click a key and drag it elsewhere and
see the score delta in real time. Requires drift to export its
results to a format consumable by JS. JSON renderer (Tier 1) is
the prerequisite.

### Plugin analyzers as separate crates

`inventory`-style registration (mentioned in the modularity
audit as an optional Phase 5). Would let third parties ship
analyzer crates that drift discovers at link time. Low urgency;
the current `register_all` pattern works fine at the current
scale.

---

## Completed (archival)

Things drift already is, for reference. If we add new aspirational
specs above, the list below is what they build on.

### Foundation
- 12-crate workspace with compiler-enforced dependency graph
- `drift-core` vocabulary types with `#[non_exhaustive]` enums
- `drift-analyzer` trait, registry, pipeline, `ConfigValue` abstraction
- 20 stock analyzers across unigram/bigram/trigram/skipgram/aggregate scopes
- `drift-score` scope-driven executor
- `drift-delta` incremental scorer with per-char dependency indexing
- `drift-generate` simulated-annealing layout search
- `drift-config` preset system and TOML loader
- `drift-report` text + stub-json renderers
- `drift-cli` with score/compare/generate subcommands
- `drift-keyboard` reader + writer for keywiz JSON5 layouts
- `drift-corpus` oxey JSON loader + blend
- 4 regression tests verifying delta bit-exact on all three non-mimic presets

### Presets
- `neutral.toml` — opinion-free baseline
- `drifter.toml` — flexion, inward-roll, hand-territory biased
- `extension.toml` — mirror of drifter for reaching-preferring philosophies
- `oxey_mimic.toml` — drift with oxey-equivalent weights, useful for
  validation

### Modeling
- Finger sub-column distinction (`FingerColumn::Outer`/`Inner`)
- SFB vs lateral-SFB split (`penalty` vs `lateral_penalty`)
- SFS vs lateral-SFS split
- Asymmetric-forward scissor exemption (fixed from its initial
  unreachable state)
- Partial-alternation trigram reward
- Per-finger-pair non-monotonic roll weights (opt-in)
- Async-hand-drift analyzer — configurable window length (3 or 4+);
  drifter preset runs at n=4 via derived n-gram data
- Flexion/extension cascade analyzers with configurable row sets
- Row-cascade penalty (all-three-rows same-hand)
- Per-finger terminal-of-trigram penalty
- Redirect/bad-redirect with configurable anchor-finger set
- Hand-territory cross-hand row-synchrony scoring

### Renderers & reports
- JSON report renderer — `--format text|json`. Envelope:
  `{layout_name, keyboard_name, corpus_name, total, categories,
  hits}`. Per-category rollup extracted into `drift-report::aggregate`
  so both renderers share the view.
- Layout-diff renderer — `drift compare --diff a b`. Per-key char
  diff sorted by physical position. Text and JSON views.

### Corpus
- N-gram derivation for `Scope::Ngram(n≥4)` — Markov chain rule
  `P(c1..cn) ≈ P(c1..cn-1) × P(c2..cn) / P(c2..cn-1)`. Lives in
  `drift-corpus::derive`. `MemoryCorpus::ensure_ngrams(n)` is the
  explicit, idempotent cache-fill entrypoint; the CLI inspects the
  pipeline's max `Ngram(n)` and derives before scoring.

### Loaders
- `drift-dof` crate — oxeylyzer `.dof` layout reader. Board
  descriptor (`ortho` / `elora` / `ansi`) selects a default keywiz
  keyboard (`keyboards/ortho.json`, `halcyon_elora_v2.json`,
  `us_intl.json`). CLI dispatches by extension. All 189 oxey
  English `.dof` layouts score cleanly.
- `keyboards/ortho.json` — flat 3×10 reference geometry added so
  ortho `.dof` layouts score against their native physical model.

### CLI
- `--set KEY=VALUE` per-analyzer weight overrides (scalar type
  inference: bool → i64 → f64 → string). `--enable NAME` and
  `--disable NAME` toggle analyzer membership without editing the
  preset. Overrides live in `drift-config::overrides` — reusable
  from any entrypoint, not CLI-only.

### Documentation
- `DRIFT_AUDIT.md` — bias audit, all items resolved
- `MODULARITY_AUDIT.md` — modularity refactor status
- `ASSUMPTIONS.md` — documented structural preconditions
- `WRITING_ANALYZERS.md` — analyzer authoring tutorial
- `STATE_OF_DRIFT.md` — pre-refactor handoff (archival)
