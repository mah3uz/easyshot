#!/usr/bin/env bash
# release.sh: once the tag v<version> exists, build it into dist/: the binary tarball, the
# vendored source tarball, SHA256SUMS (signed when a key exists), and the checksums and
# .SRCINFO of both AUR packages. It uploads and pushes nothing.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
fail() { echo "release: $*" >&2; exit 1; }

ver=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/easyshot/Cargo.toml | head -1)
tag=v$ver
git rev-parse -q --verify "refs/tags/$tag^{commit}" >/dev/null || fail "tag $tag does not exist; tag the release commit first"
[[ -z $(git status --porcelain) ]] || fail "the working tree is not clean"
for pkg in easyshot easyshot-bin; do
  [[ $(sed -n 's/^pkgver=//p' "packaging/aur/$pkg/PKGBUILD") == "$ver" ]] || fail "packaging/aur/$pkg/PKGBUILD has a different pkgver than $ver"
done
cmp -s packaging/aur/easyshot/easyshot.install packaging/aur/easyshot-bin/easyshot.install || fail "the two easyshot.install files differ"

name=easyshot-$ver-x86_64-unknown-linux-gnu
vendored=easyshot-$ver-vendored.tar.zst
dist=$root/dist
key=${MINISIGN_KEY:-$HOME/.minisign/easyshot.key}
build=$root/target/release-build
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
epoch=$(git log -1 --format=%ct "$tag")
export SOURCE_DATE_EPOCH=$epoch
pack() { tar -C "$work" --sort=name --mtime="@$epoch" --owner=0 --group=0 --numeric-owner -cf - "$1"; }

echo "==> $tag: exporting"
mkdir -p "$work/src" "$build"
rm -rf "$dist" && mkdir -p "$dist"
git archive "$tag" | tar -x -C "$work/src"

echo "==> $tag: building"
(
  cd "$work/src"
  EASYSHOT_RELEASE=1 CARGO_TARGET_DIR=$build \
    RUSTFLAGS="--remap-path-prefix=$work/src=/build/easyshot --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo" \
    cargo build --release --locked -p easyshot --features sound,player
)

echo "==> $name.tar.gz"
"$work/src/packaging/stage.sh" "$build/release" "$work/src" "$work/$name"
pack "$name" | gzip -n -9 > "$dist/$name.tar.gz"

echo "==> $vendored"
mkdir "$work/src/.cargo"
(cd "$work/src" && cargo vendor --locked vendor-crates > .cargo/config.toml)
mv "$work/src" "$work/easyshot-$ver"
pack "easyshot-$ver" | zstd -19 -T0 -q > "$dist/$vendored"

echo "==> SHA256SUMS"
(cd "$dist" && sha256sum "$name.tar.gz" "$vendored" > SHA256SUMS)
if [[ -f $key ]]; then
  minisign -S -s "$key" -m "$dist/SHA256SUMS" -x "$dist/SHA256SUMS.minisig"
else
  echo "release: no signing key at $key, so SHA256SUMS is not signed." >&2
  echo "         Create one with: mkdir -p ~/.minisign && minisign -G -p ~/.minisign/easyshot.pub -s ~/.minisign/easyshot.key" >&2
fi

echo "==> AUR checksums and .SRCINFO"
fill() {
  sed -i "s/^sha256sums=('[^']*')/sha256sums=('$(sha256sum "$dist/$2" | cut -d' ' -f1)')/" "packaging/aur/$1/PKGBUILD"
  (cd "packaging/aur/$1" && makepkg --printsrcinfo > .SRCINFO)
}
fill easyshot-bin "$name.tar.gz"
fill easyshot "$vendored"

echo
echo "Release notes for $ver:"
scripts/release-notes.sh "$ver" 2>/dev/null || scripts/release-notes.sh Unreleased

cat <<STEPS

Built in $dist:
$(cd "$dist" && ls -1)

Publish by hand:
  1. Commit the PKGBUILD and .SRCINFO changes, and push main and $tag.
  2. Create the release $tag on GitHub with the notes above and upload everything in dist/.
  3. Copy packaging/aur/easyshot and packaging/aur/easyshot-bin into their AUR repositories
     (ssh://aur@aur.archlinux.org/<name>.git), commit and push each.
STEPS
