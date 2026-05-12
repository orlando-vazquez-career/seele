---
version: 0.2.0
name: SEELE Web Design System
description: Visual contract for the SEELE landing page in `web/`. Brand-aligned to DevZen — brushed-metal black with gold + silver-cyan accents and a triangle motif inherited from the DevZen logo. Tokens use OKLCH for perceptually-correct gradient interpolation.
scope: web/
created: 2026-05-11
updated: 2026-05-12
sprint: LUMEN-01 (bloque-C2)

# v0.2.0 — DevZen brand alignment + EIP-1193 donate flow + SEELE detection.
# Breaking changes from v0.1.0: primitives recolored; semantic tokens
# repointed; components must use new metal/gold/cyan utilities for accent
# moments. MAJOR-equivalent change but pre-1.0 numbered as 0.x bump.

primitives:
  colors:
    # ───────── Black depth scale (DevZen background) ─────────
    # Pure-ish black with a 230° hue cool bias to feel premium.
    black-0: 'oklch(0.06 0 0)'        # deepest void
    black-1: 'oklch(0.10 0.003 230)'  # brushed-metal canvas
    black-2: 'oklch(0.14 0.005 230)'  # elevated card surface
    black-3: 'oklch(0.19 0.007 230)'  # popover / overlay surface

    # ───────── Silver / metal frame scale ─────────
    # Maps to the gunmetal frame around the DevZen logo.
    metal-100: 'oklch(0.92 0.018 230)'  # highlight tip
    metal-200: 'oklch(0.78 0.018 235)'  # primary silver
    metal-300: 'oklch(0.62 0.020 235)'  # mid-tone
    metal-400: 'oklch(0.45 0.018 230)'  # frame body
    metal-500: 'oklch(0.32 0.015 230)'  # shadow

    # ───────── Gold scale ("Dev" letters) ─────────
    gold-100: 'oklch(0.92 0.10 88)'     # highlight
    gold-200: 'oklch(0.82 0.13 85)'     # primary gold
    gold-300: 'oklch(0.72 0.14 82)'     # mid
    gold-400: 'oklch(0.58 0.12 80)'     # shadow

    # ───────── Cyan-silver scale ("Zen" letters) ─────────
    cyan-100: 'oklch(0.90 0.04 200)'    # highlight
    cyan-200: 'oklch(0.78 0.05 195)'    # primary
    cyan-300: 'oklch(0.65 0.06 195)'    # mid
    cyan-400: 'oklch(0.50 0.05 195)'    # shadow

    # ───────── Foreground neutrals ─────────
    fg-100: 'oklch(0.96 0 0)'           # text on dark
    fg-300: 'oklch(0.76 0.005 230)'     # secondary
    fg-500: 'oklch(0.55 0.005 230)'     # tertiary / meta
    fg-700: 'oklch(0.38 0.005 230)'     # faint

    # ───────── Border / divider ─────────
    border-subtle: 'oklch(0.20 0.005 230)'
    border-strong: 'oklch(0.30 0.008 230)'
    border-metal:  'oklch(0.55 0.020 230)'  # for metal-finish frames

    # ───────── Feedback ─────────
    success: 'oklch(0.78 0.16 145)'
    warning: 'oklch(0.82 0.16 75)'
    danger:  'oklch(0.70 0.18 25)'

    # ───────── Network brand (donate widget) ─────────
    network-btc:  '#f7931a'
    network-eth:  '#627eea'
    network-base: '#0052ff'
    network-sys:  '#1f87ff'
    network-sol:  '#9945ff'

  typography:
    font-mono: 'ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace'
    font-display: 'ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif'
    banned: ['Comic Sans MS', 'Papyrus', 'Times New Roman']
    sizes:
      xs: 12px
      sm: 14px
      base: 16px
      lg: 18px
      xl: 22px
      2xl: 30px
      3xl: 44px
      4xl: 60px
      5xl: 80px
    weights:
      normal: 400
      medium: 500
      semibold: 600
      bold: 700
      black: 900
    leading:
      tight: 1.15
      normal: 1.55
      relaxed: 1.75
    letter-spacing:
      tight: -0.02em
      normal: 0
      wide: 0.05em
      mega: 0.15em

  spacing:
    base: 4
    scale: [4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96, 128, 160]

  rounded:
    sm: 6px
    md: 10px
    lg: 16px
    xl: 22px
    pill: 999px

  motion:
    duration:
      micro: 90ms
      fast: 140ms
      base: 240ms
      slow: 480ms
      epic: 900ms
    easing:
      standard: 'cubic-bezier(0.4, 0, 0.2, 1)'
      enter: 'cubic-bezier(0, 0, 0.2, 1)'
      exit: 'cubic-bezier(0.4, 0, 1, 1)'
      spring: 'cubic-bezier(0.34, 1.56, 0.64, 1)'

  effects:
    # Brushed-metal layered gradients reused across components.
    # See tokens.css for the actual gradient definitions; this is the doc.
    metal-silver: 'linear gradient diagonal 135° between metal-300 → metal-100 → metal-300, with feTurbulence noise overlay 0.85 baseFrequency at 12% opacity'
    metal-gold:   'linear gradient diagonal 135° between gold-300 → gold-100 → gold-300, with feTurbulence noise overlay 0.85 baseFrequency at 12% opacity'
    metal-black-canvas: 'radial-gradient ellipse 80% 60% at top from black-2 → black-1 → black-0, with feTurbulence noise overlay 0.65 baseFrequency at 5% opacity (subtle texture)'

semantics:
  surface:
    canvas: '{colors.black-1}'         # body bg
    elevated: '{colors.black-2}'       # cards / panels
    overlay: '{colors.black-3}'        # popovers / tooltips
    inverse: '{colors.metal-200}'      # rare: metal-on-dark elements
  text:
    primary: '{colors.fg-100}'
    secondary: '{colors.fg-300}'
    tertiary: '{colors.fg-500}'
    quaternary: '{colors.fg-700}'
    gold: '{colors.gold-200}'          # for "Dev" parity
    silver: '{colors.metal-200}'       # for "Zen" parity
    cyan: '{colors.cyan-200}'
    accent: '{colors.gold-200}'        # primary brand accent for prose
    on-accent: '{colors.black-0}'      # text on gold/metal fills
    success: '{colors.success}'
    danger: '{colors.danger}'
  border:
    subtle: '{colors.border-subtle}'
    strong: '{colors.border-strong}'
    metal: '{colors.border-metal}'     # for metallic frames
  action:
    primary-bg-from: '{colors.gold-300}'   # gradient start
    primary-bg-to: '{colors.gold-100}'     # gradient end
    primary-text: '{colors.black-0}'
    secondary-bg: 'transparent'
    secondary-border: '{colors.border-metal}'
    secondary-text: '{colors.fg-100}'
    secondary-bg-hover: '{colors.black-2}'
  focus:
    ring: '{colors.gold-200}'
    ring-offset: 2px
  status:
    seele-detected: '{colors.success}'
    wallet-detected: '{colors.cyan-200}'
    not-detected: '{colors.fg-500}'
---

# SEELE Web Design System v0.2.0 — DevZen brand

Visual contract for the SEELE landing page in `web/`. Versioned per semver. v0.2.0 aligns to the DevZen brand: brushed-metal black canvas, gold + silver-cyan accents, triangle motif inherited from the DevZen logo.

## Why this brand

SEELE belongs to DevZen — the parent company. The DevZen logo is a stylized iron-cast frame around a triangle, with "Dev" in warm gold and "Zen" in cool silver-cyan. The landing inherits this language so SEELE is recognizably part of DevZen even before reading the name.

The aesthetic is "premium developer tool" — closer to Linear's dark mode than to a marketing SaaS landing, but with metallic richness instead of pure flat surfaces.

## OKLCH everywhere

All primitives use [OKLCH](https://oklch.com) instead of sRGB hex. Why:

- Perceptually-uniform: a step from `oklch(0.50 ...)` to `oklch(0.55 ...)` is the same lightness change everywhere on the wheel.
- Gradient interpolation between gold and silver-cyan in OKLCH stays inside the warm-bright corridor (avoids dirty olive transitions that sRGB produces).
- Wide-gamut display ready (P3 / Rec.2020).

CSS native since 2024 (all evergreen browsers). No polyfill needed.

## Color philosophy

### Black canvas

`oklch(0.06–0.19)` family. Slight cool bias (`230°` hue) keeps it from feeling tar-warm. Three depth levels:

- **black-1** body — the "brushed-metal" canvas behind everything
- **black-2** elevated — cards, install/feature panels
- **black-3** overlay — popovers, tooltips, modal dialogs (none in v0.2)

### Silver / metal

`metal-100` through `metal-500` — gunmetal frame inherited from the logo. Used for:
- Borders on cards (`border-metal`)
- The brushed-metal background effect (gradient combining 100 highlight + 200 primary + 300 mid + 500 shadow)
- Logo brand-mark fill
- "Zen" letterforms in headlines

### Gold

`gold-100` through `gold-400` — warm yellow scale matching the "Dev" letters. Used for:
- Primary action buttons (gradient `gold-300 → gold-100`)
- "Dev" letterforms in headlines
- Triangle motif accent
- Accent prose color

### Cyan-silver

`cyan-100` through `cyan-400` — cool counter-balance to gold. Used for:
- "Zen" letterforms (paired with `gold-200` for "Dev")
- Wallet-detected status indicator
- Subtle cool highlights on metal surfaces

### Network colors

Network brand colors (`network-btc/eth/base/sys/sol`) stay in sRGB hex per official brand guidelines (Bitcoin Foundation, Ethereum Foundation, etc). They're the only sRGB tokens — preserved as-is for pattern matching against wallet UIs.

## Brushed-metal technique

Brushed metal is a **CSS technique**, not a token. Composed of:

1. **Base gradient** — linear/radial gradient between 3-4 stops on the metal scale.
2. **Noise overlay** — `feTurbulence` SVG filter inlined as `data:image/svg+xml`, blended at low opacity (~12-15%) with `mix-blend-mode: overlay`.
3. **Highlight band** (optional) — diagonal linear-gradient at high opacity 0% → 20% → 0% to simulate light reflection.

Implementation in `tokens.css` as utility classes: `.metal-silver-bg`, `.metal-gold-bg`, `.metal-canvas-bg`.

## Triangle motif

The DevZen logo features a stylized triangle. We reuse it as a recurring design element:

- Hero: large outline triangle behind the title text (~600px, opacity 0.06)
- Feature cards: small filled triangle as bullet/decoration
- Footer: triangle next to the DevZen attribution

Always inline SVG with `currentColor` and a `linearGradient` interior for gold→cyan transitions. Never raster.

## Typography

Two stacks declared, both system (zero web font downloads):

- **font-mono** — `ui-monospace`. Default for body, code, hero. Signals "dev tool".
- **font-display** — `ui-sans-serif`. Reserved for: page meta (header brand), button labels where mono feels too narrow. Used sparingly.

The DevZen logo uses a custom geometric typeface; matching exactly requires a webfont download which we forbid. Instead, we apply gradient text fills (gold and silver-cyan) to recreate the brand feel with system fonts.

## Banned

- Webfont downloads (`@font-face`, Google Fonts, Adobe Fonts).
- Tailwind utility classes (`.text-blue-500`) — bare CSS only.
- Primitive colors directly in components — go through semantic tokens.
- Inline color values outside this file + `tokens.css`.
- `box-shadow` for hierarchy (use metal-finish borders).
- Hover transforms larger than `translateY(-2px)`.
- Tracking pixels, analytics, cookies.
- Carousels.

## Layout rules

- Container queries (`@container`) are the primary responsive primitive.
- Avoid global media queries except for body type-scale and mobile-only utilities.
- `min-width: 0` on flex children that contain `<pre>` or long URLs.
- `<pre><code>` wraps long lines via `white-space: pre-wrap; overflow-wrap: anywhere` — NO horizontal scroll.
- Max page width: `clamp(320px, 100%, 1200px)` for the body container.

## Coverage (global)

What components can reuse without invention:

- `--semantic-surface-{canvas,elevated,overlay,inverse}` for backgrounds.
- `--semantic-text-{primary,secondary,tertiary,gold,silver,cyan,accent,on-accent}` for text.
- `--semantic-border-{subtle,strong,metal}` for borders.
- `--semantic-action-primary-bg-{from,to}` for gradient buttons.
- `--semantic-status-{seele-detected,wallet-detected,not-detected}` for status indicators.
- `--space-{1..40}` for spacing.
- `--text-{xs..5xl}` for sizes.
- `--duration-{micro,fast,base,slow,epic}` + `--easing-{standard,enter,exit,spring}` for motion.
- `.metal-silver-bg`, `.metal-gold-bg`, `.metal-canvas-bg` for brushed-metal surfaces.
- `.gold-text`, `.silver-text`, `.cyan-text` for gradient-fill text.
- `<svg class="triangle-decoration">` for triangle motif (inline component).

## Validation (global)

- All components MUST consume semantic tokens.
- All interactive elements MUST have `:focus-visible` styled via `--semantic-focus-ring`.
- All text MUST meet WCAG 2.2 AA contrast against its background (verified per token).
- Responsive: NO viewport must produce horizontal scroll. Container queries enforce.
- `prefers-reduced-motion: reduce` MUST disable transitions, animations, and the metal-canvas noise animation.
- The brushed-metal effect MUST be implemented via CSS gradient + inline SVG only (no raster textures, no external requests).

## Component status

| Component | Status |
|---|---|
| Layout | v0.2 — adds SEELE detection script in `<head>` |
| Header | v0.2 — adds wallet/SEELE indicators in nav |
| Hero | v0.2 — brushed-metal canvas + triangle motif + gold/cyan title |
| Features + FeatureCard | v0.2 — metal-finish hover borders |
| Install + InstallCard | v0.2 — gold gradient on recommended badge, `<pre>` wraps |
| Support | v0.2 — narrative unchanged |
| DonateButtons | v0.2 — **rewritten**: EIP-6963 discovery + EIP-1193 flow for EVM (MetaMask popup opens), URI scheme preserved for BTC + SOL (Sparrow/Electrum/Phantom desktop handle it). Wallet-detected indicator per button. |
| SeeleStatus (new) | v0.2 — pings `localhost:7777/favicon.ico` via Image; shows "Detected" / "Install" in hero |
| Footer | v0.2 — DevZen attribution + triangle motif |

## Version history

| Version | Date | Sprint | Change |
|---|---|---|---|
| 0.1.0 | 2026-05-11 | LUMEN-01 bloque-B | Initial primitives + semantics. |
| 0.2.0 | 2026-05-12 | LUMEN-01 bloque-C2 | DevZen brand: OKLCH color system, brushed-metal canvas + triangle motif, gold + silver-cyan dual accent. EIP-1193 + EIP-6963 for EVM donate flow. SeeleStatus component. Container-query responsive system. |
