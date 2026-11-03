# Changelog

## Unreleased

### New

- `easyshot agent record start|stop|status|discard` lets an agent record an output, an area, a window or the focused output with the recording bar showing, with `--audio`, `--fps`, `--keystrokes` and a `--max-seconds` limit (120 by default, 600 at most); `stop` prints the video's path, and a recording also stops when agent access ends.
- `easyshot agent export VIDEO` (also under `ai`) re-encodes a recording as MP4, WebM or GIF with the trim viewer's size, quality, look and brightness options and a `--trim A..B` range, printing the new file or, with `--json`, its path, size, duration and bytes.
- `easyshot agent` (also `easyshot ai`) for AI agents and scripts: `agent list outputs`, `agent list windows`, `agent capture` (an output, an area, a window or the focused output, saved to the library or to a file you name, with secrets such as keys and passwords pixelated when Tesseract is installed) and `agent ocr` (the text and QR codes on a target or in a PNG or JPEG file), with `--json` printing one JSON object and a distinct exit code for each kind of error. Agent access is off until you run `easyshot agent enable`; `agent status` and `agent config` show and change consent (ask once per session by default), the outputs and windows agents may capture, and the log of agent actions, and only your own user is answered.
- A consent dialog for agent requests (allow for 1 hour, until logout or always, or deny), a confirmation before `easyshot agent enable` or a looser agent setting takes effect, and Settings › Agents with the access switch, consent, scope, hiding secrets, the active grant with Revoke and the recent agent actions.
- Very large screenshots open in the editor with a low-resolution preview that keeps it fast, while saving, exporting and copying use the full image; Settings › General › Always use a low-resolution preview turns it on for every screenshot.
- With Keystrokes on, each recording saves your clicks and shortcuts (never plain typing) to `input.json` beside the video, and reads nothing while your screen is locked where easyshot can see the lock; it needs your user in the `input` group; the Settings row shows whether it is working, and Settings, the start of a recording and `easyshot doctor` say how to allow it.
- Edits you make by hand to `config.toml` apply while easyshot runs, and a later change in Settings keeps them; a file with a syntax error is left alone and the last good settings stay in use.
- The `easyshot` command line: capture, record, OCR, settings and more, `easyshot doctor` with `--json` and `--bundle`, and shell completions for bash, zsh and fish that follow what you have typed: modes, flags, settings pages, `easyshot://` links and image files.
- Desktop entry (Easyshot in your application menu, like a bare `easyshot`, opens the Home window), systemd user service and D-Bus activation. The Arch package enables the service for every user, though never for a display manager's login screen, and starts it for logged-in users on install; an update restarts it where it is running.
- Starting at login leaves easyshot in the tray; with no tray, the capture bar opens so you can still reach it.
- The tray menu shows your global shortcuts beside its items, lists your recent screenshots and saved recordings, and offers Unpin All while a screenshot is pinned.
- Recording a whole screen puts the control bar on another monitor, so it is not in the video; with one monitor it sits in the bottom-right corner and is recorded. A switch in Settings › Recording turns the bar off, and the tray controls the recording either way.
- The editor remembers your last annotation style across sessions, and each tool's options have a Reset to defaults button.
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
- Settings pages rise into place and the introduction's steps slide in as you move between them; with Reduce Motion they appear at once. In the editor a crop grows into the new view, and a preset, a canvas shape or an alignment draws its way into place; the Settings home tiles fill in softly under the pointer. The swatches of a newly picked fill library fade in, and a deck card's picture and its "Saving…" label fade in and out. Tooltips open after a short rest, and at once when you move straight on to the next control.
- Settings > Shortcuts and a first-run "Set your shortcuts" step let you choose your own global keys (none are set by default; Print keys are suggested where free). easyshot keeps them in its own binds file for Hyprland, sway or niri, shows the one include line to add to your compositor config with Copy and an Included / Not found check, offers Reload on sway, and checks which keys are already bound. Other desktops use the global-shortcuts portal instead.
- The capture bar takes the keyboard when it opens, so Esc closes it, and opening it again from its key, the tray or `easyshot capture-bar` closes it.
- Without a screen recorder installed, the tray's Record item is disabled with an install hint and every other way to start a recording answers with it at once; OCR reads QR codes without Tesseract and says when text needs it.
- Screen recording, one at a time: the tray's Record item becomes Stop Recording while one runs, and system sound is recorded unless you turn it off.
- Saved recordings and trim exports go to `~/Videos/easyshot/` with dated names, and each save notice says where.
- The trim viewer exports MP4, WebM or GIF, at the original size or scaled down to 1080p, 720p or 480p, in High, Balanced or Small quality, with a brightness slider and four colour looks (Natural, Vivid, Muted, Mono) that you can preview in the player. Your last choices are remembered.
- Notifications you can click: open the file, show it in its folder, or edit a capture.
- `capture previous` repeats the last area, display or window; a window that has closed gets a toast, not a capture.
- `--timed [N]` on `capture region`, `window` and `fullscreen`, and a timer button on the capture bar: pick first, count down, then take a fresh frame.
