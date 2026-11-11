#!/usr/bin/env bash
# test.sh [--doc | --platform [sway]]: the workspace's tests with one feature set, so the gate, the
# doc tests and the platform harnesses all reuse one build. With no argument, every test but the
# platform ones; --platform runs only those, inside a harness.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

shipped=$(sed -n 's/^shipped_features := "\(.*\)"/\1/p' justfile)
[ -n "$shipped" ] || { echo "test.sh: no shipped_features in the justfile" >&2; exit 1; }
features=$shipped,easyshot/test-hooks
# es-app's platform_ui tests are ordinary tests; only these two crates hold harness tests.
platform='(package(easyshot) or package(es-backend)) and test(/(^|::)platform_/)'

case "${1:-}" in
    --doc)
        exec cargo test --workspace --doc --features "$features" ;;
    --platform)
        # The sway_harness tests refuse to run outside sway instead of skipping.
        [ "${2:-}" = sway ] || platform="$platform and not test(/^sway_harness::/)"
        exec cargo nextest run --workspace --features "$features" --run-ignored all --no-tests=pass -E "$platform" ;;
    "")
        exec cargo nextest run --workspace --features "$features" -E "not ($platform)" ;;
    *)
        echo "usage: test.sh [--doc | --platform [sway]]" >&2; exit 2 ;;
esac
