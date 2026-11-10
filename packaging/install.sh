#!/usr/bin/env bash
# install.sh [--user] [--uninstall]: install the files of this release tarball.
# PREFIX (default /usr/local) and DESTDIR apply to the system-wide install;
# EASYSHOT_NO_SYSTEMCTL=1 skips every systemctl call.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
mode=system action=install
for arg in "$@"; do
  case $arg in
    --user) mode=user ;;
    --uninstall) action=uninstall ;;
    -h|--help) echo "usage: ./install.sh [--user] [--uninstall]"; exit 0 ;;
    *) echo "install.sh: unknown option $arg" >&2; exit 2 ;;
  esac
done

if [ "$mode" = user ]; then
  prefix=${PREFIX:-$HOME/.local}
  unitdir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
else
  prefix=${PREFIX:-/usr/local}
  unitdir=$prefix/lib/systemd/user
fi
share=$prefix/share
dest=${DESTDIR:-}
bin=$prefix/bin/easyshot

[ -f "$here/easyshot" ] || { echo "install.sh: run it from the extracted release directory" >&2; exit 1; }
if [ "$mode" = system ] && [ -z "$dest" ] && [ "$(id -u)" != 0 ]; then
  echo "install.sh: a system-wide install needs root; run it with sudo, or use --user" >&2
  exit 1
fi

# One line per file: mode, path in the tarball, installed path.
manifest() {
  local f
  echo "755 easyshot $bin"
  while IFS= read -r f; do echo "644 $f $share/$f"; done < <(cd "$here" && find icons -type f | sort)
  for f in "$here"/applications/*.desktop; do echo "644 applications/${f##*/} $share/applications/${f##*/}"; done
  echo "644 systemd/easyshot.service $unitdir/easyshot.service"
  for f in "$here"/dbus/*.service; do echo "644 dbus/${f##*/} $share/dbus-1/services/${f##*/}"; done
  for f in "$here"/metainfo/*.xml; do echo "644 metainfo/${f##*/} $share/metainfo/${f##*/}"; done
  echo "644 completions/easyshot.bash $share/bash-completion/completions/easyshot"
  echo "644 completions/easyshot.zsh $share/zsh/site-functions/_easyshot"
  echo "644 completions/easyshot.fish $share/fish/vendor_completions.d/easyshot.fish"
  echo "644 MANUAL.md $share/doc/easyshot/MANUAL.md"
  echo "644 CHANGELOG.md $share/doc/easyshot/CHANGELOG.md"
  echo "644 LICENSE $share/licenses/easyshot/LICENSE"
  while IFS= read -r f; do echo "644 $f $share/licenses/easyshot/$f"; done < <(cd "$here" && find LICENSES -type f | sort)
}

sctl() {
  [ "${EASYSHOT_NO_SYSTEMCTL:-}" = 1 ] && return 0
  systemctl --user "$@" >/dev/null 2>&1 || echo "install.sh: 'systemctl --user $*' did not succeed" >&2
}

if [ "$action" = install ]; then
  while read -r m src dst; do
    install -Dm"$m" "$here/$src" "$dest$dst"
    case $src in
      systemd/*|dbus/*|applications/*)
        sed -i -E "s#^(Exec=|ExecStart=)(/usr/bin/)?easyshot( |\$)#\1$bin\3#" "$dest$dst" ;;
    esac
  done < <(manifest)
  echo "easyshot installed under $prefix"
  if [ "$mode" = user ]; then
    sctl daemon-reload
    sctl enable --now easyshot.service
    echo "easyshot is enabled and running for you"
  else
    echo "Each user runs once: systemctl --user enable --now easyshot.service"
  fi
else
  if [ "$mode" = user ]; then sctl disable --now easyshot.service; fi
  while read -r _ _ dst; do rm -f "$dest$dst"; done < <(manifest)
  rmdir "$dest$share/doc/easyshot" 2>/dev/null || true
  find "$dest$share/licenses/easyshot" -depth -type d -empty -delete 2>/dev/null || true
  if [ "$mode" = user ]; then sctl daemon-reload; fi
  echo "easyshot removed from $prefix; your captures and settings are untouched"
fi
