# State of Drift

*A comprehensive note for future Kay on what drift is, what it does, what
it doesn't do yet, and where it could go.*

---

## What drift is

drift is a **keyboard layout scorer and simulated-annealing generator**
built as a sibling crate inside the keywiz workspace. It exists because
oxey (the community standard layout analyzer) consistently ranked
gallium-family layouts higher than drifter for Kay's specific setup —
col-stag Elora, piano-informed hand biomechanics, flexion-biased
preferences — and we wanted a tool whose cost model matches Kay's
actual hands rather than the community's ortho/row-stag baseline.

### The core bet

Layout scoring traditionally treats a layout as a set of letter→finger
assignments and scores it on same-finger bigram counts, roll counts,
and alternation percentages. drift accepts that foundation and adds:

1. **Row-direction asymmetry** — flexion (curl down) is biomechanically
   cheaper than extension (reach up) on col-stag keyboards. Tunable
   multipliers let the config express how much that matters.
2. **Column-stagger aware scissor detection** — the "outer-finger-
   forward-is-natural" rule exempts cross-row motions when the hand
   is in its resting splay shape (ring forward of pinky, middle
   forward of ring, etc.). Reduces false-positive scissor counts on
   col-stag.
3. **Pluggable trigram rules** — a plugin-style system where each
   trigram scoring rule lives in its own file and can be enabled or
   disabled via config. Currently 10 rules; easy to add more.
4. **Strength-weighted finger overload** — quadratic penalty against
   each finger's "fair share" of load, with weights reflecting
   piano-pedagogy hierarchy (middle ≈ ring > index > pinky) rather
   than the typing community's "index is strongest" dogma.
5. **Delta-scored simulated annealing** — generate runs recompute only
   the n-grams whose contribution changes under a swap, giving SA
   iterations in single-digit milliseconds instead of seconds.

### Why this matters

The scoring model is *transparent and tunable*. Every weight lives in
`drift.toml` with comments explaining what it does. If drift's opinions
don't match your hands' opinions, you can read why, change a weight,
and see the effect immediately. Contrast with oxey where weights live
in Rust constants and the whole thing is a black box unless you want
to read 500 lines of scoring code.

## Current capabilities

### CLI subcommands

```
drift score <layout.json>                    # full report
drift score <layout.json> --json             # machine-readable
drift compare <a.json> <b.json>              # side-by-side
drift generate <seed.json> [-p pins]         # SA-optimize
```

Global flags:
- `--config <path>` — alternate drift.toml
- `--corpus <path>` — repeat for multi-corpus reports
- `--blend "a.json:w,b.json:w"` — weighted blended corpus
- `--keyboard <path>` — alternate keyboard geometry
- `--json` — JSON output instead of text

Generate flags:
- `-p pins` — letters to hold fixed (e.g. `-p nrtsghaei`)
- `-n iterations` — SA step count
- `--temp-start` / `--temp-end` — cooling schedule
- `--seed-rng` — deterministic runs
- `-o output.json` — write best layout to keywiz JSON5

### Env vars

- `DRIFT_CHECK_DELTA=1` — after a generate run, rescore the best
  layout from scratch and report the gap vs the delta-tracked total.
  `gap=0.000000` means delta scoring is exact. Useful if any
  scoring change is suspected of drift (pun intended).

### Trigram rules

All live under `drift/src/trigram/rules/` and register in
`registry.rs`. Each has its own subtable in `drift.toml`. Remove a
name from `[trigram] rules` to disable.

| Rule | What it measures | Default weight |
|---|---|---|
| `inward_roll` | pinky→ring→middle and similar monotonic 3-finger sequences toward the thumb | +3.0 |
| `outward_roll` | index→middle→ring etc., toward the pinky | +2.5 |
| `onehand` | same-hand trigrams that aren't clean rolls or redirects | +1.0 |
| `alternate` | L-R-L or R-L-R | +0.4 |
| `redirect` | direction-flip same-hand trigrams with an index anchor | -3.0 |
| `bad_redirect` | redirect with no index finger | -5.0 |
| `pinky_terminal` | penalty for same-hand trigrams landing on pinky (weak-finger arrest) | -0.5 |
| `flexion_cascade` | reward for same-hand trigrams confined to home+bottom rows with a row change | +1.5 |
| `row_cascade` | penalty for trigrams that hit all three alpha rows (the "key" roller-coaster pattern) | -3.0 |
| `hand_territory` | cross-hand bigram pairs scored by row delta; home-home reward, top-bot penalty | +0.5 / -0.3 / -1.0 |

### Performance

Starting points (5k SA iterations, english corpus, all rules on):
- Baseline: 43s
- + `ScoreMode::FastTotalOnly` (skip detail collection, prune tail):  4s
- + delta scoring (`ScoreAccumulator`): **0.5s**

100k iteration generate runs are now ~12s. Overnight 10M-iter runs
would take ~20 minutes, which is practical.

### Correctness

- `DRIFT_CHECK_DELTA=1` verified across 4 seeds: delta-tracked score
  exactly matches full rescoring (`gap=0.000000`).
- Layout ranking under default weights on english corpus:
  - drifter: ~24
  - gallium: ~11.5
  - qwerty: ~-113
  - matches hand-feel.

## What works and what doesn't (current quirks)

### Working well

- Scoring a single layout and getting a full breakdown (rows, fingers,
  motion categories, top SFBs/scissors/rolls, trigram hits).
- Comparing two layouts side-by-side.
- Generating layouts from a seed with pinned characters.
- Multi-corpus scoring and weighted blending.
- Delta-scored SA that's fast enough for serious exploration.
- All trigram rules additive; a layout can earn bonuses and penalties
  from multiple rules on the same trigram.

### Known quirks

- **No tests.** Zero automated coverage. If you refactor the motion
  classifier or trigram context, nothing will catch regressions. This
  is the biggest current gap.
- **Finger-overload math duplicated** between `score.rs` and
  `delta.rs`. They must stay in sync; if you change one, change both
  or bugs compound silently.
- **`_keyboard` param in `generate::generate` is unused.** Kept in
  the signature because removing it would cascade through `cli.rs`.
  Cosmetic.
- **The `asymmetric_forward_exempt` rule fires unconditionally** when
  any of the three finger-pair booleans in `[asymmetric]` are true.
  There's no per-motion "this is too extreme" override; the rule
  trusts geometry entirely.
- **Trigram details are truncated to top 30.** Fine for display but
  means JSON output also caps at 30. If you want full trigram dumps,
  bump the constant in `score.rs`.
- **Generate always starts from an explicit seed layout.** No random
  seed. If you want "generate from scratch," pass a layout with
  random-enough positions and let SA find its way. Not the same as
  a fresh random start.
- **The scorer can produce negative scores** (qwerty at -113).
  That's fine for relative comparison but surprising if you expect
  0..1 like oxey's score.

## What's missing / next-session candidates

### Near-term, cheap

1. **Tests.** Snapshot tests for `classify` (bigram motion), for each
   trigram rule (known trigrams → known classifications), for the
   delta-scoring invariant (init + random swaps = fresh rescore).
   Could be written in a few hours.

2. **`SpecificTrigrams` rule.** A lookup-table rule that reads a list
   of trigram-weight pairs from config. Lets you say "reward `you` by
   an extra +2 and `key` by an extra -3" without writing code.

3. **`roll_richness` rule.** Captures Kay's "drifting" observation:
   measure how many distinct roll pairings exist within the set of
   common letters on each hand. Layouts where a hand's common letters
   form a dense roll graph score higher. Novel — community doesn't
   have a name for this.

4. **Dedup finger-weight lookup.** Move the `finger_weight` function
   into one place (probably `config.rs`) and use it from both `score`
   and `delta`. Cheap refactor; eliminates the duplication bug risk.

5. **Heatmap output.** ASCII or SVG visualization of per-key finger
   load or per-key cost contribution. Would make "where's the
   bottleneck" obvious at a glance.

6. **Named presets in `drift.toml`.** Preset profiles like
   `[presets.flex]` that swap in flexion-biased multipliers, or
   `[presets.neutral]` for the community-aligned weights. CLI flag
   `--preset flex` activates the block.

### Medium

7. **Bigram rule pipeline.** Currently bigrams are hardcoded in
   `motion.rs` + `score.rs`. A pluggable bigram-rule system symmetric
   to the trigram one would let users add bigram-level opinions
   without recompiling. Lets `hand_territory` move from being a
   trigram rule (which is a slight conceptual stretch) to a native
   bigram rule.

8. **Delta-scoring the CLI's `score` and `compare` paths.** These
   currently run `ScoreMode::Full` and recompute everything. Not a
   bottleneck in practice (single-layout scoring is ~1ms) but would
   be cleaner if the accumulator served both paths.

9. **Live keywiz integration.** keywiz drills layouts and could
   display drift's scoring metrics live in the corner — "your top-
   row usage so far: 4.2%, your SFB rate: 0.9%". Would require
   exposing drift as a library, not just a binary.

10. **Configurable keyboard geometry.** drift currently looks up
    keyboards via path; if you wanted to score against a row-stag
    board without a separate keyboard JSON5 file, there's no fallback.
    A `keyboards/` folder under drift with generic ortho/row-stag/
    col-stag profiles would help.

11. **Multi-seed SA with best-of-N reporting.** Run 10 SA runs with
    different RNG seeds, report the best overall, show variance. SA
    is stochastic and a single run can land at a local optimum that
    a different seed would have escaped.

12. **Delta-scoring for trigrams is still "compute before and after
    of affected trigrams."** Could be further optimized by caching
    per-trigram contributions and updating only the ones that changed.
    Another 2-3x speedup probably possible, not critical.

### Ambitious

13. **A `drift_lib` crate.** Split drift into a library + thin CLI
    binary. keywiz could depend on `drift_lib` directly for live
    metric display. Other tools could use drift's scoring too.

14. **Trigram rule auto-discovery.** Right now adding a new rule
    requires editing `registry.rs`. A proc-macro or `inventory`-
    crate based auto-registration would remove that step.

15. **Per-rule unit-testing framework.** Each trigram rule could
    declare a list of `(trigram, expected_hit)` pairs that CI runs.
    When a rule misbehaves, the test shows which trigrams changed.

16. **Corpus builder.** A subcommand that takes a directory of text
    files and produces an oxey-compatible JSON corpus. Right now the
    corpus has to come from oxey's tooling. Would let you build
    domain-specific corpora (code, prose, chat) trivially.

17. **Cross-layout diff view.** `drift diff a.json b.json` shows
    which letters moved, which bigrams are better/worse on each,
    and why. Currently `compare` shows totals; a diff would show
    the per-bigram deltas that contribute to the score gap.

18. **Layout family detection.** Given a .dof or .json5 layout, drift
    could classify it as "gallium-family" / "graphite-family" /
    "hands-down-family" by matching home-row fingerprints. Useful
    when looking at random layouts.

19. **Peak-burst vs sustained-flow scoring mode.** Kay's hypothesis
    that gallium might have a peak-speed edge while drifter has
    sustained-flow advantage could be tested by a second cost model
    that weights short-window patterns differently from long-window
    patterns. Speculative but interesting.

20. **Integration with keywiz drill data.** keywiz tracks per-key
    accuracy and timing. If drift could read keywiz's stats files,
    it could *measure* per-finger strength from actual drilling
    data instead of using piano-pedagogy defaults. Your hands would
    directly calibrate the scorer.

### Research / exploratory

21. **Cross-hand row-correlation metric.** Build a proper statistical
    measure of "when one hand is at row R1, what's the distribution
    of the other hand's row during the same typing window?" Would
    formalize the hand-territory observation beyond the current
    trigram-window approximation.

22. **Bigram skipgram scoring.** Oxey measures skipgrams (letters
    separated by one char, typed with the same finger — persistent
    strain). drift doesn't. Adding it is probably a win.

23. **Finger-path smoothness.** For a sequence of keystrokes, compute
    the "path" each finger takes through space and penalize zigzag
    paths that would require active deceleration. Captures a
    biomechanical axis nobody models.

24. **Ngram-context-aware weights.** Current weights are per-rule.
    Could extend to "this rule fires stronger in words where X is
    true." Probably overengineering but worth noting.

25. **Machine-learning-calibrated weights.** Given a corpus of
    (layout, subjective-rating) pairs, regress drift's weights
    against user ratings. Would convert drift from a prescriptive
    model to a descriptive one. Ethically interesting (whose ratings
    matter?) and practically ambitious.

## Architectural notes for future-you

### File layout

```
drift/
  Cargo.toml             — workspace member
  drift.toml             — default config; tunable weights
  README.md              — short usage blurb
  docs/
    STATE_OF_DRIFT.md    — this file
  src/
    main.rs              — `mod` declarations + entry
    cli.rs               — clap parsing + subcommand dispatch
    config.rs            — drift.toml loader; keeps raw Value for
                           trigram subtables
    corpus.rs            — oxey JSON parser; supports blending
    keyboard.rs          — keywiz JSON5 keyboard loader; Finger enum
    layout.rs            — keywiz JSON5 layout loader; alpha-core
                           filter via `is_alpha_core_id`
    motion.rs            — bigram Motion classification; scissor
                           rules including index-middle forward
                           asymmetric rule
    score.rs             — full-layout scoring; ScoreMode for
                           Full vs FastTotalOnly
    report.rs            — human-readable text output; owo-colors
    delta.rs             — ScoreAccumulator for SA hot loop;
                           per-char ngram indexes
    generate.rs          — SA driver; cooling schedule; RNG
    trigram/
      mod.rs             — re-exports
      config_util.rs     — shared read_f64 helper
      context.rs         — per-trigram geometry helpers
      registry.rs        — build_pipeline + construct_rule
      rule.rs            — TrigramRule trait + RuleHit
      rules/
        mod.rs           — submodule list
        roll.rs          — inward_roll, outward_roll
        onehand.rs
        alternate.rs
        redirect.rs      — redirect, bad_redirect
        pinky_terminal.rs
        flexion_cascade.rs
        row_cascade.rs
        hand_territory.rs
```

### Adding a new trigram rule

1. Drop a new file under `trigram/rules/`. Follow the shape of
   `alternate.rs` as the minimal template.
2. Add `pub mod <name>;` to `trigram/rules/mod.rs`.
3. Add a match arm to `trigram/registry.rs::construct_rule`.
4. Add a `[trigram.<name>]` subtable to `drift.toml` with default
   values. Add the name to `[trigram] rules`.
5. Score a known layout before/after to sanity-check.

### Adding a new CLI subcommand

1. Add a variant to the `Command` enum in `cli.rs`.
2. Handle it in the `match cli.cmd` block in `run()`.
3. Factor the emit logic into a helper that takes `cli.json` to
   handle the JSON-vs-text split uniformly.

### Changing bigram classification

`motion.rs::classify` is the one place that decides what a bigram's
motion type is. If you change it, delta-scoring will still match full
scoring because they both call into the same function via
`bigram_contribution`. Verify by running `DRIFT_CHECK_DELTA=1`.

### If you change weights

All weights live in `drift.toml`. Changing them requires no recompile
— drift re-reads the file on every invocation. This is deliberate:
it should be cheap to experiment.

### If you change the `Config` struct

Both `score.rs` and `delta.rs` read config fields. The `finger_weight`
helper exists in both and must stay in sync. Refactoring this
duplication into a shared location is task #4 above.

## Calibration notes

### Default weights are biased toward Kay's preferences

`drift.toml` ships with:
- `[row]` at 1.0 / 1.0 / 1.2 (near-neutral, slight full-cross penalty)
- `[finger]` piano-pedagogy (middle/ring > index > pinky)
- `[trigram]` all 10 rules enabled

This means drift out-of-the-box favors drifter over gallium. That's
*intentional* for Kay's setup but not universal. If someone else
picks up drift, they should zero out the drifter-specific rules
(`flexion_cascade`, `row_cascade`, `pinky_terminal`, `hand_territory`)
and re-run to get oxey-comparable scores.

### What validates drift's model

Three checks have been run during development:
1. **qwerty scores catastrophically** (~-113). A scorer that didn't
   penalize qwerty would be broken. drift does.
2. **gallium scores lower than drifter** (11.5 vs 24). Matches Kay's
   hand-feel; diverges from oxey's ranking, which is the point.
3. **SA generation from drifter-v7 under drift's weights finds
   minimal improvement.** Means drift considers drifter-v7 near-
   optimal under its own model — consistent, not circular because
   the weights weren't tuned after drifter was built.

### What hasn't been validated

- Whether drift generalizes to other hands / corpora / keyboards.
  All testing has been Kay's english+kay corpora on Elora.
- Whether drift's scoring predicts typing speed. drift predicts
  *subjective flow* (or tries to); the relationship to measurable
  WPM is untested.
- Whether multi-day drilling on a drift-generated-from-scratch
  layout produces the same "drift feel" as drifter. SA output
  scores well under drift's model but hasn't been drilled.

## Where drift fits in the ecosystem

- **oxey** is the community standard. It has: optimizer, ranker,
  well-tested scoring, many corpora, many layouts. It lacks:
  flexion-aware scoring, col-stag geometry awareness, pluggable
  rules. Use oxey when you want consensus-aligned answers.
- **drift** is Kay's opinionated cousin. Use drift when oxey's
  answers don't match your hands, or when you want transparent,
  tunable scoring for col-stag / flexion-biased design.
- **keywiz** is the drill platform. Use keywiz to turn a scored
  layout into trained muscle memory.

Together: **drift to design, keywiz to drill, hands to validate.**

## Closing note

drift started as "let's build a scorer that doesn't disagree with
Kay's hands." It turned into a real piece of layout-analysis
infrastructure. The architecture (pluggable rules, transparent
weights, delta scoring) is solid enough that extending it is cheap;
the scoring model is opinionated but legible; the performance is
good enough that experimentation is practical.

The *thing drift proves* is that a layout scorer can be honest
about what its weights encode. Oxey's weights are implicit and the
author's choices are invisible in the output. drift's weights are
all in `drift.toml` and every rule is a single file with a docstring
explaining what it measures.

If future sessions do nothing else, **don't lose that property.**
Adding more rules is fine. Adding black-box scoring components is
not. Every axis of drift's cost model should be readable by a user
who wants to understand why drift ranks layout A above layout B.

Sleep well. The layout is good. drifter works. The hands drift.
