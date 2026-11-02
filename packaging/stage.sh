#!/usr/bin/env bash
# stage.sh BIN SOURCE DEST: put in DEST what a Linux package is made from: the binary in BIN,
# and beside it the files of the source tree in SOURCE that go with it.
set -euo pipefail
bin=$1 src=$2 dest=$3

mkdir -p "$dest/completions" "$dest/icons"
cp "$bin/easyshot" "$src"/{LICENSE,CHANGELOG.md,docs/MANUAL.md} "$dest/"
cp -r "$src/LICENSES" "$dest/"
app_id=$("$src/scripts/app-id.sh")
icons=$src/assets/icons/app
for dir in "$icons"/hicolor/*x*/apps; do
  size=$(basename "$(dirname "$dir")")
  install -Dm644 "$dir/easyshot.png" "$dest/icons/hicolor/$size/apps/$app_id.png"
done
install -Dm644 "$icons/easyshot.svg" "$dest/icons/hicolor/scalable/apps/$app_id.svg"
install -Dm644 "$icons/hicolor/symbolic/apps/easyshot-symbolic.svg" \
  "$dest/icons/hicolor/symbolic/apps/$app_id-symbolic.svg"
"$bin/easyshot" completions bash > "$dest/completions/easyshot.bash"
"$bin/easyshot" completions zsh > "$dest/completions/easyshot.zsh"
"$bin/easyshot" completions fish > "$dest/completions/easyshot.fish"
