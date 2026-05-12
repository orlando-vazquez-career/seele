# Sprint LUMEN-02 — Scope

**Fecha**: 2026-05-12
**Tema**: Re-hacer el frontend SEELE con LUMEN v0.10.0 protocol
**Escala declarada**: **M-L** (vista nueva con redesign completo)
**Protocolo**: LUMEN v0.10.0 (Visual Counsel patch)
**Predecesor**: Sprint LUMEN-01 cerrado 2026-05-12 con outcome "funcionalmente completo, visualmente mediocre"

## Objetivo

Producir un frontend de SEELE que matchee la calidad esperada — específicamente: que no caiga en defaults de "Tailwind genérico" y que tenga la personalidad DevZen (brushed-metal + oro + plata-cyan) **incorporada estructuralmente** en lugar de aplicada como skin.

## Diferencias con LUMEN-01

| Aspecto | LUMEN-01 (v0.9.1) | LUMEN-02 (v0.10.0) |
|---|---|---|
| Material.0 Aesthetic Pillars | _no existía_ | **OBLIGATORIA**: refs + vibe + 3-5 decisiones audaces + composición pre-código |
| Material.1 Variation | _no existía_ | **OBLIGATORIA**: 3-4 sub-agents paralelos producen direcciones radicalmente distintas |
| Gate 1 | UX scaffold | Aesthetic Pillars + Variation pick (decisión visual ANTES de código) |
| Visual Critique Loop | _no existía_ | **OBLIGATORIA**: ≤3 rounds multimodal entre Material y Evidence |
| Visual DNA en specs | _no existía_ | **OBLIGATORIA**: 5 axiomas en cada component spec |
| Sub-agents paralelos | iteración secuencial C → C2 → fix | divergencia real en paralelo |

## Entregables del sprint

1. `docs/design/plans/material/02/aesthetic-pillars.md` — refs reales + vibe + 3-5 audaces + banned + composición
2. `docs/design/plans/material/02/mood-board.md` — análisis de refs + style_vector
3. `docs/design/plans/material/02/variations/*.md` — 3-4 direcciones radicalmente distintas
4. `docs/design/adr-design/wow-NN-*.md` — ADR por decisión audaz aprobada
5. **Re-implementación del frontend** con la dirección elegida en Gate 1 — todos los componentes en `web/src/components/` actualizados
6. `docs/design/critique/02/critique-rounds.md` — gap reports cuantitativos por round
7. `docs/design/evidence/02/` — a11y + perf + heuristic nuevos
8. Devlog cierre + cost-ledger entry

## Fuera de scope

- Cambiar el stack técnico (Astro 6.3 + vanilla CSS + container queries — mantener)
- Funcionalidad nueva (donate widget EIP-1193, SeeleStatus, etc — preservar funcional)
- Donate addresses (las 5 wallets — preservar)
- ENGRAM/MNEMA attribution / Latin signature — preservar
- Internacionalización (sigue solo EN/ES mixed inicial)
- Custom domain (gh-pages.io subdomain por ahora)

## Decisión de Gate 1 humano esperada

Gate 1 = "Aesthetic Pillars approved + 1 of 3-4 variations elegida". Tiempo estimado de revisión: 5-10 minutos del director.

## Costo estimado del sprint

| Fase | Costo USD |
|---|---|
| Material.0 (1 agente — yo) | _gratis_ (parte de esta sesión) |
| Material.1 Variation (3-4 sub-agents Sonnet 4.6) | $1.20-$1.60 |
| Material.3+4 Tokens + Build (yo) | _gratis_ |
| Visual Critique Loop (3 rounds Sonnet 4.6 multimodal) | $0.30-$0.45 |
| Evidence + Narrative | _gratis_ |
| **Total** | **~$1.50-$2.00 USD** |

## Riesgos identificados

| # | Riesgo | Mitigación v0.10.0 |
|---|---|---|
| R1 | Caer otra vez en defaults Tailwind | Material.0 + wow-vocabulary.md |
| R2 | Variation sub-agents producen variantes del mismo concepto | Prompts con sesgo aesthético MUY distinto (Brutalist vs Suiza vs Editorial vs Industrial) |
| R3 | Critique loop self-confirmation | Gap reports cuantitativos vs moodboard, no narrativos |
| R4 | Scope creep (otra vez 6+ bloques) | Hard cap 4 bloques: Material.0+1 → Material.3+4 → Critique → Cierre |
| R5 | Director aprueba sin convicción ("bueno") | Disclaimer obligatorio del director sobre horas de diseño mirado |
