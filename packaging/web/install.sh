#!/bin/sh
# Install easyshot from its latest GitHub release:
#   curl -fsSL https://easyshot.site/install.sh | sh                  (system-wide, /usr/local)
#   curl -fsSL https://easyshot.site/install.sh | sh -s -- --user     (just you, ~/.local)
#   curl -fsSL https://easyshot.site/install.sh | sh -s -- --uninstall
# EASYSHOT_VERSION=x.y.z installs that release. EASYSHOT_RELEASES_URL is for tests only.
set -eu

pubkey=RWRZb1Z2R8olADDs9ZWFvon7YercRduazJtfJoN3vltGVOU8I7zXuZE5

say() { printf 'easyshot: %s\n' "$*"; }
die() { printf 'easyshot: %s\n' "$*" >&2; exit 1; }
usage() { echo "usage: curl -fsSL https://easyshot.site/install.sh | sh [-s -- [--user | --local] [--uninstall]]"; }

fetch() {
  if [ -n "$curl" ]; then
    curl -fsSL --proto "$proto" -o "$2" "$1"
  else
    wget -q "$wget_https" -O "$2" "$1"
  fi
}

latest_tag() {
  if [ -n "$curl" ]; then
    curl -fsS --proto "$proto" -o /dev/null -w '%{redirect_url}' "$base/latest"
  else
    wget -S --spider --max-redirect=0 "$wget_https" "$base/latest" 2>&1 | sed -n 's/^ \{1,\}[Ll]ocation: *//p' | tail -n 1
  fi
}

sctl() {
  [ "${EASYSHOT_NO_SYSTEMCTL:-}" = 1 ] && return 0
  systemctl --user "$@" >/dev/null 2>&1 || say "'systemctl --user $*' did not succeed"
}

main() {
  mode=system action=install
  for arg in "$@"; do
    case $arg in
      --user|--local) mode=user ;;
      --system) mode=system ;;
      --uninstall) action=uninstall ;;
      -h|--help) usage; exit 0 ;;
      *) usage >&2; exit 2 ;;
    esac
  done

  [ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] ||
    die "easyshot is built for Linux on x86_64 only; this is $(uname -s) on $(uname -m)."

  curl=''
  if command -v curl >/dev/null 2>&1; then curl=curl
  elif ! command -v wget >/dev/null 2>&1; then die "this installer needs curl or wget; install one and run it again."
  fi
  for tool in tar gzip sha256sum bash; do
    command -v "$tool" >/dev/null 2>&1 || die "this installer needs $tool; install it and run it again."
  done

  base=${EASYSHOT_RELEASES_URL:-https://github.com/mah3uz/easyshot/releases}
  case $base in
    https://*) proto='=https' wget_https=--https-only ;;
    *) proto='=http,https' wget_https=--no-https-only ;;
  esac

  if [ -n "${EASYSHOT_VERSION:-}" ]; then
    ver=${EASYSHOT_VERSION#v}
  else
    loc=$(latest_tag | tr -d '\r') || loc=''
    case $loc in
      */tag/v*) ver=${loc##*/tag/v} ;;
      *) die "could not find the latest release at $base/latest." ;;
    esac
  fi
  case $ver in
    ''|*[!0-9A-Za-z.+-]*) die "'$ver' is not a release version." ;;
  esac

  sudo=''
  if [ "$mode" = system ] && [ "$(id -u)" != 0 ]; then
    if command -v sudo >/dev/null 2>&1; then sudo=sudo
    elif command -v doas >/dev/null 2>&1; then sudo=doas
    else die "a system-wide install needs root; run it as root, install sudo or doas, or add --user to install into ~/.local."
    fi
  fi

  if command -v pacman >/dev/null 2>&1; then
    say "on Arch Linux, the AUR packages easyshot-bin (prebuilt) and easyshot (from source) are another way to install."
  fi

  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  trap 'exit 1' HUP INT TERM

  name=easyshot-$ver-x86_64-unknown-linux-gnu
  say "downloading easyshot $ver"
  for file in "$name.tar.gz" SHA256SUMS SHA256SUMS.minisig; do
    fetch "$base/download/v$ver/$file" "$tmp/$file" || die "could not download $base/download/v$ver/$file"
  done

  if command -v minisign >/dev/null 2>&1; then
    minisign -Vq -P "$pubkey" -m "$tmp/SHA256SUMS" -x "$tmp/SHA256SUMS.minisig" ||
      die "the signature on SHA256SUMS does not match easyshot's key; nothing was installed."
    say "signature checked"
  else
    say "minisign is not installed, so the release signature was not checked, only the checksum."
    say "to check it next time, install minisign from your distribution's packages."
  fi

  want=$(awk -v f="$name.tar.gz" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")
  got=$(sha256sum "$tmp/$name.tar.gz" | cut -d ' ' -f 1)
  [ -n "$want" ] && [ "$want" = "$got" ] ||
    die "$name.tar.gz does not match SHA256SUMS; nothing was installed."
  say "checksum checked"

  mkdir "$tmp/x"
  tar -xzf "$tmp/$name.tar.gz" -C "$tmp/x"
  [ -f "$tmp/x/$name/install.sh" ] || die "the release has no install.sh; nothing was installed."

  set -- "$tmp/x/$name/install.sh"
  [ "$mode" = user ] && set -- "$@" --user
  [ "$action" = uninstall ] && set -- "$@" --uninstall
  if [ -n "$sudo" ]; then
    say "a system-wide install under /usr/local needs root, so $sudo may ask for your password."
  fi
  [ -z "$sudo" ] || [ "$action" = install ] || sctl disable --now easyshot.service
  $sudo bash "$@" </dev/null

  if [ "$action" = uninstall ]; then
    [ -z "$sudo" ] || sctl daemon-reload
    exit 0
  fi
  if [ -n "$sudo" ]; then
    sctl daemon-reload
    sctl enable --now easyshot.service
    say "easyshot is enabled and running for you"
  fi
  say "installed easyshot $ver"
  if [ "$mode" = user ]; then
    case ":$PATH:" in
      *":${PREFIX:-$HOME/.local}/bin:"*) ;;
      *) say "add ${PREFIX:-$HOME/.local}/bin to your PATH to run easyshot from a terminal." ;;
    esac
  fi
  say "next: open Settings › Shortcuts to bind your capture keys."
}

main "$@"
