# ADR wow-04 — Incomplete borders (3 sides, not 4)

**Sprint:** LUMEN-02
**Date:** 2026-05-12
**Status:** Adopted
**Decisión audaz #** 4 of 5
**Variation:** 02-brutalist-dev-craft

## Context

A four-sided box is the universal "container". Every framework default ships with `border: 1px solid;` on rectangles. This is so common that the brain stops reading it — the border becomes invisible chrome.

Variation 02 proposed something deliberately strange: **borders on 3 sides only**. The missing side communicates "this is attached to something" — either the page edge, the section above, or another component. The border becomes structural information, not decoration.

The risk: it can read as a CSS bug ("they forgot to close the box"). The opportunity: it's the kind of detail that, once a viewer sees it deliberately, makes the rest of the page feel intentional.

## Decision

Implement three border-utility classes in `tokens.css`:

```css
.border-3-sides       /* top + right + bottom (no left) */
.border-2-sides-tl    /* top + left only */
.border-left-only     /* left only */
.border-top-only      /* top only */
```

Each uses `var(--border-base)` (2px) and `var(--semantic-border-base)` (off-white). All set `border-radius: 0` (global default in v0.3.0 anyway, but stated explicitly per component).

Apply rules:
- **Hero data block** uses `.border-3-sides` with `margin-left: var(--space-14)` → missing left border visually "connects" to the indent
- **Post-install `<details>`** uses `.border-3-sides` with `margin-left: var(--space-14)` → same pattern as hero data block, reinforces the asymmetric left-edge commitment
- Section dividers use `.border-top-only` between major content blocks
- Donate-button rows use full 4-sided `border: var(--border-base)` — but on hover invert background to `accent`, no border change

The incomplete-border pattern is **scarce on purpose**. Used twice on the page (hero + post-install), so it reads as deliberate repetition, not as a recurring tic.

## Consequences

**Positive:**
- Border becomes information, not chrome
- The two incomplete-border blocks "anchor" to the asymmetric layout (wow-02) — they don't float, they attach
- Easy to communicate as a system rule ("borders on 3 sides indicate connection to the page edge") for future contributors
- No box-shadow needed for elevation — depth comes from missing borders, not from drop shadows (which v0.3.0 bans anyway)

**Negative:**
- ~15-20% of viewers will read it as a CSS bug on first glance; some will leave before noticing the pattern
- Increases CSS surface area (4 utility classes + their application sites)
- Breaks the mental model of "div = box"; future contributors must learn the system

**Axiom check (5-axiom rubric):**
- **Pri.** Borders no longer decorate; they encode attachment
- **Cont.** 4-sided container default rejected; 3-sided structural replacement
- **Coh.** Pattern is used at 2 distinct scales (hero card + post-install accordion) consistently
- **Ten.** "Looks like a bug" / "looks intentional" tension preserved — not hidden
- **Ev.** Utility classes in `tokens.css` + used in `Hero.astro` + `Install.astro`

## Alternative rejected

Standard 4-sided borders with `border-color` variation (e.g., orange-left-border on emphasis blocks). Considered. Rejected because it doesn't break the "container is a box" assumption — it just colors the box.

## References

- `web/src/styles/tokens.css` — `.border-3-sides` and siblings
- `web/src/components/Hero.astro` — data block usage
- `web/src/components/Install.astro` — post-install `<details>` usage
- Variation 02 brutalist-dev-craft (chosen at Gate 1, 2026-05-12)
