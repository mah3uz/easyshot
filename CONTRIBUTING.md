# Contributing

Read `CLAUDE.md` first: it lists where each kind of fact lives and the invariants. A change
to code and the document that describes it go in the same commit.

easyshot is not usable yet. Only the workspace skeleton exists, so most of what follows
describes how work is meant to be done as the crates fill in.

## Never touch the live session

Do not grab the screen, inject input or open windows on your own compositor session while
testing. Not by hand, not in a script, not from an agent. Every GUI test runs somewhere
else:

- **Headless windows** (no compositor): GPUI's `HeadlessAppContext`, for the editor,
  settings, gallery, player and every normal-window test. The shared helper is the
  dev-only crate `es-testkit`.
- **Hyprland headless output** (`tests/harness/hypr.sh`): creates an output named
  `easyshot-test`, launches under `dbus-run-session` with
  `EASYSHOT_TEST_OUTPUT=easyshot-test`, grabs with `grim -o easyshot-test` only, refuses to
  run while a session lock is active, and removes the output at the end.
- **Headless sway** (`tests/harness/sway.sh`): invisible, with `WLR_BACKENDS=headless`.
  Virtual input is allowed here because it is not your session.
- **niri** (`tests/harness/niri.sh`): headless if niri 26.04 can run that way, otherwise
  nested as a window on the Hyprland test output.

Test mode is the `test-hooks` cargo feature plus `EASYSHOT_TEST=1`; both are required and
release builds exclude the feature. In test mode the daemon sees one output only, uses its
own bus name `dev.easyshot.EasyshotTest`, replaces tray, notifications, shortcuts portal,
clipboard, sound and rule installation with no-op or recording backends, and takes input
from `dev.easyshot.Test.Inject`. It refuses to start without `EASYSHOT_TEST_OUTPUT`.
`Test.Inject` takes a JSON array of events (`{"type":"key","keystroke":"ctrl-a"}`,
`mouse_down`, `mouse_up` and `mouse_move` with `x` and `y`), dispatched to the focused
window, else the newest one.

Some things can only be checked by hand with the maintainer: IME, drag-out into other apps,
pasting into other apps, GSR recordings with KMS, compositor binds and Orca.

## Tests

Run the tests of the crate you changed, not the whole workspace:

```
cargo test -p <crate>
```

Layer 0, 1 and 2 crates test without a display. Window tests run in their own process under
nextest with a 60 s slow-timeout and one retry, because a window test can hang as it ends.
The dependency profile uses `opt-level = 2`; without it window tests are about ten times
slower. Each window test's helper ends the window's own threads before the app, then calls
`run_until_parked`.

The full gate (`just check`) is run once on the combined branch before merging.

## Builds

Keep to two concurrent cargo builds on a shared machine. A local, gitignored
`.cargo/config.toml` may set `sccache` and `jobs`.

## Patches to vendored GPUI

`vendor/gpui-pre/` and `vendor/gpui-pre-linux/` are copied as-is from crates.io. Our
patches are git commits on top of that copy, so the diff against the original is the patch
set. Do not edit vendored code without a commit that says what it patches and why.

## Adding a compositor provider

1. Create `crates/es-backend/src/<name>/` with its provider statics.
2. Add the `cfg` lines to the registry lists.
3. Add a cargo feature.
4. Add a harness or fixture tests.
5. Add grep-guard patterns.
6. Add a row to the doctor and degradation docs.
7. Add a snippet to `BindHelper`.

Nothing in layer 3 changes, unless the compositor lacks layer-shell (GNOME), which needs a
new `SelectionFlow` and `Surfaces` implementation. Code for a compositor you have not run
it on is unverified; say so in the commit.

## Style

- `rustfmt` with defaults and edition 2024.
- Lints are in `[workspace.lints]`: `unsafe_code` is denied (allowed only in `es-wayland`
  with a `SAFETY` comment per block); `unwrap_used`, `dbg_macro`, `print_stdout` (except
  the CLI) and others are clippy lints. `dead_code` warnings are fatal in the gate.
- Comments explain why, not what.
- No file over about 600 lines outside `vendor/`.
- Duplicate dependency copies are checked with `cargo tree -d` in `just gpui-guard`.
