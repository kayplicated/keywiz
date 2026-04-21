# drift

Flexion-aware keyboard layout scorer. Built because oxey's cost
model undervalues row-direction asymmetry on col-stag keyboards,
and because we wanted a tool whose weights are transparent and
editable.

Status: v0.1 — scores layouts and compares them. No generation.

## Running

```
cargo run -- score <layout.json>
cargo run -- compare <a.json> <b.json>
```

Layouts are keywiz JSON5 format. Keyboards are keywiz JSON5 format.
Corpora are oxey JSON format (chars + bigrams as percentages).

## Weights

All tunable weights live in `drift.toml` at the crate root. Defaults
are neutral on row-direction: `flexion = extension = 1.0` means
the scorer measures bigram motions without favoring top or bottom.

To bias toward a specific philosophy, tweak `[row]`:

```toml
# Drifter-style: reward flexion, penalize extension.
flexion = 0.5
extension = 1.3

# Or the inverse, if you prefer reaching up:
flexion = 1.3
extension = 0.5
```

## What the scorer measures

- **SFB** — same-finger bigrams. Penalized by `bigram.sfb_penalty`.
- **Scissor** — adjacent-finger cross-row motion. Penalized by
  `bigram.scissor_penalty × [row multiplier]`.
- **Roll** — same-row adjacent-finger motion. Rewarded.
- **Stretch** — non-adjacent same-hand motion. Penalized lightly.
- **Alternate** — different hands. No same-hand cost.

The `[asymmetric]` section enables a biomechanical rule: if the
outer finger of an adjacent-finger pair is naturally pre-extended
forward (pinky-rest-forward pattern on col-stag), the motion is
NOT classified as a scissor. This captures the Elora's aggressive
col-stag geometry where middle-to-ring-to-pinky motions feel
natural even across rows.

## Output

Text by default. Shows:

- Row distribution (top/home/bot %)
- Per-finger load, strength-weighted
- Motion breakdown (alternate/SFB/roll/scissor/stretch)
- Top 10 SFBs, scissors, rolls by contribution
- Total score

Higher score = better. Negative scores are normal when penalties
exceed rewards, which happens for any imperfect layout.
