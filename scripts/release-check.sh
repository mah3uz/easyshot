#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

fail=0

# Captured first, so a cargo error or an early-exiting grep cannot pass the check.
tree=$(cargo tree -p easyshot -e normal,build,features --prefix none --locked)
if grep -qF 'feature "test-hooks"' <<<"$tree"; then
    echo "release-check: test-hooks is enabled in the easyshot release feature tree" >&2
    fail=1
fi

bin=${1:-}
[ -n "$bin" ] || bin=$(cargo metadata --format-version 1 --no-deps --locked | jq -r .target_directory)/release/easyshot
if [ ! -f "$bin" ]; then
    echo "release-check: $bin does not exist; run cargo build -p easyshot --release first" >&2
    exit 1
fi
test_iface=$(scripts/app-id.sh).Test
if grep -aqF "$test_iface" "$bin"; then
    echo "release-check: $bin contains the string $test_iface" >&2
    fail=1
fi

[ "$fail" = 0 ] || exit 1
echo "release-check: ok"
