#!/usr/bin/env bash

REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
export REPO

die() { echo "error: $*" >&2; exit 1; }

state_dir() {
    local d
    d="${TMPDIR:-/tmp}/easyshot-harness-$(id -u)/$1"
    mkdir -p "$d" && chmod 700 "$d"
    echo "$d"
}

# The platform_* tests need test mode: the test-hooks feature plus EASYSHOT_TEST=1 and an output name (set by the caller).
platform_tests() {
    cargo nextest run -p es-backend -p easyshot --features es-backend/test-hooks,easyshot/test-hooks \
        --run-ignored all --no-tests=pass -E 'test(/(^|::)platform_/)'
}

wait_for() {
    local n=$(($1 * 5)); shift
    while ! "$@" 2>/dev/null; do
        n=$((n - 1)); [ "$n" -gt 0 ] || return 1
        sleep 0.2
    done
}
