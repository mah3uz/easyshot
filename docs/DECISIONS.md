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
The `easyshot://` URL scheme accepts exactly eight routes with no arguments;
D-Bus `Command` parses into the closed `Route`; the session bus only; directories 0700 and
files 0600. Why: a URL or bus call from outside must not be able to do more than those
eight routes allow. A `record` link opens the area selection, so a link cannot start a
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

### KMS recording without `no_screen_share`
The recording bar cannot be hidden from the video by a window-sharing flag on KMS, so easyshot
does not rely on `no_screen_share` for it (item 9 below).
What was rejected beyond that, and why: not recorded.

## Deliberate behaviour choices

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
11. to 19. **Ergonomic additions** (arrow-key nudge, Esc deselects, middle-button and
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
