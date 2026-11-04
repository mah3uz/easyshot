# easyshot

Screenshot, annotation and screen-recording app for Linux/Wayland: built with
Rust. Rust · GPUI · Hyprland, sway and niri first.

Status: the app runs end to end. The daemon (`easyshot daemon`) connects the capture
overlays, deck, editor, settings, gallery, tray, recording and CLI. It has had two live
rounds on Hyprland (2026-11-02/03 and 2026-11-04); the bugs found are fixed, the fixed build
is not yet re-verified there, and the Hyprland platform tests pass on a headless output. The
rest is tested in headless GPUI windows and headless sway 1.12; the niri module has not run
on niri.

## Read first

Each kind of fact has one home. A change is written there, in the same commit as the code,
and nowhere else.

- `docs/MANUAL.md` — everything a user meets: behaviour, keys, settings, the CLI, files,
  limits, per-compositor notes, troubleshooting. The only user document. Written plainly,
  to the reader as "you".
- `docs/FEATURES.md` — the one-line list of every feature, in plain words, for the website,
  linking to `docs/MANUAL.md`. Updated in the same commit as a change a user would notice.
- `docs/ARCHITECTURE.md` — the crate graph, threads, the data flow from hotkey to deck
  card, the invariants, what has been run where, how to build and test. Authoritative.
- `docs/DESIGN.md` — why the interface looks and behaves as it does.
- `docs/DECISIONS.md` — each choice, and what was rejected.
- `docs/PRIOR-ART.md` — what this builds on, with sources.
- `CONTRIBUTING.md` — the live-session rule, test harnesses, per-crate tests, the patch
  workflow, adding a compositor.
- `CHANGELOG.md` — what each release changed, for its user. A change a user would notice is
  added under `Unreleased` in `### New`, `### Improvements` or `### Bug fixes`, one short
  line each. Internals, tests and docs are left out. Until the first release, everything is
  `### New`, written as what the release contains: no improvements and no fixes, since no
  user had the earlier behaviour.
- `README.md` — the front page: a summary with links. Change it only when a headline
  feature changes.
- `docs/site/` — the website and how-to guide (`docs/site/README.md` says how it is built).
  It describes what `docs/MANUAL.md` and `docs/FEATURES.md` say: a change a user would notice
  updates all three.
- `crates/<name>/README.md` — one crate's responsibility and what it may and must not
  depend on.

These documents say what the system is and does today. A capability it lacks is stated as a
plain limit, never as something to come.

`just` lists the tasks once the justfile exists.

## Invariants

Changing any of these is an architecture change: update `docs/ARCHITECTURE.md` first.

1. Crates are layered (0 data, 1 compute, 2 IO and system, 3 GPUI). Dependencies point only
   downward, and `just layers` enforces it. Layer 0, 1 and 2 crates never depend on GPUI.
2. One owner per concern: one writer per file format (`es-store` is the only writer of
   every file), one renderer, one shortcut catalogue, one route vocabulary
   (`es-protocol::Route`).
3. Capabilities come from providers, not compositor branches. Compositor-specific code
   lives in one directory per compositor in `es-backend`, behind a cargo feature. The app
   sees `Caps`, never feature flags or compositor names. `just grep-guard` enforces it.
4. The UI is small entities and typed events. No god models, and no service lives in a
   `static`.
5. Every persisted format carries a version envelope and a migration. Writes are atomic.
   A newer file opens read-only and is never written. A config that fails to parse is
   never overwritten.
6. Every user-visible string comes from the Fluent catalogue, and every control has an
   accessible role and label.
7. The CLI builds and runs with no GUI library: `easyshot <command>` does no GPUI init.
8. Entry points are closed vocabularies. The `easyshot://` URL scheme accepts only its
   allowlist, with no arguments; D-Bus `Command` parses into the closed `Route`.
9. Freeze is always on for captures. There is one renderer, shared by the editor and
   export.
10. `es-testkit` is a dev-dependency only, never a normal dependency.
11. No `unwrap` in non-test library code; `unsafe` only in `es-wayland`, with a `SAFETY`
    comment per block; no file over about 600 lines outside `vendor/`.

## Platform rule

Code for a compositor that has not been built and run on that compositor is unverified. Say
so in the commit; never report it as working. `docs/ARCHITECTURE.md` "What has been run
where" lists what has been verified; everything else is unverified.

## Never touch the live session

Tests, harnesses and agents never grab the screen, inject input or open windows on the
user's live compositor session. GUI tests run in GPUI's headless windows, on a headless
Hyprland output, in headless sway, or nested niri, and always under their own D-Bus
session. Test mode is the `test-hooks` feature plus `EASYSHOT_TEST=1`, and the daemon
refuses to start in it without `EASYSHOT_TEST_OUTPUT`. Details in `CONTRIBUTING.md`.

## Toolchain and dependencies

Rust is pinned to 1.98.1 in `rust-toolchain.toml`; `rust-version` is the same. GPUI comes
from `gpui-pre`, `gpui-pre-platform` and `gpui-component`, pinned with `=`. `just
gpui-guard` runs `cargo tree -d` and fails on a second gpui or zbus copy. There is no
cargo-deny, cargo-machete or cargo-msrv; licence and advisory checks arrive with packaging.
