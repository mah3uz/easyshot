# Changelog

## Unreleased

### New

- Shell completions for bash, zsh and fish (`easyshot completions <shell>`).
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
- Pixelate and Blur redactions keep a minimum strength tied to the text height, so covered text cannot be read back.
- Starting a camera or blur drag no longer freezes the editor for a moment, and the exact picture arrives sooner after you let go.
- The watermark stays on the canvas while you edit annotations.

### Bug fixes

- A capture you only pinned or opened in the editor no longer comes back as a deck card after a restart.
- Smart Redaction in the editor now scans the image and adds the redactions.
- Text read by OCR is no longer written to the log.
