# ADR wow-03 — 3-color palette only

**Sprint:** LUMEN-02
**Date:** 2026-05-12
**Status:** Adopted
**Decisión audaz #** 3 of 5
**Variation:** 02-brutalist-dev-craft

## Context

Sprint LUMEN-01 used a "brushed-metal editorial" palette: gold (`oklch(0.85 0.12 85)`), silver (`oklch(0.78 0.02 230)`), cyan accents, black background, and 4 network-brand colors (BTC orange, ETH purple-grey, SOL teal, Base blue) on the donate widget. Seven colors total, plus their tints and shades. The director's discomfort was visible in every Cloven critique: *"the palette is doing a lot of work but no single color is doing the load-bearing job"*.

Brutalist tradition (Wim Crouwel, Massimo Vignelli's NYC subway, modern derivatives like read.cv) shows that hierarchy can be carried by **1 chromatic accent** against **2 neutrals**. Adding a 4th color always erodes the hierarchy.

## Decision

Three colors. Only three.

```
bg     oklch(0.08 0 0)       near-black, zero chroma
fg     oklch(0.94 0 0)       off-white, zero chroma
accent oklch(0.65 0.18 50)   electric orange, chroma 0.18
```

Plus *one* derived state pair for system feedback (success/danger reuse same hue family, different lightness):

```
success oklch(0.75 0.18 145)  green (only on tx-sent feedback)
danger  oklch(0.65 0.22 25)   red  (only on rejected/error feedback)
warning oklch(0.80 0.15 90)   amber (only on pending state)
```

Status colors **only appear inside DonateButtons live states** — they never leak into branded chrome. The page itself is pure 3-color (`bg`, `fg`, `accent`).

Network-brand colors (BTC orange, ETH purple, etc.) are **removed**. The donate buttons all use the same orange `accent` on hover — there is no per-network color differentiation.

## Consequences

**Positive:**
- One color (`accent`) carries every "this matters" signal — eyebrows, hover states, key adjectives in support copy, recommended-install badge
- The page reads as a coherent system, not as a collection of components with different palettes
- Removes ~7 CSS custom properties for `--network-*` colors → simpler tokens
- Wide-gamut OKLCH renders the accent orange ~15% brighter on P3 displays than equivalent sRGB hex

**Negative:**
- Donate widget loses the network-brand recognition (BTC orange used to read as "Bitcoin")
- The page is "stark" — some users will read minimalism as missing
- Limits future skinning (no easy "dark/light theme") — the palette is the brand, not a variable

**Axiom check (5-axiom rubric):**
- **Pri.** Color is no longer decorative — only `accent` is allowed, and it earns its appearance
- **Cont.** "Brushed-metal editorial" 7-color palette rejected; 3-color explicit replacement
- **Coh.** Every component pulls from the same 3 tokens; no per-component overrides
- **Ten.** "Brushed metal con barniz de ingeniería" (anti-pattern named in LUMEN v0.10.0) preserved as cautionary
- **Ev.** `tokens.css` v0.3.0 OKLCH definitions

## Alternative rejected

4-color palette: bg + fg + accent + a single muted secondary (e.g., subtle blue for code). Considered. Rejected because the secondary always becomes a noise floor — once it exists, every component author reaches for it instead of resolving hierarchy through size/weight/spacing.

## References

- `web/src/styles/tokens.css` — OKLCH primitive + semantic tokens
- `web/src/components/DonateButtons.astro` — no per-network colors
- Variation 02 brutalist-dev-craft (chosen at Gate 1, 2026-05-12)
- LUMEN v0.10.0 banned moves: "Brushed metal con barniz de ingeniería"
