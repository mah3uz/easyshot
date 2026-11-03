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
- Tesseract for reading text (OCR). Without it, OCR still reads QR codes; when the region
  has none, it shows "Text recognition needs Tesseract" and how to install it.
- gpu-screen-recorder or wf-recorder for recording; easyshot uses gpu-screen-recorder when
  both are installed. Without one, the tray's Record item is greyed out and reads "Record (install wf-recorder)", the
  capture bar's Recording button is greyed out with the same hint, and every other way to
  start a recording (shortcut, `easyshot record`, link) answers at
  once with "No screen recorder is installed" and the install hint, before anything is
  selected or frozen.

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
  codes, which works without Tesseract.
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

The editor remembers your last style: the colour, stroke width, arrowheads and line style,
text options, strength, the Highlighter's mode and the armed tool come back the next time an
editor opens. A change is saved a moment after you make it; editors that are already open do
not change when another one saves, and the last change you made wins. Reset to defaults, at
the bottom of a tool's options, puts that tool's options back to their defaults and saves
them. If the style file was made by a newer easyshot or cannot be read, the editor starts
from the defaults and leaves the file alone.

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

A screenshot over 40 megapixels, or with a side over 16384 px, opens with a low-resolution
preview: the editor shows a copy shrunk to at most 8192 px on its long edge and 32 megapixels,
so it stays fast. A small info button in the footer explains this. Only the picture on screen
is smaller. Annotations keep their exact positions, and Save, Export, Copy, Crop, Smart
Redaction and the blur and pixelate shapes work on the full image. Settings > General > Always
use a low-resolution preview shows every screenshot at half size the same way; it applies to
editors already open as well as new ones.

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
`easyshot://settings` link open it on Settings, at that page when you name one: `home`,
`general`, `capture`, `overlay`, `recording`, `shortcuts` or `about`. There is
only one Home window: opening it again brings it to the front and switches to the tab you
asked for, or keeps the tab it shows when you asked for none.

## History and gallery

Captures are kept in a history with retention, search, sorting, icon and list views, and a
trash. The default limit is 100 entries.

## Recording

A capture bar and recording picker start a recording of an area, a display or a window.
System sound is recorded unless you turn it off in Settings, Recording; the microphone is
off until you choose one there. There is a countdown, and then a control bar with stop,
pause and resume, restart and discard. The recorded area is highlighted. Without
gpu-screen-recorder or wf-recorder, nothing starts: you get a notice that names both and the
Arch package to install.

wf-recorder records one audio track. With it, system sound is the default output's monitor, and
a recording with a microphone, or with no default output found, is made without system sound;
easyshot says so when it starts.

The capture bar opens with `easyshot capture-bar`, its key, if you set one in Settings > Shortcuts, or the
tray; opening it again by any of these closes it. While it is open it has the keyboard, so
Esc closes it, or closes its open menu first.

One recording runs at a time. While it runs, the tray's Record item reads Stop Recording,
and a second start from a shortcut, the command line, an `easyshot://` link or the capture
bar shows "A recording is already running" and opens no picker.

Each recording is a project folder in `$XDG_DATA_HOME/easyshot/recordings/`. A finished
recording appears as a deck card, then opens in a trim viewer with export. Saving the card
and exporting from the trim viewer both write to `easyshot` inside your Videos folder
(usually `~/Videos/easyshot/`), with a dated name that never replaces an earlier file.

With Settings, Recording, Keystrokes on, the folder also gets an `input.json` with your
clicks and shortcuts, timed on the video's clock with pauses removed (keys to the nearest
hundredth of a second), for click effects and key badges such as "Ctrl+C". Each click has its
button and, where easyshot can read the pointer (Hyprland, or a compositor with the cursor
capture protocol), its place in the recording, and a click outside the recorded area is left
out; elsewhere every click is kept, without a place.
A key is saved only when, under your keyboard layout as the compositor sends it, it types no
text: shortcuts such as Ctrl+C, and special keys (Enter, Esc, Tab, Backspace, Delete, arrows,
Home, End, Page Up and Down, Insert, F1 to F12). Apart from Enter and Tab, a key that types a
character is never saved, whatever is held with it: plain typing, Shift, AltGr or any other third-level key, a second
layout, dead keys and Compose, Alt or Super with a letter, and Ctrl+Shift+U Unicode entry.
Until easyshot has your layout, only special keys are saved, and keys pressed as your layout
changes are left out. Each keyboard counts its own modifiers and starts from its Num Lock and
Caps Lock lights. Nothing is read while your session is switched away from or locked, or when
easyshot cannot ask logind. A lock is seen on Hyprland, on niri started as a session, on
desktops that tell logind, and whenever it goes through `loginctl lock-session` (then until
`loginctl unlock-session`); on sway, swaylock started any other way is not seen, and Enter,
Backspace and Esc typed into it are saved. Pointer travel is not recorded: `travel` is always empty. With
Keystrokes off nothing reads your keyboard or mouse and `input.json` is empty; switching it
off during a recording stops reading at once and saves nothing from that recording.
`input.json` stays in the recording's folder under `~/.local/share/easyshot/recordings/`
until you delete that folder; Discard and Start over delete it at once.

Reading the keyboard and mouse needs your user in the `input` group. Run
`sudo usermod -aG input $USER`, then log out and in. Until then a recording with Keystrokes on
still records, without clicks and keys, and says so when it starts. The Keystrokes row in
Settings, Recording shows whether it is off, working, or without access, and `easyshot
doctor` gives the same answer. Right after you join the group they say you were added and
need to log out and in; a session started before that cannot read the devices.

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

Pages: Home, General, Capture, Overlay, Recording, Shortcuts, Agents, About. Agents is
described under Agent use. Sharing is not part of
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
it offers follows what you have typed: the subcommands; the flags of each command, which
include some that a particular mode then refuses; the modes of `capture`, the actions of
`record` and `deck`, the eight pages of `settings`, the shells for `completions`; and for
`open` the `easyshot://` links and the PNG and JPEG files and folders under the path you
have typed. In bash a link completes across its colon. Completion reads no daemon state
and starts no window. To try it without installing, run `source <(easyshot completions bash)`
in bash, `source <(easyshot completions zsh)` in zsh after `compinit`,
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

## Agent use

`easyshot agent` (or `easyshot ai`, the same command) is for AI agents and scripts. Tab
completion offers `agent` and `ai` at the top level, and their subcommands and flags only
after you have typed `easyshot agent ` or `easyshot ai `. Agent commands go to the daemon
over your session bus like every other command; `easyshot://` links never reach them.

Commands:

- `agent list outputs` lists your outputs: name, description, position and size in logical
  pixels, scale, and whether it is focused.
- `agent list windows` lists the open windows: id, app id, title, position and size in
  logical pixels, output, and `focused`, which is true for the window that has or last had
  focus. What your compositor does not report is left out rather than guessed.
- `agent capture` takes a screenshot and prints the path of the PNG. Give exactly one target:
  `--output NAME`, `--geometry "x,y WxH"` (logical pixels, within one output),
  `--window <id|app_id|title>` or `--fullscreen` (the focused output). A window name is tried
  as an id, then as an exact app id, then as an exact title; a name that fits several windows
  is refused with the candidates listed, so name one by its id. The capture is a still frame
  taken at once, cropped to the target, with no selector. It is kept in the library like any
  other capture, and is saved to your folder too when auto-save is on. Secrets in it are
  hidden (see Hiding secrets below).
  - `--out PATH` also writes the PNG to an absolute or relative `.png` path whose folder
    exists.
  - `--delay SECONDS` waits 0 to 60 seconds first.
  - `--no-ui` shows no deck card or notification and runs no after-capture step except the
    save. With `--out` as well, the file is all that is written: nothing goes to the library.
  - `--wait` (the default) returns when the capture is done; `--no-wait` returns once the
    daemon has accepted it.
- `agent export VIDEO` re-encodes a recording with the same options as the trim viewer's
  export and prints the path of the new file. It needs `ffmpeg` and `ffprobe`; a file that is
  not a video they can read is refused, and a missing one is `not_found`.
  - `--format mp4|webm|gif` (default: the type of `--out`, else `mp4`).
  - `--size original|1080p|720p|480p` scales down to that height and never enlarges.
  - `--quality high|balanced|small` (default `balanced`).
  - `--trim A..B` keeps the seconds from A to B, decimals allowed; either end may be left out,
    as in `2.5..8` or `..10`. An end past the video's length is the end of the video.
  - `--look natural|vivid|muted|mono` and `--brightness AMOUNT` (-0.3 to 0.3) change the picture.
  - `--out PATH` writes the file there; its folder must exist and its type must match the
    format. Without it the file is `NAME-export.EXT` beside the video. An existing file is
    never replaced unless you add `--force`.
  - `--wait` (the default) returns when the export is done; `--no-wait` returns once the
    daemon has accepted it. With `--json` the answer is
    `{"ok":true,"kind":"export","path":...,"format":...,"width":...,"height":...,"duration_ms":...,"bytes":...}`.
    It reads no screen, so it needs no more than agent access being on.
- `agent ocr` reads the text and QR codes on a target or in an image. Give exactly one of
  the capture targets (`--output`, `--geometry`, `--window`, `--fullscreen`, read from a
  still frame that is not saved anywhere) or `--file PATH`, a PNG or JPEG. `--lang eng+deu`
  names the Tesseract languages; the default is
  the `[ocr] languages` setting. Without `--json` it prints the text, QR codes first. QR codes are
  read without Tesseract; text needs it, and without it (and with no QR code) the command
  exits 6 with how to install it. With `--json` the answer holds `text`, `lines`
  (`text`, `x`, `y`, `width`, `height`, `confidence`), `qr` (`text`, `x`, `y`, `width`,
  `height`) and `source` (`output`, `window` or `file`); positions are pixels of the image
  that was read.

- `agent record start` starts recording one target and prints the recording's id. Give exactly
  one of `--output NAME`, `--geometry "x,y WxH"`, `--window <id|app_id|title>` or
  `--fullscreen`. It records as you would from the capture bar, with the control bar and the
  tray indicator showing, so you can see that a recording is running and stop it yourself. It
  never touches a recording you started: if one is running the command exits 4.
  - `--audio system|mic|both|none` (default: your recording settings), `--fps 30|60`
    (default: the setting) and `--keystrokes`, which also records clicks and keys for the
    editor, apply to this recording only.
  - `--max-seconds N` (1 to 600, default 120) stops the recording by itself after that time.
    A recording an agent started also stops, and is kept, when its access ends: when the
    grant is revoked or runs out, or when you run `agent disable`.
  - With `--json` the answer is `{"ok":true,"kind":"recording","id":...,"state":"recording","target":...,"max_seconds":...}`.
- `agent record stop [--id ID]` stops it and prints the path of the video: the file in the
  recording's project folder, or the MP4 in your videos folder when the setting that saves recordings is on.
  With `--json`: `{"ok":true,"kind":"recording","id":...,"path":...,"duration_ms":...,"width":...,"height":...,"bytes":...}`.
  After a recording ended by itself, `stop` still gives its path. The width and height are 0
  when `ffprobe` is not installed.
- `agent record status` says `idle` or `recording`, and for an agent's recording its id,
  target, `elapsed_ms` and `max_seconds`. `agent record discard [--id ID]` stops it and
  deletes it. Both `stop` and `discard` answer `not_found` (exit 5) when no recording
  started by an agent is running.

Without `--json` each output or window is one line of tab-separated text, with geometry
written `x,y WxH`. With `--json` (before or after the subcommand) easyshot prints exactly
one JSON object on standard output, errors included:

```json
{"ok":true,"kind":"outputs","outputs":[{"name":"DP-1","description":"Dell U2720Q","x":0,"y":0,"width":2560,"height":1440,"scale":1.5,"focused":true}]}
{"ok":true,"kind":"capture","path":"/home/you/.local/share/easyshot/library/easyshot_2026-11-03-15-29-54.png","width":480,"height":300,"scale":1.5,"output":"DP-1"}
{"ok":false,"kind":"windows","error":{"code":"unsupported","message":"this desktop gives easyshot no window list"}}
```

A capture answer gives the size in pixels, not logical pixels, and `scale`, and `redaction`
(see Hiding secrets); a window capture adds a `window` object like an entry of
`agent list windows`.

`kind` is `outputs`, `windows`, `capture`, `export`, `ocr`, `recording`, `access` (enable, disable and status), `config`, or `usage` when the command line itself could not be read.
Fields may be added to these objects in later versions; existing ones keep their meaning.

Exit codes and error codes:

| Exit | `error.code` | Meaning |
|---|---|---|
| 0 | | done |
| 1 | `failed` | the command failed |
| 2 | `cancelled` | cancelled, for example with Ctrl+C |
| 3 | `no_daemon` | the daemon is not running, could not be started or went away |
| 4 | `invalid_arguments` | the command line or its arguments are wrong, or the daemon is from a different easyshot version |
| 5 | `not_found` | the output, window or area asked for does not exist |
| 6 | `unsupported` | your compositor gives easyshot no output list, window list or capture |
| 7 | `access_off` | agent access is turned off |
| 8 | `access_denied` | agent access was refused |

`agent capture` reads your screen: it takes the one still frame you asked for, and nothing
watches it. `agent record` records until it is stopped, and it is the one agent command that
keeps running after it answers.

### Hiding secrets

While `redact_secrets` is on (the default), an agent never receives a screenshot with secrets
in it. Before `agent capture` hands over the file, easyshot reads the image with Tesseract
and pixelates what looks like a secret: API keys and tokens, passwords and other labelled
values, emails, links, IP addresses, phone numbers and card numbers, the same text Smart
Redaction covers in the editor. The `--out` file and the file in the library that the answer
points to are the covered image. When the capture also goes to the deck (without `--no-ui`),
the deck card and your library copy keep the original, and the agent's file is a covered copy
kept apart from them. The answer says what happened in `redaction`: `{"applied":true,"regions":2}`,
`{"applied":false,"reason":"off"}` when you turned the setting off, or
`{"applied":false,"reason":"tesseract_missing"}`.

Without Tesseract the capture still succeeds but is not covered, and easyshot tells you once
per daemon session how to install it. This is a best effort: text Tesseract cannot read, such
as small, low-contrast or rotated text, dots in a password field and secrets in pictures, is
not covered. `agent ocr` returns the text it reads without hiding anything.

### Turning agent access on

Agent access is off until you turn it on. While it is off, every agent command except the
ones below exits with status 7 and says how to turn it on. These always work:

- `agent enable` turns access on once you click Allow in the "Turn on agent access?" dialog
  easyshot shows; Cancel leaves it off and the command exits with status 8. `agent disable`
  turns it off and ends every grant.
- `agent status` shows whether access is on, the consent setting, the grant in force and
  when it ends, the outputs and windows agents may capture, whether secrets are hidden, and
  how many recent actions are logged. `--json` gives the same as one object.
- `agent config get <key>` and `agent config set <key> <value>` read and change one setting
  of the `[agent]` section of `config.toml`: `enabled` (`true` or `false`), `consent`,
  `allowed_outputs` and `allowed_windows` (comma-separated output names and app ids; empty
  means all), `redact_secrets` (`true` or `false`) and `log_limit` (1 to 10000). Setting
  `enabled` to `true`, `consent` to `always-allow`, or a scope that allows more than before
  shows the same dialog first; Cancel exits with status 8 and changes nothing.

Settings › Agents has the same settings, and changes there need no extra click.

Only your own user can send agent commands: easyshot asks the session bus which user sent
each one and refuses any other user with status 8. Nothing is ever sent over the network.

### Consent

`consent` decides when easyshot asks you before it answers an agent:

- `once-per-session` (the default): the first request asks; you can allow it for 1 hour,
  until you log out, or always, or deny it. Requests that arrive while easyshot is asking
  wait for the same answer.
- `always-ask`: every request asks, and an answer covers that request only.
- `always-allow`: nothing is asked.

"Always" is kept in `$XDG_STATE_HOME/easyshot/agent.json` and survives a restart; the other
grants end when the daemon stops. `agent disable` ends every grant, "Always" included.

The consent dialog appears above every window on the screen under the pointer and names the
program (its process name and id) and what it asks for. Deny is first; Esc or closing the
dialog denies, and so does leaving it unanswered for 60 seconds. Enter does nothing until you move to a button with Tab, so text being typed
when the dialog opens cannot answer it. While it is open, the keyboard goes to the dialog.
If the dialog cannot be shown, the request is denied.

Settings › Agents shows the grant in force with a Revoke button, and the recent actions
(time, program, what it asked for and the result) as they happen. Times are in UTC.

### Scope

`allowed_outputs` limits agents to those outputs: an agent command whose target reaches any other output is
refused with status 8, and `agent list outputs` shows only those outputs. `allowed_windows`
limits agents to windows with those app ids: only commands that target those windows are
allowed, and `agent list windows` shows only those windows. Both are empty by default, which
allows everything.

### The log

easyshot logs each agent request it answers or refuses, with the time, the command, its
target, the result and the program that sent it, newest first, in
`$XDG_STATE_HOME/easyshot/agent.json`. It keeps the last `log_limit` entries (200 by
default). `agent status` and `agent config get` are not logged.

## Files

| What | Where |
|---|---|
| Settings | `$XDG_CONFIG_HOME/easyshot/config.toml` |
| Last annotation style | `$XDG_CONFIG_HOME/easyshot/annotation-style.json` |
| Screenshots and edit sidecars | `$XDG_DATA_HOME/easyshot/library/` |
| Recordings | `$XDG_DATA_HOME/easyshot/recordings/` |
| Logs and crash reports | `$XDG_STATE_HOME/easyshot/log/`, `crash/` |
| Agent grant and log | `$XDG_STATE_HOME/easyshot/agent.json` |
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
| Recorder | The tray's Record item is disabled with an install hint; other ways to record answer with the hint at once. |
| Tesseract | OCR reads QR codes only; a region without one shows a notice that text recognition needs Tesseract. |

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
