#!/usr/bin/env bash
# stage.sh BIN SOURCE DEST: put in DEST what a Linux package is made from: the binary in BIN,
# and beside it the files of the source tree in SOURCE that go with it.
set -euo pipefail
bin=$1 src=$2 dest=$3

mkdir -p "$dest/completions" "$dest/icons"
cp "$bin/easyshot" "$src"/{LICENSE,CHANGELOG.md,docs/MANUAL.md} "$dest/"
cp -r "$src/LICENSES" "$dest/"
cp -r "$src/assets/icons/app/hicolor" "$dest/icons/"
install -Dm644 "$src/assets/icons/app/dev.easyshot.Easyshot.svg" \
  "$dest/icons/hicolor/scalable/apps/dev.easyshot.Easyshot.svg"
"$bin/easyshot" completions bash > "$dest/completions/easyshot.bash"
"$bin/easyshot" completions zsh > "$dest/completions/easyshot.zsh"
"$bin/easyshot" completions fish > "$dest/completions/easyshot.fish"
