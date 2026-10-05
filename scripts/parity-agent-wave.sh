#!/usr/bin/env bash
# Operator helper: run gate; on failure print wave-3 agent table and exit 1.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WAVE="${1:-3}"
if "$ROOT/scripts/parity-loop.sh" 1; then
  echo "parity: P06–P16 complete"
  exit 0
fi
echo ""
echo "=== spawn ${WAVE} parallel Cursor agents (see docs/PARITY-WAVE${WAVE}.md) ==="
rg '^\| [0-9]+ \|' "$ROOT/docs/PARITY-WAVE${WAVE}.md" 2>/dev/null || true
exit 1
