<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/banner/banner-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/banner/banner-light.svg">
    <img alt="easyshot" src="assets/banner/banner-light.png">
  </picture>
</h1>

A screenshot and screen-recording app for Linux/Wayland, with annotation, built with Rust
and GPUI.

easyshot does not run as an app yet. Its parts are built and tested on their own, but the
background service that connects them is not, so a hotkey or tray click does nothing. Read
`docs/MANUAL.md` for how each part behaves.

## What is built

- Capture overlays: an area, a window, a text area for OCR, a colour, and a scrolling page,
  with a countdown for timed captures.
- A floating preview deck for each capture, and screenshots pinned above other windows.
- An annotation editor with 12 tools, editable text, crop, rotate and flip, and an inspector
  for background, camera angle, progressive blur, border, watermark and presets. It saves,
  exports and copies, and recovers unsaved edits after a crash.
- Screen recording through gpu-screen-recorder, wf-recorder or the screen-cast portal, with
  the capture bar, the recording controls and a trim viewer.
- Settings, a media gallery, a first-run guide and a tray menu.
- The `easyshot` command line, including `easyshot doctor`, which reports what your desktop
  supports.

## Limits

- No background service yet, so the parts cannot be used together.
- Tested in headless windows and in a headless sway session. Not yet run on Hyprland or
  niri.
- No packages; build from source (see `CONTRIBUTING.md`).

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
