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
echo "grep-guard: ok"
