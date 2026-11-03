# Packaging

- `stage.sh BIN SOURCE DEST` puts the binary and the files that go with it (icons, desktop
  entry, systemd and D-Bus units, metainfo, completions, manual, licences) into one
  directory. Every package is made from its output.
- `data/` holds the templates `stage.sh` fills in.
- `aur/easyshot-bin/` installs the release's prebuilt tarball.
- `aur/easyshot/` builds the release's vendored source tarball offline. It needs `rustup`
  because the source pins a newer Rust than the repositories carry.

Each AUR directory carries the same `easyshot.install` (`release.sh` fails if they differ):
it enables the user unit for all users, starts it for logged-in users on install, restarts it
where it is running on upgrade, and stops it on removal.

The two AUR packages conflict with each other and provide `easyshot`. Neither is published
yet.

## Cutting a release

1. Set the version in `crates/easyshot/Cargo.toml` and in both PKGBUILDs' `pkgver`, move the
   changelog's `Unreleased` entries under the new version, and commit.
2. Tag the commit `v<version>`.
3. Run `just release`. It builds the tag from a `git archive` export into `dist/`: the
   binary tarball, the vendored source tarball and `SHA256SUMS`. If `~/.minisign/easyshot.key`
   (or `$MINISIGN_KEY`) exists, it signs `SHA256SUMS`; otherwise it prints how to create the
   key. It fills `sha256sums` and regenerates `.SRCINFO` in both AUR packages, and prints the
   release notes (`just release-notes <version>` prints them alone).
4. Commit the PKGBUILD and `.SRCINFO` changes, push the branch and the tag, create the
   GitHub release with the notes and everything in `dist/`, then push each package to its
   AUR repository.

`just release` uploads and pushes nothing.
