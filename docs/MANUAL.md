# easyshot manual

**easyshot is early software.** It runs end to end, but so far it has had one real session,
on Hyprland, and that only partly. sway has been run only headless, and niri not at all.
Expect rough edges, and report what you find.

easyshot takes screenshots and screen recordings on Linux/Wayland, lets you annotate them,
and keeps a history.

## What it needs

- A Wayland compositor. Version 1 targets Hyprland 0.57+, sway 1.12 and niri 26.04. Any
  compositor with layer-shell and a capture protocol should work for basic capture.
  river, labwc and COSMIC come later; KDE and GNOME are not supported in version 1.
- Tesseract for text capture (OCR). Without it, the OCR items are disabled with an install
  hint.
- gpu-screen-recorder or wf-recorder for recording; easyshot uses gpu-screen-recorder when
  both are installed. Without one, the record items are disabled with an install hint. The
  portal screen-cast recorder is not available.

`easyshot doctor` reports what your compositor and tools provide.

## Installing it

On Arch Linux there are two AUR packages: `easyshot`, which builds the release from source,
and `easyshot-bin`, which installs the prebuilt release. They conflict with each other, so
install one. Neither is published yet, and there are no packages for other distributions.

Installing the package enables `easyshot.service` for every user and starts it for each user
who is logged in. Updating or reinstalling it restarts the daemon where it is running and
leaves a stopped one stopped. Removing the package stops it.

## Running it

easyshot runs as one background program, the daemon, started by a systemd user unit
(`easyshot.service`), by D-Bus activation or by XDG autostart. Commands like `easyshot capture region` talk to
it, and start it if it is not running.

With the package installed, the daemon starts with each graphical session of a regular
user; it does not start for a display manager's login screen. To stop it from
starting for your user only, run `systemctl --user mask --now easyshot.service`; a plain
`disable` does not override the package's enable for all users. `systemctl --user unmask
easyshot.service` undoes it. Without the package, the first command you run starts the daemon
through D-Bus activation, and `systemctl --user enable --now easyshot.service` keeps it
running for the whole session. The package installs a desktop entry (Easyshot in your application menu opens the Home window) and registers
`easyshot://` links, which open through `easyshot open`.

easyshot cannot register global keyboard shortcuts for you on Hyprland or wlroots
compositors. You bind keys in your own compositor config to call `easyshot <command>`.
easyshot sets no global key by default, so none clashes with your own binds: you choose each
key in Settings > Shortcuts, or accept the suggestions in the first-run step "Set your
shortcuts" (Print, Shift+Print, Alt+Print and Ctrl+Print where your compositor leaves them
free, otherwise a Super+Shift key). easyshot keeps the chosen keys in its own binds file in
`~/.config/easyshot/` and rewrites it whenever they change; it never edits your compositor
config. You add one line to that config yourself, once:

| Compositor | Binds file | Line to add |
|---|---|---|
| Hyprland, Lua config | `binds.lua` | `pcall(require, "~/.config/easyshot/binds")` |
| Hyprland, `hyprland.conf` | `binds.conf` | `source = ~/.config/easyshot/binds.conf` |
| sway | `binds.conf` | `include ~/.config/easyshot/binds.conf` |
| niri (26.04 or later) | `binds.kdl` | `include optional=true "~/.config/easyshot/binds.kdl"` |

Settings > Shortcuts shows the line for your compositor with Copy Line, and Check reads your
compositor config, and the files it includes directly, and shows Included or Not found. sway
reads the file only on reload, so Settings has a Reload button that runs `swaymsg reload`;
Hyprland and niri reload when the file changes. In sway, put the line after your own binds,
and on niri after your `binds { }` block, so easyshot's keys win a clash. Every command in the
file is one `easyshot --help` lists.

On a desktop where easyshot can write no binds file, it uses the desktop's global-shortcuts
portal instead, when there is one, and your desktop asks you to confirm the shortcuts the
first time easyshot starts. The binds file always comes first where it can be written.

## Capturing

- **Region.** The screen freezes, you drag a selection, and you can adjust it. The previous
  area is preselected. Space switches to window mode.
- **Previous.** Captures the last target again: the same area, the same display, or the
  same window if it is still open. If that window has closed, a toast says so and nothing is
  captured.
- **Fullscreen.** The output under the pointer.
- **Window.** Hover highlights a window; click to capture it.
- **Timed.** Works with region, window and fullscreen. You pick first on the frozen screen,
  then a countdown runs, then easyshot takes a fresh frame of what you picked, so a menu or
  hover you open during the countdown is in the shot. From the command line, add `--timed`
  for the self-timer setting (3 seconds unless you change it) or `--timed N` for N seconds,
  1 to 60. On the capture bar, the timer button sets Off, 3, 5 or 10 seconds for Area,
  Fullscreen and Window.
- **Text (OCR).** Reads text from a region, with or without line breaks, and reads QR
  codes.
- **Colour picker.** Copies the colour as `#RRGGBB`.
- **Scrolling capture.** Select an area, then scroll the page under it. easyshot joins what
  scrolls into view into one tall image, up to 30000 pixels. A bar under the area shows the
  size so far, with Stop and, where your compositor allows it, Auto Scroll, which scrolls for
  you and stops at the end of the page. A preview beside the area shows the image as it
  grows. Press the scrolling-capture shortcut again to stop. Escape does not cancel it; use
  the ✕ on the bar. If the area fills the whole screen, the bar sits over it and is captured.
- **Variants.** Region capture also comes as "& Copy", "& Save", "& Annotate" and "& Pin".

If you start a capture while another is running, it waits its turn.

The screen always freezes during a capture. There is no option to turn that off.

## After a capture

Depending on your settings, easyshot plays a shutter sound (Settings, General, "Play a
shutter sound"; only builds with the `sound` feature make it), saves the file, copies it, and
opens the editor. Each capture also appears as a card in a floating preview deck:

- up to 5 cards;
- hover actions in slots you can configure; on a card that auto-save has already saved, Save
  becomes Show in Folder, which opens your file manager on the saved file;
- drag a card out into another app or a file manager, which gets a copy of the file;
- cards disappear on their own, and pause while you hover over them;
- Save All and Clear All.

Cards on the deck survive a restart if they are younger than 24 hours. A card shows the
capture inside your default look, and the editor opens it with that look still editable.
`easyshot deck toggle` hides the deck and shows it again; hidden cards do not disappear on
their own, and the next capture shows the deck. A pin that you never save is gone when you
close it or restart.

## Pinned screenshots

A pin is a screenshot that stays on top of other windows. Drag to move it. Scroll to resize
it; the top-left corner stays fixed. Hover for a close button. The context menu has Copy
and Close.

## The editor

The editor has 13 tools. You can select, move, resize and rotate shapes, bind arrows to
shapes, edit text, and undo and redo. You can crop, rotate and flip, and zoom and pan.

The window has the title bar (undo, redo, the name, Save and Export) on top, the tool row
under it, the image in the middle, the inspector on the right and the footer (zoom, Copy)
at the bottom. The button at the right end of the tool row hides and shows the inspector.
Tab moves through the title bar, the tool row, the inspector, then the footer.

A tool's options open in a panel under its button and stay open until you pick another
tool; clicking on the image does not close them. Shapes and pens offer colour and stroke
width; Arrow adds a head for each end (None, Arrow, Triangle, Square, Dot, Diamond,
Inverted, Bar) and a line style (Solid, Dashed, Dotted), which Line also has; Text offers
the font, colour, size, bold, italic, underline and alignment, and a background and a
border for the text box (None or a colour; a border is 2, 3 or 5 px wide); Blur and Pixelate offer the
strength; the Highlighter offers colour, width and its mode. The Highlight tool has no
options. With the Select tool, selecting shapes opens the options of the first one, with a
button to delete the selection. A change applies to new shapes and to the selection, as one
undo step.

Over the image, each tool shows its own pointer: a cross for shapes, lines and arrows, a pen
for Freehand and the Highlighter, a text cursor for Text, a numbered badge for the counter,
and a dotted square, whose solid corner is where the drag starts, for Highlight, Pixelate
and Blur.

The Highlighter (key M) paints translucent colour over text without hiding it: the stroke
is multiplied into the image at about 40% strength, so dark text stays dark. It has two
modes. Freehand follows the pointer; Straight draws one line from where you press, and Shift
snaps it to 15 degrees. Press M again to switch mode; the tool button and its tooltip show
which mode is on, and its options switch it too. Pick the colour and the thickness in its
options. Strokes stay on the image, and the editor and export show the same result. The
Highlight tool, which dims everything outside an area, is separate.

New text starts at size 36. Change the size in the Text options. While you type, a dashed box in the
accent colour shows where the text goes, and an empty text shows a small box so you can see
where you clicked. The box is only a guide: it is not saved or exported.

Annotations stay on the image. A shape starts only on the image, and drawing, moving,
resizing and rotating stop at its edge, whatever the shape's rotation. After a crop, the
edge is the cropped image's. A stroke is held by its points, so its thick line can reach
slightly past the edge.

To crop, press the crop button in the tool row. Drag a corner or edge handle to resize the
selection, or drag inside it to move it; hold Alt to resize about the centre. The crop
options open under the crop button: the aspect menu offers Freeform, Original, 1:1, 16:9, 9:16, 4:3 and 3:2; while an
aspect is locked only the corners resize. The footer shows the size the image will have.
Enter or Crop applies; Escape, Cancel or the crop button again leaves without a change, and
Reset returns to the whole image. Save and Export wait until the crop is applied or left. Rotate and flip are in the menu beside the crop button.
Your annotations move with the pixels under them. Crop, rotate and flip are undoable: undo
first steps back through the edits you made after them. The editor keeps the last 16 image
steps, and drops the oldest sooner when they would hold more than 768 MiB of pixels.

Beautify options: background (colour, gradient, wallpaper), padding, corners, shadow and
its style, alignment, canvas shape, camera 3D, progressive blur, border, watermark and
presets.

Canvas shape, in the inspector's Background section, sets the shape of the whole picture:
Free (the image's own shape plus padding), 1:1, 4:3, 3:2 or 16:9. The background grows
around the image to fill the shape. Picking a shape while padding is 0 sets padding to 8%,
so you see the change at once. Canvas shape is greyed out while there is no background,
camera effect or bleeding blur, since then there is nothing around the image to shape.

In the Camera section you tilt and roll the camera, set its lens (FOV), zoom and pan, and
rotate the card itself. Progressive Blur blurs the screenshot alone or the whole scene,
around a round or a band-shaped sharp area. Drag on its focus pad to move the sharp area;
double-click the pad to centre it. Drag anywhere on a slider to change it, or type a value
in its field; a slider that runs through zero stops at zero on the way.

Smart Redaction (the eye-off button) reads the image with Tesseract and covers what looks
sensitive with Pixelate or Blur shapes: e-mail addresses, web links, IPv4 addresses, phone
numbers, card numbers that pass the Luhn check, JSON web tokens, API keys and tokens, long
random-looking strings, and values after labels such as `password:` or `token=`. Choose
Pixelate or Blur in the popover. The button's tooltip reads "Hide emails, keys and other
private text". The footer shows "Scanning screenshot…" while it works (the button is greyed
meanwhile). The regions found flash briefly, and a message appears: "Hid N items: emails,
keys" naming the kinds, with an Undo button that removes everything just added in one step,
or "No sensitive text found." The message goes after six seconds. All the shapes are added as
one undo step and stay selected, so you can move, resize or delete them. Without Tesseract the button is disabled and its tooltip says why.

Smart Redaction only covers text Tesseract can read. Small, low-contrast, rotated or
stylised text, and languages other than English, can be missed. Look over the result
before you share the image.

Redaction shapes are Blur or Pixelate only; there is no solid fill. To keep text
unreadable, their strength has a floor that grows with the shape's short side (the text
height): pixel blocks are at least half that side and never under 12 px, and the blur
radius is at least 40% of it and never under 8 px. The Strength slider can only raise
these. Export uses the same values, and the edges of an unrotated shape are covered to
whole pixels. Smart Redaction uses a high strength.

Saving keeps a sidecar file so you can reopen and edit again. Export writes PNG or JPEG.
Closing with unsaved changes asks first. If easyshot stops unexpectedly, it reopens your
unsaved edits the next time it starts ("Recovered unsaved edits").

### Editor keys

These are additional editor keys. They are fixed: only Ctrl+Shift+C also appears in Settings > Shortcuts, and none of the others can be rebound.

| Key | What it does |
|---|---|
| Arrow keys | Nudge the selection by 1 pixel. Shift: 10 pixels. Both steps are set by `nudge_step` and `nudge_step_shift` under `[editor]`. |
| Esc | Steps back one level per press: commits the text you are typing, leaves crop, clears the selection, then switches to the Select tool. |
| Middle-button drag, or hold Space and drag from empty canvas | Pan. It stops when you let go, even outside the canvas. |
| Hold Space and drag an annotation | Move it, with any tool; the pointer is a pointing hand over it. |
| Mouse wheel, Ctrl/Alt + wheel, or pinch | Zoom about the pointer. |
| Touchpad two-finger scroll | Pan. |
| Ctrl+D | Duplicate the selection. |
| Ctrl+C / Ctrl+V | Copy and paste shapes (with Select active and a selection). |
| Ctrl+Shift+C | Copy the image. |
| Ctrl+] / Ctrl+[ | Move the selection one step forward or back. |
| Ctrl+Shift+] / Ctrl+Shift+[ | Bring to front or send to back. |
| Tab / Shift+Tab | Select the next or previous shape. |

## The Home window

The Home window holds the media gallery and Settings, one per tab; the switcher at the top
of the window moves between them. Easyshot in your application menu, a bare `easyshot` and
an `easyshot://home` link open it, on the gallery the first time. `easyshot gallery` and the tray's Media
Gallery open it on the gallery; `easyshot settings [page]`, the tray's Settings and an
`easyshot://settings` link open it on Settings, at that page when you name one. There is
only one Home window: opening it again brings it to the front and switches to the tab you
asked for, or keeps the tab it shows when you asked for none.

## History and gallery

Captures are kept in a history with retention, search, sorting, icon and list views, and a
trash. The default limit is 100 entries.

## Recording

A capture bar and recording picker start a recording of an area, a display or a window.
System sound is recorded unless you turn it off in Settings, Recording; the microphone is
off until you choose one there. There is a countdown, and then a control bar with stop,
pause and resume, restart and discard. The recorded area is highlighted.

wf-recorder records one audio track. With it, system sound is the default output's monitor, and
a recording with a microphone, or with no default output found, is made without system sound;
easyshot says so when it starts.

The capture bar opens with `easyshot capture-bar`, its key (Super+Shift+2 by default) or the
tray; opening it again by any of these closes it. While it is open it has the keyboard, so
Esc closes it, or closes its open menu first.

One recording runs at a time. While it runs, the tray's Record item reads Stop Recording,
and a second start from a shortcut, the command line, an `easyshot://` link or the capture
bar shows "A recording is already running" and opens no picker.

Each recording is a project folder in `$XDG_DATA_HOME/easyshot/recordings/` with an
`input.json` holding cursor travel, clicks and keystrokes when permitted. A finished
recording appears as a deck card, then opens in a trim viewer with export. Saving the card
and exporting from the trim viewer both write to `easyshot` inside your Videos folder
(usually `~/Videos/easyshot/`), with a dated name that never replaces an earlier file.

The trim viewer's export has a few plain choices under the timeline. Format is MP4, WebM or
GIF; a GIF has no sound. Size is Original, 1080p, 720p or 480p, which only ever shrinks a
video and keeps its shape, so a video already smaller than the choice stays as it is. Quality
is High, Balanced or Small: Small makes the smallest file. Colour is Natural (as recorded),
Vivid, Muted or Mono (black and white), and the Brightness slider lightens or darkens by up
to 30%. The player shows the colour and brightness while you choose them; format, size and
quality apply only to the exported file. Fast (copy without re-encoding) is available only
for an MP4 at its original size and colour; any other choice re-encodes. Your last choices
are remembered in `config.toml` under `[recording]` as `export_format`, `export_size`,
`export_quality`, `export_look` and `export_brightness`.

When you record a whole screen, Settings, Recording, "Show the control bar when recording a
whole screen" (on by default) decides where the control bar goes. With a second monitor that
is not being recorded, the bar goes there and nothing of it is in the video. With one
monitor, or every monitor recorded, the bar sits in the bottom-right corner of the recorded
screen, lifted clear of a bottom panel, and it is in the video: crop it out afterwards. With
the switch off there is no bar, and a notice says to stop from the tray or the shortcut. The
tray's Stop Recording works in every case. Your compositor has no rule that hides the bar
from a recording.

## Settings

Pages: Home, General, Capture, Overlay, Recording, Shortcuts, About. Sharing is not part of
version 1.

Settings are stored in `config.toml`; see Files. You can edit it by hand. While easyshot
runs it watches the file and applies your edits at once, without a restart, and a later change
in Settings keeps them. If the file has a syntax error, easyshot keeps running on the last
good settings and tells you the line and column. It does not write to the file while the
error is there, so changes made in Settings stay until you quit; fix the file and its
contents apply again.

Home shows the capture buttons with their keys, your five most recent captures, and Setup:
a list of what your compositor and installed tools offer, such as the selection overlay,
window capture, recording and text recognition.

Every change is applied at once and written to `config.toml` 0.3 seconds after you stop
changing things.

Turn off Settings > General > Save screenshots automatically and a capture is kept only as a
card in the deck until you save or export it. Capture Region & Save still writes a file. In
Settings > Capture, "Show the selector on" a specific screen puts the selector's keyboard
focus on that screen; if it is unplugged, the pointer's screen is used.

Show the capture bar at launch opens the bar when the daemon starts. It is off, so a start at
login leaves easyshot in the tray with no window open. Open editors in full
screen opens each editor window fullscreen. Open the editor when I stop (Recording) opens the
trim viewer as soon as a recording ends, and the card still appears.

Shortcuts lists every action. Global actions start with no key and show the command your compositor runs, such
as `easyshot capture region`. Click a key field and press a key to record it. For global
actions, the keys are saved under `[global_shortcuts]` in `config.toml` and written to the
binds file; Copy Line puts the one include line for your compositor on the clipboard. easyshot
never edits your compositor config. Check reads your compositor's current binds and marks
actions that are already bound, and keys that run something else, and shows whether the
include line is found. Keys for editor actions are saved under `[shortcuts]`.

About shows the version, links, and Show Onboarding Again.

Editor settings that exist only in the config file:

- `[editor] nudge_step` (default 1) and `nudge_step_shift` (default 10);
- `[editor] zoom_anchor` (default `"pointer"`; `"centre"` restores centre-anchored zoom).

## Command line

`easyshot <command>` sends a command to the daemon and waits for it to finish; `easyshot`
with no command opens the Home window, and `easyshot --help` lists the commands. Press
Ctrl+C to cancel. If the daemon is not running, easyshot tries to start it; if that fails
it tells you to run `easyshot daemon` or `systemctl --user start easyshot.service`.
`easyshot open <url>` handles `easyshot://` links; `easyshot open <file>` opens a PNG or
JPEG in the editor.

Exit codes: 0 done, 1 error, 2 cancelled, 3 the daemon is not running or went away.

`easyshot --version` prints the version and, for a build from a git checkout, the commit it
was built from: `v0.1.0+1a2b3c4d`, with `-dirty` when the checkout had uncommitted changes.

`easyshot completions bash|zsh|fish` prints a short registration script for that shell; the
Arch package installs all three. After that, pressing Tab asks `easyshot` itself, so what
it offers follows what you have typed: the subcommands; `capture` then the modes, and for
`capture region` its `--copy`, `--save`, `--annotate` and `--pin`; the actions of `record`
and `deck`; the pages of `settings`; `--json` and `--bundle` for `doctor`; the shells for
`completions`; and for `open` the `easyshot://` links and the PNG and JPEG files and
folders under the path you have typed. Completion reads no daemon state and starts no
window. To try it without installing, run `source <(easyshot completions bash)` in bash,
`easyshot completions zsh > ~/.zfunc/_easyshot` in zsh (with `~/.zfunc` in `fpath`), or
`easyshot completions fish | source` in fish.

Commands:

- `capture region|fullscreen|window|previous|timed|scroll`; `capture region` also takes one
  of `--copy`, `--save`, `--annotate`, `--pin`; `capture region|window|fullscreen` take
  `--timed [N]`, and `capture timed` is `capture region --timed`
- `ocr [--single-line]`, `color-picker`
- `record [area|stop|pause|restart|discard]`
- `capture-bar`, `gallery`, `restore-last`, `pin-last`, `unpin-all`, `edit-clipboard`,
  `settings [page]`, `deck toggle|save-all|close-all`, `quit`
- `doctor [--json] [--bundle]` asks the daemon, or probes by itself when it is not running.
  It lists each capability's chosen and rejected providers with the reason, the Wayland
  globals, the portal interfaces and the missing tools. When the daemon is not running it
  also says whether a tray watcher and a notification server are on your session bus, the
  versions of `gpu-screen-recorder`, `wf-recorder`, `ffmpeg` (with the video encoders it
  has) and `tesseract` (with its installed languages), and whether your settings file is
  fine, missing, from a newer easyshot, or has an error. `--bundle` writes
  `easyshot-doctor-<time>.tar` to the current directory with the report and your logs, and
  no images. The logs hold file paths, so look through the bundle before you attach it to a
  public report. They never hold text read from your screenshots.
- `daemon` runs the daemon in the foreground.

`easyshot://` links accept only these nine, with no arguments: `capture/region`,
`capture/fullscreen`, `capture/window`, `capture/scroll`, `record`, `ocr`, `color-picker`,
`home`, `settings`. A `record` link asks you to select the area first; it never starts recording the
whole screen.

## Files

| What | Where |
|---|---|
| Settings | `$XDG_CONFIG_HOME/easyshot/config.toml` |
| Screenshots and edit sidecars | `$XDG_DATA_HOME/easyshot/library/` |
| Recordings | `$XDG_DATA_HOME/easyshot/recordings/` |
| Logs and crash reports | `$XDG_STATE_HOME/easyshot/log/`, `crash/` |
| Default save folder | `Screenshots` inside your Pictures folder |
| Saved recordings and video exports | `easyshot` inside your Videos folder |

If a file was made by a newer easyshot, it opens read-only and is never changed.

If easyshot crashes, it writes `crash/<time>.txt` with its version, the features it was
built with, the providers it chose, a backtrace and its last 200 log lines. The next time it
starts, a notification offers "Open crash report", which opens the file in your default
text viewer. The report stays on your machine; nothing is uploaded.

When a save or capture notice shows as a desktop notification, it says where the file went.
Clicking it opens the file; it also offers Show in folder, which asks your file manager to
show the file (or opens its folder), and a capture's notice offers Edit, which opens it in
the editor. Whether the buttons show depends on your notification server.

## When your compositor lacks something

| Missing | What you get |
|---|---|
| Window capture | The window's rectangle is cropped from the frozen screen. |
| Window geometry | Window mode shows a list of windows instead of hover highlight. |
| Virtual input | No Auto Scroll; manual scroll capture still works. |
| File picker portal | Export saves to your save folder and tells you where. |
| Tray | The capture bar opens when the daemon starts, so you can still reach easyshot. |
| Recorder or OCR tools | Those items are disabled with an install hint. |

`easyshot doctor` shows what applies to you.

For compositor window rules, the Home window uses the app ID `dev.easyshot.app`, the same
as the desktop entry, and easyshot's other windows use `dev.easyshot.app.editor`,
`.onboarding` and `.player` (the trim viewer).

## Limits in version 1

KDE and GNOME, the video Studio (timeline, zooms, cursor effects, captions, camera
layouts), camera recording, the teleprompter, cloud sharing, a built-in updater and
Flatpak are not part of version 1.

## Troubleshooting

Start with `easyshot doctor`, and set `EASYSHOT_LOG` to change
what is logged.

If the pointer disappears when you press Space in the editor and comes back when you move
the mouse, your compositor is hiding it because you pressed a key. On Hyprland that is
`cursor:hide_on_key_press`; on sway it is `seat * hide_cursor when-typing enable`. Turn it
off to see the hand while you pan.
