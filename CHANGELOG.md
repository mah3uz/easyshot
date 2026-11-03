# Changelog

## Unreleased

### New

- Edits you make by hand to `config.toml` apply while easyshot runs, and a later change in Settings keeps them; a file with a syntax error is left alone and the last good settings stay in use.
- The `easyshot` command line: capture, record, OCR, settings and more, `easyshot doctor` with `--json` and `--bundle`, and shell completions for bash, zsh and fish that follow what you have typed: modes, flags, settings pages, `easyshot://` links and image files.
- Desktop entry (Easyshot in your application menu, like a bare `easyshot`, opens the Home window), systemd user service and D-Bus activation. The Arch package enables the service for every user, though never for a display manager's login screen, and starts it for logged-in users on install; an update restarts it where it is running.
- Starting at login leaves easyshot in the tray; with no tray, the capture bar opens so you can still reach it.
- Recording a whole screen puts the control bar on another monitor, so it is not in the video; with one monitor it sits in the bottom-right corner and is recorded. A switch in Settings › Recording turns the bar off, and the tray controls the recording either way.
- Crop, rotate and flip the image in the editor; annotations follow, and each step can be undone.
- In the editor the mouse wheel zooms about the pointer and touchpad scrolling pans; annotations stay on the image.
- Scrolling capture: select an area, scroll, and get one tall image.
- The editor's inspector on the right, with a button in the tool row to hide it, and its Background, Camera and Progressive Blur sections: colours, gradients, seven built-in wallpapers or your own, padding, shadow, corners, a canvas shape (Free, 1:1, 4:3, 3:2, 16:9), alignment, and a focus pad for the sharp area.
- Each editor tool's options open in a panel under its button and stay until you change tools: colour and stroke, arrowheads and line style for arrows, font, size, bold, italic and underline for text, blur strength, the Highlighter's mode, and crop's ratio with Reset, Cancel and Apply.
- Each kind of editor tool has its own pointer: a cross for shapes and arrows, a pen for drawing, a text cursor for text, a badge for the counter and a dotted square for Highlight, Pixelate and Blur.
- The Highlighter tool (key M) paints translucent colour that keeps the text under it readable, freehand or as a straight line (Shift snaps to 15 degrees; M again switches mode).
- Smart Redaction scans the image for private text and covers it, flashes what it found, and says what it hid ("Hid 3 items: emails, keys") with an Undo button; Pixelate and Blur keep a minimum strength tied to the text height, so covered text cannot be read back.
- Hold Space and drag an annotation to move it, with any tool; dragging empty canvas pans.
- Text can have a background and a border.
- New text starts at size 36, and a dashed box shows where it goes while you type.
- Esc steps back one level: it commits the text you are typing, leaves crop, clears the selection, then switches to the Select tool.
- Deck cards show the capture inside your default look, and saved captures reopen in the editor with that look still editable. `easyshot deck toggle` hides and shows the deck. Cards drag into file managers as a copy, and a card that auto-save has saved offers Show in Folder in place of Save.
- The Home window: the media gallery of your captures and recordings and Settings, one tab each. Settings has Home, General, Capture, Overlay, Recording, Shortcuts and About, saved as you change them. "System" theme and Reduce Motion follow your desktop.
- Settings > Shortcuts and a first-run "Set your shortcuts" step let you choose your own global keys (none are set by default; Print keys are suggested where free). easyshot keeps them in its own binds file for Hyprland, sway or niri, shows the one include line to add to your compositor config with Copy and an Included / Not found check, offers Reload on sway, and checks which keys are already bound.
- The capture bar takes the keyboard when it opens, so Esc closes it, and opening it again from its key, the tray or `easyshot capture-bar` closes it.
- Screen recording, one at a time: the tray's Record item becomes Stop Recording while one runs, and system sound is recorded unless you turn it off.
- Saved recordings and trim exports go to `~/Videos/easyshot/` with dated names, and each save notice says where.
- The trim viewer exports MP4, WebM or GIF, at the original size or scaled down to 1080p, 720p or 480p, in High, Balanced or Small quality, with a brightness slider and four colour looks (Natural, Vivid, Muted, Mono) that you can preview in the player. Your last choices are remembered.
- Notifications you can click: open the file, show it in its folder, or edit a capture.
- `capture previous` repeats the last area, display or window; a window that has closed gets a toast, not a capture.
- `--timed [N]` on `capture region`, `window` and `fullscreen`, and a timer button on the capture bar: pick first, count down, then take a fresh frame.
