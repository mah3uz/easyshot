#!/usr/bin/env bash
# The outputs are committed; run after changing a master.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

if ! command -v rsvg-convert > /dev/null; then
    echo "logo: rsvg-convert not found; install librsvg (Arch: librsvg, Debian/Ubuntu: librsvg2-bin, Fedora: librsvg2-tools)" >&2
    exit 1
fi

app=dev.easyshot.Easyshot
colour=branding/logo.svg
symbolic=branding/logo-symbolic.svg
out=assets/icons/app

rm -rf "$out"
mkdir -p "$out/hicolor/symbolic/apps"
cp "$colour" "$out/$app.svg"
for n in 16 22 24 32 48 64 128 256 512; do
    mkdir -p "$out/hicolor/${n}x${n}/apps"
    rsvg-convert -w "$n" -h "$n" -o "$out/hicolor/${n}x${n}/apps/$app.png" "$colour"
done
cp "$symbolic" "$out/hicolor/symbolic/apps/$app-symbolic.svg"
cp "$symbolic" assets/icons/easyshot.svg

echo "logo: regenerated $out/ and assets/icons/easyshot.svg"
