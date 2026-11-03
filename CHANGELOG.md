# Changelog

## Unreleased

### New

- The `easyshot` command line: capture, record, OCR, settings and more, `easyshot doctor` with `--json` and `--bundle`, and shell completions for bash, zsh and fish that follow what you have typed: modes, flags, settings pages, `easyshot://` links and image files.
- Desktop entry, systemd user service and D-Bus activation. The Arch package enables the service for every user, though never for a display manager's login screen, and starts it for logged-in users on install; an update restarts it where it is running.
- Starting at login leaves easyshot in the tray; with no tray, the capture bar opens so you can still reach it.
- Crop, rotate and flip the image in the editor; annotations follow, and each step can be undone.
- Scrolling capture: select an area, scroll, and get one tall image.
- The editor's Background, Camera and Progressive Blur sections: colours, gradients, seven built-in wallpapers or your own, padding, shadow, corners, alignment, and a focus pad for the sharp area.
- Smart Redaction scans the image for private text and covers it; Pixelate and Blur keep a minimum strength tied to the text height, so covered text cannot be read back.
- Hold Space and drag an annotation to move it, with any tool; dragging empty canvas pans.
- New text starts at size 36.
- Esc steps back one level: it commits the text you are typing, leaves crop, clears the selection, then switches to the Select tool.
- Deck cards show the capture inside your default look, and saved captures reopen in the editor with that look still editable. `easyshot deck toggle` hides and shows the deck. Cards drag into file managers as a copy, and a card that auto-save has saved offers Show in Folder in place of Save.
- The Settings window: Home, General, Capture, Overlay, Recording, Shortcuts and About, saved as you change them. "System" theme and Reduce Motion follow your desktop.
- Settings > Shortcuts lets you choose your own global keys (none are set by default), writes a bind snippet with them for Hyprland, sway or niri, and checks which keys are already bound.
- The capture bar takes the keyboard when it opens, so Esc closes it, and opening it again from its key, the tray or `easyshot capture-bar` closes it.
- Screen recording, one at a time: the tray's Record item becomes Stop Recording while one runs, and system sound is recorded unless you turn it off.
- Saved recordings and trim exports go to `~/Videos/easyshot/` with dated names, and each save notice says where.
- Notifications you can click: open the file, show it in its folder, or edit a capture.
