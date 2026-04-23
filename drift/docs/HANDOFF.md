# Drift — session handoff

Current as of 2026-04-22, end of the Tier 1 completion pass.
This doc tells a new session (fresh Claude instance, future you,
whoever) what's stable, what's in flight, and what to read first.

Supersedes `STATE_OF_DRIFT.md` — that one is a pre-refactor
snapshot kept for archival reasons. If you're picking up work
now, read this one.

---

## Read order

1. **This file** — orient yourself.
2. **`ROADMAP.md`** — what drift could grow into. Tiered by
   certainty, with scope estimates on concrete items.
3. **Audit docs as needed:**
   - `DRIFT_AUDIT.md` — bias audit (all items ✓).
   - `MODULARITY_AUDIT.md` — structural refactor status.
   - `ASSUMPTIONS.md` — structural preconditions that aren't
     configurable.
4. **`WRITING_ANALYZERS.md`** — if you're adding a new analyzer.
5. **Tier 1 items in `ROADMAP.md`** — if you want to just pick up
   work, these are the ready-to-build ones.

Don't start by reading the code. The docs index it.

---

## What drift is, in one paragraph

Drift is a keyboard layout scorer and SA-based generator, built
as a multi-crate workspace inside the keywiz repo. It's an
alternative to oxeylyzer for people who want to score structural
properties (row symmetry, flexion cascades, async hand drift,
direction-consistent rolls) that oxey's model doesn't capture.
The architecture is an analyzer-trait-plus-registry pipeline:
every scoring rule is a drop-in module, weights live in TOML
presets, and users pick or write a preset that matches their
philosophy. Four presets ship: neutral (philosophy-free),
drifter (flexion/hand-sync biased), extension (mirror), and
oxey_mimic (tuned to approximate oxey's ranking for
cross-validation).

---

## State of the world

### Stable

Everything below has been through multiple sessions, has tests
or equivalent validation, and isn't actively being changed.

- **Workspace structure.** 13 crates under `drift/crates/`.
  Compiler-enforced dependency graph. No cycles.
- **Analyzer trait.** `drift_analyzer::Analyzer` with
  `Scope::{Unigram, Bigram, Trigram, Ngram(n), Skipgram(gap),
  Aggregate}`. Registry-based pipeline built from config.
- **20 stock analyzers.** All in `drift-analyzers`, all
  registered via `register_all`. See `ROADMAP.md` "Completed"
  for the full list.
- **Delta scoring.** `drift-delta::ScoreAccumulator` with
  per-char dependency indexing. Regression test in
  `drift-delta/tests/delta_matches_full.rs` verifies bit-exact
  agreement with full rescoring on all three non-mimic presets,
  including the drifter preset's `Scope::Ngram(4)` analyzer.
- **Four presets.** `neutral.toml`, `drifter.toml`,
  `extension.toml`, `oxey_mimic.toml` under
  `drift-config/presets/`.
- **CLI.** `drift score`, `drift compare`, `drift generate` all
  work. `--preset` selects one of the four; `--corpus` accepts
  single or multiple (blended) paths; `--output` on `generate`
  writes keywiz JSON5. `--format text|json` picks the renderer;
  `--set KEY=VALUE` / `--enable NAME` / `--disable NAME` apply
  per-run config overrides; `compare --diff` adds a per-key
  diff; `.dof` layout paths dispatch through `drift-dof` with
  the default keyboard auto-picked from the `.dof` board field.
- **Reproducibility.** Seeded SA runs produce identical output
  across invocations (we had to add sort-by-char to every
  iteration path to enforce this; mentioned here because it'd
  be tempting to remove as "optimization" and that'd break
  determinism).
- **Asymmetric-forward exemption.** Works correctly (fixed a
  match-arm bug in an earlier session — arms had been
  unreachable since the refactor).
- **SFB / SFS vertical-vs-lateral split.** Drift distinguishes
  vertical same-column SFBs from lateral index-column
  crossings. Weights differ per preset; mimicking oxey requires
  setting `lateral_penalty = penalty`.
- **N-gram derivation.** `drift-corpus::derive` produces n≥4
  tables via the Markov chain rule
  `P(c1..cn) ≈ P(c1..cn-1) × P(c2..cn) / P(c2..cn-1)`.
  `MemoryCorpus::ensure_ngrams(n)` is the explicit cache-fill
  entrypoint — the caller decides what compute to pay for. The
  CLI inspects `pipeline.scopes()` and derives up to the max
  `Ngram(n)` before scoring.
- **`.dof` import.** `drift-dof` reads the oxeylyzer format.
  All 189 oxey English `.dof` layouts score cleanly. Board
  descriptor → default keywiz keyboard mapping lives in
  `drift-dof::board`. `keyboards/ortho.json` ships as the
  flat-grid reference geometry for ortho layouts.

### Drifter layout (the motivating case)

The Drifter layout at `layouts/drifter.json` has an extensive
header comment documenting the current shape, the five design
theses, per-key rationales, and a current-scoring snapshot. If
you're asked about Drifter, read the file — it's self-describing.

Current state:

```
q v m j [    ] - = x z
n r t s g    p h e a i
b l d c w    k f u o y

right thumb cluster (k3..k6):  ; ' , .
```

Scores (end-of-session, drifter preset now runs
`async_hand_drift` at n=4 with weight `-2.6`):
- drift drifter preset:  Drifter +29.04, Gallium +14.13
  (gap +14.91, unchanged from the n=3 baseline's +14.92)
- drift neutral preset:  Drifter +19.22, Gallium +12.40
- drift extension preset: Drifter +17.40, Gallium +9.81
- oxey english:          Drifter 0.407, Gallium 0.465

### Recent modeling additions

Added this session; worth calling out because they might
surprise someone reading older docs.

- **`FingerColumn::{Outer, Inner}`** — index-finger sub-column.
  Non-index fingers always `Outer`. Analyzers that compare
  `same_finger_column` distinguish vertical SFBs from lateral
  column crossings.
- **`same_row_skip_fingerpair`** — per-pair-weight table for
  non-adjacent same-row bigrams. Encodes kinesthetic
  preferences like "ring→index-inner feels good, pinky→ring
  feels slightly awkward." Drifter preset populates these;
  others leave zero.
- **`async_hand_drift`** — analyzer for opposite-direction hand
  movement. Configurable window length (default 3; drifter
  preset runs at 4 via derived n-gram data). The n=4 window
  count is ~26× the n=3 count, so weights tune to smaller
  per-window magnitude; drifter uses `weight = -2.6` at n=4.
- **Partial alternation** — `alternate` analyzer now has a
  `partial_weight` field for L-L-R / R-L-L / L-R-R / R-R-L
  trigrams. Default 0.15 (strict reward is 0.4).
- **`Skipgram(gap)` scope** — drift reads oxey's skipgrams1/2/3
  tables and dispatches skipgram-scope analyzers. Current
  skipgram analyzer is `sfs` (same-finger skipgram).

### In flight

Nothing is actively in flight. End-of-session state is stable.
Every tracked open item is closed or explicitly deferred to
`ROADMAP.md`.

**Tier 1 is complete.** All five Tier 1 items (JSON renderer,
layout-diff renderer, per-analyzer CLI overrides, `.dof` import,
n-gram derivation) landed this session.

Notable: no uncommitted edits to the codebase that would break
a fresh `cargo build --workspace`. Full workspace compiles clean,
**29 unit tests + 4 delta regression tests pass** (drift-config
6, drift-corpus 9, drift-delta 4, drift-dof 10), zero clippy
warnings across all 13 drift crates.

### Known tension points

Things where drift and oxey (or intuition) disagree and where
that disagreement is *intended*, not a bug:

- **Gallium scoring near zero on drifter preset, positive on
  neutral.** Correct by design — the drifter preset penalizes
  row-territory asymmetry (via hand_territory and
  async_hand_drift), which Gallium has structurally. Not
  evidence of a bug; evidence that the preset works.
- **Drifter scoring lower than Gallium on oxey.** Correct by
  design — oxey doesn't measure hand-territory symmetry or
  direction-consistent rolls. Drifter trades SFB/scissor
  minimization for those properties. 0.058 gap is "about what
  you'd expect for giving up the top row to brackets" territory.
- **`ph` and `sg` as SFBs but at lower weight.** Lateral same-
  finger motions on index. Oxey full-penalizes; drift correctly
  discounts. If you see a diff-analysis reporting "drift thinks
  these are cheap," that's the intended model.

### Known holes

Things we know are missing but chose not to build yet. Each
links to a Tier in `ROADMAP.md`:

- Generator is plain single-char SA (Tier 2).
- No rank-correlation analysis tool (Tier 2, `.dof` prereq
  now satisfied).
- No trigram-level non-monotonic rolls (Tier 2, definition
  question).
- No raw-text corpus ingestion (Tier 3).
- No keywiz-side integration surface yet — drift is used as a
  standalone CLI. A `keywiz --drift` entrypoint (or equivalent
  in-process API) is the next natural consumer now that Tier 1
  is done; see `../../docs/` in the keywiz tree for that
  discussion.

---

## Layout of the repo

```
drift/
├── Cargo.toml is absent — drift/ is no longer a package root.
│   The workspace root is keywiz/Cargo.toml, which lists all
│   drift-* crates plus keywiz itself.
├── crates/
│   ├── drift-core/         vocabulary types (Finger, Key, Row,
│   │                       Layout, Keyboard, Hit, Scope, Window)
│   ├── drift-analyzer/     Analyzer trait + Registry + Pipeline
│   ├── drift-motion/       geometric primitives (rolls, scissors,
│   │                       asymmetric-forward exemption)
│   ├── drift-corpus/       oxey JSON loader + blend + n-gram
│   │                       derivation for n≥4
│   ├── drift-keyboard/     keywiz JSON5 loader for keyboard +
│   │                       layout, plus layout writer
│   ├── drift-dof/          oxeylyzer .dof layout reader
│   ├── drift-analyzers/    20 stock analyzers + shared util modules
│   ├── drift-score/        pipeline executor
│   ├── drift-delta/        incremental scoring for SA
│   ├── drift-generate/     simulated-annealing generator
│   ├── drift-config/       TOML loader + preset management +
│   │                       CLI-overrides layer
│   ├── drift-report/       text + json renderers + layout diff
│   └── drift-cli/          the binary (score/compare/generate)
├── docs/
│   ├── DRIFT_AUDIT.md      bias audit — all items resolved
│   ├── MODULARITY_AUDIT.md structural refactor status
│   ├── ASSUMPTIONS.md      documented preconditions
│   ├── WRITING_ANALYZERS.md analyzer authoring guide
│   ├── ROADMAP.md          aspirational specs (tier-organized)
│   ├── HANDOFF.md          this file
│   └── STATE_OF_DRIFT.md   pre-refactor archival snapshot
├── (old pre-refactor src/ and Cargo.toml preserved as *.old;
│    can be deleted any time, kept for reference during migration)
└── drift.toml              (also superseded by preset files)
```

`src.old/` and `Cargo.toml.old` are safe to delete but not yet
done — kept for reference until we're confident we don't need
to check how something worked before.

---

## Common commands

From the repo root (`keywiz/`):

```bash
# Score a layout on a preset
cargo run --release -p drift-cli -- --preset drifter \
  score layouts/drifter.json

# Compare two layouts
cargo run --release -p drift-cli -- --preset drifter \
  compare layouts/drifter.json layouts/gallium-v2.json

# Generate from a seed layout
cargo run --release -p drift-cli -- --preset drifter \
  generate layouts/drifter.json \
  --iterations 200000 --rng-seed 42 \
  --output /tmp/generated.json

# Override corpus (single or blended)
cargo run --release -p drift-cli -- --preset drifter \
  --corpus oxeylyzer/static/language_data/english.json:2 \
  --corpus oxeylyzer/static/language_data/german.json:1 \
  score layouts/drifter.json

# Run delta regression tests
cargo test -p drift-delta

# Lint everything
cargo clippy --workspace --no-deps
```

---

## Tips for productive sessions

- **Scoring is deterministic** across runs with a given preset
  and seed. If you see non-determinism, that's a regression of
  the sort-by-char fix — investigate before assuming it's
  correct.
- **Delta tests are the single most important regression check.**
  If `cargo test -p drift-delta` fails, the scoring pipeline
  has diverged from the delta pipeline; fix before anything else.
- **Adding an analyzer is cheap.** Drop a file, register it, add
  to presets. Follow `WRITING_ANALYZERS.md`. The hard part isn't
  the code — it's deciding whether the rule is universal or
  philosophy-specific.
- **Don't delete `src.old/` unnecessarily.** If you're unsure
  how an old analyzer worked, the pre-refactor source is still
  there.
- **Oxey sits at `keywiz/oxeylyzer/`.** Running oxey commands
  happens from that directory (it loads corpora relative to
  itself). Layouts need to be added to
  `oxeylyzer/static/layouts/english/*.dof` to be visible to its
  `rank`/`analyze`/`compare` commands.
- **When in doubt about a design choice, check `DRIFT_AUDIT.md`
  for past reasoning.** Many "why is this configurable / why
  isn't this configurable" questions have already been answered
  there.

---

## What the next session might reasonably pick up

Tier 1 is done. The natural next moves:

- **Keywiz `--drift` integration.** Now that drift has a stable
  CLI, JSON output, and `.dof` import, wiring it into keywiz
  opens up in-tutor layout scoring / comparison. Discussed but
  not designed; the intended framing is that drift and keywiz
  stay separate tools (drift doesn't become a keywiz plugin,
  and keywiz doesn't mandate the drift format). Likely shape:
  keywiz invokes drift with a layout path it already knows, and
  shells out or calls the library. Needs design before code.
- **Rank-correlation tool** (ROADMAP Tier 2). Now unblocked by
  `.dof` import. Score all 189 oxey layouts against each drift
  preset, compare rank to oxey's. Tells us whether drift's
  unique analyzers add signal or just reshuffle the ranking.
- **Smarter generator** (ROADMAP Tier 2). 3-char rotations,
  restart, multi-seed, early stopping. Needs benchmarking
  thought before picking a shape.
- **Drifter tuning.** Still scope for per-key rationalization
  if actual typing surfaces pain points. Drift-enabled work,
  not drift work.

Or: nothing. End-of-session is a complete state. Drift is a
tool that works; Drifter is a layout that works; Tier 1 shipped.
