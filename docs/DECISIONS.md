# Decisions

Why easyshot is built the way it is. One section per decision, with what was rejected and
why. Background and sources are in `PRIOR-ART.md`. A reversed decision is removed; the one
that replaces it says so. "Reason not recorded" means no reason was written down at the time.

## Architecture and tooling

### GPUI, with vendored patches
Stack: Rust stable (pinned to 1.98.1) and GPUI from longbridge/gpui-kit's crates, declared
explicitly: `gpui-pre =0.3.10`, `gpui-pre-platform =0.3.10`, `gpui-component =0.8.1`,
`gpui-kit-assets =0.8.1`. `gpui-component` pins `gpui-pre` exactly, so `just gpui-guard`
fails if two gpui copies appear. `vendor/gpui-pre/` and `vendor/gpui-pre-linux/` are copied
as-is from crates.io and our patches are git commits on top, so the patch set is the diff.
Alternatives rejected: not recorded.

### Layered crates with enforced direction
Layers 0 to 3, with `just layers` failing on an upward edge or on a GUI, tokio, zbus,
wayland or ashpd dependency where it does not belong. Why: duplicated
stores, god files and singletons do not scale, and easyshot is meant to grow across
compositors and distros. Rejected: a flat workspace without enforcement.

### One owner per concern, one renderer
One writer per file format (`es-store`), one renderer (`es-render`, tiny-skia), one
shortcut catalogue, one route vocabulary. Why: duplicated stores drift apart. Rejected:
separate stores and renderers per surface. Reason for a single renderer beyond that: not
recorded.

### Provider registry per capability
Each capability has an ordered list of providers chosen at runtime by probing; the app sees
only `Caps`. Generic `wayland::*` providers are chosen by protocol globals, and compositor
modules add only what protocols cannot give. Why: a new compositor is a new directory and
one `cfg` line per list, not a change in the app. Rejected: compositor branches in app code.
`just grep-guard` enforces it.

### No cargo-deny, cargo-machete or cargo-msrv for now
User simplification, 2026-10-31. The duplicate-crate check is `cargo tree -d` in `just
gpui-guard`. Licence and advisory checks arrive with packaging. The toolchain is pinned and
dependencies are declared up front, so MSRV and unused-dependency tools are not needed.

### Lean dev builds
`[profile.dev]` has `debug = "line-tables-only"` and `incremental = false`; dependencies have
`debug = false`; `[profile.dbg]` restores full debug info. Why: every test binary links GPUI
and wgpu, about half of each was debug info, and the debug target reached 483 GB; relinking
after a change wrote gigabytes and stalled the desktop on disk I/O. Incremental output is not
cached by sccache, so each worktree recompiled every workspace crate. Rejected: a shared
target dir across worktrees (Cargo's unit hash drops the workspace path, so worktrees collide),
`split-debuginfo = "unpacked"` (unverified with sccache), and a build I/O throttle (user's
choice).

### `es-testkit` is dev-only
The shared headless-window helper is a layer 3 crate that is never a normal dependency.
Rejected: duplicating the helper per crate. Reason not recorded beyond that.

### Fluent for strings
`i18n-embed` and `i18n-embed-fl`, with a `.ftl` file per layer-3 crate and compile-time
checked `fl!` ids. Why: strings are data, so `just i18n-check` can assert every required
id exists, and plurals come from Fluent. Rejected: strings in code. Other catalogue formats:
reason not recorded.

### Recovery journal
The editor writes a journal after each committed undo entry (debounced 2 s) and reopens
each journal at startup. Why: one tested path covers SIGTERM, panic, OOM and power loss.
Rejected: a SIGTERM-only snapshot.

### Entry points are closed vocabularies
The `easyshot://` URL scheme accepts exactly nine routes with no arguments;
D-Bus `Command` parses into the closed `Route`; the session bus only; directories 0700 and
files 0600. Why: a URL or bus call from outside must not be able to do more than those
nine routes allow. A `record` link opens the area selection, so a link cannot start a
recording on its own. Rejected: accepting the full route vocabulary, or arguments, over URLs.

### Compositor targets and providers
v1: Hyprland (primary), sway 1.12 and niri 26.04, plus any compositor that passes the
generic providers. river, labwc and COSMIC come after v1; KDE and GNOME later still.
Rejected for v1: KDE and GNOME backends.

### Distro packaging replaces a self-updater
Releases for several distros; no self-updater and no Flatpak in v1. Reason not recorded
beyond that distro packaging replaces the updater.

### MIT licence
easyshot is MIT and written as its own Rust code; ideas and algorithms are ported, not
code. `LICENSES/` lists only what easyshot ships or links, the Rust crates through
`cargo about` (`just third-party`).
Rejected: a clean-room rewrite of those parts. Reason not recorded.

### Own types for niri's JSON IPC
`es-backend` decodes the niri socket with its own serde types for the few requests, replies
and events it uses, written from niri's documented JSON wire format. Events it does not
track fail to parse and are skipped.
Rejected: the `niri-ipc` crate, which is GPL-3.0-or-later and would make the default MIT
binary a GPL work.

## Desktop integration

### One app ID, `dev.easyshot.app`
The D-Bus name, the desktop entry, the unit and the window IDs
(`dev.easyshot.app.{editor,settings,gallery,onboarding,player}`) all derive from the one
constant in `es-protocol`; `scripts/app-id.sh` reads it for packaging. Why: an ID that
disagrees between the bus name, the desktop entry and the window class breaks single
instance, icon lookup and compositor window rules. Rejected: separate IDs per file.

### Tray icon is a full-colour pixmap with an empty icon name
The tray item sends its icon as an ARGB pixmap and an empty `IconName`. Why: Qt-based
status-bar hosts draw a themed symbolic icon that uses `currentColor` in black, which made
the icon invisible on dark bars. Rejected: a named symbolic icon from the theme.

### The pointer is never hidden on key presses
GPUI's `CursorHideMode::Never` is set at startup. Why: with the default, typing Space or a
tool letter hides the pointer until the mouse moves, which is wrong in an editor where
those keys are part of a pointer gesture. Rejected: the default hide-while-typing mode.

### Package install enables and starts the user service
The Arch install script runs `systemctl --global enable easyshot.service`, then starts the
service for each logged-in user on install and `try-restart`s it on upgrade; removal stops it.
The unit carries `ConditionUser=!@system`. Why: the tray and `easyshot://` links need the
daemon, and an old daemon would keep running old code after an upgrade. The global enable is
what Arch's own pipewire, wireplumber and xdg-user-dirs packages do, but it also reaches a
display manager's greeter, which runs as a system user; easyshot is the only one of these
that shows windows, so the condition keeps it off the login screen (the guard bluez's
`mpris-proxy` and `fluidsynth` use). The upgrade restarts only a running daemon, so a user who
stopped it is not overridden. Rejected: leaving the user to enable it (sysman's approach),
`enable --now` for one user only, which misses the other users, and XDG autostart
(insensical's approach), which plain Hyprland, sway and niri ignore.

### The daemon starts in the tray
`show_capture_bar_at_launch` is off by default, and the daemon opens the capture bar at start
only when it is on or no tray watcher was on the bus. Why: the daemon starts at login, and a
bar on every login is noise; the launcher and commands open their own window. Rejected:
telling a login start from a launcher start, which D-Bus activation through systemd hides.

## Capture and recording

### Freeze is always on
Captures always freeze the screen; there is no native-selector option and no config key.
Rejected: a native-selector option. Reason not recorded.

### A timed capture freezes only while you pick
A timed capture is pick, then countdown, then a fresh frame of the picked target: the area
on its output, the window by its id, or the display. The frozen frame is used for the pick
alone, so freeze still holds for every pick, and what the countdown waits for (an open menu,
a hover) is in the shot. Rejected: countdown first, then a frozen pick, which made a timed
capture only possible for areas and could not catch a menu that closes when you click.

### `capture previous` repeats the last target
The state file keeps the last target (area, window id with its app and title, or display)
next to the area the region selector starts from; format version 2 moves a version 1
`last_region` into it. A window that has closed gives a toast and no capture. Rejected:
falling back to the window's last rectangle, which would capture whatever is there now.

### KMS recording without `no_screen_share`
The recording bar cannot be hidden from the video by a window-sharing flag on KMS, so easyshot
does not rely on `no_screen_share` for it (item 9 below).
What was rejected beyond that, and why: not recorded.

## Deliberate behaviour choices

- **One gate for every recording start.** `Recordings::admit` refuses while a recording runs
  or one is starting (between the request and the recorder running, countdown and area pick
  included). The capture coordinator asks it before selecting anything, so a refused `record`
  route never freezes the screen or opens a picker. Rejected: checking only when the recorder
  starts, which let the picker and countdown run first; making a second `record` stop the
  running one, which turns a mistaken press into a lost take.
- **Videos go to the XDG videos folder.** Saved recordings and trim exports are written to
  `easyshot` inside the Videos folder with dated, never-overwriting names; project folders
  stay in the data directory. Rejected: the screenshot save folder, which put videos among
  pictures; a fixed `trimmed.mp4` beside the project, which no one finds and each export
  overwrote.
- **Recording defaults: system sound on, microphone off.** A silent recording surprised the
  first user; the microphone stays opt-in.
- **The editor's inspector is on the right.** Its toggle sits at the right end of the tool
  row, directly above it, and Tab reaches it after the tool row. Rejected: the reference
  design's left column, which the user did not want.
- **Canvas shape lives in the Background section.** A fixed shape grows the stage around
  the card, so it sits with padding, and picking one while padding is 0 sets the default
  padding so the change shows. It is off while nothing puts the card on a stage. Rejected:
  a ratio menu in the tool row, which did nothing visible without padding and confused the
  first user.

1. **Undo.** No entry is pushed for a press that changes nothing: `mark_undo` is deferred
   until the first mutation of the gesture. Typing in a text shape creates one undo entry
   per editing session. Rejected: an entry per press. Reason not recorded.
2. **Arrows.** Bound arrows sync `props.start` and `props.end` on every resolve, so export
   with `bindings: []` stays correct. Reason not recorded.
3. **Dashes.** Patterns are fitted per segment.
4. **Stores.** One history store and one preference set. Rejected: duplicated
   stores.
5. **Capture.** Freeze is always on; there is no native-selector option.
6. **Style-slider undo.** A drag on an annotation style slider (stroke, strength, font
   size) makes one undo entry per drag. Rejected: one entry per style-apply
   call. Beautify sliders stay non-undoable.
7. **Deck staging survives restarts.** Items younger than 24 h are restored at start,
   older ones purged. Reason not recorded.
8. **Recovery journal** for dirty editors, covering SIGTERM, crash and power loss (see
   above).
9. **Recording controls during a full-display recording.** The bar cannot be hidden from
    the video by a window-sharing flag, which has no KMS equivalent.
10. **Affordance cursors** (maintainer's decision, 2026-10-31): resize, rotate, endpoint,
    bend, crop and pin cursors, and an arrow during marquee.
    See `DESIGN.md`.
11. to 19. **Ergonomic additions** (arrow-key nudge, Esc stepping back (item 28), middle-button and
    Space-hold pan, pointer-anchored zoom, Duplicate, copy and paste of shapes, z-order
    keys, Tab cycling, shell robustness). They add behaviour and never new chrome, so the
    interface stays identical to the reference design. Rejected: snapping guides, an eraser tool
    (a 13th button), Alt-drag duplicate (Alt already means centre-resize), region selection
    across outputs, numeric geometry fields. See `DESIGN.md`.
20. **Resizing through zero** (maintainer's decision, 2026-11-02): dragging a resize handle
    past the opposite edge flips the shape so it keeps following the pointer, and a curved
    arrow mirrored on one axis reverses its bend. Rejected: keeping the scaled origin, which
    leaves a flipped shape on the wrong side of the pointer.
21. **Shell completion asks the binary.** `easyshot completions <shell>` prints a small
    registration script, and each Tab runs `easyshot` with `COMPLETE=<shell>` (clap_complete's
    `unstable-dynamic` engine, pinned by `Cargo.lock`). Values come from the same
    definitions the parser uses (`SettingsPage::ALL`, the `ValueEnum`s, the `easyshot://`
    allowlist), so the candidates cannot drift from the commands. The engine runs before
    anything else in `main`, with no GPUI and no daemon. Rejected: static scripts generated
    at package time, which cannot filter `open` to image files or list its URLs, and go
    stale against the installed binary.
22. **No default global shortcuts** (maintainer's decision, 2026-11-03): every global action
    starts with no key, and the Settings > Shortcuts snippet holds only keys the user chose.
    Editor keys keep their defaults. Rejected: Super+Shift+1 to 5, which clashed with users'
    own Hyprland binds.
23. **One Home window for the gallery and Settings** (maintainer's decision, 2026-11-03):
    the launcher, a bare `easyshot`, `easyshot gallery` and `easyshot settings [page]` all
    open one window with a Media Gallery tab and a Settings tab, and a second open focuses
    it and switches the tab only when one was named. `Route::Home` joins the closed
    vocabulary and `easyshot://home` the URL allowlist, with no arguments, because opening a
    window can do nothing a link should not. The window takes the app's own ID, so the
    desktop entry that launches it is the one compositors and docks associate it with. The
    tabs are a segmented control above both panes, which keep their own sidebars unchanged.
    Rejected: two separate windows, which the launcher could open only one of; a `home`
    subcommand, since a bare `easyshot` says it already.
24. **Tool options in the inspector** (maintainer's decision, 2026-11-04, replacing a panel
    under the active tool from 2026-11-03): colour, stroke, arrowheads, line style, text
    style, strength, highlighter mode and crop's ratio, Reset, Cancel and Apply are a page of
    the inspector with a back arrow, in labelled sections. Picking a tool shows it and opens
    a hidden inspector; clicking the armed tool keeps it instead of returning to Select.
    Rejected: the panel hung under the tool's button, which covered the canvas and hid what
    you were drawing on. Rejected: the style strip in the tool row, which had no room for
    arrowheads and line style, and crop's buttons in the title bar, far from the crop
    button.
25. **A cursor per tool family** (maintainer's decision, 2026-11-03): a cross for shapes and
    arrows, a pen for freehand and the highlighter, an I-beam for text, a badge for the
    counter, a dotted square for Highlight, Pixelate and Blur. Rejected: one plus for every
    drawing tool, which did not say what a press would make.
26. **The whole-screen control bar goes elsewhere or is recorded; no hide rules** (maintainer's
    decision, 2026-11-03): with a second monitor that is not recorded the bar goes there; with
    one monitor it sits in the bottom-right corner of the recorded screen and appears in the
    video, to be cropped afterwards; one switch in Settings › Recording turns it off. Rejected:
    compositor hide rules. Hyprland's `no_screen_share` layer rule paints a black rectangle
    over the bar in the video rather than removing it, niri's `block-out-from` needs
    `screen-capture` to cover wf-recorder, sway has no rule, and gpu-screen-recorder reads the
    scanout buffer, which bypasses every compositor rule. Rejected: shipping rules for some
    compositors only, which would make the same recording differ by compositor.
27. **easyshot keeps its own binds file and never edits the compositor config** (maintainer's
    decision, 2026-11-03): `binds.lua` or `binds.conf` (Hyprland, by which config exists),
    `binds.conf` (sway), `binds.kdl` (niri), in `~/.config/easyshot/`, rewritten atomically
    through `es-store` whenever a global key changes; global keys persist in `[global_shortcuts]`.
    The user adds one include line; Settings checks the main config and files it includes
    directly. The file is created empty first, and the niri line is `optional=true`, so a
    missing file cannot break the compositor config. Keys are `Mod4`, not `$mod`, because an
    included file cannot rely on the variable. Rejected: editing the compositor config, and a
    paste-in snippet, which was lost when the window closed.
28. **Esc steps back one level per press** (maintainer's decision, 2026-11-03): finish the
    text being typed, then leave crop, then clear the selection, then switch to the Select
    tool. Rejected: Esc only clearing the selection, which left the first user stuck in a
    drawing tool and in crop.
29. **The mouse wheel zooms** (maintainer's decision, 2026-11-03) at the pointer; touchpad
    two-finger scroll pans; pinch and Ctrl+wheel zoom too. Rejected: the wheel panning with
    Ctrl or Alt to zoom, which made a mouse user hunt for the modifier.
30. **Annotations stay on the image** (maintainer's decision, 2026-11-03): drawing starts only
    on the image, and drawing, moving and resizing stop at its edge. Rejected: a free canvas,
    where shapes dragged off the picture were cut off or lost in the export.
31. **Video export options in one place** (maintainer's decision, 2026-11-03): format (MP4,
    WebM, GIF), size (Original, 1080p, 720p, 480p, never enlarging), quality (High, Balanced,
    Small), brightness and four looks. Rejected: film grain, which the user did not want.
32. **An auto-saved card offers Show in folder** (maintainer's decision, 2026-11-03) where an
    unsaved one offers Save. Rejected: a Save button on a file already saved, which did
    nothing new.
33. **Highlighter is its own tool** (maintainer's decision, 2026-11-03): translucent colour,
    straight or freehand, key M, M again switches the mode. Highlight (the spotlight) stays
    as it was. Rejected: turning the spotlight into a highlighter, which would remove a tool
    people use for a different job.
34. **The capture bar toggles** (maintainer's decision, 2026-11-03): its key and the tray's
    item open it and close it again, and Esc closes it. Rejected: a second open only
    focusing it, which left no way to dismiss it from the key that opened it.
35. **New text starts at size 24** (maintainer's decision, 2026-11-04, replacing 36 from
    2026-11-03): 36 was too large on everyday captures, the original 20 too small to read on a
    full-screen shot.
36. **easyshot never starts a desktop service by touching it at startup** (maintainer's
    decision, 2026-11-04): at login the daemon can start before the user's own notification
    server has claimed `org.freedesktop.Notifications`, and any call to the unowned name makes
    D-Bus activate whichever installed server it finds, which then keeps the name. The
    notifier sends nothing to the name until it has an owner or a notification is shown, and
    learns the server's capabilities then and whenever the owner changes. If no server runs
    when a notification is shown, activating an installed one is fine. Rejected: a startup
    `GetCapabilities` call, and retrying it after a delay.
37. **Scroll capture starts by picking a window** (maintainer's decision, 2026-11-04): the
    scrolled area is that window and the wheel goes to it; Space switches to drawing a
    region, as for screenshots. Shift-click on the capture bar's Scroll button and
    `capture scroll --region` start on a region. Rejected: a modifier held at the start,
    because a key bind's own modifiers are still down when it fires.
