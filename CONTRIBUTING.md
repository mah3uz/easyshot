# Contributing

Read `CLAUDE.md` first: it lists where each kind of fact lives and the invariants. A change
to code and the document that describes it go in the same commit.

easyshot runs end to end; `CLAUDE.md` says what has been run where. The app ID is
`dev.easyshot.app`, and the daemon is `easyshot daemon`, run by the user unit
`easyshot.service`.

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
- **niri** (`tests/harness/niri.sh`): nested as a window on the Hyprland test output.
  It starts and stops niri cleanly; every platform test skips there.

Test mode is the `test-hooks` cargo feature plus `EASYSHOT_TEST=1`; both are required and
release builds exclude the feature. In test mode the daemon sees one output only, uses its
own bus name `dev.easyshot.app.test`, replaces tray, notifications, shortcuts portal,
clipboard, sound and rule installation with no-op or recording backends, records a window
recording from the window's own frames without sound or microphone, and takes input
from `dev.easyshot.app.Test.Inject`. It refuses to start without `EASYSHOT_TEST_OUTPUT`.
`Test.Inject` takes a JSON array of events (`{"type":"key","keystroke":"ctrl-a"}`,
`mouse_down`, `mouse_up` and `mouse_move` with `x` and `y`), dispatched to the focused
window, else the newest one.

Some things can only be checked by hand with the maintainer: IME, drag-out into other apps,
pasting into other apps, GSR recordings with KMS, compositor binds and Orca.

## Tests

Run the tests of the crate you changed, not the whole workspace:

```
cargo nextest run -p <crate>
just test-changed
```

`just test-changed` runs the packages changed against `main`; `just test` runs everything
(nextest plus doc tests). `just test`, the doc tests and the platform harnesses build once, with
the shipped features (`shipped_features` in the justfile) plus `test-hooks`, through
`scripts/test.sh`; `just lint` runs clippy once with the shipped features. Code that only builds
without a shipped feature is checked by the default-feature dev build, not by the gate.

A crate's integration tests live in `tests/it/` as modules of one test binary; add a test file
there and declare it in `tests/it/main.rs`.

Layer 0, 1 and 2 crates test without a display. Window tests run in their own process under
nextest with a 60 s slow-timeout and one retry, because a window test can hang as it ends.
The dependency profile uses `opt-level = 2`; without it window tests are about ten times
slower. Dev builds carry line tables only for our crates and no debug info for dependencies,
and build without incremental output so sccache can reuse them across worktrees; build with
`--profile dbg` to step through code in a debugger. Cargo never deletes stale build output:
`just sweep [MAX_GB]` removes `target/debug` when it is over the limit (80 GB by default). Each window test's helper ends the window's own threads before the app, then calls
`run_until_parked`.

The full gate (`just check`: lint, layers, grep-guard, file-size, i18n-check, wiring-check, gpui-guard, tests, the share service's tests, the sway, niri and
Hyprland platform harnesses) is run once on the combined branch before merging. The release
build and `just release-check` run in `just release`, on the tagged commit.

## Wiring checks

Code that nothing reaches stays unnoticed, so three checks guard it. The daemon's wiring tests
(`crates/easyshot/src/daemon/wiring_tests/`) assert what booting registers: editor keys, the
config watch, the theme, the cursor mode, the tray menu, a handler for every `Route::all()`
variant, an answer for every notification button, and, in `settings_readers_tests.rs`, a
reader outside Settings for every config field. Add a setting or a route and the tests tell you
where the missing row goes. `just wiring-check` fails on a `pub fn` named `init*`, `install*`,
`register*`, `bind*`, `attach*` or `follow*` that nothing outside tests calls, and on an
`EventEmitter<T>` whose `T` nothing subscribes to. A deliberate exception is one line in
`scripts/wiring-check.allow` (`fn:<name>` or `event:<Type>`, then why); a line that is no
longer needed fails the check too. `just i18n-check` covers Fluent ids and icons the same way.

## Builds

`cargo run --features dev-inspector` turns on the GPUI element inspector in every window; use it only in a nested
session, never on your live desktop.

Rust is pinned by `rust-toolchain.toml`. `just build` makes the release binary, `just run
<args>` runs it from the checkout and `just dev` runs a debug build.

On Arch Linux, `just install` builds the package from your working tree and installs it
with pacman (it asks for sudo); `just uninstall` removes it. `just package [profile]` only
builds it, into `dist/`, from the same `packaging/aur/easyshot-bin/PKGBUILD` a release uses.
A release build sets `EASYSHOT_RELEASE=1` so its version carries no commit hash.

`just release X.Y.Z` cuts a release: it checks everything it needs first, runs `just check`,
bumps and stamps the version, and on your yes tags, pushes, publishes the GitHub release and the
AUR packages and deploys the website. `packaging/README.md` lists each step.

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
  with a `SAFETY` comment per block, and `just grep-guard` rejects the word `unsafe`
  anywhere else in `crates/`); `unwrap_used`, `dbg_macro`, `print_stdout` (except
  the CLI) and others are clippy lints. `dead_code` warnings are fatal in the gate.
- Comments explain why, not what.
- No file over about 600 lines outside `vendor/`.
- Duplicate dependency copies are checked with `cargo tree -d` in `just gpui-guard`.
