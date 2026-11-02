# Changelog

## Unreleased

### New

- The `easyshot` command line: capture, record, OCR, settings and more, `easyshot doctor` with `--json` and `--bundle`, and shell completions for bash, zsh and fish.
- Desktop entry, systemd user service and D-Bus activation.
- Crop, rotate and flip the image in the editor; annotations follow, and each step can be undone.
- Scrolling capture: select an area, scroll, and get one tall image.
- The editor's Background, Camera and Progressive Blur sections: colours, gradients, seven built-in wallpapers or your own, padding, shadow, corners, alignment, and a focus pad for the sharp area.
- The Settings window: Home, General, Capture, Overlay, Recording, Shortcuts and About, saved as you change them.
- Settings > Shortcuts writes a bind snippet for Hyprland, sway or niri and checks which keys are already bound.

### Improvements

- The Arch package enables the daemon's systemd user service and starts it for logged-in users on install; an update restarts it.
- Deck cards show the capture inside your default look, and saved captures reopen in the editor with that look still editable.
- `easyshot deck toggle` hides and shows the deck.
- Captures from a rotated or flipped output are saved upright.
- Pixelate and Blur redactions keep a minimum strength tied to the text height, so covered text cannot be read back.
- Camera and blur drags no longer freeze the editor, and the exact picture arrives sooner after you let go.
- The watermark stays on the canvas while you edit annotations.
- Wallpaper backgrounds draw much faster, and the editor keeps its pictures within a shared memory limit.
- Hold Space and drag an annotation to move it, with any tool; dragging empty canvas still pans.
- New text starts at size 20 instead of 48.

### Bug fixes

- The tray icon now shows on bars that draw symbolic icons black.
- Panning with Space or the middle button stops when you let go, even outside the canvas.
- The editor's keyboard shortcuts work: Delete, undo, redo and the other edit keys, including keys you change in Settings > Shortcuts.
- The theme setting changes the look of every open window at once, and "System" follows your desktop's light or dark setting.
- Reduce Motion follows your desktop's animation setting.
- The editor's plus and rotate cursors no longer flash or vanish when they change, and pressing a key, such as Space or a tool letter, no longer hides the pointer.
- An `easyshot://record` link asks for an area instead of recording the whole screen at once.
- A screen recording stops when easyshot is killed, instead of running on unseen.
- A document from someone else can no longer use a file outside your wallpapers as its background.
- A capture you only pinned or opened in the editor no longer comes back as a deck card after a restart.
- Smart Redaction in the editor now scans the image and adds the redactions.
- Text read by OCR is no longer written to the log.
- With a wallpaper background, the camera, blur and watermark previews now show the wallpaper, not a gradient.
