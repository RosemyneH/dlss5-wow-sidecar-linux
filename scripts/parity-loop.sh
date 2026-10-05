#!/usr/bin/env bash
# Re-run parity gate until docs/PARITY.md has no open P06–P18 rows (or max rounds).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAX_ROUNDS="${1:-10}"

for round in $(seq 1 "$MAX_ROUNDS"); do
  echo "=== parity loop round $round / $MAX_ROUNDS ==="
  cargo test --workspace
  open="$(rg '\| (todo|partial) \|' "$ROOT/docs/PARITY.md" | rg 'P(0[6-9]|1[0-8])' || true)"
  if [[ -z "$open" ]]; then
    echo "parity gate: all P06–P18 marked done"
    exit 0
  fi
  echo "$open"
  echo "still open — spawn another agent batch or continue implementation"
  exit 1
done

echo "max rounds reached without full parity"
exit 1
