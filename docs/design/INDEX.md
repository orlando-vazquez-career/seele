# SEELE — docs/design INDEX

Ledger de sprints LUMEN ejecutados en este repo. Cada sprint cierra con devlog acá listado.

## Sprints

| Sprint | Estado | Tema | Devlog | Outcome |
|---|---|---|---|---|
| LUMEN-01 | ✓ cerrado 2026-05-12 | Landing + donate widget (génesis frontend SEELE) | [`2026-05-12-sprint-lumen-01.md`](./devlogs/2026-05-12-sprint-lumen-01.md) | Funcionalmente completo, visualmente mediocre. Disparó MNEMA counsel → LUMEN v0.10.0. |
| LUMEN-02 | _pending_ | Re-hacer frontend SEELE con v0.10.0 protocol + sub-agents paralelos | _pending_ | _pending_ |

## DESIGN.md

El contrato visual vive en [`/DESIGN.md`](../../DESIGN.md). Versionado semver propio.

Versiones:
- v0.1.0 (2026-05-11) — Initial primitives + semantics (LUMEN-01 Bloque B)
- v0.2.0 (2026-05-12) — DevZen brand alignment: OKLCH color system, brushed-metal + triangle motif, gold + silver-cyan dual accent, EIP-1193 + EIP-6963, SeeleStatus, container queries (LUMEN-01 Bloque C2)

## MNEMA counsel disparado

Sprint LUMEN-01 disparó counsel completo (5 advisors + 5 reviewers + verdict + Cloven post-review) sobre cómo mejorar LUMEN protocolo. Verdict: `vrd_2026-05-12_lumen-v0.10.0`. Persistido en SEELE bajo `project=mnema`. Cost-ledger en `C:/dev/protocols/MNEMA/cost-ledger.jsonl`.

## Estructura

- `plans/lens/<N>/` — research, personas, JTBD, journey, audit, perf budget.
- `plans/scaffold/<N>/` — sitemap, flows, ORCA, wireframes textuales.
- `plans/executed/` — planes cerrados (read-only, histórico).
- `components/<Name>.md` — component specs (Coverage + Validation).
- `evidence/<N>/` — a11y, perf, heuristic, visual regression del sprint.
- `devlogs/<YYYY-MM-DD>-<sprint>.md` + `cost-ledger.jsonl`.

## Protocolo

LUMEN v0.9.1 — `C:/dev/protocols/LUMEN/LUMEN-PROTOCOL.md` (pending bump a v0.10.0 con findings Cloven).
