# Packaging

- `stage.sh BIN SOURCE DEST` puts the binary and the files that go with it (icons, desktop
  entry, systemd and D-Bus units, metainfo, completions, manual, licences) into one
  directory. Every package is made from its output.
- `install.sh` is staged into the tarball root. `./install.sh` installs system-wide under
  `PREFIX` (default `/usr/local`), `./install.sh --user` under `~/.local`, and `--uninstall`
  removes what it put there. It lays files out as the AUR packages do, points the unit, the
  D-Bus service and the desktop entries at the installed binary, and honours `DESTDIR`.
  `EASYSHOT_NO_SYSTEMCTL=1` skips systemctl; `just install-test` runs it that way.
- `web/install.sh` is the one-line installer, served by the website at
  `https://easyshot.site/install.sh` (the site's Nuxt config serves `web/` at its root, so
  this is its only copy). It finds the latest release from the redirect of
  `releases/latest`, checks `SHA256SUMS.minisig` with the key it embeds when minisign is
  installed, then the tarball against `SHA256SUMS`, and runs the tarball's `install.sh`:
  system-wide through sudo or doas by default, `--user` (or `--local`) into `~/.local`.
  `EASYSHOT_RELEASES_URL` replaces the GitHub releases URL, for tests only.
  `scripts/install-bootstrap-test.sh` (in `just install-test`) runs it against a fake
  release on a local server and fails if the embedded key differs from `release.pub`.
- `data/` holds the templates `stage.sh` fills in.
- `aur/easyshot-bin/` installs the release's prebuilt tarball.
- `aur/easyshot/` builds the release's vendored source tarball offline. It needs `rustup`
  because the source pins a newer Rust than the repositories carry.

Each AUR directory carries the same `easyshot.install` (`just release` and `just aur` fail if they differ):
it enables the user unit for all users, starts it for logged-in users on install, restarts it
where it is running on upgrade, and stops it on removal.

The two AUR packages conflict with each other and provide `easyshot`.

## Cutting a release

Write the release's entries under the changelog's `## Unreleased`, commit them on `main`, then
run `just release X.Y.Z`. It is the whole release.

1. Before it changes anything, it checks that the version is `x.y.z` and newer than the last
   `v*` tag (0.0.0 when there is none), that `## Unreleased` has entries and there is no
   `## X.Y.Z` yet, that you are on a clean `main` that is not behind `origin/main`, that the tag
   `vX.Y.Z` exists neither here nor on origin, that `gh` is logged in, that the AUR answers over
   SSH, that wrangler is logged in (in `docs/site`), that the minisign key
   `~/.minisign/easyshot.key` (or `$MINISIGN_KEY`) exists, and that the two `easyshot.install`
   files are the same. Then it runs `just check`.
2. It sets the version in `crates/easyshot/Cargo.toml` and `Cargo.lock` (only the easyshot
   crate's; nothing to do when it is already there), sets `pkgver=X.Y.Z` and `pkgrel=1` in both
   PKGBUILDs with their checksums back to `SKIP`, moves the `Unreleased` entries under
   `## X.Y.Z - <local time with its UTC offset, to the second>`, sets the same time on the
   metainfo's `<release version="X.Y.Z">` (adding it first when missing), and commits that as
   "easyshot X.Y.Z".
3. It asks before anything public. On no, the commit stays local and it prints how to undo it.
   On yes, it tags `vX.Y.Z` (annotated), pushes `main` and the tag, and runs `just publish X.Y.Z`.

`just publish X.Y.Z` can be run again when a step fails. It builds the tag with
`scripts/release-build.sh` from a `git archive` export into `dist/`: the binary tarball (which
must hold `install.sh`), the vendored source tarball, `SHA256SUMS` and `SHA256SUMS.minisig`
(checked against `release.pub`), and runs `scripts/release-check.sh` on that binary. It creates
the GitHub release with the notes from `just release-notes X.Y.Z` and everything in `dist/`, or
updates the notes and assets of one that exists. The release is created as not latest and marked
latest only after its assets are up, because the one-line installer downloads `releases/latest`.
It then writes the tarballs' checksums into the PKGBUILDs, commits "Release X.Y.Z: AUR
checksums", pushes `main`, runs `just aur` and deploys the website with `just site-deploy`.

`just aur` pushes `easyshot` and `easyshot-bin` (PKGBUILD, `easyshot.install` and a `.SRCINFO`
made there) to the AUR through throwaway clones in `target/aur`. It refuses while a checksum is
`SKIP` or the GitHub release lacks the file a PKGBUILD downloads. For a packaging-only fix, bump
`pkgrel`, commit, and run `just aur`.

Making the GitHub repository public is a step of its own, by hand: until then, the AUR packages
and the one-line installer cannot download the release.
