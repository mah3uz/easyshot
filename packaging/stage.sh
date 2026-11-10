#!/usr/bin/env bash
# stage.sh BIN SOURCE DEST: put in DEST what a Linux package is made from: the binary in BIN,
# and beside it the files of the source tree in SOURCE that go with it.
set -euo pipefail
bin=$1 src=$2 dest=$3

mkdir -p "$dest/completions" "$dest/icons"
cp "$bin/easyshot" "$src"/{LICENSE,CHANGELOG.md,docs/MANUAL.md} "$dest/"
install -m755 "$src/packaging/install.sh" "$dest/install.sh"
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

data=$src/packaging/data
sub() { sed "s/@APP_ID@/$app_id/g" "$1" > "$2"; }
mkdir -p "$dest"/{applications,systemd,dbus,metainfo}
sub "$data/app.desktop.in" "$dest/applications/$app_id.desktop"
sub "$data/open.desktop.in" "$dest/applications/$app_id.open.desktop"
sub "$data/easyshot.service.in" "$dest/systemd/easyshot.service"
sub "$data/dbus.service.in" "$dest/dbus/$app_id.service"
sub "$data/metainfo.xml.in" "$dest/metainfo/$app_id.metainfo.xml"
