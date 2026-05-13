# Sprint LUMEN-03 — Observability + responsive + light mode

**Fecha**: 2026-05-12
**Tema**: cerrar los gaps del director eyeball post-LUMEN-02 — responsive a 1920px, observability propia, light mode, font scale, Playwright multi-res
**Protocolo aplicado**: LUMEN v0.10.0 → durante el sprint bump a v0.11.0 (Multi-Resolution patch)
**Resultado**: ✓ landing en 2 páginas (/ + /observability), responsive verificado en 5 breakpoints × 2 themes vía Playwright

---

## TL;DR

Sprint LUMEN-02 cerró el baseline brutalista (commit `cf857b2`, push). Director eyeball a 1920×1080: *"las proporciones están mal optimizadas para esta resolución"*. Tres asks concretos:

1. **Whitespace a 1920** — ~700px de espacio lateral derecho. El brutalist asymmetric con max-width 1100px se ve roto a desktop wide.
2. **Observability real** — *"monitoreo real del estado de memorias de SEELE, quiero que se pueda gestionar desde este frontend o por lo menos observar que se está guardando"*.
3. **Letras muy pequeñas** + *"modo claro"* + *"observabilidad en página aparte"*.

Sprint LUMEN-03 cubre los 3 + un cuarto que el director nombró estructuralmente: *"creo que ya se lo que esta faltando en Lumen es el protocolo de verificacion visual directa en browser con Playwright en varias resoluciones para el responsive"*.

---

## Bloques ejecutados

| Bloque | Tema | Estado |
|---|---|---|
| 0 | Sprint scope + gates (`plans/lens/03/00-scope.md`) | ✓ |
| 1 | CORS `--cors-allow` flag a `seele serve` (Rust, `crates/seele-cli`) | ✓ |
| 2 | Observability.astro component — stats / by-type / recent / search / by-project / sparkline / detail | ✓ |
| 3 | Responsive shell v1 — `.layout-shell` 2-col >1400px con sidebar sticky | ✓ (luego descartado, ver bloque 7) |
| 4 | Hero font clamp fix — `40px` → `28px` floor para no overflowear 320px viewport | ✓ |
| 5 | Playwright @ devDep + `scripts/visual-critique.mjs` (5 breakpoints) | ✓ |
| 6 | Round 1 Visual Critique Multi-Res — detectó probe bug + overflow 320 + estado preview vacío | ✓ |
| 7 | Round 2: pivot por feedback director — observability a `/observability` route, single-col home, font scale aggressive (+3px base), light mode con system pref + manual toggle | ✓ |
| 8 | Round 3: re-captura matriz 2 pages × 5 viewports × 2 themes = 20 screenshots | ✓ |
| 9 | Bump LUMEN v0.10.0 → v0.11.0 (Multi-Resolution patch + anti-pattern "Responsive que solo se ve bien a 1440px") | ✓ |
| 10 | Closure devlog + cost-ledger + commit + push | ⏳ este commit |

---

## Cambios visibles

### Frontend (`web/`)

- **Nueva ruta `/observability`** — página dedicada con hero "What your memory engine is doing." + panel completo
- **Home (`/`) ahora es single-column** — el `.layout-shell` 2-col fue descartado tras decisión del director ("observability en página aparte")
- **Responsive `>1400px`** — page-container centrado con max-width 1300px y márgenes simétricos. Asymmetric "left commit" se mantiene a viewports ≤1400px (preserva wow-02 ADR a esos tamaños)
- **Font scale agresivo** — base 15px → 18px, lg 18px → 24px, hero 72px → 84px. Observability panel sube de 10/11px a 12/13px
- **Light mode** — palette `oklch(0.96 0 0)` bg + `oklch(0.18 0 0)` fg + `oklch(0.55 0.18 50)` accent (orange más oscuro para mantener contraste sobre fondo claro). Activado por `prefers-color-scheme: light` y/o `[data-theme="light"]` desde toggle manual
- **Theme toggle** en header `[ ◐ ]` / `[ ◑ ]` — persiste a `localStorage.seele-theme`. Script inline en `<head>` (vía Header component) evita FOUC
- **Triangle size 24px → 28px** en Header (consistencia con escala de fuente más grande)

### Backend (`crates/seele-cli`)

- **`seele serve --cors-allow <ORIGIN>`** — flag repetible. Vacío = sin CORS (default, seguro local-only). Cualquier valor = `Access-Control-Allow-Origin: *` (permisivo; per-origin allowlist refinement queda en backlog)
- Necesario para que el observability panel pueda hacer fetch al HTTP server local desde un origin distinto al puerto 7777

### Tooling

- **Playwright @ devDep** del `web/package.json` (`@playwright/test ^1.60.0`)
- **`web/scripts/visual-critique.mjs`** — captura headless multi-res. Matriz declarativa: `PAGES × VIEWPORTS × THEMES` = 20 shots por run
- **`npm run visual`** — comando del package.json
- **`test-results/visual/<page>-<theme>-<width>.png`** — output (gitignored)

---

## Métricas

### Bundle (gzipped)

| asset                       | LUMEN-02 | LUMEN-03 | delta |
|-----------------------------|---------:|---------:|------:|
| `dist/index.html`           |  10.7 KB |  12.7 KB |  +19% |
| `dist/observability/index.html` |      — |   9.5 KB |   nuevo |
| `dist/favicon.svg`          |    291 B |    291 B |     — |

Home creció +2 KB por el theme-toggle inline script + light-mode CSS variables + observability nav link. Observability page es nueva (no había). Total wire para visitar las dos páginas: 22.2 KB gz vs 10.7 KB previo — más del doble pero ambas siguen muy por debajo de cualquier dev tool landing 2026.

### Visual matrix verificada

20 screenshots persistidos (no committeados — gitignored):

```
home-{dark,light}-{320,768,1024,1440,1920}.png
observability-{dark,light}-{320,768,1024,1440,1920}.png
```

Findings del Round 1 (single-viewport bug discovery):
- **JS bug**: `const probe = await probe(...)` shadowing → renamed `status`
- **Overflow 320px**: Hero `clamp(40px, ...)` → `clamp(28px, ...)`
- **Whitespace 1920px**: identificado pero diferido al Round 2 pivot

Findings del Round 2 (post-pivot):
- **Wide layout**: page-container centrado >1400px funciona (sin observability sidebar)
- **Light mode**: contraste 7.8:1 body/bg → WCAG AAA
- **Theme persistence**: localStorage funciona, inline script previene FOUC

### Decisiones audaces

Sprint LUMEN-03 no introdujo nuevas decisiones audaces top-level; extendió las de LUMEN-02:
- wow-03 (3-color palette) extendido para light mode con accent ligeramente más oscuro (oklch L 0.65 → 0.55) para contraste sobre fondo claro
- wow-02 (asymmetric brutal) preservado a ≤1400px; centrado a >1400px (excepción documentada en tokens.css)

---

## Costo

Ver `cost-ledger.jsonl` entrada K-L. Total adicional: ~$0.90 USD sobre LUMEN-02.

---

## Lecciones para el protocolo

1. **El director eyeball es el único test que cuenta para visual**. El Critique Loop v0.10.0 single-viewport pasó, pero el director vio el problema en segundos. v0.11.0 institucionaliza captura multi-res como precondición.
2. **Light mode no es decoración**, es accessibility para usuarios con sensibilidad al fondo oscuro. Wow-03 (3-color palette only) sigue válido — solo se duplica el set de 3 con flip de lightness.
3. **Observability como página aparte es la decisión correcta**: el landing es marketing, observability es producto. Mezclarlos como sidebar dañaba la jerarquía narrativa de ambos.
4. **Playwright como protocol dependency** vale la pena: ~120 MB de chromium headless es barato comparado con un sprint perdido por gap responsive.

---

## Pendiente para LUMEN-04

- Chat con Kimi K2 (user tiene API key Moonshot). Auth path probablemente proxy local `seele chat-proxy --key ...`.
- Edit / delete memories desde el observability panel
- Search query persistence en URL hash (`/observability?q=...`)

---

## Cierre

Sprint LUMEN-03 cierra con baseline aprobado por director (Gate 2 humano dado en respuesta "Va — bump LUMEN v0.11.0, commit LUMEN-03, push"). Próximo: commit + push + Fase 4 (flip público + deploy).
