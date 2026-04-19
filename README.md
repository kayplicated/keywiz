# keywiz

A terminal typing tutor with a visual keyboard — built for custom layouts that no other tool supports.

## Screenshots

| | |
|---|---|
| ![Mode selection menu](assets/menu.png) | ![Key drills on us_intl](assets/drill.png) |
| **Mode selection** | **Key Drills** — adaptive level progression |
| ![Typing Practice on Kyria](assets/words.png) | ![Text Practice on Elora with heatmap](assets/text.png) |
| **Typing Practice** on a Kyria split keyboard with the Hyperroll layout | **Text Practice** on an Elora keyboard with the Gallium layout — F2 heatmap revealing the trouble keys |

## Features

- **Visual keyboard** with color-coded finger zones
- **Key drills** with adaptive difficulty — starts on home row, levels up/down based on rolling accuracy
- **Typing practice** with scrolling word display and live WPM/accuracy
- **Endless mode** — continuous practice without a word limit
- **Text practice** — type through real passages, arrow keys to switch texts
- **Heatmap overlay** — F2 colors the keyboard by where you actually struggle, accumulated across sessions
- **Smart word selection** — words you type quietly bias toward your weak keys, so practice targets itself
- **Many layouts + keyboards** — 13 shipped layouts (QWERTY, Dvorak, Colemak, Colemak-DH, Workman, Graphite, Sturdy, Gallium, Canary, Hyperroll, Engram, Semimak, ISRT) across US, Kyria, and Elora hardware — cycle between them with Ctrl+arrows while the app is running
- **Data-driven** — keyboards and layouts are JSON files; drop your own in `keyboards/` or `layouts/` and they show up in the cycle
- **Toggle keyboard** with Tab — fly blind when you're ready
- **Kanata escape hatch** — point keywiz at a `.kbd` config for one-off layouts not in the shipped catalog
- Runs in the terminal, no GUI dependencies

## Install

```sh
cargo install --path .
```

## Usage

```sh
# Default: us_intl keyboard + qwerty layout
keywiz

# Pick keyboard and layout by name
keywiz -k us_intl -l colemak
keywiz -k elora -l gallium
keywiz -k kyria -l canary

# Cycle at runtime without restarting
#   Ctrl+↑ / Ctrl+↓  — previous / next keyboard
#   Ctrl+← / Ctrl+→  — previous / next layout

# Load a kanata .kbd directly (escape hatch for custom layouts)
keywiz --kanata /path/to/your.kbd
keywiz --kanata /path/to/your.kbd gallium_v2   # specific layer

# Practice a layout while typing on a different physical keyboard
keywiz -l colemak --from qwerty
```

### Training on a different physical keyboard

`--from <layout>` tells keywiz what your input keyboard *actually sends*, so each keypress is translated to the equivalent position in the target layout. Useful for SSHing into your machine from a vanilla QWERTY laptop while practicing Gallium, or for testing a layout you haven't switched the OS to yet. Pressing physical `j` on QWERTY registers as whatever the target layout puts at that position.

Shipped keyboards (`keyboards/`): `us_intl`, `kyria`, `elora`, `halcyon_elora_v2`.
Shipped layouts (`layouts/`): `qwerty`, `dvorak`, `colemak`, `colemak-dh`, `workman`, `graphite`, `sturdy`, `gallium`, `canary`, `hyperroll`, `engram`, `semimak`, `isrt`.

### Adding your own

Both are just JSON. Drop a new file in `keyboards/` or `layouts/` and it appears in the cycle on next launch. A **keyboard** declares physical buttons with home-row-centered coordinates (x grows right, y grows down, home row at `y=0`). A **layout** maps evdev keycodes (`KEY_A`, `KEY_SEMICOLON`, …) to `{ lower, upper }` characters. See the shipped files for templates.

For hardware-specific overrides, name a layout file `{layout}-{keyboard}.json` (e.g. `gallium-elora.json`) — it wins over the generic when paired with that keyboard, and stays hidden from the layout list otherwise.

### Modes

- **[1] Key Drills** — random keys, starts with home row. Levels up at >90% accuracy, back down below 70%.
- **[2] Typing Practice** — type 20 words with a scrolling display and keyboard guide.
- **[3] Endless Mode** — like typing practice, but it never ends. ESC to stop.
- **[4] Text Practice** — type through real passages from the `texts/` directory. Arrow left/right to switch between texts.

### Controls

- **Tab** — toggle keyboard visibility
- **F2** — toggle heatmap overlay on the keyboard
- **Ctrl + ↑ / ↓** — cycle keyboards
- **Ctrl + ← / →** — cycle layouts
- **◀ ▶** — switch passages (text practice mode)
- **ESC** — go back / quit

### Heatmap & smart practice

Every keystroke feeds a per-key *heat* score: missing a key bumps its heat up by one step, two correct presses drop it back down. Heat accumulates across sessions (stored under your OS data directory, per layout) so a key that's been trouble for weeks stays visible.

Two things read from it:

- **The F2 heatmap overlay** colors the keyboard from cool violet → blue → yellow → orange → red. Green is deliberately absent — correctness should feel calm, not "good."
- **Word selection in modes [2] and [3]** quietly weights toward words containing your hot keys. If `y` is giving you trouble you'll see more `yet`, `holiday`, `by`. With no heat, selection is uniform random.

There's no "drill X" menu — practice targets itself as you type.

## Custom Texts

Add `.txt` files to the `texts/` directory for text practice mode. Format:

```
Title Goes Here
The rest of the file is the passage text that you'll type through.
Multiple lines are fine — they get word-wrapped to fit the display.
```

## License

AGPL-3.0 — see [LICENSE](LICENSE).
