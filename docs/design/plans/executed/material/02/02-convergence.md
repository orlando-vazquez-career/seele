# Material.2 Convergence — Sprint LUMEN-02

**Fecha**: 2026-05-12
**Gate 1**: APROBADO
**Director eligió**: **Variation 2 — Brutalist dev-craft**

---

## Decisión final

Las 4 variations producidas por sub-agents paralelos (Sonnet 4.6) fueron evaluadas. El director Orlando eligió **Brutalist dev-craft** sobre las otras 3:

| Variation | Status | Razón de descarte |
|---|---|---|
| 1. Brushed-metal editorial | descartada | Demasiado parecida al extension safe del LUMEN-01 (DevZen brand). Director quiere ruptura, no fine-tuning. |
| **2. Brutalist dev-craft** | ✓ **ELEGIDA** | Audaz, distinta, alineada con audiencia dev tool real (no marketing premium). |
| 3. Studio Suizo cold-precision | descartada | Austeridad llevada a extremo — perdería el carácter DevZen completamente, demasiado neutral. |
| 4. Editorial monumental warm | descartada | Riesgo identificado por el sub-agent mismo: "proyecto de autor" vs "infraestructura confiable". Mal fit para dev tool. |

## Implicaciones del cambio de dirección

El Aesthetic Pillars original (que asumía continuación brand DevZen) queda **parcialmente refactored**. Las 5 decisiones audaces se reformulan:

### Decisiones audaces FINALES (Brutalist dev-craft)

1. **Typography monospace-only axiomatic** — JetBrains Mono ÚNICO family. Hierarchy via weight (800 hero, 400 body) + size jump (72px → 13px sin escalas intermedias). NO display serif, NO sans-serif. → ADR `wow-01-monospace-only.md`

2. **Asymmetric brutal layout** — left-heavy desequilibrio: margen izquierdo 16px, margen derecho 80px. Sin centrado. Bloques offset deliberadamente. → ADR `wow-02-asymmetric-brutal.md`

3. **Paleta extrema 3-color** — negro absoluto + off-white crudo + naranja eléctrico SOLO accent. Prohibido agregar un 4to color. La restricción es la decisión. → ADR `wow-03-3-color-palette.md`

4. **Bordes 2px sólidos como herramienta jerárquica primaria** — reemplazan TODA shadow, gradient, blur. Bordes INCOMPLETOS (3 lados en lugar de 4) como sintaxis visual de flow entre elementos. → ADR `wow-04-incomplete-borders.md`

5. **Motion: transition:none en todo** — sin interpolación de hover, sin animaciones de load, sin scroll-jacking, sin reveals. Estados binarios cambian instantáneamente. → ADR `wow-05-motion-zero.md`

## Brand DevZen — qué se preserva y qué no

DevZen es la organización dueña de SEELE. El brand tradicional (oro + plata-cyan + brushed metal) **NO se aplica visualmente al landing de SEELE** en LUMEN-02. Se preserva solo:

- Atribución "a DevZen tool · SEELE" en header brand mark (texto monospace, sin gradient)
- Footer attribution "© DevZen SpA" (texto plano, sin metallic accent)
- Triangle del logo DevZen: **solo en header brand mark 24px** (framed, color off-white sólido, sin gradient interior). Eliminado del footer y de cualquier decoration grande.

Esto NO es traición al brand — es honestidad sobre la audiencia de SEELE (devs que detectan design theater) vs la audiencia broader de DevZen (clientes corporativos premium). Cada producto puede tener su propia voz visual dentro del paraguas DevZen.

## Variations no-elegidas — persistencia para futuro

Las 3 variations descartadas se persisten en SEELE como `kind=advisor_output`, `topic_key=design/variation/lumen-02/<bias>`. Quedan disponibles para futuros sprints (LUMEN-03 en adelante) si la dirección requiere pivotear hacia ellas.

## Próximo paso — Material.3 + Material.4

Con la dirección comprometida:
- Material.3: re-escribir DESIGN.md → v0.3.0 (Brutalist brand) + tokens.css completo
- Material.4: re-escribir TODOS los componentes en `web/src/components/` con tokens nuevos
- Visual Critique Loop: ≤3 rounds con screenshot vs variation 2 spec
- Evidence: re-correr a11y + perf (paleta 3-color simplifica contrast checks)
