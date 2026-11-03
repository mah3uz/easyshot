# easyshot — architecture

Companion to `MANUAL.md` (what it does), `DESIGN.md` (why it behaves so), `DECISIONS.md`
(each choice) and `PRIOR-ART.md` (sources).

easyshot is a screenshot, annotation and screen-recording app for Linux/Wayland.
Rust, GPUI for the interface, tiny-skia for rendering. Hyprland, sway and
niri are the v1 compositors.

## 1. What has been run where

The libraries, overlays, editor, shell windows, CLI and daemon are built, and the app runs
end to end. Only what the table lists has been observed; everything else in this document is
design that has not been run on that compositor. Rows name the compositor and date.

| What | Where it ran | Date |
|---|---|---|
| GPUI layer-surface margin, custom cursors at scale 1 and 1.5, drag icon (`es-ui` cursors example) | headless sway 1.12 | 2026-11-01 |
| Wayland probe, output capture at scale 1 and 1.5 (ext-image-copy-capture and wlr-screencopy give identical pixels), output hot-plug events, window capture (`es-wayland`) | headless sway 1.12 | 2026-11-02 |
| Window list with open, focus and close events; clipboard serving PNG, text and uri-list over ext and wlr data-control (read back with `wl-paste`); virtual-pointer scroll; cursor session positions (`es-wayland`) | headless sway 1.12 | 2026-11-02 |
| Backend detection picks ext-image-copy-capture; sway window rects from `GET_TREE` match the captured window size; the editor `for_window` rule floats the window (`es-backend`) | headless sway 1.12 | 2026-11-02 |
| Layer-shell surfaces (overlay, toast, pin) on the right output at scale 1.5, scale gate, pin moved by margins, no decoration padding after GPUI patch P7 (`es-app` platform_ui) | headless sway 1.12 | 2026-11-02 |
| Region selector on a frozen output at scale 1.5: a drag returns an edge-rounded crop of the exact physical size (`es-overlays` capture) | headless sway 1.12 | 2026-11-02 |
| Window mode over a floating window at scale 1.5 returns its 1200×900 frame; the colour picker returns the pixel under the pointer, matching a grim sample (`es-overlays` capture) | headless sway 1.12 | 2026-11-02 |
| Scrolling capture of a terminal scrolled by wheel input: the stitched 948×4230 image holds each visible line once, in order (`es-overlays` scroll, `es-scroll`); automatic scrolling not run on real content | headless sway 1.12 | 2026-11-02 |
| Deck of three cards at scale 1.5 with hover tools painted, timers paused while hovered and resumed when the pointer leaves for a pin; a pin opens centred, resizes from its held top-left corner and follows a 150×100 drag exactly, with rounded corners (`es-overlays` deck, pin) | headless sway 1.12 | 2026-11-02 |
| Recording control bar with a fake recorder at scale 1.5 is placed below the recorded area, or above it when there is no room below (`es-overlays` record) | headless sway 1.12 | 2026-11-02 |
| The daemon in test mode at scale 1.5: `capture/region` sent over a private bus, a 304×151 drag, and the 456×227 PNG lands in the save folder with one deck card shown; the card's Edit opens an editor window (`easyshot` daemon, `es-app` pipeline) | headless sway 1.12 | 2026-11-02 |
| The daemon in test mode with a fake recorder at scale 1.5: `record/area`, a dragged area, the control bar placed outside it, `record/stop`, then a video card and a project folder (`easyshot` daemon recording) | headless sway 1.12 | 2026-11-02 |
| The daemon in test mode at scale 1.5: a window shot of a floating terminal, then `capture/previous` after the terminal moved saves the same 900×600 window again; after the terminal closed, `capture/previous` shows the toast and saves nothing (`easyshot` daemon capture) | headless sway 1.12 | 2026-11-03 |
| Headless GPUI windows (hello, component gallery) | no compositor (GPUI headless renderer) | 2026-11-02 |
| First run in a real session: the daemon started by its systemd user unit, the tray icon registered, a region capture, the editor and Settings. Found and since fixed: editor keys unbound, theme setting ignored, tray icon invisible on Qt bars, stuck Space pan, pointer hiding and cursor flicker. The fixed build is not re-verified | Hyprland (first run, partial) | 2026-11-03 |

Not yet run: niri (its backend module is tested only against recorded replies and fake
sockets), any other compositor, the packages installed from the AUR; rotated outputs; 10-bit
formats; wlr-screencopy older than version 3.

## 2. Design rules

easyshot avoids duplicated stores, god files and singletons, and is built to grow across compositors, desktops and distros:

1. Layered crates, with dependency direction enforced in CI.
2. One owner per concern: one writer per file format, one renderer, one shortcut
   catalogue, one route vocabulary.
3. Capability providers, not compositor branches.
4. Small entities and typed events in the UI. No god models and no service singletons.
5. Every persisted format is versioned and migratable, writes are atomic, and the user's
   hand edits are respected.
6. Every user-visible string comes from a catalogue, and every control is accessible.

## 3. Process model

One binary, `easyshot`.

- `easyshot daemon` is a long-running GPUI application (`QuitMode::Explicit`). It owns the
  tray, the D-Bus name `dev.easyshot.app` (the single-instance lock), every window and
  overlay, the capture and recording engines and the clipboard sources. A systemd user unit
  (`easyshot.service`) or XDG autostart starts it. At start it opens and closes a 1x1
  transparent layer surface so the first real overlay avoids the cold `open_window` cost.
- `easyshot <command>` is a thin CLI with no GPUI init. It calls
  `dev.easyshot.app.Command(route, args)` on the session bus. If the name is not
  owned it starts the daemon (D-Bus activation, or a spawn fallback), waits up to 3 s, then
  calls.
- `easyshot open <url>` handles `easyshot://`.
- Global shortcuts on Hyprland and wlroots are the user's own compositor binds calling the
  CLI.

`Command` returns a `job_id` at once. The CLI waits on `JobFinished(job_id, status,
result)` with no timeout; Ctrl+C calls `Cancel(job_id)`. `JobFinished` is sent only to the
connection that issued the command, and only that connection may cancel the job. Routes are
parsed off the bus executor; `open` reads the file through a non-blocking, no-follow handle
that must be a regular file. If the daemon disappears the CLI exits with code 3. The bus is the session bus only; directories are created 0700 and files
0600.

Routes are one closed vocabulary (`es-protocol::Route`) shared by the CLI, URLs, tray,
D-Bus, shortcuts and notifications. The URL scheme accepts exactly nine routes with no
arguments (`capture/region`, `capture/fullscreen`, `capture/window`, `capture/scroll`,
`record`, `ocr`, `color-picker`, `home`, `settings`). `home`, `gallery` and `settings`
open the one Home window (`es-shell::home`) on its tab. D-Bus `Command` rejects anything outside the
closed `Route`; `open <path>` canonicalises the path and requires a regular image file.

Every recorder helper runs in its own process group beside a `/bin/sh` watcher in that
group. The watcher reads a pipe only the daemon writes: a newline after the helper is reaped
ends it; end of file means the daemon died, so it sends SIGINT to the group and SIGKILL ten
seconds later. A parent-death signal is not used because the kernel drops it on exec of a
binary with file capabilities, as gpu-screen-recorder often is. gpu-screen-recorder's IPC
socket lives in a fresh 0700 folder.

## 4. Crate graph

Arrows point only downward. `just layers` enforces it.

```
Layer 0  pure data — no IO, no GPUI, no tokio/zbus/wayland
  es-geom      Point/Size/Rect/Affine (kurbo), Rgba, BezPath; geometry, paths, dash, freehand, arrows
  es-model     serde schemas for every persisted format with version envelopes and migrations;
               pure policies after_capture_plan() and resolve_new_capture_look()        → es-geom
  es-protocol  Route + RouteArgs, SettingsPage, JobStatus, URL allowlist parser, D-Bus names,
               the shortcut catalogue
Layer 1  pure compute
  es-text      bundled fonts, cosmic-text layout, glyph outlines                         → es-geom
  es-anno      editor engine: state machine, undo, hit-test, crop, DrawList build        → es-geom, es-model, es-text
  es-render    tiny-skia DrawList painter, effects, beautify composer, image ops, export → es-geom, es-model, es-anno
  es-scroll    scroll stitching on plain BGRA frames                                     → es-geom
  es-ocr       tesseract runner, QR (rxing), smart-redaction rules                       → es-geom
Layer 2  IO and system mechanisms — no GPUI
  es-store     the only writer of every file: atomic writes, config (toml_edit), state,
               history index, deck staging, presets, sidecar, retention, naming          → es-model
  es-wayland   our own wayland-client connection: registry probe, outputs, capture
               protocols, foreign-toplevel, data-control clipboard, virtual pointer       → es-geom, es-model
  es-desktop   freedesktop over D-Bus: Command service/client, tray, notifications,
               portals, trash, autostart, sound (feature)                                → es-protocol
  es-record    recorder providers, audio devices, input streams, project folder          → es-model, es-protocol, es-geom
  es-backend   capability traits, Caps, the provider registry, compositor modules,
               test-mode decorators, doctor report                                       → es-wayland, es-desktop, es-record, es-protocol, es-geom, es-model
Layer 3  GPUI
  es-ui        tokens, theme, components, motion, icons, strings, a11y helpers           → es-geom, es-model, es-protocol, es-text, gpui-component
  es-app       Services global, PlatformEvent pump, RouteDispatcher, Jobs, Windows,
               CaptureCoordinator, platform_ui seams, toasts, errors::present            → layers 0–2, es-ui
  es-editor    EditorSession entities, canvas, toolbar, inspector, footer                → es-app, es-ui, es-anno, es-render, es-text, es-store, es-ocr
  es-overlays  selector, window mode, loupe, countdown, scroll UI, deck, pins, bars,
               recording UI, trim player                                                 → es-app, es-ui, es-render, es-scroll, es-store
  es-shell     settings, gallery, onboarding, tray model                                 → es-app, es-ui
  easyshot     bin: clap → cli::run (no GPUI) | daemon::run                              → all of layer 3
  es-testkit   dev-only: shared headless-window test helper; never a normal dependency  → gpui, es-geom
```

Layer-3 crates may depend on any layer 0–2 crate; the arrows list only the layer-3 edges.

Enforcement:

- `just layers` reads `cargo metadata` and fails if a layer 0, 1 or 2 crate depends, even
  transitively, on `gpui*`, `es-ui` or a layer-3 crate, or if a layer 0 or 1 crate depends
  on `tokio`, `zbus`, `wayland-client` or `ashpd`.
- `just grep-guard` fails if `hyprctl`, `HYPRLAND_INSTANCE_SIGNATURE`, `.socket2.sock`,
  `SWAYSOCK` or `NIRI_SOCKET` appear in `crates/` outside
  `crates/es-backend/src/{hyprland,wlroots,niri}/` (`tests/harness/` and `docs/` exempt).
  It also fails if the word `unsafe` or `unsafe_code` appears in a `.rs` file or
  `Cargo.toml` under `crates/` outside `crates/es-wayland/`, so no crate can allow it.
- `just gpui-guard` fails if a second gpui or zbus copy appears (`cargo tree -d`).

Cargo features cover only what changes system dependencies or must never ship: `hyprland`,
`wlroots`, `niri` (default on); `player`, `sound`, `evdev-input`, `portal-recorder`; `test-hooks` (never in release builds);
`dev-inspector`. Everything else is runtime detection.

## 5. Threads and async

- GPUI main thread: UI, windows and every entity.
- Platform runtime: one tokio multi-thread runtime for zbus, ashpd, ksni, notify-rust and
  GStreamer bus watches. Inbound, one `async_channel<PlatformEvent>` pump into GPUI;
  outbound, typed handles that send over channels and never block the UI.
- Wayland worker (`es-wayland`) has its own connection: capture, window lists, output
  hot-plug, the clipboard server, virtual pointer and cursor session. Frames return as
  `Arc<Frame>`.
- Compositor IPC lives inside each `es-backend/src/<compositor>/`: short-lived socket
  requests with a 50 ms timeout plus one event-reader thread. Only pointer queries may run
  on the GPUI thread.
- CPU work (effects, export, thumbnails, OCR, stitching) runs on GPUI's background
  executor with `rayon` for pixel loops, each job under `catch_unwind`.

## 6. Providers

Capability traits are defined in the lowest crate that implements them: `Shortcuts`,
`Tray`, `Notifier`, `FilePicker`, `Appearance` in es-desktop; `Recorder` in es-record;
`CursorSource` in `es-model::platform`; the rest (`Capture`, `WindowList`, `PointerQuery`,
`WindowRules`, `BindHelper`, `Clipboard`, `VirtualInput`) in es-backend. Each is
independent and optional.

Each capability has an ordered list of providers. `Backend::detect(probe)` picks the
highest-priority provider whose `probe()` returns `Some` and whose `build()` succeeds, and
records the chosen and rejected providers (with the reason) for logs and `easyshot doctor`.
`EASYSHOT_PROVIDER_<CAP>=<name>` forces a choice. `Caps` is derived from the chosen
providers and the probed tools, never set by hand.

The recorder providers are gpu-screen-recorder, then wf-recorder. The portal ScreenCast
recorder is listed but not available: its provider builds no client, so it is never chosen.

The `wayland::*` providers are chosen by protocol globals, not compositor name. Any
compositor with layer-shell and a capture protocol works without new code. Compositor
modules add only what protocols cannot give: `hyprland/` (window rects and z-order, cursor
position, runtime rules, bind check), `wlroots/` for sway (tree rects, focused output,
rules, bind snippet) and `niri/` (derived window rects with no z-order, focused output,
bind snippet; capture uses wlr-screencopy).

Degradation, each reported by doctor:

| Missing | Behaviour |
|---|---|
| window capture source | crop the window rectangle from the frozen frame |
| window geometry | window mode becomes a picker list; with no toplevel list, Window items are disabled with a tooltip |
| pointer query | last surface with pointer focus, then the focused output, then the first output |
| virtual input | Auto Scroll hidden; manual scroll capture remains |
| file picker portal | Export writes to the save folder, with a toast and a doctor hint |
| appearance portal | theme "system" means dark; Reduce Motion off |
| tray watcher | capture bar shown at launch whatever the setting |
| recorder / OCR tools | items disabled with an install hint |
| cursor source | `input.json` has no cursor travel |
| runtime rules | the rule snippet is shown in Settings > About > Compositor |

## 7. Storage

`es-store` is the only writer.

| What | Path |
|---|---|
| Config | `$XDG_CONFIG_HOME/easyshot/config.toml` |
| Editor style preset | `$XDG_CONFIG_HOME/easyshot/annotation-style.json` |
| Background presets | `$XDG_CONFIG_HOME/easyshot/background-presets.json` |
| Library | `$XDG_DATA_HOME/easyshot/library/` (`<name>.png`, `.png.easyshot` sidecar, `.base.png`) |
| History index | `library/index.json`, a cache rebuilt by scanning `library/` |
| Deck staging | `$XDG_DATA_HOME/easyshot/deck/<uuid>/` |
| Recordings | `$XDG_DATA_HOME/easyshot/recordings/<name>.easyshotrec/` |
| Recovery journal | `$XDG_STATE_HOME/easyshot/recovery/<session>.json`, edited base `<session>.base-<uuid>.png` |
| State | `$XDG_STATE_HOME/easyshot/state.json` |
| Logs, crash reports | `$XDG_STATE_HOME/easyshot/{log,crash}/` |
| Thumbnails | `$XDG_CACHE_HOME/easyshot/thumbs/` |
| Built-in wallpapers, written on first use | `$XDG_DATA_HOME/easyshot/wallpapers/wall-<n>.jpg` |
| Temp | `$XDG_CACHE_HOME/easyshot/{clipboard,drag,unstaged}/`; sockets in `$XDG_RUNTIME_DIR/easyshot/` |

Format rules:

- Every JSON file is `{ "format": "easyshot.<kind>", "version": N, ... }`.
- `es-model` has `migrate(kind, version, value)`, with a fixture for every shipped version
  under `tests/fixtures/formats/<kind>/v<N>.json`, tested forever.
- A file with a newer version opens read-only, is never written, and shows "Created by a
  newer easyshot".
- Every write is temp file, fsync, rename.
- If `config.toml` fails to parse, keep running on the last good config, never overwrite
  it, and notify with line and column. Unknown keys are logged and kept.
- The config file is watched; edits are reloaded, validated and applied live.

The keys and defaults of `config.toml` are the `es_model::Config` type.

A wallpaper path in a document opened from disk is kept only when it names a file in the
wallpapers folder or one the user picked (the recent list in `state.json`); otherwise it opens
as a missing wallpaper. `es_store::wallpapers::read` reads only a regular file of at most
64 MiB through a non-blocking, no-follow handle. Images from disk decode through
`es_store::decode` (32768 px a side, 256 MiB per decode).

## 8. App structure (es-app)

- The `Services` GPUI global holds handles, not state. `daemon::start` initialises them in
  a fixed order. No service lives in a `static`, and no code reaches stores directly.
- One pump task reads `PlatformEvent` (command, tray, shortcut, notification action,
  appearance, outputs, windows, config file change, recorder, and `TestInject` under
  `test-hooks`) and dispatches typed handlers.
- `RouteDispatcher::dispatch(route, origin, cx) -> JobId`: capture and record routes go to
  the `CaptureCoordinator` (a FIFO queue; a second scroll-capture trigger stops it); window
  routes go to the `Windows` registry, which focuses an existing settings, gallery or
  editor window instead of opening a second; `Jobs` owns `job_id -> (Task, cancel)`.
- es-app defines the seam traits and registries. Higher crates register implementations at
  daemon start.
- After capture, `es_model::after_capture_plan(variant, &Config) -> Vec<Step>` is pure;
  `pipeline.rs` only executes the steps.
- `SettingsModel::update` emits `SettingsChanged(SectionMask)`; a 300 ms debounced writer
  goes through `es-store`.
- The editor is an `EditorSession` of small entities (doc, viewport, look, crop, io). The
  toolbar, inspector and footer observe only their own entities, so a stroke never rebuilds
  the inspector.

## 9. Errors, logging and crash safety

- Each library crate has `pub enum Error` (thiserror, `#[non_exhaustive]`) and its own
  `Result`. `anyhow` only in layer 3 and the binary. `clippy::unwrap_used` is denied in
  non-test library code.
- One presenter, `es_app::errors::present`, turns an error into a toast, notification,
  inline message or nothing. D-Bus maps Cancelled to `"cancelled"` and every other error to
  `"error"`.
- Logging is `tracing` to stderr plus daily rotation keeping 7 files; filter with
  `EASYSHOT_LOG`. Window titles, clipboard contents, OCR text and paths under `$HOME` are
  logged only at `debug`.
- Recovery journal: after each committed undo entry, debounced 2 s, the editor writes
  `recovery/<session>.json` atomically (sidecar document, base image path and revision),
  deleted on save or discard. At startup each journal reopens in the editor.
  When crop, rotate or flip has changed the base, the journal also references
  `<session>.base-<uuid>.png` with those pixels. It is encoded off the main thread only when
  the base revision changed, written under a new name before the journal that references it,
  and the older one is deleted only after that journal is on disk, so a crash at any point
  leaves a journal whose files match. One lock per session orders the writes; a write that a
  newer one or a removal overtook is dropped. A missing or unreadable staged base reopens the
  original base with a warning.
- A panic hook writes `crash/<ts>.txt` (version, features, chosen providers, backtrace, last
  200 log lines, kept in an in-memory tracing layer); the next start offers "Open crash
  report" and renames the file to `<ts>.reported.txt` so it is offered once. Nothing is
  uploaded. CPU jobs run under `catch_unwind`.

## 10. Hotkey to deck card

1. The user's compositor bind runs `easyshot capture/region`. The CLI calls `Command` on
   the session bus (starting the daemon if needed) and gets a `job_id`.
2. The daemon's pump turns the call into `PlatformEvent::Command`; `RouteDispatcher`
   queues it on the `CaptureCoordinator`.
3. The coordinator asks the `Capture` provider for a frame of the output under the pointer
   and opens the selector through `Surfaces` and `SelectionFlow`. The frame is frozen.
4. The user selects a region. The frame is cropped (`es-render`).
5. `after_capture_plan` returns the steps for the variant and config; `pipeline.rs`
   executes them. The frame is first turned upright by its output transform. When the
   new-capture look (`resolve_new_capture_look`) draws anything, `es-render` composes the
   image inside it; the card, the clipboard and the save folder get that image and the raw
   pixels are kept beside it.
6. The library is written only through `es_store::commit` / `commit_new`: the image, its
   `.base.png` raw pixels and the sidecar holding the look, so the editor reopens it with the
   look editable. The pipeline (`pipeline.rs`) and a deck card's Save (`deck_save.rs`) both
   use it; a capture without a look is a plain image. The deck stages a capture only to show
   a card that is not yet in the library. A capture that is only pinned or opened in the
   editor lives in `$XDG_CACHE_HOME/easyshot/unstaged/` instead, as do the copies a card
   hands to the editor or a pin, so nothing unsaved returns as a card after a restart.
   The daemon purges that directory of files a day old at start.
7. The deck shows a card.
8. `JobFinished` returns to the waiting CLI.

Run through the daemon in headless sway and, in part, on Hyprland (see section 1).

## 11. Build and test

Rust 1.98.1, pinned in `rust-toolchain.toml`. `[profile.dev.package."*"] opt-level = 2`,
because GPUI is too slow unoptimised. `[profile.dev]` keeps line tables only for workspace
crates, no debug info for dependencies and no incremental output; `[profile.dbg]` is dev with
full debug info. `[profile.release]` uses thin LTO, one codegen unit
and stripped symbols; `[profile.fast]` is release without LTO for local installs and bench
iterations; it builds the pixel crates (tiny-skia, png, image, libblur, es-render and the
others listed in `Cargo.toml`) with one codegen unit, so its timings match a shipped build.

The install files (desktop entry, `easyshot://` handler, systemd user unit, D-Bus activation
file, AppStream metainfo) are templates in `packaging/data/`; `packaging/stage.sh` fills in the
app ID from `scripts/app-id.sh` and the PKGBUILD installs them. The packages
are checked only by the desktop-file, AppStream and systemd validators and have not been
installed. The Arch
packages' install script enables `easyshot.service` for all users (`systemctl --global`),
starts it for each logged-in user on install, and restarts it where it is running on upgrade.
The unit carries `ConditionUser=!@system`, so a display manager's greeter session (sddm,
gdm), which runs as a system user, never starts it.

Decoded wallpapers live in one `WallpaperCache` (4 images, 192 MB), and the pictures the
editor shows count against one 256 MB `ImageBudget` (LRU); both are shared by every window
through the `ImageStores` global. An evicted picture is dropped by its window on its next
frame and built again when needed; a picture held again before that frame stays.

Test environments are described in `CONTRIBUTING.md`: unit tests per crate, headless GPUI
windows (through the dev-only `es-testkit`), a Hyprland headless output, headless sway and
niri.

| Check | Harness |
|---|---|
| Geometry, engine, store, OCR, render, export | unit |
| Editor, settings, gallery, player UI; canvas performance | headless windows |
| Overlay, deck, pin, toast, bars; fractional scale; window capture; rules | Hyprland headless output (not yet run) |
| Generic providers, sway module, input-region pass-through, Auto Scroll | headless sway |
| wlr-screencopy-only capture, niri module | nested niri (not yet run) |
| IME, drag-out, real clipboard paste, GSR recordings with KMS, binds, Orca | manual with the maintainer |

Lints are in `[workspace.lints]`. `dead_code` warnings are fatal in the gate. CI warns on
any `.rs` file over 600 lines outside `vendor/`.

## 12. Strings and accessibility

Strings use `i18n-embed` and `i18n-embed-fl` (Fluent), with `i18n/en-US/<crate>.ftl` per
layer-3 crate and `fl!("id")` checked at compile time; `just i18n-check` asserts every
required string id exists. v1 ships English. GPUI's AccessKit support over AT-SPI is used:
every `es-ui` component sets role, label and value, and the canvas exposes zoom and the
selection count.
