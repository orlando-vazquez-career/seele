# Sprint LUMEN-01 — landing + donate widget (cierre honesto)

**Fechas**: 2026-05-11 → 2026-05-12
**Tema**: primer frontend de SEELE — landing one-page + donate widget cripto + SEELE detection
**Protocolo**: LUMEN v0.9.1 (génesis sprint piloto)
**Resultado**: ⚠ **funcionalmente completo, visualmente mediocre** — registrado honestamente

---

## TL;DR — la verdad sin adornar

Sprint piloto cerró con landing build de 12 KB gzipped, donate widget EIP-1193 funcional, SEELE detection via fetch local, WCAG 2.2 AA pass, perf budget cumplido. **Pero** el resultado visual final fue descripto por el director como "diseño tan básico sacado directo de Tailwind me da cosita" en la primera iteración y "triángulo negro sin contexto al medio y encima corrido no queda bien" en la segunda iteración.

LUMEN v0.9.1 falló en su promesa central: *"diseños deslumbrantes con pocos prompts"*. Esto disparó un counsel MNEMA completo (5 advisors + 5 reviewers + verdict + Cloven post-review) que produjo el verdict `vrd_2026-05-12_lumen-v0.10.0`. Persistido en SEELE bajo `project=mnema`.

**Decisión de cierre**: este sprint se cierra con la landing actual como **baseline funcional sin pretender excelencia visual**. Sprint LUMEN-02 (con protocolo v0.10.0) re-hace el frontend usando Material.0 + sub-agents paralelos + critique loop + validación retroactiva del template.

---

## Bloques ejecutados

| Bloque | Tema | Commit | Estado |
|---|---|---|---|
| A | Lens + UX scaffold (LUMEN fases 1-2) — scope, persona Marisol, JTBD, journey, audit, perf budget, sitemap, flows, ORCA, wireframes textuales | `0d8e372` | ✓ |
| B | Material setup — Astro 6.3.1 + DESIGN.md v0.1.0 + tokens.css (OKLCH layer) + Layout placeholder | `e761152` | ✓ |
| C | Components + donate widget v1 — 8 componentes (Header/Hero/Features/FeatureCard/Install/InstallCard/Support/DonateButtons/Footer) + URI-scheme widget | `9512334` | ✓ |
| D | Evidence + CI deploy — a11y/perf/heuristic reports + `.github/workflows/deploy-web.yml` | `836a32d` | ✓ |
| C2 | DevZen brand + EIP-1193 + SEELE detect + responsive — pivote completo tras feedback usuario | `3c62367` | ✓ |
| C2-fix | Remove hero triangle + scroll-margin-top sections | `6f0ee53` | ✓ |
| **E** | **Cierre honesto + state-sync + retrospectiva** | _este commit_ | ⏳ |

---

## Qué se construyó

### Software entregado y funcional
- `web/` — Astro 6.3.1 static site, build 12.3 KB gzipped, deploy-ready
- `DESIGN.md` v0.2.0 al root con tokens primitives + semantics OKLCH (DevZen brand)
- 10 componentes Astro: Layout · Header · Hero · Features · FeatureCard · Install · InstallCard · Support · DonateButtons · Footer · SeeleStatus · Triangle
- `DonateButtons.astro`: EIP-6963 multi-provider discovery + EIP-1193 flow para EVM (eth_requestAccounts → wallet_switchEthereumChain → eth_sendTransaction) + BIP-21 URI scheme para BTC + Solana Pay para SOL + clipboard fallback
- `SeeleStatus.astro`: detección local backend via `fetch(localhost:7777/health, mode='no-cors')` con cache 30s sessionStorage
- `.github/workflows/deploy-web.yml`: Action pipeline Node 20 + npm ci + astro build + actions/deploy-pages@v4 (pending repo público para correr)

### Decisiones de diseño tomadas
- Stack vanilla CSS + OKLCH nativo + container queries (sin Tailwind, sin React islands)
- Brushed-metal effect via linear-gradient + feTurbulence SVG noise inline
- Triangle motif del logo DevZen como SVG inline (`Triangle.astro` con linearGradient oro→cyan)
- Gradient text fills para "SEELE" (S gold · E silver · E cyan · L silver · E gold)
- 5 donate networks con brand colors oficiales

### Evidence collected
- A11y WCAG 2.2 AA — todos los contraste ratios verificados (post BTC glyph fix)
- Perf budget — 12.3 KB gzipped total (50% del soft budget de 25 KB, dentro del hard de 50 KB)
- Heuristic Nielsen — 9/10 verde, 1 amarillo documentado
- Build verde con 0 vulnerabilities post Astro 5→6 bump

---

## Qué falló — autoanálisis del director (sin endulzar)

### 1. El protocolo no forzó dirección visual antes de código
Pipeline `Lens → UX scaffold (ASCII wireframes) → Gate 1 → Material → tokens → componentes`. El primer pixel real lo vio el humano POST-Material en navegador. Costo de revertir código ~10× mayor que rechazar moodboard. **Material.0 (Aesthetic Pillars) llega con LUMEN v0.10.0 a moverlo 1 fase atrás.**

### 2. Iteraciones secuenciales pagaron paralelismo sin obtenerlo
Bloque C → C2 → C2-fix: tres iteraciones secuenciales del mismo concepto. Si hubieran sido 3 sub-agents paralelos con direcciones ortogonales (brutalist editorial / swiss precision / brushed-metal premium), humano elegía 1 en 1 gate barato. **Cloven dijo "cobardía disfrazada de prudencia" sobre haberlos descartado del verdict original — corrección aplicada en v0.10.0.**

### 3. DESIGN.md captura tokens pero no DNA visual
Tokens primitives + semantics es contrato API, no dirección de arte. Textura/motion-curve/hierarchy-ratio/border-grammar no estructurados. **Visual DNA extension a Coverage+Validation llega con v0.10.0** (los 5 axiomas: coherencia + diferenciación + densidad + acabado + personalidad emergente).

### 4. Wireframes textuales ASCII destruyen info visual
Codifican slot+jerarquía, no composición. Agente satisface el wireframe y rellena gaps con training-distribution defaults = Tailwind-default. **Para LUMEN-02 se reemplazan por moodboards de referencias reales.**

### 5. Triángulo decoration centered-without-context
Aplicado por iniciativa del agente sin que estuviera en el brief original. Sobrevivió la primera Material build porque no había Aesthetic Pillars que lo vetaran. **El test de calibración del template aesthetic-pillars.md será: ¿este triángulo sobrevive a su sección "Decisiones audaces"? Si no, template bien calibrado.**

### 6. Restricciones solo negativas, no positivas
DESIGN.md prohíbe Inter, marketing speak, carousels. LLM optimiza dentro del espacio admisible que queda = sigue siendo modal del corpus = Tailwind-genérico. **wow-vocabulary.md (con replace-with positivos, no solo bans) en v0.10.0.**

---

## MNEMA counsel disparado por este fallo

Persistido en SEELE bajo `project=mnema`:

| Observation | Type | Topic key |
|---|---|---|
| MNEMA Verdict — LUMEN v0.10.0 Visual Counsel | `verdict` | `mnema/verdict/lumen-v0.10.0` |
| 5 advisor outputs (Contrarian/First-Principles/Expansionist/Outsider/Executor) | `advisor_output` | `mnema/advisor/*` |
| 5 blind reviews | `review` | `mnema/review/r{1..5}` |
| Cloven post-verdict review (6 findings) | `review` | `mnema/cloven-review/lumen-v0.10.0` |

Costo del counsel: ~$11 USD estimado. Ledger en `C:/dev/protocols/MNEMA/cost-ledger.jsonl`.

**Resumen del verdict**: LUMEN v0.9.1 → v0.10.0 con 4 cambios estructurales:
1. Material.0 Aesthetic Pillars sub-fase antes de tokens
2. Visual Critique Loop con visión multimodal entre Material y Evidence
3. Visual DNA extension a Coverage + Validation (5 axiomas)
4. wow-vocabulary.md + sección "Sobre el ojo del director"

**Cloven encontró 6 gaps** en el verdict original que se aplican antes de bumpear protocolo (Fase 2 de hoy, no parte de este sprint).

---

## Estado del Bloque D Evidence

Las evidences de Bloque D (`docs/design/evidence/01/{a11y-report,perf-report,heuristic-eval}.md`) se preservan como **histórico válido**. Reflejan el estado real medido del sprint. Para LUMEN-02 se generarán evidences nuevas; no se sobrescriben las del 01.

---

## Lo que NO se completó (deferred a sprintes siguientes)

- ⏸ **Visual regression Playwright** — deferred. Requiere setup playwright + baselines. No bloquea cierre.
- ⏸ **Lighthouse remoto real** (deploy-web.yml todavía no corrió por repo privado). Numbers en `perf-report.md` son proyecciones.
- ⏸ **Flip público + deploy** — Fase 4 separada de este sprint
- ⏸ **`aria-live` en donate status** — non-blocking AA, queda como v0.2 nit
- ⏸ **Re-diseño completo del frontend con dirección elegida** — Sprint LUMEN-02

---

## Lessons learned (a memoria persistente)

1. **El protocolo de diseño es un protocolo de DIRECCIÓN DE ARTE, no de ingeniería**. Frame mental del agente importa más que cantidad de specs.
2. **Iteraciones secuenciales no producen lo mismo que sub-agents paralelos**. Ambos cuestan parecido pero los segundos producen DIVERGENCIA real.
3. **El test de un template de diseño es validación retroactiva contra failures conocidos**. Si aesthetic-pillars.md no hubiera detectado el triángulo de SEELE-01, no está calibrado.
4. **Anchorage visual externo es no-negociable**. Empezar por moodboard de refs reales, no por tokens primitivos.
5. **Restricciones positivas > restricciones negativas**. "No usar Inter" desplaza el mediocre; "Usá Editorial New y romp el grid con esta textura" produce carácter.
6. **El "ojo del director" pone techo, el protocolo pone piso**. Cloven lo dijo: si miro diseño 2 hrs/día durante 6 meses, mi protocolo va a producir wow sin cambiar una línea de código. La herramienta es mi ojo.

---

## Cost-ledger entry

Append a `docs/design/devlogs/cost-ledger.jsonl`:

```json
{"date":"2026-05-12","sprint":"LUMEN-01","phases_executed":["Lens","UX scaffold","Material","Evidence","Narrative"],"model":"claude-opus-4-7","commits":7,"loc_added":2800,"bundle_gzipped_kb":12.3,"counsel_triggered":true,"counsel_verdict_id":"vrd_2026-05-12_lumen-v0.10.0","outcome":"functionally_complete_visually_mediocre","next":"Sprint LUMEN-02 with v0.10.0"}
```

---

## Próximo paso — Fase 2 hoy

Antes de Sprint LUMEN-02, aplicar los 6 findings de Cloven al verdict y bumpear LUMEN-PROTOCOL.md v0.9.1 → v0.10.0. Crear los 4 templates con validación retroactiva del aesthetic-pillars contra el fallo de este sprint.

Ver `mnema/cloven-review/lumen-v0.10.0` en SEELE para los findings detallados.

— Fin sprint LUMEN-01
