# drift

A **flexion-aware keyboard layout scorer and generator** — sibling crate
to keywiz. Built for people who want to score structural properties that
[oxeylyzer](https://github.com/O-X-E-Y/oxeylyzer)'s cost model doesn't
capture (row symmetry, flexion cascades, async hand drift, direction-
consistent rolls), and for whom "more alternation" isn't the answer to
every question.

Status: v0.1 — scores any layout, runs SA generation, imports `.dof`,
computes layout diffs, ships four presets.

## Running

```sh
# Score one layout
drift score layouts/drifter.json

# Compare two layouts
drift compare layouts/drifter.json layouts/gallium-qxjz.json

# Generate a layout with simulated annealing
drift generate --preset drifter

# Use a preset
drift --preset oxey_mimic score layouts/drifter.json

# Or load drift as a subcommand of keywiz (in-process, same binary)
keywiz --drift score layouts/drifter.json
```

Layouts are keywiz JSON5 (`main_k*` id scheme) or DOF
(`.dof` with an embedded board descriptor). Keyboards are keywiz JSON5.
Corpora are oxeylyzer-format JSON (chars + bigrams as percentages).

## What it measures

Drift is analyzer-based: each scoring rule is a drop-in module that
emits hit-contribution pairs, and the pipeline sums them. The stock
set runs 20+ analyzers spanning:

**Bigram** — SFB, SFS, scissor, stretch, roll (direction-weighted,
inward/outward), alternate (partial + full).

**Trigram** — inward_roll, outward_roll, redirect, bad_redirect
(no index anchor), onehand, pinky_terminal.

**Structural** — row_distribution, finger_load (strength-weighted,
quadratic overload penalty), flexion_cascade (reward home+bot
sequences on col-stag), row_cascade (penalize all-three-rows
sequences), hand_territory (cross-hand row-synchrony in
alternation).

**Asymmetric** — on col-stag boards, adjacent-finger cross-row motions
where the outer finger is naturally pre-extended (ring-above-pinky,
middle-above-ring) are exempt from scissor classification. Captures
the fact that "reaching" *into* the resting splay shape isn't a scissor.

The full list is in
[`drift/docs/ASSUMPTIONS.md`](docs/ASSUMPTIONS.md); adding a new one is
four files and a registry entry — see
[`docs/WRITING_ANALYZERS.md`](docs/WRITING_ANALYZERS.md).

## Presets

| Preset      | Bias                                                     |
|-------------|----------------------------------------------------------|
| `neutral`   | Default. Top and bottom rows treated equivalently.       |
| `drifter`   | Flexion-biased. Rewards home↔bottom; penalizes home↔top. |
| `extension` | The inverse. Rewards reaching up.                        |
| `oxey_mimic`| Approximates oxeylyzer's weights for A/B comparison.     |

All live in `drift/crates/drift-config/presets/*.toml`. Copy any file
to start your own preset — keys are documented inline.

Ad-hoc overrides without a config file:

```sh
drift --set sfb.penalty=-5.0 --enable same_row_skip score foo.json
```

## Workspace layout

Thirteen crates with a compiler-enforced dependency graph:

```
drift-core          — types: Keyboard, Layout, Scope, Hit, Finger
  └ drift-motion    — classify a motion: roll / scissor / stretch / alt
      └ drift-analyzer    — trait Analyzer, registry, pipeline
          └ drift-analyzers  — 20+ stock rules
              └ drift-score     — run a pipeline over a layout+corpus
                  └ drift-delta    — delta scoring for SA hot loop
                  └ drift-generate — simulated-annealing generator
                  └ drift-report   — text + JSON output
                      └ drift-cli   — the `drift` binary
drift-corpus        — oxey JSON reader + n-gram derivation
drift-keyboard      — keywiz JSON5 loader
drift-dof           — `.dof` file import + board→keyboard mapping
drift-config        — TOML presets, overrides, pipeline builder
```

Score runs in under 200 ms on a 50k-word corpus; SA generates a layout
in a few minutes on a laptop (delta scoring makes ~10× difference over
full-score-per-swap).

## Output

Text is the default; `--format json` emits a serializable envelope.

```
Layout: drifter
Board:  halcyon_elora
Corpus: shai

Category breakdown:
  alternate     13816 hits   cost   +21.809
  roll             44 hits   cost   +17.290
  redirect       1497 hits   cost    -6.473
  sfb              76 hits   cost    -6.134
  ...

Overall score: +19.216
```

Higher is better. Negative totals happen when penalties exceed rewards —
normal for any imperfect layout. The categories and hit counts are the
real signal; the total is a weighted sum.

## Going deeper

- [`docs/ASSUMPTIONS.md`](docs/ASSUMPTIONS.md) — structural
  preconditions and biomechanical assumptions baked into the scorer.
- [`docs/WRITING_ANALYZERS.md`](docs/WRITING_ANALYZERS.md) — add a
  new analyzer in four files and a registry entry.

## License

AGPL-3.0, inherited from keywiz. See the workspace root.
