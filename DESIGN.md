---
version: 0.3.0
name: SEELE Web Design System
description: Brutalist dev-craft visual contract for the SEELE landing. Single-family monospace typography, 3-color palette, 2px solid borders as hierarchical primitive, motion restraint to zero. Honest infrastructure tool aesthetic — credibility before warmth.
scope: web/
created: 2026-05-11
updated: 2026-05-12
sprint: LUMEN-02 (bloque Material.2 Convergence + Material.3 Tokens)
direction_chosen: brutalist-dev-craft
counsel_reference: vrd_2026-05-12_lumen-v0.10.0

primitives:
  colors:
    # ───────── 3-color extreme palette ─────────
    # Brutalist dev-craft: prohibido agregar un 4to color.
    # La restricción es la decisión.
    bg: 'oklch(0.08 0 0)'              # negro absoluto, sin tinte warm/cool
    fg: 'oklch(0.94 0 0)'              # off-white crudo, no pure white
    accent: 'oklch(0.65 0.18 50)'      # naranja eléctrico, único accent saturado

    # Derivados del fg para hierarchy (NO son colores nuevos, son variaciones de opacity/mix del fg)
    fg-muted: 'oklch(0.65 0 0)'        # texto secundario, derivado de fg
    fg-faint: 'oklch(0.42 0 0)'        # meta/captions, derivado de fg

    # Feedback colors (solo si absolutamente necesario)
    danger: 'oklch(0.60 0.20 25)'      # error states únicamente
    success: 'oklch(0.78 0.16 145)'    # solo para SeeleStatus detected

    # Network brand colors (preservados literally para donate widget)
    network-btc: '#f7931a'
    network-eth: '#627eea'
    network-base: '#0052ff'
    network-sys: '#1f87ff'
    network-sol: '#9945ff'

  typography:
    # Una sola familia. Sin display. Sin sans. Sin serif.
    font-mono: 'ui-monospace, "JetBrains Mono", "Cascadia Code", Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace'
    banned:
      - 'Inter'
      - 'Roboto'
      - 'Helvetica'
      - 'Arial'
      - 'Times New Roman'
      - 'Comic Sans MS'
      - 'Papyrus'
      - 'Display serif italic of any kind'
      - 'Any non-monospace family'

    sizes:
      # Salto brusco entre hero y body, sin escalas intermedias decorativas
      meta: 11px          # ALL CAPS labels, captions, tracking 0.12em
      xs: 12px            # legal / footer
      sm: 13px            # body small, meta inline
      base: 15px          # body principal
      lg: 18px            # subheadlines (rare)
      hero: 72px          # H1 hero — único uso de tamaño grande

    weights:
      light: 300
      normal: 400
      medium: 500
      bold: 700
      black: 800          # solo para hero

    leading:
      tight: 1.1          # hero
      normal: 1.4         # body
      relaxed: 1.6        # paragraph reading

    letter-spacing:
      tight: -0.02em      # hero
      normal: 0
      wide: 0.04em        # mono inline
      caps: 0.12em        # ALL CAPS labels

  spacing:
    base: 4
    scale: [4, 8, 12, 16, 20, 24, 32, 40, 56, 80, 120, 200]

  rounded:
    # Brutalist: cero radius en todo. Period.
    none: 0
    # Valor permitido excepcionalmente (interpretación honesta):
    pill: 999px           # solo si una pill semantic es OBLIGATORIA (status indicator)
    # NO usar sm/md/lg radius en ningún componente

  border-width:
    # Bordes son la herramienta jerárquica primaria
    hair: 1px             # separadores sutiles
    base: 2px             # default — la mayoría de bordes son 2px
    bold: 3px             # énfasis raro

  motion:
    # transition: none en TODO. Por excepción aceptable solo color/opacity cambio instantáneo.
    duration:
      none: 0ms           # default
      instant: 1ms        # técnico: equivalente a none pero permite GPU acceleration hint
    easing:
      step: 'step-start'  # cambios binarios sin interpolación

semantics:
  surface:
    base: '{colors.bg}'                    # único surface — todo es negro absoluto
    inverse: '{colors.fg}'                 # raros casos de fg como bg (CTA hover)
  text:
    primary: '{colors.fg}'
    secondary: '{colors.fg-muted}'
    tertiary: '{colors.fg-faint}'
    accent: '{colors.accent}'              # naranja eléctrico
    on-accent: '{colors.bg}'               # negro sobre naranja
    success: '{colors.success}'
    danger: '{colors.danger}'
  border:
    base: '{colors.fg}'                    # bordes son fg color (off-white)
    accent: '{colors.accent}'              # bordes naranja para énfasis
    muted: '{colors.fg-faint}'             # separadores sutiles
  action:
    primary-bg: '{colors.accent}'
    primary-text: '{colors.bg}'
    primary-border: '{colors.accent}'
    secondary-bg: 'transparent'
    secondary-text: '{colors.fg}'
    secondary-border: '{colors.fg}'
  focus:
    ring: '{colors.accent}'
    ring-offset: 2px
  status:
    seele-detected: '{colors.success}'
    seele-not-detected: '{colors.fg-faint}'
    wallet-detected: '{colors.accent}'     # naranja también es "wallet detected" — consolidamos accent

visual-dna:
  # 5 axiomas del wow del counsel MNEMA, aplicados a Brutalist:
  coherencia-sistemica: |
    Una sola familia tipográfica (JetBrains Mono). Una sola escala de bordes (1/2/3px solid).
    Tres colores únicos. Cero excepciones decorativas. Coherencia EXTREMA es el axioma central.
  diferenciacion-con-proposito: |
    Bordes incompletos como sintaxis visual de flow entre elementos. NINGUNA otra landing
    2026 lo hace. Es la decisión que distingue SEELE de cualquier "modern dark dev tool".
  densidad-informacional-calibrada: |
    Hero tiene mucho whitespace deliberado. Bloques de datos (latency, version, status)
    son densos. Información concreta vs prose marketing.
  acabado: |
    Bordes pixel-perfect (sin antialiasing artifacts). Tracking calibrado manualmente por
    size. Step-start transitions = ningún ease que pueda parecer "casi animado".
  personalidad-emergente: |
    "Honest infrastructure tool" — no premium, no SaaS-friendly, no warm editorial.
    El landing dice "instalá esto si entendés Rust, sino seguí buscando".
---

# SEELE Web Design System v0.3.0 — Brutalist dev-craft

Visual contract for the SEELE landing page, redesigned in Sprint LUMEN-02 under LUMEN v0.10.0 protocol. Direction chosen: **Brutalist dev-craft** (variation 2 of 4 propuestas en Material.1).

## Why this direction

SEELE es memory engine para devs que usan Claude Code, Cursor, Windsurf. Audiencia: platform engineers, infra devs, ML engineers, freelancers técnicos. NO es producto B2B SaaS para C-levels. NO es proyecto de autor con narrativa editorial. Es **infrastructure tool**.

El director eligió Brutalist sobre 3 alternativas (Brushed-metal editorial / Studio Suizo / Editorial warm) porque la audiencia premia **credibilidad técnica antes que calidez visual**. La estética brutalist comunica "esto es serio antes de ser bonito" — exactamente lo que Marisol persona necesita ver en 30 segundos.

Las otras 3 direcciones quedan persistidas como `advisor_output` en SEELE — disponibles si en LUMEN-03+ la dirección cambia.

## Three colors, period

`oklch(0.08 0 0)` background + `oklch(0.94 0 0)` foreground + `oklch(0.65 0.18 50)` orange accent. **Prohibido agregar un cuarto color.** La restricción ES la decisión.

Excepciones permitidas:
- `danger` solo para error states reales (no usado en v0.3 del landing)
- `success` solo para SeeleStatus pill "detected"
- Network brand colors (BTC/ETH/Base/Sys/SOL) solo en donate buttons (literalismo cromático)

## Single typography family

JetBrains Mono everywhere. Sin display serif. Sin sans-serif. Sin Inter. Sin Roboto. La hierarchy emerge del **contraste de weight** (800 hero, 400 body) + **salto brusco de size** (72px → 13px sin escalas intermedias) + **color accent** solo en datos.

Por qué: tipografía mono comunica "código" sin metáforas. Eliminar serif/sans elimina el riesgo de "diseño SaaS friendly" — exactamente lo que Brutalist rechaza.

## Borders as primary hierarchical tool

2px solid `oklch(0.94 0 0)` reemplazan TODA shadow, gradient, blur. La jerarquía se lee por:
- **Cuántos lados tiene el borde**: bloque principal (4 lados), elemento secundario (2 lados top+left), dato inline (1 lado left como indicador)
- **Color del borde**: off-white (default) vs naranja (énfasis)

`box-shadow: none` en todo. `border-radius: 0` en todo (excepción: `pill` solo para SeeleStatus indicator). `gradient` prohibido.

## Incomplete borders as visual syntax

**El detalle distintivo del Brutalist dev-craft**: bordes incompletos (3 lados en lugar de 4) NO son error. Son **conectores abiertos** hacia el siguiente elemento. Crean flow sin necesidad de flechas/icons/connectors decorativos.

Pattern de uso:
- Bloque de datos en hero: `border-top` + `border-right` + `border-bottom`, sin `border-left` (= "connects to left margin")
- Section dividers: `border-top` solo, ancho full
- Inline data: `border-left` solo (= "this is data, not prose")

Esto firma "este sitio fue diseñado con criterio brutalist" para cualquier diseñador que lo lea. No es bug, es feature.

## Motion: zero

`transition: none` en todo. Sin excepción.

Hover state = cambio de color instantáneo. Sin fade. Sin scale. Sin translateY. Sin spring curves. `step-start` easing si TypeScript exige una value.

Por qué: SEELE es infraestructura. Movement suave miente sobre su naturaleza. Una herramienta seria responde binariamente.

Excepciones permitidas:
- SeeleStatus pulse animation (mientras checkea localhost) — única animación del sitio
- Donate button "opening..." status text change (no animation, solo content update)

## Layout philosophy

**Asimetría brutal**: left-heavy. Margen izquierdo 16-32px. Margen derecho 80px+ deliberadamente desequilibrante. NO centered max-width simétrico.

Container max-width: `1100px` con `margin-inline-start: clamp(16px, 4vw, 64px)` y `margin-inline-end: clamp(80px, 12vw, 200px)`.

Mobile: el desequilibrio se mantiene (margen izquierdo 16px, derecho 32px), no se "fixea" a centered.

## Banned (extending DESIGN.md banned)

- Cualquier `font-family` excepto JetBrains Mono / monospace fallback
- Cualquier `border-radius` excepto 0 o 999px (pill)
- Cualquier `box-shadow` excepto 0 (none)
- Cualquier `transition` con duration > 1ms
- Cualquier `transform` excepto `none` o `translateY(0)` reset
- Gradients de cualquier tipo (`linear-gradient`, `radial-gradient`, `conic-gradient`)
- Filters CSS (`blur`, `brightness`, `backdrop-filter`)
- Webfonts download (system stack solo, JetBrains Mono via ui-monospace fallback chain)
- Cookie banners, analytics, tracking pixels (heredados)
- Marketing speak headlines ("revolutionary", "AI-powered", "10x faster")
- Carousels, parallax, scroll-jacking
- Icons SVG decorativos (excepción: triangle del logo DevZen en brand mark 24px)
- Hover transforms `translateY > 0`
- Animations on initial page load (excepción: SeeleStatus pulse)

## Coverage (global)

What components can reuse:
- `--semantic-surface-base` / `--semantic-surface-inverse` — surfaces
- `--semantic-text-{primary,secondary,tertiary,accent,on-accent}` — text
- `--semantic-border-{base,accent,muted}` — borders
- `--semantic-action-{primary,secondary}-{bg,text,border}` — buttons
- `--space-{1..200}` — spacing scale
- `--text-{meta,xs,sm,base,lg,hero}` — type sizes
- `--weight-{light,normal,medium,bold,black}` — weights
- `--tracking-{tight,normal,wide,caps}` — letter-spacing
- `--border-width-{hair,base,bold}` — border widths
- `--rounded-{none,pill}` — radii (none por default, pill solo SeeleStatus)

## Validation (global)

- All components MUST use mono family. NO excepciones.
- All borders MUST be solid, 1-3px, in fg/accent/muted color.
- All hover states MUST be instant (no transition).
- All sizing MUST come from the scale (no raw `padding: 7px`).
- `prefers-reduced-motion` MUST be honored — no animation siquiera el SeeleStatus pulse.
- WCAG 2.2 AA contrast: fg(0.94) on bg(0.08) = 15.6:1 AAA. accent(0.65) on bg(0.08) = 5.8:1 AA. Fácil pass.

## Version history

| Version | Date | Sprint | Change |
|---|---|---|---|
| 0.1.0 | 2026-05-11 | LUMEN-01 B | Initial primitives + semantics (genérico minimalist) |
| 0.2.0 | 2026-05-12 | LUMEN-01 C2 | DevZen brand: OKLCH + brushed metal + gold/cyan dual accent + triangle motif (LUMEN v0.9.1) |
| **0.3.0** | **2026-05-12** | **LUMEN-02 Material.2+3** | **Brutalist dev-craft pivot. 3-color palette extreme. JetBrains Mono single family. 2px solid borders as hierarchical tool. Incomplete borders as visual syntax. Motion: zero (transition:none). DevZen brand preservado solo en text attribution. LUMEN v0.10.0** |
