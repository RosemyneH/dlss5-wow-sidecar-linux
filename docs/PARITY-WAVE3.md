# Parity wave 3 — close to 1:1 (P02, P06, P07, P11)

**Gate:** `./scripts/parity-loop.sh` until P06–P16 Linux cells start with `yes`.

| Agent | ID | Scope | Owner crate |
|-------|-----|--------|-------------|
| 1 | P06 | Auto PipeWire node from Hypr/window hint; fewer env vars | sidecar-capture |
| 2 | P06 | Portal capture UX, `capture-test`, `docs/CAPTURE.md` | sidecar-capture |
| 3 | P07 | Always-on-top borderless overlay synced to WoW rect | sidecar-overlay |
| 4 | P07 | Layer-shell or compositor overlay path (feature flag) | sidecar-overlay |
| 5 | P11 | Neural chain parity with Windows presets (pass counts) | sidecar-neural |
| 6 | P11 | Doctor/manager neural backend visibility | sidecar-neural + manager |
| 7 | P02 | Hypr/Sway discovery = HWND equivalent; PARITY `yes` | sidecar-core |
| 8 | — | `wowsidecar-manager` clippy `-D warnings` — **done** | wowsidecar-manager |
| 9 | — | Runtime pipeline + mock capture hardening | sidecar-runtime |
| 10 | — | Workspace clippy CI + `PARITY.md` rows — **done** | repo root |

After each wave: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
