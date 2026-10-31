# easyshot

A screenshot and screen-recording app for Linux/Wayland, with annotation, built with Rust
and GPUI.

easyshot is not usable yet. The workspace skeleton exists; nothing is implemented. Read
`docs/MANUAL.md` for the behaviour v1 is meant to have.

## What v1 is meant to do

- Capture a region, the full screen, a window, a timed capture, text (OCR), a colour, or a
  scrolling page.
- Show each capture in a floating preview deck, pin screenshots on top of other windows,
  and keep a history and gallery.
- Edit in an annotation editor with 12 tools, beautify options and smart redaction.
- Record the screen with audio, and trim the result.

Compositors for v1: Hyprland 0.57+, sway 1.12 and niri 26.04, plus any compositor with the
protocols the generic providers need.

## Documents

- `docs/MANUAL.md` — what you can do with it
- `docs/ARCHITECTURE.md` — how it is built
- `docs/DESIGN.md` — why it looks and behaves as it does
- `docs/DECISIONS.md` — each choice and what was rejected
- `docs/PRIOR-ART.md` — what it builds on
- `CONTRIBUTING.md` — how to work on it
- `CHANGELOG.md` — what changed

## Licence

MIT. Bundled fonts and icons keep their own terms; see `LICENSES/`.
