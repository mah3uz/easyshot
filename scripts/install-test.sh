#!/usr/bin/env bash
# Stages a release with a stand-in binary and runs install.sh into temporary directories:
# system and --user, then --uninstall. It never calls systemctl or touches the real system.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
root=$PWD
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail() { echo "install-test: $*" >&2; exit 1; }

mkdir "$tmp/bin"
printf '#!/bin/sh\necho stand-in\n' > "$tmp/bin/easyshot"
chmod +x "$tmp/bin/easyshot"
packaging/stage.sh "$tmp/bin" "$root" "$tmp/stage"
[ -x "$tmp/stage/install.sh" ] || fail "stage.sh did not put an executable install.sh in the release"

export EASYSHOT_NO_SYSTEMCTL=1
app_id=$(scripts/app-id.sh)

list() { (cd "$1" && find . -type f | sort); }

check_exec() {
  local file=$1 want=$2
  grep -q "^Exec\(Start\)\?=$want " "$file" || fail "$file does not run $want"
  if grep -q '/usr/bin/easyshot' "$file"; then fail "$file still names /usr/bin/easyshot"; fi
}

sys=$tmp/sys
DESTDIR=$sys PREFIX=/opt/es "$tmp/stage/install.sh" >/dev/null
want="./opt/es/bin/easyshot
./opt/es/lib/systemd/user/easyshot.service
./opt/es/share/applications/$app_id.desktop
./opt/es/share/applications/$app_id.open.desktop
./opt/es/share/bash-completion/completions/easyshot
./opt/es/share/dbus-1/services/$app_id.service
./opt/es/share/doc/easyshot/CHANGELOG.md
./opt/es/share/doc/easyshot/MANUAL.md
./opt/es/share/fish/vendor_completions.d/easyshot.fish
./opt/es/share/licenses/easyshot/LICENSE
./opt/es/share/metainfo/$app_id.metainfo.xml
./opt/es/share/zsh/site-functions/_easyshot"
got=$(list "$sys" | grep -v '/share/icons/\|/share/licenses/easyshot/LICENSES/')
[ "$got" = "$want" ] || fail "system file list differs:
$got"
list "$sys" | grep -q "/share/icons/hicolor/scalable/apps/$app_id.svg" || fail "system install has no icon"
check_exec "$sys/opt/es/lib/systemd/user/easyshot.service" /opt/es/bin/easyshot
check_exec "$sys/opt/es/share/dbus-1/services/$app_id.service" /opt/es/bin/easyshot
check_exec "$sys/opt/es/share/applications/$app_id.open.desktop" /opt/es/bin/easyshot
DESTDIR=$sys PREFIX=/opt/es "$tmp/stage/install.sh" >/dev/null || fail "a second install failed"
DESTDIR=$sys PREFIX=/opt/es "$tmp/stage/install.sh" --uninstall >/dev/null
[ -z "$(list "$sys")" ] || fail "system uninstall left files:
$(list "$sys")"

home=$tmp/home
mkdir "$home"
export HOME=$home XDG_CONFIG_HOME=$home/.config
"$tmp/stage/install.sh" --user >/dev/null
for f in .local/bin/easyshot .config/systemd/user/easyshot.service \
  ".local/share/dbus-1/services/$app_id.service" ".local/share/applications/$app_id.desktop" \
  ".local/share/icons/hicolor/scalable/apps/$app_id.svg" ".local/share/metainfo/$app_id.metainfo.xml" \
  .local/share/bash-completion/completions/easyshot .local/share/zsh/site-functions/_easyshot \
  .local/share/fish/vendor_completions.d/easyshot.fish .local/share/doc/easyshot/MANUAL.md; do
  [ -f "$home/$f" ] || fail "--user install has no $f"
done
check_exec "$home/.config/systemd/user/easyshot.service" "$home/.local/bin/easyshot"
check_exec "$home/.local/share/dbus-1/services/$app_id.service" "$home/.local/bin/easyshot"
check_exec "$home/.local/share/applications/$app_id.open.desktop" "$home/.local/bin/easyshot"
"$tmp/stage/install.sh" --user --uninstall >/dev/null
[ -z "$(list "$home")" ] || fail "--user uninstall left files:
$(list "$home")"

echo "install-test: ok"
