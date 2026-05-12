# Wireframes textuales — Sprint LUMEN-01

Wireframes descritos en ASCII + texto semántico. Resoluciones target: 1440×900 desktop, 414×896 mobile.

---

## Desktop (1440px)

```
┌──────────────────────────────────────────────────────────────────────────┐
│  SEELE             Features  Install  Support  [⊕ GitHub]               │  ← <header> sticky (alto: 56px)
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│                                                                          │
│            ┌──────────────────────────────────────────┐                  │
│            │  SEELE                                   │                  │  ← Hero
│            │  Memoria local para tus agentes de IA    │                  │     - h1 logotype
│            │                                          │                  │     - tagline (1 línea)
│            │  Rust · SQLite · Hybrid FTS + vec        │                  │     - subline tech
│            │                                          │                  │
│            │  [▶ Get started]  [GitHub →]             │                  │     - 2 CTAs
│            └──────────────────────────────────────────┘                  │
│                                                                          │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│  Features                                                                │  ← #features (id anchor)
│                                                                          │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐                    │
│  │ ⌨  CLI   │ │ 🜲 MCP    │ │ ↗ HTTP   │ │ ▦ TUI    │                    │     - 4 FeatureCards
│  │          │ │          │ │          │ │          │                    │     - grid 4 cols desktop
│  │ 17 cmds  │ │ 19 tools │ │ 18 paths │ │ 5 vistas │                    │
│  │          │ │          │ │          │ │          │                    │
│  │ View →   │ │ View →   │ │ View →   │ │ View →   │                    │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘                    │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│  Install                                                                 │  ← #install
│                                                                          │
│  Three paths, pick whichever fits:                                       │
│                                                                          │
│  ┌────────────────────┐ ┌────────────────────┐ ┌────────────────────┐   │
│  │ Install scripts    │ │ cargo install      │ │ Build from source  │   │
│  │ (recomendado)      │ │                    │ │                    │   │     - 3 InstallPath cards
│  │                    │ │                    │ │                    │   │     - grid 3 cols desktop
│  │ ┌────────────────┐ │ │ ┌────────────────┐ │ │ ┌────────────────┐ │   │
│  │ │ curl … |bash   │ │ │ │ cargo install  │ │ │ │ cargo build    │ │   │
│  │ └────────────────┘ │ │ └────────────────┘ │ │ └────────────────┘ │   │
│  │                    │ │                    │ │                    │   │
│  │ [Copy]             │ │ [Copy]             │ │ [Copy]             │   │
│  └────────────────────┘ └────────────────────┘ └────────────────────┘   │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│  Support / Apoyar                                                        │  ← #support
│                                                                          │
│  SEELE is built by one developer in his spare hours.                     │
│  If it saves you time:                                                   │
│                                                                          │
│  ┌────────────────────────────────────────────────────┐                 │
│  │  [₿ Bitcoin]  [Ξ Ethereum]  [⬡ Base]  [⬢ Syscoin]  │                 │     ← DonateButtons widget
│  │  [◎ Solana]                                        │                 │       - flex row, wrap
│  └────────────────────────────────────────────────────┘                 │
│                                                                          │
│  Or hire / consult — open a Discussion on GitHub.                        │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│  MIT · v0.1.0 · ENGRAM credit · This page weighs 23 KB · GitHub          │  ← Footer
└──────────────────────────────────────────────────────────────────────────┘
```

## Mobile (414px)

```
┌──────────────────────────┐
│  SEELE      ☰ Menu      │  ← header con drawer
├──────────────────────────┤
│                          │
│  SEELE                   │
│  Memoria local           │
│  para tus agentes        │  ← Hero stacked
│                          │
│  Rust · SQLite           │
│                          │
│  [▶ Get started]         │  ← CTAs full-width stacked
│  [GitHub →]              │
│                          │
├──────────────────────────┤
│  Features                │
│  ┌────────────────────┐  │
│  │ ⌨  CLI             │  │  ← FeatureCards stacked
│  │ 17 commands        │  │     1 col mobile
│  │ View →             │  │
│  └────────────────────┘  │
│  ┌────────────────────┐  │
│  │ 🜲 MCP              │  │
│  │ 19 tools           │  │
│  │ View →             │  │
│  └────────────────────┘  │
│  ... (×4 total)          │
├──────────────────────────┤
│  Install                 │
│  ┌────────────────────┐  │
│  │ Install scripts    │  │  ← 1 col stacked
│  │ ...                │  │
│  └────────────────────┘  │
│  ... (×3 total)          │
├──────────────────────────┤
│  Support / Apoyar        │
│                          │
│  [₿ Bitcoin]             │
│  [Ξ Ethereum]            │
│  [⬡ Base]                │  ← Donate buttons stacked
│  [⬢ Syscoin]             │
│  [◎ Solana]              │
│                          │
├──────────────────────────┤
│  MIT · v0.1.0            │
│  23 KB · GitHub          │
└──────────────────────────┘
```

## Componentes derivados

Cada wireframe se descompone en componentes Astro que se construirán en Bloque C:

| Componente | Variants | Slots |
|---|---|---|
| `Layout.astro` | - | `default` (todo el body) |
| `Header.astro` | `default \| mobile-drawer-open` | - |
| `Hero.astro` | - | - |
| `Features.astro` | - | - |
| `FeatureCard.astro` | `default` | - |
| `Install.astro` | - | - |
| `InstallCard.astro` | `recommended \| default` | `command` |
| `Support.astro` | - | - |
| `DonateButtons.astro` (island con `client:visible`) | - | - |
| `Footer.astro` | - | - |

## Estados visuales del DonateButtons

```
┌────────────────────────┐
│ ₿  Bitcoin             │  ← idle
└────────────────────────┘

┌────────────────────────┐
│ ₿  Bitcoin    copied   │  ← post-fallback (verde, fade 2s)
└────────────────────────┘

┌────────────────────────┐
│ ₿  Bitcoin  ↑ opening… │  ← entre clic y window.open (50ms)
└────────────────────────┘
```

## Tipografía propuesta (formal en DESIGN.md en Bloque B)

- `--font-display`: `ui-monospace, "Cascadia Code", "JetBrains Mono", monospace` (toda la página — proyecto dev)
- Sin font-face download. Cero fontfile.

## Color (formal en DESIGN.md en Bloque B)

- Mode default: **dark**.
- Background: `#0a0a0a` (near-black, soft)
- Foreground: `#e5e5e5`
- Accent: `#7e3eff` (purple SEELE — alma)
- Brand colors por network respetando logos oficiales:
  - BTC: `#F7931A`
  - ETH: `#627EEA`
  - Base: `#0052FF`
  - Syscoin: `#1F87FF`
  - SOL: `#9945FF`

## Spacing scale

`4 · 8 · 12 · 16 · 24 · 32 · 48 · 64 · 96` (px).
