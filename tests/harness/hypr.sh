#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

OUT=easyshot-test
STATE=$(state_dir hypr)
SCALE=1.5

# Adding or removing an output under a session lock hung Hyprland once.
no_lock() { ! pgrep -f '[l]ock_shell.qml' > /dev/null || die "a lock is running: not now"; }

monitors() { hyprctl monitors -j | jq -r '.[].name' | sort; }
out_field() { hyprctl monitors -j | jq -r --arg o "$OUT" '.[] | select(.name == $o) | '"$1"; }
out_exists() { [ -n "$(out_field .name)" ]; }
out_gone() { ! out_exists; }
placed() { [ "$(out_field .x)" = 20000 ]; }

parse_scale() {
    if [ "${1:-}" = --scale ]; then
        [ -n "${2:-}" ] || die "--scale needs a value"
        SCALE=$2
    fi
}

up() {
    no_lock
    hyprctl monitors -j > /dev/null || die "no running Hyprland"
    ! out_exists || die "$OUT already exists (hypr.sh down first)"
    monitors > "$STATE/before"
    hyprctl output create headless "$OUT" > /dev/null
    if ! wait_for 10 out_exists; then
        down
        die "$OUT did not appear"
    fi
    hyprctl eval "hl.monitor({ output = \"$OUT\", mode = \"1920x1080@60\", position = \"20000x0\", scale = $SCALE })" > /dev/null
    if ! wait_for 10 placed; then
        down
        die "$OUT was not placed at 20000x0"
    fi
    echo "up: $OUT at 20000x0, scale $(out_field .scale)"
}

clients_on_out() {
    local id
    id=$(out_field .id)
    [ -n "$id" ] || return 0
    hyprctl clients -j | jq -r --argjson id "$id" '.[] | select(.monitor == $id) | .address'
}

no_clients() { [ -z "$(clients_on_out)" ]; }

down() {
    no_lock
    local a
    # Windows left on a removed output would be moved onto a real monitor.
    if out_exists; then
        for a in $(clients_on_out); do
            hyprctl dispatch "hl.dsp.window.close({ window = \"address:$a\" })" > /dev/null || true
        done
        wait_for 10 no_clients || die "windows still open on $OUT; refusing to remove it"
        hyprctl output remove "$OUT" > /dev/null
    fi
    wait_for 10 out_gone || die "$OUT is still there"
    if [ -f "$STATE/before" ]; then
        [ "$(monitors)" = "$(cat "$STATE/before")" ] || die "monitor list differs from before up: $(monitors | tr '\n' ' ')"
        rm -f "$STATE/before"
    fi
    echo "down: monitors restored: $(monitors | tr '\n' ' ')"
}

# Launched by Hyprland on the test output's workspace so windows land there; output and exit code return through files.
run() {
    out_exists || die "$OUT is not up (hypr.sh up first)"
    local ws job rc log
    ws=$(out_field .activeWorkspace.id)
    job=$STATE/job.sh rc=$STATE/rc log=$STATE/job.log
    rm -f "$rc" "$log"
    {
        echo "cd $(printf %q "$PWD")"
        echo "export PATH=$(printf %q "$PATH") HOME=$(printf %q "$HOME")"
        while IFS= read -r v; do printf 'export %q\n' "$v"; done < <(env | grep -E '^(EASYSHOT_|CARGO_|RUST|SCCACHE_|NEXTEST_)' || true)
        echo "export EASYSHOT_TEST=1 EASYSHOT_TEST_OUTPUT=$OUT"
        printf 'dbus-run-session -- '
        printf '%q ' "$@"
        printf '> %q 2>&1\n' "$log"
        echo "echo \$? > $(printf %q "$rc")"
    } > "$job"
    : > "$log"
    tail -n +1 -f "$log" &
    local tailer=$!
    hyprctl dispatch "hl.dsp.exec_cmd(\"bash $job\", { workspace = \"$ws silent\" })" > /dev/null
    local ok=0
    wait_for "${EASYSHOT_HARNESS_TIMEOUT:-1800}" test -s "$rc" && ok=1
    sleep 0.3
    kill "$tailer" 2> /dev/null || true
    [ "$ok" = 1 ] || die "job timed out"
    return "$(cat "$rc")"
}

shot() {
    out_exists || die "$OUT is not up"
    mkdir -p "$REPO/tests/harness/out"
    grim -o "$OUT" "$REPO/tests/harness/out/hypr-$1.png"
    echo "$REPO/tests/harness/out/hypr-$1.png"
}

cmd=${1:-}
[ $# -eq 0 ] || shift
case "$cmd" in
    up) parse_scale "$@"; up ;;
    down) down ;;
    run)
        [ "${1:-}" != -- ] || shift
        [ $# -gt 0 ] || die "usage: hypr.sh run -- CMD..."
        run "$@" ;;
    shot) [ -n "${1:-}" ] || die "usage: hypr.sh shot NAME"; shot "$1" ;;
    ctl) hyprctl "$@" ;;
    platform)
        parse_scale "$@"
        trap down EXIT
        up
        cd "$REPO"
        run bash -c "source tests/harness/lib.sh && platform_tests" ;;
    *) die "usage: hypr.sh up [--scale N] | down | run -- CMD... | shot NAME | ctl ARGS... | platform [--scale N]" ;;
esac
