# Drift bias audit

**Goal of this document:** identify every place where drift encodes an
ergonomic or philosophical claim as code rather than as configurable
policy, so they can be surfaced, documented, or eliminated.

The guiding principle is that drift should express **only configured
bias**. A user should be able to say "I prefer top row and rolls" or "I
prefer bottom row and heavy alternation" and have drift score accordingly,
without drift silently applying a different philosophy on top.

**Status as of audit date:** drift is roughly 60% of the way to that
goal. The `[row]` section is genuinely neutral by default. Everything
else ships with Drifter-favorable defaults, and several biases are not
configurable at all — they're baked into rule definitions or structural
choices in code.

**Update (post-refactor):** the modularity refactor resolved the
structural half of this audit. Analyzers are now pluggable modules;
opinion-bearing claims are configuration, not code. The bias-audit
work tracked below has partially landed as analyzer/preset changes.
Status markers inline:

- ✓ **done** — the knob exists and a neutral default is shipped.
- ⟳ **deferred** — the structural refactor exposed the knob but
  nothing consumes it yet, or the opinion was moved from code to
  config but the preset defaults still lean Drifter-favorable.
- ✗ **open** — not yet addressed.

---

## Category 1 — Hardcoded biases not exposed to config

These are items where a philosophy is present in code with no config
knob. Each needs either a knob or documentation that it's a fixed
modeling choice.

### 1.1 Roll rules reward only monotonic finger sequences

**Where:** `src/trigram/context.rs` (`is_roll3`, `is_roll3_skip`),
consumed by `src/trigram/rules/roll.rs`.

**What's hardcoded:** the *definition* of "roll." Only strictly
monotonic finger stepping (pinky→ring→middle, index→middle→ring, and
single-skip variants) qualifies. Non-monotonic patterns get nothing
from this rule.

**Why it's a bias:** "roll = monotonic finger stepping" is one school
of thought. Another definition (e.g. "roll = any comfortable same-row
adjacent-finger sweep, even with returns") is not expressible.

**Remediation options:** keep the definition but name it `monotonic_roll`
in docs; *or* add a second rule (`flexible_roll`) that uses a looser
definition and let users pick.

### 1.2 `pinky_terminal.rs` singles out the pinky, and only the pinky

**Where:** `src/trigram/rules/pinky_terminal.rs`.

**What's hardcoded:** the claim "pinky is the unstable finger to land
on." There is no `ring_terminal`, `index_terminal`, or `middle_terminal`
rule. Compare the finger-weight table, which *is* per-finger configurable.

**Why it's a bias:** users with a weak ring, strong pinky, or no
terminal-instability preference at all have no way to express that
without editing code.

**Remediation:** generalize to a single `terminal_penalty` rule with a
per-finger penalty table, default all-zero except perhaps pinky:

```toml
[trigram.terminal_penalty]
l_pinky = -0.5
l_ring = 0.0
l_middle = 0.0
l_index = 0.0
r_index = 0.0
r_middle = 0.0
r_ring = 0.0
r_pinky = -0.5
```

Delete the pinky-specific rule file.

### 1.3 `flexion_cascade` is flexion-only by construction

**Where:** `src/trigram/rules/flexion_cascade.rs`, condition at lines
42–46 (`if ctx.row(i) == -1 { return None; }`).

**What's hardcoded:** top-row participation disqualifies the reward. The
rule cannot reward a same-hand home+top cascade even if someone wants it
to.

**Why it's a bias:** flexion-preferring layouts can earn this reward;
extension-preferring layouts cannot earn any reward for analogous
home+top sequences. There is no `extension_cascade` mirror rule.

**Remediation:** either parameterize the rule to accept a row-set
(`{home, bot}` by default, user can change to `{home, top}` or other),
*or* add a mirror `extension_cascade` rule so the two philosophies are
symmetrically expressible.

### 1.4 Redirect rule hardcodes index as "the anchor finger"

**Where:** `src/trigram/rules/redirect.rs`, `has_index()`.

**What's hardcoded:** the claim that an index finger in a redirect
trigram provides a stable pivot; redirects without index are worse.

**Why it's a bias:** biomechanically reasonable but not universal. Users
with strong middles or specific hand shapes might anchor on a different
finger. Not expressible.

**Remediation:** replace `has_index` with a configurable anchor-finger
set:

```toml
[trigram.redirect]
weight = -3.0
anchor_fingers = ["l_index", "r_index"]

[trigram.bad_redirect]
weight = -5.0    # applies when redirect has no anchor_fingers
```

### 1.5 Alternate rule defines alternation as strict L-R-L / R-L-R only

**Where:** `src/trigram/rules/alternate.rs`, `ctx.is_alternating()`.

**What's hardcoded:** a trigram must alternate on *both* adjacent
pairs to count. Partial-alternation trigrams (L-L-R, R-L-L etc.) fall
through and receive no alternate reward — they may pick up other rules'
rewards, but nothing acknowledges their partial-alternation character.

**Why it's a bias:** some alternation-preferring philosophies would want
partial-alternation trigrams to earn partial credit.

**Remediation:** add a `partial_alternate_weight` knob (default 0), or
add a separate `partial_alternate` rule.

### 1.6 Asymmetric-forward exemption has hardcoded direction and threshold

**Where:** `src/motion.rs`, `asymmetric_forward_exempt()`.

**What's hardcoded:**
- The direction of the asymmetry: outer finger forward of inner = "natural
  rest." A user with a flat-splay or inverted-splay resting shape can't
  invert this claim.
- The threshold: any y-delta > 0 qualifies. No minimum — a 0.01-unit
  forward offset exempts the motion the same as a 1.0-unit offset.

**Why it's a bias:** col-stag keyboards vary significantly in how
aggressive their stagger is. A shallow stagger shouldn't exempt scissors
as readily as a steep one.

**Remediation:**

```toml
[asymmetric]
index_middle_forward_ok = true
middle_ring_forward_ok = true
ring_pinky_forward_ok = true
forward_threshold = 0.0    # minimum y-delta to count as "forward"
direction = "outer_forward" # or "inner_forward" to invert
```

---

## Category 2 — Hardcoded structural assumptions

Defensible modeling choices that nonetheless limit who can use drift.
Document at minimum; consider making configurable.

### 2.1 Four-finger-per-hand model

**Where:** `finger_column` in `src/trigram/context.rs`, `outer_inner` and
`col_weight` in `src/motion.rs`, `Finger` enum in `src/keyboard.rs`.

All roll and redirect logic assumes exactly 4 alpha fingers per hand
(pinky=0..index=3). A layout that uses thumbs as alpha keys, or a
reduced 3-finger layout, cannot be modeled correctly.

**Status:** document as a precondition. Probably fine for now; keywiz
itself shares this assumption.

### 2.2 Three alpha rows, indexed `-1/0/1`

**Where:** row-related logic throughout `score.rs`, `motion.rs`, and
trigram rules.

Score and rules assume exactly three alpha rows indexed `top=-1`,
`home=0`, `bot=1`. A 4-row alpha layout would not be scored correctly on
row-dependent rules.

**Status:** document as a precondition.

### 2.3 `cross_row_kind()` lumps any home↔bot as flexion

**Where:** `src/motion.rs`, `cross_row_kind()`.

**What's hardcoded:** mapping `(home, bot) → Flexion` and
`(top, home) → Extension`. This assumes a keyboard orientation where
"down" = curling = flexion.

**Why it's a limit:** on steeply tented or heavily-tilted custom
boards the biomechanical mapping can invert. Rare case, but worth
documenting as an assumption.

**Status:** document as assumption; keep the default mapping.

### 2.4 `MIN_BIGRAM_FREQ` and `MIN_TRIGRAM_FREQ` hardcoded

**Where:** `src/score.rs` lines 22 and 27.

**What's hardcoded:** frequency thresholds below which n-grams are
pruned in `FastTotalOnly` mode. Currently 0.001% for bigrams and 0.01%
for trigrams.

**Why it matters:** these thresholds affect scores and someone tuning
drift for a low-frequency-sensitive application can't adjust them
without recompiling.

**Remediation:** move to config:

```toml
[score]
min_bigram_freq = 0.001
min_trigram_freq = 0.01
```

### 2.5 Sign conventions are mixed

**Where:** `drift.toml`, `src/score.rs`.

**What's inconsistent:**
- `sfb_penalty = -7.0` stored negative; applied as-is (penalty).
- `scissor_penalty = -2.0` stored negative; multiplied by row
  multipliers then applied.
- `same_row_adjacent = 2.0` stored positive; applied as reward.
- `overload_penalty = -0.05` stored negative; multiplied by squared
  load (positive) to produce a negative contribution.
- Trigram `weight` fields: negative for penalties (redirect, row_cascade,
  pinky_terminal), positive for rewards (rolls, onehand, flexion_cascade,
  hand_territory, alternate).

So the convention is "sign of the stored value = sign of the
contribution," which is consistent across all eight bigram/trigram
weights once you trace it through. That's fine, but it's not
documented, and one footgun remains: `overload_penalty`. The code
treats this as `(squared load) * overload_penalty`. If a user tunes it
to a positive number expecting "more penalty," they'll get a reward
instead.

**Remediation:** document the convention at the top of `drift.toml`:
"Rewards are positive, penalties are negative. This applies uniformly
to all `*_penalty`, `*_weight`, and `*_reward` fields. Sign mistakes
will flip the meaning."

---

## Category 3 — Defaults that embed a philosophy without labeling it

The `drift.toml` header says "deliberately NEUTRAL on row-direction."
That's true of `[row]` specifically. It is **not** true of the rest of
the file. These defaults currently encode the Drifter philosophy
without saying so.

### 3.1 `[finger]` weights encode a piano-pedagogy strength model

**Where:** `drift.toml` `[finger]` section.

**Current defaults:**
```toml
left_pinky = 1.0, left_ring = 1.8, left_middle = 2.0, left_index = 1.5
right_index = 1.5, right_middle = 2.0, right_ring = 1.8, right_pinky = 1.0
```

**Why this is a philosophy:** the comment says "piano-pedagogy
hierarchy." This *is* a defensible model, but calling the file
"neutral" while shipping it as default is misleading. A truly neutral
default would be all 1.0 (finger load evaluated purely by balance,
no strength assumptions).

**Remediation:** ship `neutral.toml` with all 1.0s as the default;
move the current values to a labeled `piano.toml` preset.

### 3.2 `[roll] inward_multiplier = 1.1 > outward_multiplier = 1.0`

**Where:** `drift.toml` `[roll]` section.

Small (10%) inward preference baked into defaults. Philosophy, not
neutral.

**Remediation:** default 1.0 / 1.0; ship an `inward_preference.toml`
preset with the 1.1 / 1.0 values.

### 3.3 Default trigram rule weights encode the full Drifter philosophy

**Where:** `drift.toml` `[trigram.*]` sections.

- `inward_roll = 3.0 > outward_roll = 2.5` — inward preferred.
- `flexion_cascade = 1.5` — flexion-only reward active with no extension
  mirror, so this is a one-sided bonus for flexion-preferring layouts.
- `hand_territory same_row_reward = 0.5, two_row_penalty = -1.0` —
  row synchrony rewarded, which favors layouts with symmetric row
  distribution (= Drifter by design).
- `pinky_terminal = -0.5` — pinky-only penalty baked in (category 1.2).

**Remediation:** in `neutral.toml`, either disable all opinion-laden
rules (remove from `[trigram].rules` list) or set all their weights to
0. The `drifter.toml` preset retains current values.

### 3.4 The default `[trigram].rules` list activates Drifter-favorable rules

**Where:** `drift.toml` `[trigram].rules` list.

Current default enables `flexion_cascade` (Drifter-favorable) and
`hand_territory` (Drifter-favorable, motivated by the Gallium row-split
observation). Rules that would favor other philosophies don't exist
(there is no `extension_cascade`, no per-finger terminal table with
non-pinky entries, etc.).

**Remediation:** see 3.3. The fix is coupled to category 1: add mirror
rules so other philosophies are *expressible*, then set defaults that
don't silently prefer one.

---

## Category 4 — Correctness issues flagged during audit

Not bias, but worth fixing while we're in here.

### 4.1 `lateral_penalty` usage mismatch

**Where:** `drift.toml` config doc vs. `src/score.rs` line 421.

The config comment for `lateral_penalty` says:

> Same-finger motion with significant dx component.

But in `score.rs`, `lateral_penalty` is applied to `Motion::Stretch`,
which is defined as "non-adjacent, non-same-row" — i.e. *not* a
same-finger motion. Something like `pinky → middle across rows`.

Either the docs or the usage is wrong. Most likely the docs: the stretch
cost needs a penalty knob and this one was repurposed without updating
the comment. Either rename the field (`stretch_penalty`) and keep the
current behavior, or split into two fields and give stretches their own.

### 4.2 `Motion::SameRowSkip` has no cost and no reward

**Where:** `src/score.rs`, case `Motion::SameRowSkip` in `apply_motion`.

Same-row non-adjacent motions (e.g. home-row pinky→middle directly) are
tallied but never scored. They're treated as free.

**Why it matters:** these are real motions — some people find them
comfortable same-row sweeps, others find them effortful mid-row jumps.
Leaving them at zero is a modeling choice that should be explicit.

**Remediation:** add a knob:

```toml
[bigram]
same_row_skip_weight = 0.0    # neutral by default
```

with documentation that positive rewards treat them as sweep-like,
negative penalizes as sub-rolls.

---

## Meta-issue summary

The header of `drift.toml` claims neutrality. In practice:

- `[row]` is genuinely neutral (flexion = extension = 1.0).
- Nothing else is.

This matters because the stated goal is "Gallium scores on its merits,
Drifter scores on its merits, reality wins." With the current defaults
this is not quite true: Drifter has several rules built specifically to
score the properties Drifter was designed to have, with no symmetric
rules for competing philosophies.

The 2× score gap between Gallium and Drifter under default config is
therefore partly real (flexion and row-territory are genuinely being
measured) and partly an artifact (the scorer has no way for an
extension/asymmetric-territory philosophy to earn analogous points back).

## Recommended remediation — prioritized

**P0 — ship a neutral baseline.**

1. Rename current `drift.toml` to `presets/drifter.toml`.
2. Write `presets/neutral.toml`: all finger weights 1.0, all roll
   multipliers 1.0, all opinion-bearing trigram rules disabled or at 0.
3. Load presets via `--preset <name>`; default to `neutral`.

**P1 — add mirror rules so opposite philosophies are expressible.**

4. Generalize `pinky_terminal` to per-finger `terminal_penalty` (1.2).
5. Add `extension_cascade` as symmetric counterpart to
   `flexion_cascade`, or parameterize flexion_cascade's row-set (1.3).
6. Parameterize redirect's anchor-finger set (1.4).

**P2 — surface hidden knobs and fix mismatches.**

7. Move `MIN_BIGRAM_FREQ` and `MIN_TRIGRAM_FREQ` to config (2.4).
8. Add configurable y-threshold and direction for asymmetric-forward
   exemption (1.6).
9. Score `Motion::SameRowSkip` explicitly (4.2).
10. Fix `lateral_penalty` docs-vs-usage mismatch (4.1).
11. Document sign convention at top of `drift.toml` (2.5).

**P3 — documentation of structural assumptions.**

12. Write `docs/ASSUMPTIONS.md` covering the 4-finger model, 3-row
    alpha, y-down coordinate, cross-row mapping (2.1, 2.2, 2.3).
13. Add partial-alternation handling (1.5) — low priority, mostly
    expressiveness.

After P0+P1, drift can honestly claim "only configured bias." P2 and P3
are hygiene. The total budget is roughly one focused week of work.

---

## What's landed (post-modularity-refactor)

The modularity refactor (see `MODULARITY_AUDIT.md`) resolved the
structural scaffolding for most of this audit. What changed since
this document was written:

**P0 — neutral baseline: ✓ done.**
- `presets/neutral.toml` ships and is the default preset.
- `presets/drifter.toml` holds the original opinionated values.
- `presets/extension.toml` added as a reference mirror preset
  (rewards extension cascades, penalizes flexion-direction scissors).
- `--preset <name>` selects; `--config <path>` overrides with a
  file. Default is `neutral`.

**P1 — mirror rules: ✓ done.**
- 1.2 (pinky-hardcoded terminal): `terminal_penalty` analyzer is
  per-finger configurable. Both presets ship all eight weights
  explicitly. ✓
- 1.3 (flexion-only cascade): `flexion_cascade` has a config-
  driven `allowed_rows` list. A parallel `extension_cascade`
  analyzer is registered with default `["home", "top"]`. Both
  analyzers are registered in every preset so opposite
  philosophies are expressible. ✓
- 1.4 (index-hardcoded redirect anchor): the analyzer reads
  `anchor_fingers` from config via `strings_or` + `parse_finger_name`.
  Default `["l_index", "r_index"]`; users can override to any
  subset of the eight alpha fingers. Verified: switching to
  `["l_middle", "r_middle"]` shifts the redirect/bad_redirect
  populations and the total score as expected. ✓

**P2 — surface hidden knobs:**
- 4.2 (same-row-skip unscored): `same_row_skip` analyzer
  registered, default weight 0. ✓
- 1.6 (asymmetric-forward hardcoding): the `forward_threshold`
  config knob exists on the `scissor` analyzer and is read from
  config. Per-pair toggles (`index_middle_forward_ok` etc.) are
  also config-driven. A bug in the match arms was fixed — the
  arms had been written with (outer, inner) reversed from the
  code's ordering convention and were unreachable, so the
  exemption never fired at all. With the fix, ~40 scissor hits
  on the Elora example layouts are now correctly exempted (a
  real-world effect of ~3.5 points on typical drifter-preset
  scores). The direction of the rule ("outer pulled back toward
  user = natural splay") remains hardcoded; that's a rule-design
  property rather than a bug. ✓
- 2.4 (MIN_BIGRAM_FREQ / MIN_TRIGRAM_FREQ hardcoded): these no
  longer exist. The new executor scores every window emitted by
  the corpus; delta scoring uses per-char dependency indexing
  rather than pruning. ✓ (via deletion)
- 4.1 (`lateral_penalty` docs-vs-usage mismatch): the
  `Motion` enum no longer exists. Stretch is now its own
  analyzer with a clearly-named `penalty` field. ✓
- 2.5 (sign convention): every analyzer uses positive-reward,
  negative-penalty consistently. Overload's sign is documented
  at `finger_load::FingerLoad::overload_weight`. ✓ (by construction)

**Other structural fixes delivered by the refactor:**

- Bigram scoring was a fixed match on a fixed `Motion` enum.
  Now every bigram classification is a separate analyzer module.
- Trigram rules used to share `TrigramContext` with hardcoded
  helpers; those are now free functions in `trigram_util.rs`
  consumed by analyzer modules that need them.
- `ScoreResult` used to have named fields for every category;
  now it's a generic `Vec<Hit>` with category-based rollups,
  so new analyzers appear in the report automatically.
- Delta scoring used to be bigram+trigram specific with
  duplicated logic; now it's scope-generic and consumes
  `Analyzer::dependencies()` to narrow re-evaluation scope.
  A regression test (`drift-delta/tests/delta_matches_full.rs`)
  verifies bit-level agreement between delta and full rescoring.

**Still open (tracked for later):**
- 1.1 (monotonic-only roll definition): two-part resolution.
  Non-monotonic rolls — same-hand same-row bigrams with ≥1
  column physically skipped — are now scoreable in two flavors.
  The flat `same_row_skip` analyzer gives every such bigram one
  configurable weight regardless of finger pair; the opinionated
  `same_row_skip_fingerpair` analyzer uses per-finger-pair
  weights keyed by direction and endpoint (including index
  sub-column, so `ring→index-outer` scores differently from
  `ring→index-inner`). Drifter's preset uses the per-pair
  variant to encode Kay's subjective kinesthetic map; neutral
  and extension ship the analyzer enabled with all weights at
  zero; oxey_mimic leaves it off. Same-hand trigrams with
  non-monotonic finger columns (e.g. `middle→index→middle`
  rocking patterns) remain unscored — that's a definition
  question left for later. ✓ (with the trigram gap noted)
- 1.5 (partial-alternation not rewarded): the `alternate`
  analyzer now reads a `partial_weight` config field. When
  non-zero, trigrams with exactly one cross-hand pair (L-L-R,
  R-R-L, L-R-R, R-L-L) emit hits at the partial weight. Presets
  default to `0.15` (roughly proportional to strict alternation
  being half-met). Set to `0.0` to restore the pre-partial
  behavior. ✓
- 1.6 (asymmetric-forward direction hardcoded — rule-design
  choice, kept as-is). See above for the match-arm bug fix. ✓
- 2.1–2.3 (structural assumptions unstated). Documented in
  `docs/ASSUMPTIONS.md`, which lists the seven baked-in
  preconditions (4-finger hand, 3 alpha rows, y-up = toward
  user, etc.) and what each assumption breaks. ✓

**Bias-audit bottom line:** drift can now honestly claim
"only configured bias" for the items where it matters most —
row direction, terminal-finger identity, flexion vs extension,
non-monotonic roll quality. All audit items that started as ✗
are now ✓ or tracked with an explicit "known limit" (the one
remaining is a definition question, not a bias).
