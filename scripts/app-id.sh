#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
grep -m1 -oE '"dev\.easyshot\.[a-z]+"' crates/es-protocol/src/lib.rs | tr -d '"'
