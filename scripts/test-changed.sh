#!/usr/bin/env bash
# A change outside crates/ that can affect every crate runs the workspace.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

base=${1:-main}
changed=$( { git diff --name-only "$base"...HEAD; git diff --name-only HEAD; git ls-files --others --exclude-standard; } | sort -u)

if grep -qE '^(Cargo\.toml|Cargo\.lock|rust-toolchain\.toml|clippy\.toml|\.config/)' <<< "$changed"; then
    exec cargo nextest run --workspace
fi

pkgs=$(sed -nE 's|^crates/([^/]+)/.*|\1|p' <<< "$changed" | sort -u)
if [ -z "$pkgs" ]; then
    echo "test-changed: no crate changed against $base"
    exit 0
fi

args=()
for p in $pkgs; do args+=(-p "$p"); done
echo "test-changed: ${pkgs//$'\n'/ }"
exec cargo nextest run "${args[@]}"
