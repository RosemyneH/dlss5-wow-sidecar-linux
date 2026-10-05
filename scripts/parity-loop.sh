#!/usr/bin/env bash
# Gate for multi-agent parity: test, print status, fail while P06–P16 are open.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAX_ROUNDS="${1:-10}"

open_p06_p16() {
  rg '\| (todo|partial) \|' "$ROOT/docs/PARITY.md" | rg 'P(0[6-9]|1[0-6])' || true
}

for round in $(seq 1 "$MAX_ROUNDS"); do
  echo "=== parity loop round $round / $MAX_ROUNDS ==="
  cargo test --workspace
  "$ROOT/scripts/parity-status.sh"
  open="$(open_p06_p16)"
  if [[ -z "$open" ]]; then
    echo "parity gate: P06–P16 complete (P17–P18 may still be open)"
    exit 0
  fi
  echo "$open"
  echo "still open — spawn another agent batch or continue implementation"
  exit 1
done

echo "max rounds reached without P06–P16 parity"
exit 1
