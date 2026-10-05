#!/usr/bin/env bash
# Mirror .github/workflows/ci.yml locally (fmt, clippy -D warnings, workspace tests).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
