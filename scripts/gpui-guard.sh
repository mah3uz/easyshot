#!/usr/bin/env bash
# The same version appearing for host and target is fine.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

dups=$(cargo tree -d --workspace --target all --depth 0 --prefix none --locked |
    awk '$1 ~ /^(gpui|zbus)/ { sub(/:$/, "", $2); print $1, $2 }' | sort -u |
    awk '{ n[$1]++; v[$1] = v[$1] " " $2 } END { for (c in n) if (n[c] > 1) print c ":" v[c] }')

if [ -n "$dups" ]; then
    echo "gpui-guard: more than one version in the graph:" >&2
    printf '  %s\n' "$dups" >&2
    exit 1
fi
echo "gpui-guard: ok"
