# keywiz

A terminal typing tutor with a visual keyboard — built for custom layouts that no other tool supports.

![keywiz drill mode](https://github.com/user-attachments/assets/placeholder)

## Features

- **Visual keyboard** with color-coded finger zones
- **Key drills** with adaptive difficulty (home row -> top row -> all rows, scales up/down based on accuracy)
- **Typing practice** with scrolling word display and live WPM/accuracy tracking
- **Reads kanata configs** directly — no need to maintain a separate layout file
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
```

### Modes

- **[1] Key Drills** — random keys, starts with home row. Levels up automatically when you hit >90% accuracy, levels back down if you drop below 70%.
- **[2] Typing Practice** — type 20 words with a scrolling display and keyboard guide. Shows WPM and accuracy.

Press **ESC** to go back or quit.

## Layout Support

Keywiz reads keyboard layouts from [kanata](https://github.com/jtroo/kanata) configuration files, including `tap-hold` aliases. The layout system is modular — adding parsers for other formats (QMK, KMonad, etc.) is straightforward.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
