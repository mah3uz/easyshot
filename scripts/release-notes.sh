#!/usr/bin/env bash
# release-notes.sh VERSION [CHANGELOG]: one release's section of the changelog, as its
# release notes. "Unreleased" shows what is coming.
set -euo pipefail
v=${1:?usage: release-notes.sh VERSION [CHANGELOG]}
file=${2:-$(dirname "$0")/../CHANGELOG.md}
awk -v v="$v" '
  $1 == "##" && found { exit }
  $1 == "##" && $2 == v { found = 1; i = index($0, " - "); if (i) stamp = substr($0, i + 3); next }
  found { lines[++n] = $0 }
  END {
    if (!found) { print "release-notes: no \"## " v "\" in the changelog" > "/dev/stderr"; exit 1 }
    first = 1; while (first <= n && lines[first] == "") first++
    while (n >= first && lines[n] == "") n--
    if (stamp != "" && first <= n) print "_Released " stamp "_\n"
    for (i = first; i <= n; i++) print lines[i]
  }
' "$file"
