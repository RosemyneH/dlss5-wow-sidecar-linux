#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

linux_cell() {
  echo "$1" | awk -F'|' '{gsub(/^ +| +$/,"",$5); print $5}'
}

echo "=== wowsidecar-linux parity ==="
rg '^\| P[0-9]+' "$ROOT/docs/PARITY.md" | while IFS= read -r line; do
  linux="$(linux_cell "$line")"
  if [[ "$linux" == yes* ]]; then
    continue
  fi
  echo "OPEN  $line"
done
echo "=== crates ==="
ls -1 "$ROOT/crates" 2>/dev/null || true
echo "=== build ==="
cargo build --workspace 2>&1 | tail -3
