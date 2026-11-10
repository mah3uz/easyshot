#!/usr/bin/env bash
# release-build.sh VERSION: build the tag v<VERSION> into dist/: the binary tarball, the vendored
# source tarball, SHA256SUMS and its minisign signature. `just publish` runs it and uploads dist/.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
fail() { echo "release-build: $*" >&2; exit 1; }

ver=${1:?usage: release-build.sh VERSION}
tag=v$ver
key=${MINISIGN_KEY:-$HOME/.minisign/easyshot.key}
git rev-parse -q --verify "refs/tags/$tag^{commit}" >/dev/null || fail "there is no tag $tag"
[[ -f $key ]] || fail "no signing key at $key; create one with: mkdir -p ~/.minisign && minisign -G -p ~/.minisign/easyshot.pub -s ~/.minisign/easyshot.key"

name=easyshot-$ver-x86_64-unknown-linux-gnu
vendored=easyshot-$ver-vendored.tar.zst
dist=$root/dist
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
    cargo build --release --locked -p easyshot --features sound,player,evdev-input
)
"$work/src/scripts/release-check.sh" "$build/release/easyshot"

echo "==> $name.tar.gz"
"$work/src/packaging/stage.sh" "$build/release" "$work/src" "$work/$name"
pack "$name" | gzip -n -9 > "$dist/$name.tar.gz"
tar -tzf "$dist/$name.tar.gz" | grep -x "$name/install.sh" >/dev/null || fail "$name.tar.gz has no install.sh"

echo "==> $vendored"
mkdir "$work/src/.cargo"
(cd "$work/src" && cargo vendor --locked vendor-crates > .cargo/config.toml)
mv "$work/src" "$work/easyshot-$ver"
pack "easyshot-$ver" | zstd -19 -T0 -q > "$dist/$vendored"

echo "==> SHA256SUMS"
(cd "$dist" && sha256sum "$name.tar.gz" "$vendored" > SHA256SUMS)
minisign -S -s "$key" -m "$dist/SHA256SUMS" -x "$dist/SHA256SUMS.minisig"
# The one-line installer checks the signature against this key, so a release signed with any other fails for everyone.
minisign -Vq -p packaging/release.pub -m "$dist/SHA256SUMS" -x "$dist/SHA256SUMS.minisig" ||
  fail "SHA256SUMS.minisig does not verify against packaging/release.pub; $key is not the release key"
