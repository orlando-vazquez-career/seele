---
version: 0.1.0
name: SEELE Web Design System
description: Visual contract for the SEELE landing page in `web/`. The Rust crates have no UI surface — design tokens here apply only to the web subproject.
scope: web/
created: 2026-05-11
sprint: LUMEN-01

primitives:
  colors:
    bg-near-black: '#0a0a0a'
    bg-elevated: '#131313'
    bg-overlay: '#1a1a1a'
    fg-near-white: '#e5e5e5'
    fg-muted: '#888888'
    fg-faint: '#555555'
    purple-500: '#7e3eff'
    purple-700: '#5a23c4'
    purple-300: '#a87aff'
    border-subtle: '#1f1f1f'
    border-strong: '#2a2a2a'
    success: '#4ade80'
    warning: '#fbbf24'
    danger: '#f87171'
    network-btc: '#f7931a'
    network-eth: '#627eea'
    network-base: '#0052ff'
    network-sys: '#1f87ff'
    network-sol: '#9945ff'

  typography:
    font-mono: 'ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace'
    font-sans: 'ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif'
    banned:
      - 'Comic Sans MS'
      - 'Papyrus'
      - 'Times New Roman'
    sizes:
      xs: 12px
      sm: 14px
      base: 16px
      lg: 18px
      xl: 24px
      2xl: 32px
      3xl: 48px
    weights:
      normal: 400
      medium: 500
      bold: 700
    leading:
      tight: 1.2
      normal: 1.5
      relaxed: 1.7

  spacing:
    base: 4
    scale: [4, 8, 12, 16, 24, 32, 48, 64, 96, 128]

  rounded:
    sm: 4px
    md: 8px
    lg: 12px
    pill: 999px

  motion:
    duration:
      fast: 120ms
      base: 220ms
      slow: 400ms
    easing:
      standard: 'cubic-bezier(0.4, 0, 0.2, 1)'
      enter: 'cubic-bezier(0, 0, 0.2, 1)'
      exit: 'cubic-bezier(0.4, 0, 1, 1)'

semantics:
  surface:
    base: '{colors.bg-near-black}'
    elevated: '{colors.bg-elevated}'
    overlay: '{colors.bg-overlay}'
  text:
    primary: '{colors.fg-near-white}'
    secondary: '{colors.fg-muted}'
    tertiary: '{colors.fg-faint}'
    accent: '{colors.purple-500}'
    on-accent: '#ffffff'
    success: '{colors.success}'
  border:
    subtle: '{colors.border-subtle}'
    strong: '{colors.border-strong}'
  action:
    primary-bg: '{colors.purple-500}'
    primary-bg-hover: '{colors.purple-700}'
    primary-text: '#ffffff'
    secondary-bg: 'transparent'
    secondary-bg-hover: '{colors.bg-overlay}'
    secondary-text: '{colors.fg-near-white}'
  focus:
    ring: '{colors.purple-500}'
    ring-offset: 2px
---

# SEELE Web Design System v0.1.0

Visual contract for the SEELE landing page in `web/`. Living document — bump the `version:` field in frontmatter according to semver when changing primitives or semantics.

## Why monospace everywhere

SEELE is a developer tool. The audience reads code daily. Monospace typography signals "this is the same kind of thing as the terminal you live in", aligns with the project's identity (Rust + CLI-first + textual MCP), and avoids the marketing-page feel that betrays trust with dev audiences. Inspired by Frost's 2026 essays on agentic design systems and the visual approach of nushell.sh, modal.com early docs, and the Plan9 papers.

## Color philosophy

Dark mode default. Background `#0a0a0a` is near-black but not pure black — pure black on OLED creates contrast that can feel harsh and is unforgiving for sub-pixel rendering on LCD. `#0a0a0a` reads dark without strain.

Single accent (`#7e3eff` — purple). The name SEELE means "soul" in German; purple historically connotes the interior life, the alma. It also separates SEELE from the sea-of-blue OSS landings.

Network brand colors used **only** in donate buttons, per official brand guidelines from each chain:

- Bitcoin Foundation orange `#f7931a`
- Ethereum Foundation blue `#627eea`
- Coinbase Base `#0052ff`
- Syscoin Platform blue `#1f87ff`
- Solana brand purple `#9945ff`

This is the only place the design system intentionally breaks the "single accent" rule. Network colors are external signals — using them lets the user pattern-match against their wallet/exchange UI instantly.

## Spacing scale

Base `4px`. Scale of multiples (`4, 8, 12, 16, 24, 32, 48, 64, 96, 128`). Components must use scale values — no raw `padding: 7px`.

## Banned

Components must NOT use:

- Fonts from `typography.banned` (Comic Sans, Papyrus, Times New Roman).
- Webfont downloads (no `@font-face`, no Google Fonts, no Adobe Fonts).
- Primitive colors directly — consume via semantic tokens.
- Inline color hex outside this file or `tokens.css`.
- `box-shadow` for hierarchy — prefer `border` and `background` over shadows.
- `transition` durations >400ms.
- Analytics SDKs of any kind.
- Tracking pixels.
- Cookie banners (there are no cookies set).
- Carousels / autoplay video.
- Modals (the landing has no flow that requires interruption).

## Coverage (global)

What agents can reuse without inventing:

- **Surface**: `--semantic-surface-{base,elevated,overlay}` for backgrounds.
- **Text**: `--semantic-text-{primary,secondary,tertiary,accent,on-accent,success}` for foregrounds.
- **Border**: `--semantic-border-{subtle,strong}`.
- **Action**: `--semantic-action-primary-{bg,bg-hover,text}` for buttons.
- **Focus**: `--semantic-focus-ring` — always use, never strip.
- **Type ramp**: `--text-{xs,sm,base,lg,xl,2xl,3xl}`.
- **Spacing**: `--space-{1..32}`.
- **Motion**: `--duration-{fast,base,slow}` + `--easing-{standard,enter,exit}`.

## Validation (global)

- All components MUST consume semantic tokens, not primitives.
- All interactive elements MUST have `:focus-visible` styles using `--semantic-focus-ring`.
- All text MUST meet WCAG 2.2 AA contrast against its background.
- `prefers-reduced-motion: reduce` MUST disable transitions/animations.
- `prefers-color-scheme: light` is NOT supported in v0.1 (dark only). Adding light mode = MINOR semver bump.

## Component specs

Detailed Coverage + Validation per component in `docs/design/components/`:

- `Hero.md` (Bloque C)
- `Features.md` (Bloque C)
- `FeatureCard.md` (Bloque C)
- `Install.md` (Bloque C)
- `InstallCard.md` (Bloque C)
- `Support.md` (Bloque C)
- `DonateButtons.md` (Bloque C) — the only client-side hydrated island
- `Footer.md` (Bloque C)

## Version history

| Version | Date | Sprint | Change |
|---|---|---|---|
| 0.1.0 | 2026-05-11 | LUMEN-01 | Initial primitives + semantics for landing + donate widget. |
