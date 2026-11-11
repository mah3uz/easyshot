#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

declare -A layer
for c in es-geom es-model es-protocol; do layer[$c]=0; done
for c in es-text es-anno es-render es-scroll es-ocr; do layer[$c]=1; done
for c in es-store es-share es-wayland es-desktop es-record es-backend; do layer[$c]=2; done
for c in es-ui es-app es-editor es-overlays es-shell es-testkit easyshot; do layer[$c]=3; done

layers_json=$(for c in "${!layer[@]}"; do printf '%s %s\n' "$c" "${layer[$c]}"; done |
    jq -Rn '[inputs | split(" ") | {key: .[0], value: (.[1] | tonumber)}] | from_entries')

violations=$(cargo metadata --format-version 1 --locked | jq -r --argjson layers "$layers_json" '
  (reduce .packages[] as $p ({}; .[$p.id] = $p.name)) as $name
  | (reduce .resolve.nodes[] as $n ({};
      .[$n.id] = [$n.deps[] | select(any(.dep_kinds[]; .kind != "dev")) | .pkg])) as $adj
  | def reach($s):
      {seen: {}, todo: [$s]}
      | until(.todo | length == 0;
          .todo[0] as $x | .todo |= .[1:]
          | if .seen[$x] then . else .seen[$x] = true | .todo += $adj[$x] end)
      | [.seen | keys[] | $name[.]];
    .workspace_members[] as $m
  | $name[$m] as $me
  | if $layers[$me] == null then "\($me): not in the layer table (scripts/layers.sh)" else
      $layers[$me] as $l
      | reach($m)[] as $dep
      | select($dep != $me)
      | (
          if $l <= 2 and ($dep | test("^gpui")) then "layer \($l) crate \($me) reaches gpui"
          elif $l <= 2 and $dep == "es-ui" then "layer \($l) crate \($me) reaches es-ui"
          elif $l <= 1 and ($dep | IN("tokio", "zbus", "wayland-client", "ashpd")) then "layer \($l) crate \($me) reaches \($dep)"
          elif $layers[$dep] != null and $layers[$dep] > $l then "layer \($l) crate \($me) reaches layer \($layers[$dep]) crate \($dep)"
          elif $dep == "es-testkit" then "\($me) depends on es-testkit outside dev-dependencies"
          else empty end
        )
    end
' | sort -u)

if [ -n "$violations" ]; then
    echo "layers: violations" >&2
    printf '  %s\n' "${violations//$'\n'/$'\n'  }" >&2
    exit 1
fi
echo "layers: ok"
