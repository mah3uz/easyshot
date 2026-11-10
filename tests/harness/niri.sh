#!/usr/bin/env bash
# niri has no headless mode a user can start, and its tty/DRM path must never run in the live session, so it runs nested on the Hyprland test output.
set -euo pipefail
# shellcheck source=lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

OUT=easyshot-test
STATE=$(state_dir niri)
HYPR=$REPO/tests/harness/hypr.sh

env_ok() { [ -s "$STATE/env" ]; }

up() {
    [ ! -e "$STATE/env" ] || die "already up (niri.sh down first)"
    "$HYPR" up
    rm -f "$STATE/env"
    # niri writes its own environment (WAYLAND_DISPLAY, NIRI_SOCKET, the bus) once it is running.
    cat > "$STATE/config.kdl" <<EOF
spawn-at-startup "sh" "-c" "env > '$STATE/env.tmp' && mv '$STATE/env.tmp' '$STATE/env'"
EOF
    niri validate -c "$STATE/config.kdl" > /dev/null || { down; die "generated niri config is invalid"; }
    cat > "$STATE/niri.sh" <<EOF
#!/usr/bin/env bash
exec dbus-run-session -- niri -c '$STATE/config.kdl'
EOF
    chmod +x "$STATE/niri.sh"
    # hypr.sh run waits for its job; niri lives until it is closed, so launch it through Hyprland directly.
    local ws
    ws=$(hyprctl monitors -j | jq -r --arg o "$OUT" '.[] | select(.name == $o) | .activeWorkspace.id')
    hyprctl dispatch "hl.dsp.exec_cmd(\"$STATE/niri.sh > $STATE/niri.log 2>&1\", { workspace = \"$ws silent\" })" > /dev/null
    if ! wait_for 20 env_ok; then
        tail -n 20 "$STATE/niri.log" >&2
        down
        die "niri did not start"
    fi
    echo "niri up on $OUT"
}

down() {
    local sig=0
    if [ -s "$STATE/env" ]; then
        # shellcheck disable=SC2046
        # niri can exit before it answers the quit, so only its socket going away counts.
        env $(grep -E '^(NIRI_SOCKET|WAYLAND_DISPLAY)=' "$STATE/env") niri msg action quit --skip-confirmation > /dev/null 2>&1 || true
        wait_for 5 test ! -S "$(sed -n 's/^NIRI_SOCKET=//p' "$STATE/env")" || sig=1
    fi
    rm -f "$STATE/env" "$STATE/env.tmp"
    "$HYPR" down
    return "$sig"
}

run() {
    env_ok || die "niri is not up (niri.sh up first)"
    local v
    v=$(grep -E '^(NIRI_SOCKET|WAYLAND_DISPLAY|DBUS_SESSION_BUS_ADDRESS)=' "$STATE/env")
    # shellcheck disable=SC2086
    env -u DISPLAY -u HYPRLAND_INSTANCE_SIGNATURE $v \
        EASYSHOT_TEST=1 EASYSHOT_TEST_OUTPUT=winit "$@"
}

cmd=${1:-}
[ $# -eq 0 ] || shift
case "$cmd" in
    up) up ;;
    down) down ;;
    run)
        [ "${1:-}" != -- ] || shift
        [ $# -gt 0 ] || die "usage: niri.sh run -- CMD..."
        run "$@" ;;
    platform)
        trap down EXIT
        up
        cd "$REPO"
        run bash -c "source tests/harness/lib.sh && platform_tests" ;;
    *) die "usage: niri.sh up | down | run -- CMD... | platform" ;;
esac
