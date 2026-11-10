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
tooltip_hits=$(grep -rnIF \
    -e '.tooltip(' -e 'managed_tooltip' -e 'Tooltip::new' -e 'tooltip_show_delay' -e 'tooltip_hide_after' \
    --include='*.rs' --exclude-dir=target crates |
    grep -v '^crates/es-ui/src/components/tooltip.rs:' || true)

if [ -n "$tooltip_hits" ]; then
    echo "grep-guard: a tooltip made outside crates/es-ui/src/components/tooltip.rs (use Tip::tip)" >&2
    printf '%s\n' "$tooltip_hits" >&2
    exit 1
fi
menu_hits=$(grep -rnIF \
    -e 'Role::MenuItem' -e 'Role::Menu,' \
    --include='*.rs' --exclude-dir=target --exclude-dir=tests crates |
    grep -v '^crates/es-ui/src/components/menu.rs:' || true)

if [ -n "$menu_hits" ]; then
    echo "grep-guard: a menu or menu row built outside crates/es-ui/src/components/menu.rs (use menu() and MenuItem)" >&2
    printf '%s\n' "$menu_hits" >&2
    exit 1
fi
toast_timer_hits=$(grep -rnIE \
    -e 'timer\((TOAST_DURATION|ERROR_DURATION|UNDO_DURATION|TOAST_HOLD)' \
    --include='*.rs' --exclude-dir=target --exclude-dir=tests crates |
    grep -v -e '^crates/es-ui/src/components/toast.rs:' || true)

if [ -n "$toast_timer_hits" ]; then
    echo "grep-guard: a toast timed outside crates/es-ui/src/components/toast.rs (use ToastClock)" >&2
    printf '%s\n' "$toast_timer_hits" >&2
    exit 1
fi
echo "grep-guard: ok"
