# keywiz

A terminal typing tutor with a visual keyboard — built for custom layouts that no other tool supports.

## Features

- **Visual keyboard** with color-coded finger zones
- **Key drills** with adaptive difficulty — starts on home row, levels up/down based on rolling accuracy
- **Typing practice** with scrolling word display and live WPM/accuracy
- **Endless mode** — continuous practice without a word limit
- **Split keyboard view** for columnar split boards (Elora, Corne, Sweep, etc.)
- **Toggle keyboard** with Tab — fly blind when you're ready
- **Reads kanata configs** directly — no separate layout file needed
- Runs in the terminal, no GUI dependencies

## Install

```sh
cargo install --path .
```

## Usage

```sh
# Uses ~/.config/kanata/kanata.kbd with layer "gallium_v2" by default
keywiz

# Specify a config and layer
keywiz /path/to/kanata.kbd my_layer

# Split keyboard mode
keywiz --split
```

### Modes

- **[1] Key Drills** — random keys, starts with home row. Levels up at >90% accuracy, back down below 70%.
- **[2] Typing Practice** — type 20 words with a scrolling display and keyboard guide.
- **[3] Endless Mode** — like typing practice, but it never ends. ESC to stop.

### Controls

- **Tab** — toggle keyboard visibility
- **Shift+Tab** — toggle split / standard keyboard
- **ESC** — go back / quit

## Layout Support

Keywiz reads keyboard layouts from [kanata](https://github.com/jtroo/kanata) configuration files, including `tap-hold` aliases. The layout system is modular — adding parsers for other formats (QMK, KMonad, etc.) is straightforward.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
