#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

hits=$(grep -rnIF \
    -e 'hyprctl' -e 'HYPRLAND_INSTANCE_SIGNATURE' -e '.socket2.sock' -e 'SWAYSOCK' -e 'NIRI_SOCKET' \
    --exclude-dir=target crates |
    grep -vE '^crates/es-backend/src/(hyprland|wlroots|niri)/' || true)

if [ -n "$hits" ]; then
    echo "grep-guard: compositor-specific names outside crates/es-backend/src/{hyprland,wlroots,niri}/" >&2
    printf '%s\n' "$hits" >&2
    exit 1
fi
unsafe_hits=$(grep -rnIE -e '\bunsafe\b' -e 'unsafe_code' \
    --include='*.rs' --include='Cargo.toml' --exclude-dir=target crates |
    grep -v '^crates/es-wayland/' || true)

if [ -n "$unsafe_hits" ]; then
    echo "grep-guard: unsafe code outside crates/es-wayland/" >&2
    printf '%s\n' "$unsafe_hits" >&2
    exit 1
fi
echo "grep-guard: ok"
