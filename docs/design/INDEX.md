# SEELE — docs/design INDEX

Ledger de sprints LUMEN ejecutados en este repo. Cada sprint cierra con devlog acá listado.

## Sprints

| Sprint | Estado | Tema | Devlog | Outcome |
|---|---|---|---|---|
| LUMEN-01 | ✓ cerrado 2026-05-12 | Landing + donate widget (génesis frontend SEELE) | [`2026-05-12-sprint-lumen-01.md`](./devlogs/2026-05-12-sprint-lumen-01.md) | Funcionalmente completo, visualmente mediocre. Disparó MNEMA counsel → LUMEN v0.10.0. |
| LUMEN-02 | ✓ cerrado 2026-05-12 | Brutalist dev-craft re-design con v0.10.0 protocol + 4 sub-agents paralelos | [`2026-05-12-sprint-lumen-02.md`](./devlogs/2026-05-12-sprint-lumen-02.md) | Variation 02 elegida. Landing v0.3.0: 10.7 KB gz, 3-color palette, JetBrains Mono, motion zero. Pendiente: Sprint LUMEN-03 con observability + responsive fix antes de flip público. |
| LUMEN-03 | ✓ cerrado 2026-05-12 | Observability como página aparte + responsive multi-res + light mode + font scale + Playwright matrix | [`2026-05-12-sprint-lumen-03.md`](./devlogs/2026-05-12-sprint-lumen-03.md) | Landing v0.4.0 + /observability route. Bundle home 12.7 KB gz, observability 9.5 KB gz. Light mode + dark mode con system pref + manual toggle. LUMEN bump a v0.11.0 (Multi-Resolution patch). |
| LUMEN-04 | _pending_ | Kimi K2 chat against SEELE DB (deferred) | _pending_ | _pending_ |

## DESIGN.md

El contrato visual vive en [`/DESIGN.md`](../../DESIGN.md). Versionado semver propio.

Versiones:
- v0.1.0 (2026-05-11) — Initial primitives + semantics (LUMEN-01 Bloque B)
- v0.2.0 (2026-05-12) — DevZen brand alignment: OKLCH color system, brushed-metal + triangle motif, gold + silver-cyan dual accent, EIP-1193 + EIP-6963, SeeleStatus, container queries (LUMEN-01 Bloque C2)
- v0.3.0 (2026-05-12) — Brutalist dev-craft: 3-color palette only (bg / fg / accent), JetBrains Mono single family, 2px solid borders, incomplete borders as visual syntax, transition zero, no gradients, no shadows (LUMEN-02 Variation 02)
- v0.4.0 (2026-05-12) — Multi-page + light mode: font scale aggressive (base 18 / lg 24 / hero 84), `prefers-color-scheme` + `data-theme` toggle, light palette (bg `oklch(0.96 0 0)` / fg `oklch(0.18 0 0)` / accent `oklch(0.55 0.18 50)`), centered page-container >1400px, `/observability` route separada (LUMEN-03)

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

LUMEN v0.11.0 — `C:/dev/protocols/LUMEN/LUMEN-PROTOCOL.md` (Multi-Resolution patch aplicado 2026-05-12; Playwright headless + 5 breakpoints obligatorios en Fase 3.5 Visual Critique Loop).

## ADRs visuales

Sprint LUMEN-02 produjo 5 ADRs en `plans/executed/material/02/adr/`:

- `wow-01-monospace-only.md` — JetBrains Mono única familia
- `wow-02-asymmetric-brutal.md` — page-container con márgenes asimétricos
- `wow-03-3-color-palette.md` — solo bg + fg + accent
- `wow-04-incomplete-borders.md` — bordes en 3 lados como sintaxis visual
- `wow-05-motion-zero.md` — `transition: none` global

## Sub-agents y variations exploradas

Sprint LUMEN-02 lanzó 4 sub-agents Sonnet 4.6 paralelos. 3 variations rechazadas persistidas en SEELE (project=mnema):

- `lumen/sprint-02/variation-01-rejected` — Brushed-metal editorial
- `lumen/sprint-02/variation-03-rejected` — Studio Suizo cold-precision
- `lumen/sprint-02/variation-04-rejected` — Editorial monumental warm
