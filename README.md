# keywiz

A terminal typing tutor with a visual keyboard — built for custom layouts that no other tool supports.

## Features

- **Visual keyboard** with color-coded finger zones
- **Key drills** with adaptive difficulty — starts on home row, levels up/down based on rolling accuracy
- **Typing practice** with scrolling word display and live WPM/accuracy
- **Endless mode** — continuous practice without a word limit
- **Split keyboard view** for columnar split boards (Elora, Corne, Sweep, etc.)
- **Toggle keyboard** with Tab — fly blind when you're ready
- **Input translation** — practice any layout on any keyboard (e.g. train Gallium v2 on a QWERTY tablet over SSH)
- **Reads kanata configs** directly — no separate layout file needed
- Runs in the terminal, no GUI dependencies

## Install

```sh
cargo install --path .
```

## Usage

```sh
# Uses ~/.config/kanata/kanata.kbd, auto-detects the first layer
keywiz

# Specify a config and layer to train
keywiz /path/to/kanata.kbd my_layer

# Split keyboard mode
keywiz --split

# Practice on a QWERTY keyboard (translates input to your target layout)
keywiz --from qwerty

# Translate from any layout defined in your kanata config
keywiz --from qwerty my_custom_layer
```

### Training on a different keyboard

If your physical keyboard doesn't run your target layout (e.g. SSHing from a tablet), use `--from` to tell keywiz what layout your keyboard actually sends. Keywiz translates each keypress by physical position — pressing QWERTY `j` registers as whatever your target layout has in that position (e.g. `h` on Gallium v2).

```sh
# SSH into your desktop from an Android tablet with a QWERTY keyboard
ssh desktop
keywiz --from qwerty
```

The `--from` value can be `qwerty` (built-in) or any layer name defined in your kanata config. This means if you have two custom layouts in the same config file, you can practice one while typing on the other.

This way you can practice anywhere without needing kanata, custom Android IMEs, or any special setup on the device you're typing on.

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
