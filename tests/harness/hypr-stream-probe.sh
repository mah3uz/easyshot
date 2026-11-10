#!/usr/bin/env bash
# Adds the easyshot-test output to the live Hyprland session: run only with the user present.
# Usage: tests/harness/hypr-stream-probe.sh   (the other subcommands are called by the probe itself)
set -euo pipefail
# shellcheck source=lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

SELF=$REPO/tests/harness/hypr-stream-probe.sh
APP=es-stream-probe
COVER=es-stream-cover
FILLER=es-stream-filler
HIDDEN=easyshot-hidden
BUSY=easyshot-busy
STATE=$(state_dir hypr-stream)

addr() { hyprctl clients -j | jq -r --arg c "$1" '[.[] | select(.class == $c)][0].address // empty'; }
has_client() { [ -n "$(addr "$1")" ]; }
field() { hyprctl clients -j | jq -r --arg a "$1" ".[] | select(.address == \$a) | $2"; }
test_ws() { hyprctl monitors -j | jq -r '.[] | select(.name == "easyshot-test") | .activeWorkspace.id'; }
test_mon() { hyprctl monitors -j | jq -r '.[] | select(.name == "easyshot-test") | .id'; }
shown() { hyprctl monitors -j | jq -r '.[].activeWorkspace.name'; }

close() {
    local a
    a=$(addr "$1")
    [ -z "$a" ] || hyprctl dispatch "hl.dsp.window.close({ window = \"address:$a\" })" > /dev/null || true
}

close_all() { close "$APP"; close "$COVER"; close "$FILLER"; }

open_on_test() {
    printf 'exec alacritty --class %s %s\n' "$1" "$2" > "$STATE/open-$1.sh"
    hyprctl dispatch "hl.dsp.exec_cmd(\"bash $STATE/open-$1.sh\", { workspace = \"$(test_ws) silent\" })" > /dev/null
    wait_for 10 has_client "$1" || die "$1 did not open"
    sleep 0.5
}

move_to() {
    hyprctl dispatch "hl.dsp.window.move({ workspace = \"$2\", window = \"address:$(addr "$1")\", follow = false })" > /dev/null
}

step() {
    local a ws
    a=$(addr "$APP")
    [ -n "$a" ] || die "no $APP window"
    case "$1" in
        visible) ;;
        hidden-workspace)
            move_to "$APP" "name:$HIDDEN"
            sleep 0.3
            [ "$(field "$a" .workspace.name)" = "$HIDDEN" ] || die "window did not move to $HIDDEN"
            ! shown | grep -qx "$HIDDEN" || die "$HIDDEN is shown on a monitor"
            ;;
        covered-by-fullscreen)
            move_to "$APP" "$(test_ws)"
            open_on_test "$COVER" ""
            hyprctl dispatch "hl.dsp.window.fullscreen({ mode = \"fullscreen\", action = \"toggle\", window = \"address:$(addr "$COVER")\" })" > /dev/null
            sleep 0.3
            [ "$(field "$(addr "$COVER")" .fullscreen)" != 0 ] || die "cover window is not fullscreen"
            ;;
        visible-again)
            close "$COVER"
            wait_for 5 bash -c "! hyprctl clients -j | jq -e '.[] | select(.class == \"$COVER\")' > /dev/null" || die "cover did not close"
            ;;
        beside-another-window)
            open_on_test "$FILLER" ""
            ;;
        moved-to-busy-workspace)
            move_to "$FILLER" "name:$BUSY"
            sleep 0.5
            move_to "$APP" "name:$BUSY"
            sleep 0.3
            [ "$(field "$a" .workspace.name)" = "$BUSY" ] || die "window did not move to $BUSY"
            ! shown | grep -qx "$BUSY" || die "$BUSY is shown on a monitor"
            ;;
        moved-back)
            move_to "$APP" "$(test_ws)"
            ;;
    esac
    [ "$(field "$a" .monitor)" = "$(test_mon)" ] || die "window left easyshot-test"
    ws=$(field "$a" .workspace.name)
    echo "STEP $1: window on workspace $ws, size $(field "$a" '.size | join("x")'), shown: $(shown | tr '\n' ' ')"
}

inner() {
    trap close_all EXIT
    open_on_test "$APP" "-e sh -c 'i=0; while :; do i=\$((i+1)); echo \$i; sleep 0.005; done'"
    EASYSHOT_PROBE_APP_ID=$APP EASYSHOT_PROBE_STEP=$SELF \
        cargo nextest run -p es-wayland --release -j 1 --retries 0 --run-ignored only \
        -E 'test(hypr_stream_probe)' --no-capture
}

main() {
    cd "$REPO"
    cargo nextest run -p es-wayland --release --no-run
    trap 'close_all; "$REPO/tests/harness/hypr.sh" down' EXIT
    "$REPO/tests/harness/hypr.sh" up
    # Without a rule a new workspace opens on the focused monitor, which is a real one.
    for ws in "$HIDDEN" "$BUSY"; do
        hyprctl eval "hl.workspace_rule({ workspace = \"name:$ws\", monitor = \"easyshot-test\" })" > /dev/null
    done
    "$REPO/tests/harness/hypr.sh" run -- bash "$SELF" inner
}

case "${1:-}" in
    "") main ;;
    inner) inner ;;
    visible | hidden-workspace | covered-by-fullscreen | visible-again | beside-another-window | moved-to-busy-workspace | moved-back) step "$1" ;;
    *) die "usage: hypr-stream-probe.sh" ;;
esac
