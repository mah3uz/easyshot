#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

limit=600
# Files allowed over the limit, one "path  reason" per line.
exceptions=''

bad=$(find crates -name '*.rs' -not -path '*/vendor/*' -not -path '*/target/*' -print0 |
    xargs -0 wc -l | grep -v ' total$' |
    awk -v limit="$limit" '$1 > limit { print $1 " " $2 }' || true)

while read -r path _; do
    [ -n "$path" ] && bad=$(printf '%s\n' "$bad" | grep -vF " $path" || true)
done <<<"$exceptions"

if [ -n "$bad" ]; then
    echo "file-size: files over $limit lines; split them into modules" >&2
    printf '%s\n' "$bad" >&2
    exit 1
fi
