#!/usr/bin/env bash
# WLR_BACKENDS=headless is fully separate from the user's session, so virtual input is allowed.
set -euo pipefail
# shellcheck source=lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

OUTPUT=HEADLESS-1
STATE=$(state_dir sway)
RUN=$STATE/run

sock() { compgen -G "$RUN/sway-ipc.*.sock" | head -n1 || true; }
wl() { compgen -G "$RUN/wayland-[0-9]*" | grep -v '\.lock$' | head -n1 || true; }
ready() { [ -n "$(sock)" ] && [ -n "$(wl)" ]; }
alive() { [ -f "$STATE/pid" ] && kill -0 "$(cat "$STATE/pid")" 2>/dev/null; }

# Everything lives in one process group (setsid) so down can always take it all away.
up() {
    ! alive || die "already up (sway.sh down first)"
    rm -rf "$RUN" "$STATE/bus" "$STATE/pid"
    mkdir -m 700 "$RUN"
    setsid bash -c '
        echo $$ > "$1/pid"
        exec dbus-run-session -- bash -c '\''
            echo "$DBUS_SESSION_BUS_ADDRESS" > "$1/bus"
            exec env -u DISPLAY -u WAYLAND_DISPLAY -u HYPRLAND_INSTANCE_SIGNATURE -u SWAYSOCK -u NIRI_SOCKET \
                XDG_RUNTIME_DIR="$1/run" WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 \
                WLR_RENDERER_ALLOW_SOFTWARE=1 sway -c "$2"
        '\'' _ "$1" "$2"
    ' _ "$STATE" "$REPO/tests/harness/sway.conf" > "$STATE/sway.log" 2>&1 < /dev/null &
    if ! wait_for 20 ready; then
        tail -n 20 "$STATE/sway.log" >&2
        down
        die "sway did not start"
    fi
    echo "sway up: output $OUTPUT"
}

down() {
    if [ -f "$STATE/pid" ]; then
        local pid
        pid=$(cat "$STATE/pid")
        if kill -0 "$pid" 2>/dev/null; then
            [ -z "$(sock)" ] || swaymsg -s "$(sock)" exit > /dev/null 2>&1 || true
            wait_for 5 bash -c "! kill -0 $pid" || kill -TERM -- "-$pid" 2>/dev/null || true
            wait_for 5 bash -c "! kill -0 $pid" || kill -KILL -- "-$pid" 2>/dev/null || true
        fi
    fi
    rm -rf "$RUN" "$STATE/bus" "$STATE/pid"
}

run() {
    alive || die "not up (sway.sh up first)"
    ready || die "sway sockets missing"
    env -u DISPLAY -u HYPRLAND_INSTANCE_SIGNATURE -u NIRI_SOCKET \
        XDG_RUNTIME_DIR="$RUN" WAYLAND_DISPLAY="$(basename "$(wl)")" SWAYSOCK="$(sock)" \
        DBUS_SESSION_BUS_ADDRESS="$(cat "$STATE/bus")" \
        EASYSHOT_TEST=1 EASYSHOT_TEST_VIRTUAL_INPUT=1 EASYSHOT_TEST_OUTPUT="$OUTPUT" "$@"
}

cmd=${1:-}
[ $# -eq 0 ] || shift
case "$cmd" in
    up) up ;;
    down) down ;;
    run)
        [ "${1:-}" != -- ] || shift
        [ $# -gt 0 ] || die "usage: sway.sh run -- CMD..."
        run "$@" ;;
    platform)
        trap down EXIT
        up
        cd "$REPO"
        run bash -c "source tests/harness/lib.sh && platform_tests sway" ;;
    *) die "usage: sway.sh up | down | run -- CMD... | platform" ;;
esac
