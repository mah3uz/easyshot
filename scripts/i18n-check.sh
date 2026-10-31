#!/usr/bin/env bash
# Skipped while i18n/ does not exist.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ ! -d i18n ]; then
    echo "i18n-check: no i18n/ directory yet, nothing to check"
    exit 0
fi

fail=0
ftl_ids() { sed -nE 's/^(-?[A-Za-z][A-Za-z0-9_-]*)[[:space:]]*=.*/\1/p' "$1"; }

for ftl in i18n/en-US/*.ftl; do
    [ -e "$ftl" ] || continue
    dups=$(ftl_ids "$ftl" | sort | uniq -d)
    if [ -n "$dups" ]; then
        printf 'i18n-check: duplicate ids in %s: %s\n' "$ftl" "$(echo "$dups" | tr '\n' ' ')" >&2
        fail=1
    fi
done

for dir in crates/*/; do
    crate=$(basename "$dir")
    [ -d "$dir/src" ] || continue
    ftl=i18n/en-US/${crate}.ftl
    ids=$(find "$dir/src" -name '*.rs' -print0 | xargs -0 -r perl -0777 -ne '
        while (/\bfl!\(\s*(?:[^"),]*,\s*)?"([^"]+)"/g) { print "$ARGV: $1\n" }' | sort -u)
    [ -n "$ids" ] || continue
    if [ ! -f "$ftl" ]; then
        printf 'i18n-check: %s uses fl! but %s does not exist\n' "$crate" "$ftl" >&2
        fail=1
        continue
    fi
    known=$(ftl_ids "$ftl")
    while IFS= read -r line; do
        id=${line##*: }
        if ! grep -qxF -- "$id" <<< "$known"; then
            printf 'i18n-check: %s: id "%s" is not in %s\n' "${line%%: *}" "$id" "$ftl" >&2
            fail=1
        fi
    done <<< "$ids"
done

[ "$fail" = 0 ] || exit 1
echo "i18n-check: ok"
