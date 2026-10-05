#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
echo "=== wowsidecar-linux parity ==="
rg '^\| P[0-9]+' "$ROOT/docs/PARITY.md" | while read -r line; do
  if [[ "$line" == *"| todo |"* ]] || [[ "$line" == *"| partial |"* ]]; then
    echo "OPEN  $line"
  fi
done
echo "=== crates ==="
ls -1 "$ROOT/crates" 2>/dev/null || true
echo "=== build ==="
cargo build --workspace 2>&1 | tail -3
