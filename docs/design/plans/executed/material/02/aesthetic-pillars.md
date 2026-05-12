# Aesthetic Pillars — Sprint LUMEN-02

**Fase**: Material.0 (LUMEN v0.10.0)
**Sprint**: 02 (re-hacer frontend SEELE)
**Fecha**: 2026-05-12
**Director**: Orlando Nahuel Vazquez Gonzalez

---

## Disclaimer del director (PENDIENTE FIRMA)

> ⏸ **Director — firmá honestamente o pausa el sprint**:
>
> *"Confirmo que en las últimas 2 semanas miré ___ horas de diseño excelente fuera de pantallas de código. Lista de fuentes consultadas:*
>  *- ___*
>  *- ___*
>  *- ___*
>
> *Esta sección no es ritualista — el counsel MNEMA estableció que el ojo del director pone techo a la calidad del output. Si X<5 hrs, este sprint produce mediocridad sistémicamente independiente del protocolo."*

**Pre-sprint check del orquestador**: Orlando ha estado profundamente inmerso en código + protocolo + counsel MNEMA durante las últimas 48 hrs. Las horas de "mirar diseño excelente" recientes son probablemente limitadas. **Si la respuesta honesta es <5 hrs, el protocolo recomienda pausar 30-60 min de curación visual antes de proceder.** Si Orlando firma "sí, suficiente horas", procedemos sin más discusión.

---

## 1. Referencias visuales

**Propuesta del orquestador** (5 refs). Director revisa, swap si querés, agregá tus propias. Mínimo 3 deben sobrevivir tu cull.

### Ref 1 — read.cv (editorial monumental)
- **URL**: https://read.cv/
- **Screenshot path**: `docs/design/refs/02/01-read-cv.png` _(pendiente captura)_
- **Qué tomamos**:
  - Type hierarchy extrema: H1 display serif italic ~80px sobre body sans 14px (ratio ~5.7×)
  - Asimetría con generous left margin
  - Caption metadata rotated 90° en margen — pattern de revista editorial
- **Qué NO tomamos**: su paleta paper-white (nosotros vamos dark brushed-metal)
- **Por qué este**: matchea con "editorial monumental" del vibe — tensión high-end vs technical

### Ref 2 — pitch.com (luxury dark + metallic accents)
- **URL**: https://pitch.com/
- **Screenshot path**: `docs/design/refs/02/02-pitch.png` _(pendiente captura)_
- **Qué tomamos**:
  - Brushed metallic surfaces en cards/buttons (no flat shadows)
  - Hover responses sutiles (cambio de color tinte, no transform)
  - Paleta dark cool con accent warm — exacto inverso o paralelo a nuestro gold+cyan
- **Qué NO tomamos**: su layout es centered max-width (anti-pattern para nosotros)
- **Por qué este**: prueba que dark luxury con metal funciona sin ser cheesy

### Ref 3 — modal.com (dev tool premium honesto)
- **URL**: https://modal.com/
- **Screenshot path**: `docs/design/refs/02/03-modal.png` _(pendiente captura)_
- **Qué tomamos**:
  - Code blocks como ciudadanos primarios (no decorativos)
  - Density information calibrated — código real, no marketing speak
  - Typography: monospace dominant pero con jerarquía con sans display
- **Qué NO tomamos**: su paleta cool blue / verde tech (nosotros gold warm)
- **Por qué este**: matchea con la audiencia dev de SEELE — Marisol Persona

### Ref 4 — typewolf "metallic" featured site
- **URL**: https://www.typewolf.com/site-of-the-day
- **Screenshot path**: `docs/design/refs/02/04-typewolf-feat.png` _(pendiente captura)_
- **Qué tomamos**:
  - Curación typography-first — confirma que el orden axiomático "tipografía primero, todo lo demás se acomoda" funciona
  - Examples de sites donde display typeface es decisión audaz, no default
- **Por qué este**: curaduría reciente independiente de gusto del orquestador

### Ref 5 — works.studio o agency portfolio similar
- **URL**: https://works.studio/ (alternativa: https://order.design/, https://locomotive.ca/)
- **Screenshot path**: `docs/design/refs/02/05-works-studio.png` _(pendiente captura)_
- **Qué tomamos**:
  - Asymmetric grids como default, no excepción
  - Composición que prioriza una decisión audaz por viewport
  - Motion vocabulary sutil pero consistente
- **Qué NO tomamos**: su scroll-jacking exuberante (perf budget no permite)
- **Por qué este**: standard de "agencia premium" tipo studio brand

**Anti-refs** explícitamente NO usadas (conforme `guides/wow-vocabulary.md`):
- ❌ Linear.app, Stripe.com, Vercel.com, Notion.so (banned por sobre-uso)
- ❌ shadcn/ui examples (training distribution del LLM)
- ❌ Dribbble (vaporware non-implementable)

---

## 2. Vibe statement

**Primary**: `Brushed-metal editorial`
**Sub-tag**: `dev-craft premium`

**Test del vibe** (otro diseñador puede dibujar 3 elementos sin info extra):
1. Background: textura grano metálico sobre paleta dark cool
2. Hero: typography display italic generosa sobre body monospace técnica — tensión editorial vs technical
3. Layout: asimétrico con margen left dominante, caption metadata en rotación

✅ El vibe constraint NO es genérico ("modern dark" no pasa este test).

---

## 3. Decisiones audaces declaradas (5)

### Decisión audaz #1 — Triple typography hierarchy (axiomática primera)
- **Qué**: 3 typefaces con roles claros:
  - **Display serif italic** (PP Editorial New italic / fallback Authentic Sans / Migra) — H1 hero + section eyebrows
  - **Sans-serif** (system-ui Söhne-like) — body, descriptions, paragraph prose
  - **Monospace** (ui-monospace JetBrains Mono) — code blocks, install commands, button labels, technical meta
- **ADR**: [`adr-design/wow-01-typography-axiomatic.md`](../../adr-design/wow-01-typography-axiomatic.md)
- **Por qué es audaz**: rompe la coherencia "mono-everything" default para proyectos dev. Justificado porque la marca DevZen busca tensión editorial × technical, y mono-only colapsa esa tensión.
- **Riesgo si sale mal**: +60kb webfont si optamos por PP Editorial New downloaded; mitigado con preload + font-display:optional, o usando display sans alternativo del system stack
- **Cost**: 60 KB extra worst-case, dentro del hard budget 50 KB → necesita subset latin-only crítico

### Decisión audaz #2 — Asymmetric 7-col grid con margin-left dominance
- **Qué**: layout principal con grid 7-col asimétrico:
  - Cols 1: left margin (~12% viewport) con caption metadata rotada 90° (estilo revista editorial)
  - Cols 2-5: contenido principal (60% width — readable column ~65ch)
  - Cols 6-7: right margin con secondary info (status pills, version) o whitespace generoso
- **ADR**: [`adr-design/wow-02-asymmetric-grid.md`](../../adr-design/wow-02-asymmetric-grid.md)
- **Por qué es audaz**: ROMPE el centered-max-width-1280 default que aparece en >70% landings 2024-2026. Es la decisión visual que más fuertemente firma "esto no es plantilla SaaS".
- **Riesgo si sale mal**: mobile responsive requiere reflow inteligente (margen rotado desaparece en <768px). Si mal hecho rompe lectura mobile.
- **Cost**: 0 bytes extra, solo CSS grid template

### Decisión audaz #3 — Background con textura brushed-metal real (no plano)
- **Qué**: layered backgrounds:
  - Base: radial-gradient cool dark `oklch(0.06 0 0)` → `oklch(0.10 0.005 230)`
  - Capa 2: feTurbulence SVG inline @ baseFrequency 0.65, 8-12% opacity con mix-blend-mode overlay (textura grano fino)
  - Capa 3: scan lines horizontales sutiles via repeating-linear-gradient @ 2px height, 0.5-1% opacity (sensación monitor CRT antiguo / metal lined)
  - Sin gradient mesh genérico, sin gradients diagonales
- **ADR**: [`adr-design/wow-03-brushed-metal-canvas.md`](../../adr-design/wow-03-brushed-metal-canvas.md)
- **Por qué es audaz**: la mayoría de "dark mode landings" son `bg-zinc-950` plano. Brushed metal real = textura activa que el usuario percibe sin nombrar.
- **Riesgo si sale mal**: scan lines pueden parecer "retro gamer" si mal calibradas. Test crítico: en monitor común, las scan lines NO deben ser conscientes — solo agregar profundidad. Si las ves explícitas, son demasiado intensas.
- **Cost**: SVG inline ~400 bytes, gradients en CSS ~200 bytes. Negligible.

### Decisión audaz #4 — Triangle motif COMO LOGO (small, framed) — NO como background hero
- **Qué**: el triángulo del logo DevZen aparece en:
  - Header brand mark (28px, framed dentro del cuadrado del logo)
  - Section delimiters entre secciones (12px decoration vertical entre Hero/Features/Install/Support/Footer)
  - Footer attribution junto a "a DevZen tool" (36px framed)
  - **NUNCA 520px centered behind hero**. Regla del "triángulo huérfano" (wow-vocabulary anti-pattern #1).
- **ADR**: [`adr-design/wow-04-triangle-framed-only.md`](../../adr-design/wow-04-triangle-framed-only.md)
- **Por qué es audaz**: contradice el agente codificador del LUMEN-01 (yo) que naturalmente quería el triángulo grande "como motif visual del hero". El triángulo del logo SIEMPRE está framed — preservarlo así mantiene coherencia con la marca.
- **Riesgo si sale mal**: ningún riesgo grave. El downside es "menos drama visual" pero eso es FINE — la dramaticidad la trae la typography hierarchy + asymmetric grid, no el triángulo.
- **Cost**: SVG inline reutilizable, 0 bytes adicionales

### Decisión audaz #5 — Motion language calibrado y restringido
- **Qué**: vocabulario motion:
  - `--duration-micro: 90ms` para acknowledgments (button press, focus changes)
  - `--duration-base: 220ms` con `cubic-bezier(0.34, 1.56, 0.64, 1)` spring para transitions principales (color, opacity)
  - `--duration-slow: 480ms` con `cubic-bezier(0.16, 1, 0.3, 1)` para reveals on scroll-intersect
  - **NO transitions on hover para elementos non-interactive** (texto cambiando color suavemente cuando hovereás = anti-pattern)
  - **NO animations on initial page load** excepto el SeeleStatus pill que pulsa mientras checkea
  - **Reveal on scroll-intersect** solo para primera viewport (Hero + primer Feature card)
- **ADR**: [`adr-design/wow-05-motion-restraint.md`](../../adr-design/wow-05-motion-restraint.md)
- **Por qué es audaz**: industria default 2024-2026 es "animate-on-load todo + hover-transform en todo + smooth scroll injection". Audaz = restraint deliberado.
- **Riesgo si sale mal**: ninguno significativo. Less is more aquí.
- **Cost**: 0 bytes (solo tokens motion definidos en tokens.css)

---

## 4. Banned aesthetic moves PER-SPRINT

Este sprint específicamente, además de los `banned` globales del DESIGN.md:

- ❌ **Cero decoration o motif >30% viewport sin estar declarado en sección 3** (regla del "triángulo huérfano" — herencia LUMEN-01)
- ❌ **Cero `text-align: center` para layout primario** (hero, features, install). Centered solo para footer/legal microcopy.
- ❌ **Cero `max-width: 1280px` con margin-inline: auto simétrico** — usar grid 7-col asimétrico
- ❌ **Cero hover transforms `translateY` >2px** — restraint motion
- ❌ **Cero gradient diagonales sin propósito narrativo declarado** (el gradient `gold-300 → gold-100` para CTAs SÍ tiene propósito; gradient mesh decorativo NO)
- ❌ **Cero `box-shadow` para hierarchy** — usar bordes 1px o background change
- ❌ **Cero classes Tailwind-like en componentes** (`text-blue-500`, `bg-zinc-900`) — siempre tokens semantic
- ❌ **Cero animations on page load** excepto el SeeleStatus pulse
- ❌ **Cero smooth-scroll injection** — comportamiento default del browser está bien
- ❌ **Cero scroll-jacking, parallax, ScrollMagic**
- ❌ **Cero placeholder logos o "lorem ipsum"** — todo content real desde día 1

---

## 5. Composición pre-código

Sketch de las 5 secciones principales. Cajas grises con ratios y márgenes definidos.

### Hero (desktop 1440×900, mobile 414×800)

```
DESKTOP (1440px wide)
+--------+----------------------------------+--------+
|        |                                  |        |
|   ↓    |   a DevZen tool                  |  ●     |
|   M    |                                  |  Detect|
|   A    |   ╔═══════════════════════════╗  |  ed    |
|   R    |   ║                           ║  |        |
|   G    |   ║   Memoria local           ║  |  v0.1.0|
|   E    |   ║   para tus agentes        ║  |  ──    |
|   N    |   ║   de IA.                  ║  |  322   |
|        |   ║                           ║  |  tests |
|   c    |   ║   ↑ display italic 96px   ║  |  ──    |
|   a    |   ╚═══════════════════════════╝  |  MIT   |
|   p    |                                  |        |
|   t    |   Rust · SQLite · ONNX           |        |
|   i    |   one binary, three transports   |        |
|   o    |   ↑ body mono 14px               |        |
|   n    |                                  |        |
|        |   [Get started →] [GitHub ↗]     |        |
|   ↻    |   ↑ buttons gold gradient        |        |
|        |                                  |        |
| 12% w  |          60% w                   | 12% w  |
+--------+----------------------------------+--------+

MOBILE (414px wide)
+------------------------+
|  a DevZen tool         |  ← eyebrow
|                        |
|  Memoria local         |  ← H1 stacked
|  para tus agentes      |
|  de IA.                |
|                        |
|  Rust · SQLite · ONNX  |  ← subline
|                        |
|  [Get started →]       |  ← full-width
|  [GitHub ↗]            |
|                        |
|  ● Detected · v0.1.0   |  ← status inline
+------------------------+
```

### Section divider (entre Hero/Features/Install/Support)
```
+----------- horizontal rule, 1px metal gradient, 60% width centered ----------+
                                ▴   ← 12px triangle framed
+------------------------------------------------------------------------------+
```

### Features (4 tiles, ASIMÉTRICOS — no grid de 4 igual)
```
DESKTOP
+--------+-------------+---------------+----------+---------+
|        |             |               |          |         |
|        |  CLI        | MCP           | HTTP     |  TUI    |
|        |  30% width  | 25% width     | 25% wide |  20%    |
|        |             |               |          |         |
|        |  ─→ ⌨       |  ─→ ◉         | ─→ ↗     | ─→ ▦    |
|        |  17 cmds    | 19 tools      | 18 paths | 5 panes |
|        |  ↑ icon     |               |          |         |
|        |  ↑ count    |               |          |         |
|        |  ↑ description en mono 14px each                  |
|        |  ↑ link "View →" gold                             |
|        |                                                   |
| 12% mg |     76% width content                | 12% mg    |
+--------+--------------------------------------+-----------+
```

Asimétrico ratio: 30/25/25/20 — NO 25/25/25/25. CLI domina visualmente porque es el modo más usado.

### Install (3 cards, stacked en md/mobile, vertical en desktop ancho)
```
DESKTOP wider than 1080
+--------+----------------------+----------------------+----------------------+--------+
|        |  ▸ Install script    |   cargo install      |   Build from source  |        |
|        |     [RECOMENDADO]    |                      |                      |        |
|        |     ╔══════════════╗ |   ╔══════════════╗   |   ╔══════════════╗   |        |
|        |     ║ curl ...     ║ |   ║ cargo install║   |   ║ git clone... ║   |        |
|        |     ║ install.sh   ║ |   ║ --git seele  ║   |   ║ cargo build  ║   |        |
|        |     ║ | bash       ║ |   ║ -cli         ║   |   ║              ║   |        |
|        |     ╚══════[copy]══╝ |   ╚══════[copy]══╝   |   ╚══════[copy]══╝   |        |
|        |                      |                      |                      |        |
| 12% mg |                                                                    | 12% mg |
+--------+--------------------------------------------------------------------+--------+
```

### Support (donate grid + sub-narrative + DevZen attribution)
```
+--------+----------------------------------+--------+
|        |  / Support · Apoyar              |        |
|        |                                  |        |
|        |  Built by ONE DEVELOPER in       |        |
|        |  spare hours.                    |        |
|        |  ↑ "ONE DEVELOPER" gold text     |        |
|        |                                  |        |
|        |  ▴ Star · share repo             |        |
|        |  ▴ Open issues with feedback     |        |
|        |  ▴ Hire · consult via Discussion |        |
|        |                                  |        |
|        |  Crypto donations — one click    |        |
|        |  ↑ "Crypto donations" gold       |        |
|        |                                  |        |
|        |  Wallets detected: MetaMask · Pali  ← cyan pill if available  |
|        |                                  |        |
|        |  [₿ BTC]    [Ξ ETH]    [⬢ Base]  |        |
|        |  [⬡ SYS]    [◎ SOL]              |        |
|        |  ↑ 5 buttons, 2-3 per row mobile |        |
|        |                                  |        |
+--------+----------------------------------+--------+
```

### Footer
```
+--------+----------------------------------+--------+
|        |  ▴ a DevZen tool · SEELE         |        |
|        |  ↑ triangle 36px framed          |        |
|        |                                  |        |
|        |  © 2026 DevZen SpA · MIT         |        |
|        |  Changelog · Discussions ·       |        |
|        |  Inspired by ENGRAM · GitHub ↗   |        |
|        |                                  |        |
|        |  No tracking · no analytics ·    |        |
|        |  no cookies · no SDKs.           |        |
|        |  Super stellatum firmamentum...  |        |
|        |  ↑ italic latin signature        |        |
+--------+----------------------------------+--------+
```

---

## 6. Validación retroactiva del template (checklist obligatoria pre-Gate 1)

- [x] ¿Las 3-5 refs están propuestas con URLs? (5 propuestas, pendientes screenshot real)
- [x] ¿El vibe statement tiene ≤2 palabras y no usa palabras-veto? ("Brushed-metal editorial" — 2 palabras compuestas, no usa "modern/clean/minimal")
- [x] ¿Cada decisión audaz tiene ADR planificado? (5 ADRs identificados, pendientes escritura — wow-01 a wow-05)
- [x] ¿La composición está dibujada? (ASCII sketches arriba, 5 secciones)
- [ ] ¿El director firmó honestamente el disclaimer inicial? **← PENDIENTE**
- [x] ¿Hay al menos 3 banned moves específicos del sprint? (11 declarados)

**Estado**: 5/6 checks. Falta solo firma del director. Si firma → procede a Material.1 Variation. Si no firma honestamente → pause 30-60 min para curación visual.

---

## 7. Anti-pattern detection (auto-check completed)

- ✓ Refs NO son design system pages (read.cv / pitch / modal / typewolf / works.studio son agency/portfolio/editorial — no Shadcn examples)
- ✓ Vibe statement NO contiene palabras-veto
- ✓ Decisiones audaces son layout/hierarchy/texture/motion/motif — NO tokens (esos van en Material.3)
- ✓ Banned moves son específicos del sprint, no repeat del global DESIGN.md
- ✓ Composición NO es "centered hero + features + CTA" — es asimétrica con 7-col grid
- ✓ **Decoration >30% viewport declarada o ausente** — el triángulo grande está EXPLÍCITAMENTE banned en decisión audaz #4 (regla del triángulo huérfano)

---

## 8. Output de Gate 1 esperado

Director Orlando lee este artifact + las 3-4 variations producidas por Material.1 (sub-agents paralelos) y declara:

- [ ] **Aprobado** — proceder a Material.2 Convergence con la variation elegida
- [ ] **Rechazado con feedback** — qué sección refinar
- [ ] **Pausado** — necesito más curación visual antes de continuar

---

## Próximo paso

Después de firma del disclaimer del director, lanzar Material.1 Variation: 3-4 sub-agents paralelos con sesgos aestéticos radicalmente distintos:

1. **"Brushed-metal editorial"** (cerca del Pillars actual, fine-tuned)
2. **"Brutalist dev-craft"** (radical: bordes duros, monospace dominant, asimetría extrema, sin gradients metálicos)
3. **"Studio Suizo cold-precision"** (escuela suiza extrema: grid rígido, paleta neutral con accent frío singular)
4. **"Editorial monumental warm"** (revista magazine: serif italic display dominante, paleta paper-tone warm cream, márgenes generosos)

Cada uno producirá 1 markdown describiendo su dirección sin código. Director compara 4 y elige UNA en Gate 1.
