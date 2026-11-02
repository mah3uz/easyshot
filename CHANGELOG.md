# Changelog

## Unreleased

### New

- The `easyshot` command line: capture, record, OCR, settings and the other commands, plus `easyshot doctor` with `--json` and `--bundle`.
- Crop, rotate and flip the image in the editor; annotations follow, and each step can be undone.
- Scrolling capture: select an area, scroll, and get one tall image.
- The editor's Background section: colours, gradients, seven built-in wallpapers or your own, with padding, shadow, corners and alignment.
- The editor's Camera and Progressive Blur sections, with a focus pad for the sharp area.
- The Settings window: Home, General, Capture, Overlay, Recording, Shortcuts and About, saved as you change them.
- Settings > Shortcuts writes a bind snippet for Hyprland, sway or niri and checks which keys are already bound.

### Improvements

- Deck cards show the capture inside your default look, and saved captures reopen in the editor with that look still editable.
- `easyshot deck toggle` hides and shows the deck.
- Captures from a rotated or flipped output are saved upright.

### Bug fixes

- A capture you only pinned or opened in the editor no longer comes back as a deck card after a restart.
