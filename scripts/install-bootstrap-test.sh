#!/usr/bin/env bash
# Builds a fake release, serves it on 127.0.0.1 and runs the one-line installer
# (packaging/web/install.sh) against it with a fake HOME and stand-ins for sudo and systemctl.
# It never touches the real system, the real HOME or the network.
set -euo pipefail
if [ "$(id -u)" = 0 ]; then
  echo "install-bootstrap-test: skipped, because as root the system-wide install would not go through the stand-in sudo"
  exit 0
fi
cd "$(dirname "${BASH_SOURCE[0]}")/.."
root=$PWD
tmp=$(mktemp -d)
server=''
cleanup() {
  if [ -n "$server" ]; then kill "$server" 2>/dev/null || true; fi
  rm -rf "$tmp"
}
trap cleanup EXIT
fail() { echo "install-bootstrap-test: $*" >&2; [ ! -f "$tmp/out" ] || cat "$tmp/out" >&2; exit 1; }

boot=packaging/web/install.sh
grep -qx "pubkey=$(tail -n 1 packaging/release.pub)" "$boot" ||
  fail "$boot does not embed the public key in packaging/release.pub"

ver=0.0.1
name=easyshot-$ver-x86_64-unknown-linux-gnu
dl=$tmp/www/releases/download/v$ver
mkdir -p "$tmp/bin" "$dl" "$tmp/good" "$tmp/home" "$tmp/sys"
printf '#!/bin/sh\necho stand-in\n' > "$tmp/bin/easyshot"
chmod +x "$tmp/bin/easyshot"
packaging/stage.sh "$tmp/bin" "$root" "$tmp/$name"
tar -C "$tmp" -czf "$tmp/good/$name.tar.gz" "$name"
(cd "$tmp/good" && sha256sum "$name.tar.gz" > SHA256SUMS)

# A release whose install.sh would leave a mark, to prove nothing unverified runs.
printf '#!/bin/sh\ntouch %s/pwned\n' "$tmp" > "$tmp/$name/install.sh"
tar -C "$tmp" -czf "$tmp/evil.tar.gz" "$name"

cp "$boot" "$tmp/boot.sh"
signed=''
if command -v minisign >/dev/null 2>&1; then
  minisign -G -W -p "$tmp/test.pub" -s "$tmp/test.key" >/dev/null
  minisign -S -s "$tmp/test.key" -m "$tmp/good/SHA256SUMS" -x "$tmp/good/SHA256SUMS.minisig" >/dev/null
  sed -i "s|^pubkey=.*|pubkey=$(tail -n 1 "$tmp/test.pub")|" "$tmp/boot.sh"
  grep -qx "pubkey=$(tail -n 1 "$tmp/test.pub")" "$tmp/boot.sh" || fail "could not swap in the test key"
  signed=1
else
  echo "install-bootstrap-test: minisign is not installed, so the signature checks are skipped"
  echo untrusted > "$tmp/good/SHA256SUMS.minisig"
fi
use_good() { cp "$tmp/good/$name.tar.gz" "$tmp/good/SHA256SUMS" "$tmp/good/SHA256SUMS.minisig" "$dl/"; }
use_good

cat > "$tmp/server.py" <<'PY'
import functools, http.server, os, sys
www, ver, portfile = sys.argv[1:4]
class Handler(http.server.SimpleHTTPRequestHandler):
    def send_head(self):
        if self.path == "/releases/latest":
            self.send_response(302)
            self.send_header("Location", f"http://{self.headers['Host']}/releases/tag/v{ver}")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return None
        return super().send_head()
    def log_message(self, *args):
        pass
httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=www))
with open(portfile + ".new", "w") as f:
    f.write(str(httpd.server_address[1]))
os.rename(portfile + ".new", portfile)
httpd.serve_forever()
PY
python3 -I "$tmp/server.py" "$tmp/www" "$ver" "$tmp/port" &
server=$!
for _ in $(seq 50); do [ -s "$tmp/port" ] && break; sleep 0.1; done
[ -s "$tmp/port" ] || fail "the test server did not start"


# sudo and systemctl are stand-ins first on PATH: sudo installs into $tmp/sys through DESTDIR,
# systemctl only logs. With no runtime dir or session bus, a real systemctl --user reaches nothing.
mkdir "$tmp/shim"
printf '#!/bin/sh\ntouch %s/sudo-used\nexec env DESTDIR=%s "$@"\n' "$tmp" "$tmp/sys" > "$tmp/shim/sudo"
printf '#!/bin/sh\necho "$*" >> %s/systemctl.log\n' "$tmp" > "$tmp/shim/systemctl"
chmod +x "$tmp/shim/sudo" "$tmp/shim/systemctl"
mkdir -m 700 "$tmp/run"

unset PREFIX DESTDIR EASYSHOT_VERSION DBUS_SESSION_BUS_ADDRESS
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY ALL_PROXY all_proxy
export PATH=$tmp/shim:$PATH XDG_RUNTIME_DIR=$tmp/run
export HOME=$tmp/home XDG_CONFIG_HOME=$tmp/home/.config EASYSHOT_NO_SYSTEMCTL=1
port=$(cat "$tmp/port")
export EASYSHOT_RELEASES_URL=http://127.0.0.1:$port/releases
app_id=$(scripts/app-id.sh)

# As `curl ... | sh -s -- ARGS` runs it: the script on standard input.
run() { sh -s -- "$@" < "$tmp/boot.sh" > "$tmp/out" 2>&1; }
files() { (cd "$1" && find . -type f | sort); }
untouched() {
  [ -z "$(files "$tmp/home")" ] || fail "$1 left files in HOME:
$(files "$tmp/home")"
  [ -z "$(files "$tmp/sys")" ] || fail "$1 left files in the system:
$(files "$tmp/sys")"
  [ ! -e "$tmp/pwned" ] || fail "$1 ran the downloaded install.sh"
}
says() { grep -qF -- "$1" "$tmp/out" || fail "the output does not say '$1'"; }

EASYSHOT_NO_SYSTEMCTL='' run || fail "the default install failed"
[ -f "$tmp/sudo-used" ] || fail "the default install did not go through sudo"
for f in bin/easyshot lib/systemd/user/easyshot.service "share/applications/$app_id.desktop"; do
  [ -f "$tmp/sys/usr/local/$f" ] || fail "the default install has no /usr/local/$f"
done
[ -z "$(files "$tmp/home")" ] || fail "the default install wrote into HOME"
grep -qx -- "--user enable --now easyshot.service" "$tmp/systemctl.log" ||
  fail "the default install did not start easyshot for the user"
says "sudo may ask for your password"
says "checksum checked"
says "Settings › Shortcuts"
if [ -n "$signed" ]; then says "signature checked"; else says "signature was not checked"; fi

EASYSHOT_NO_SYSTEMCTL='' EASYSHOT_VERSION=$ver run --uninstall || fail "--uninstall failed"
grep -qx -- "--user disable --now easyshot.service" "$tmp/systemctl.log" ||
  fail "--uninstall did not stop easyshot for the user"
untouched "--uninstall"

rm "$tmp/sudo-used" "$tmp/systemctl.log"
run --user || fail "the --user install failed"
for f in .local/bin/easyshot .config/systemd/user/easyshot.service \
  ".local/share/applications/$app_id.desktop" ".local/share/dbus-1/services/$app_id.service"; do
  [ -f "$HOME/$f" ] || fail "the --user install has no $f"
done
[ ! -e "$tmp/sudo-used" ] || fail "the --user install used sudo"
EASYSHOT_VERSION=$ver run --local --uninstall || fail "--local --uninstall failed"
untouched "--local --uninstall"

cp "$tmp/evil.tar.gz" "$dl/$name.tar.gz"
if run; then fail "a tampered tarball was installed"; fi
says "does not match SHA256SUMS"
untouched "a tampered tarball"

if [ -n "$signed" ]; then
  (cd "$dl" && sha256sum "$name.tar.gz" > SHA256SUMS)
  if run; then fail "a tampered SHA256SUMS was accepted"; fi
  says "signature on SHA256SUMS does not match"
  untouched "a tampered SHA256SUMS"
fi
use_good

mkdir "$tmp/arm"
printf '#!/bin/sh\n[ "$1" = -m ] && echo aarch64 || exec %s "$@"\n' "$(command -v uname)" > "$tmp/arm/uname"
chmod +x "$tmp/arm/uname"
if PATH=$tmp/arm:$PATH run; then fail "it installed on aarch64"; fi
says "x86_64 only"
untouched "the wrong architecture"

if run --bogus; then fail "an unknown option was accepted"; fi
says "usage:"

if command -v wget >/dev/null 2>&1; then
  mkdir "$tmp/nocurl"
  IFS=: read -ra dirs <<< "$PATH"
  for d in "${dirs[@]}"; do
    [ -d "$d" ] && find "$d" -maxdepth 1 ! -name curl -exec ln -s -t "$tmp/nocurl" {} + 2>/dev/null || true
  done
  [ "$(readlink "$tmp/nocurl/sudo")" = "$tmp/shim/sudo" ] || fail "the wget PATH does not use the stand-in sudo"
  PATH=$tmp/nocurl run || fail "the install with wget failed"
  [ -f "$tmp/sys/usr/local/bin/easyshot" ] || fail "the install with wget has no easyshot"
  EASYSHOT_VERSION=$ver PATH=$tmp/nocurl run --uninstall || fail "--uninstall with wget failed"
  untouched "--uninstall with wget"
else
  echo "install-bootstrap-test: wget is not installed, so the wget path is skipped"
fi

echo "install-bootstrap-test: ok"
