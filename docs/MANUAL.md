# easyshot manual

**easyshot is not usable yet.** The project has a workspace skeleton and nothing is
implemented. This manual describes what version 1 is meant to do. Until it is built, treat
every statement here as intended behaviour, not something you can rely on.

easyshot takes screenshots and screen recordings on Linux/Wayland, lets you annotate them,
and keeps a history.

## What it needs

- A Wayland compositor. Version 1 targets Hyprland 0.57+, sway 1.12 and niri 26.04. Any
  compositor with layer-shell and a capture protocol should work for basic capture.
  river, labwc and COSMIC come later; KDE and GNOME are not supported in version 1.
- Tesseract for text capture (OCR). Without it, the OCR items are disabled with an install
  hint.
- A screen recorder for recording. Without one, the record items are disabled with an
  install hint.

`easyshot doctor` reports what your compositor and tools provide.

## Running it

easyshot runs as one background program, the daemon, started by a systemd user unit
(`easyshot.service`) or by XDG autostart. Commands like `easyshot capture/region` talk to
it, and start it if it is not running.

easyshot cannot register global keyboard shortcuts for you on Hyprland or wlroots
compositors. You bind keys in your own compositor config to call `easyshot <command>`.
Settings > Shortcuts shows the snippet to paste for your compositor.

## Capturing

- **Region.** The screen freezes, you drag a selection, and you can adjust it. The previous
  area is preselected. Space switches to window mode.
- **Previous region.** Captures the last area again.
- **Fullscreen.** The output under the pointer.
- **Window.** Hover highlights a window; click to capture it.
- **Timed.** Waits 3, 5 or 10 seconds first.
- **Text (OCR).** Reads text from a region, with or without line breaks, and reads QR
  codes.
- **Colour picker.** Copies the colour as `#RRGGBB`.
- **Scrolling capture.** Captures a scrolling page.
- **Variants.** Region capture also comes as "& Copy", "& Save", "& Annotate" and "& Pin".

If you start a capture while another is running, it waits its turn.

The screen always freezes during a capture. There is no option to turn that off.

## After a capture

Depending on your settings, easyshot plays a shutter sound, saves the file, copies it, and
opens the editor. Each capture also appears as a card in a floating preview deck:

- up to 5 cards;
- hover actions in slots you can configure;
- drag a card out into another app;
- cards disappear on their own, and pause while you hover over them;
- Save All and Clear All.

Cards on the deck survive a restart if they are younger than 24 hours.

## Pinned screenshots

A pin is a screenshot that stays on top of other windows. Drag to move it. Scroll to resize
it; the top-left corner stays fixed. Hover for a close button. The context menu has Copy
and Close.

## The editor

The editor has 12 tools. You can select, move, resize and rotate shapes, bind arrows to
shapes, edit text, and undo and redo. You can crop, rotate and flip, and zoom and pan.

To crop, press the crop button in the tool row. Drag a corner or edge handle to resize the
selection, or drag inside it to move it; hold Alt to resize about the centre. The aspect
menu in the title bar offers Freeform, Original, 1:1, 16:9, 9:16, 4:3 and 3:2; while an
aspect is locked only the corners resize. The footer shows the size the image will have.
Enter or Crop applies; Escape, Cancel or the crop button again leaves without a change, and
Reset returns to the whole image. Rotate and flip are in the menu beside the crop button.
Your annotations move with the pixels under them. Crop, rotate and flip are undoable: undo
first steps back through the edits you made after them. The editor keeps the last 16 image
steps, and drops the oldest sooner when they would hold more than 768 MiB of pixels.

Beautify options: background (colour, gradient, wallpaper), padding, corners, shadow and
its style, alignment, aspect ratio, camera 3D, progressive blur, border, watermark and
presets.

Smart redaction finds sensitive text in the image and covers it with redaction shapes, all
in one undo step.

Saving keeps a sidecar file so you can reopen and edit again. Export writes PNG or JPEG.
Closing with unsaved changes asks first. If easyshot stops unexpectedly, it reopens your
unsaved edits the next time it starts ("Recovered unsaved edits").

### Editor keys

These are additional editor keys, and all appear in Settings > Shortcuts.

| Key | What it does |
|---|---|
| Arrow keys | Nudge the selection by 1 pixel. Shift: 10 pixels. |
| Esc | Clear the selection. A second Esc does nothing. |
| Middle-button drag, or hold Space and drag | Pan. |
| Ctrl/Alt + wheel, or pinch | Zoom about the pointer. |
| Ctrl+D | Duplicate the selection. |
| Ctrl+C / Ctrl+V | Copy and paste shapes (with Select active and a selection). |
| Ctrl+Shift+C | Copy the image. |
| Ctrl+] / Ctrl+[ | Move the selection one step forward or back. |
| Ctrl+Shift+] / Ctrl+Shift+[ | Bring to front or send to back. |
| Tab / Shift+Tab | Select the next or previous shape. |

## History and gallery

Captures are kept in a history with retention, search, sorting, icon and list views, and a
trash. The default limit is 100 entries.

## Recording

A capture bar and recording picker start a recording of an area, a display or a window,
with system audio and microphone if you choose. There is a countdown, and then a control
bar with stop, pause and resume, restart and discard. The recorded area is highlighted.

Each recording is a project folder with an `input.json` holding cursor travel, clicks and
keystrokes when permitted. A finished recording appears as a deck card, then opens in a
trim viewer with export.

During a full-display recording the control bar can appear in the video. Your compositor
has no equivalent of the macOS setting that hides it.

## Settings

Pages: Home, General, Capture, Overlay, Recording, Shortcuts, About. Sharing is not part of
version 1.

Settings are stored in `config.toml`; see Files. You can edit it by hand. easyshot watches
the file and applies changes live. If the file has a syntax error, easyshot keeps running
on the last good settings, never overwrites your file, and tells you the line and column.

Editor settings that exist only in the config file:

- `[editor] nudge_step` (default 1) and `nudge_step_shift` (default 10);
- `[editor] zoom_anchor` (default `"pointer"`; `"centre"` restores centre-anchored zoom).

## Command line

`easyshot <command>` sends a command to the daemon and waits for it to finish. Press
Ctrl+C to cancel. If the daemon disappears, the command exits with code 3. `easyshot open
<url>` handles `easyshot://` links.

Commands:

- capture: `capture/region`, `capture/fullscreen`, `capture/window`, `capture/previous`,
  `capture/timed`, `capture/region-copy`, `capture/region-save`, `capture/region-annotate`,
  `capture/region-pin`, `capture/scroll`
- `ocr`, `ocr/single-line`, `color-picker`
- `record`, `record/area`, `record/stop`, `record/pause`, `record/restart`,
  `record/discard`
- `capture-bar`, `gallery`, `restore-last`, `pin-last`, `unpin-all`, `open <path>`,
  `edit-clipboard`, `settings[/<page>]`, `deck/toggle`, `deck/save-all`, `deck/close-all`,
  `quit`

`easyshot://` links accept only these eight, with no arguments: `capture/region`,
`capture/fullscreen`, `capture/window`, `capture/scroll`, `record`, `ocr`, `color-picker`,
`settings`.

## Files

| What | Where |
|---|---|
| Settings | `$XDG_CONFIG_HOME/easyshot/config.toml` |
| Screenshots and edit sidecars | `$XDG_DATA_HOME/easyshot/library/` |
| Recordings | `$XDG_DATA_HOME/easyshot/recordings/` |
| Logs and crash reports | `$XDG_STATE_HOME/easyshot/log/`, `crash/` |
| Default save folder | `Screenshots` inside your Pictures folder |

If a file was made by a newer easyshot, it opens read-only and is never changed.

Crash reports stay on your machine; nothing is uploaded.

## When your compositor lacks something

| Missing | What you get |
|---|---|
| Window capture | The window's rectangle is cropped from the frozen screen. |
| Window geometry | Window mode shows a list of windows instead of hover highlight. |
| Virtual input | No Auto Scroll; manual scroll capture still works. |
| File picker portal | Export saves to your save folder and tells you where. |
| Tray | The capture bar is shown at launch so you can still reach easyshot. |
| Recorder or OCR tools | Those items are disabled with an install hint. |

`easyshot doctor` shows what applies to you.

## Limits in version 1

KDE and GNOME, the video Studio (timeline, zooms, cursor effects, captions, camera
layouts), camera recording, the teleprompter, cloud sharing, a built-in updater and
Flatpak are not part of version 1.

## Troubleshooting

Nothing here is tested yet. Start with `easyshot doctor`, and set `EASYSHOT_LOG` to change
what is logged.
