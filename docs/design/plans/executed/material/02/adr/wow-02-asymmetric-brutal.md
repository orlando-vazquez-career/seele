# ADR wow-02 — Asymmetric brutal layout

**Sprint:** LUMEN-02
**Date:** 2026-05-12
**Status:** Adopted
**Decisión audaz #** 2 of 5
**Variation:** 02-brutalist-dev-craft

## Context

Sprint LUMEN-01 used a centered-hero composition with symmetric padding on both sides of the page-container. The director's reaction: *"ese triangulo negro sin contexto al medio y encima corrido no queda bien"*. The triangle was orphaned because it sat in a layout that promised balance and then broke it.

Two failure modes can produce that effect:
1. **Symmetric layout with one off-axis element** — feels like a bug, not a choice
2. **Genuinely asymmetric layout that commits** — feels like a brand decision

The first one is what Sprint-01 shipped.

## Decision

Commit to asymmetric. Specifically:

- Page-container uses **asymmetric margins**: `margin-inline-start: clamp(16px, 4vw, 64px)` and `margin-inline-end: clamp(80px, 12vw, 200px)` — content hugs the left, leaves visible breathing room on the right
- Hero data block is offset with `margin-left: var(--space-14)` — sits indented, like a continuation of the headline, not aligned with it
- Features grid is asymmetric column ratios: `1.5fr 1.25fr 1.25fr 1fr` — CLI tile is the widest (30%), TUI tile is the narrowest (20%), in deliberate descending hierarchy
- `<details>` post-install block uses `.border-3-sides` and offsets `margin-left: var(--space-14)` — the missing left border "connects" it to the indent it's already sitting in
- Section heading uses two-line layout: line 1 + `<br />` + line 2, both flush-left, no centering

The viewer should feel that the page has a left edge it commits to, and a right edge that breathes — like a magazine spread with a strong gutter on one side.

## Consequences

**Positive:**
- No element is "orphaned in the middle" because nothing reaches for a middle that doesn't exist
- The whitespace on the right becomes part of the brand voice — confident, not empty
- Grid asymmetry on features makes the visual hierarchy match the verbal one (CLI is the headline interface)
- Easy to maintain: one rule (left-align everything) replaces a dozen alignment decisions per component

**Negative:**
- Some users expect centered content from "landing pages" — this breaks that expectation
- On viewports < 540px the asymmetric margins compress to symmetric; the brand identity is weakest on mobile
- The right-side whitespace looks "wasted" to anyone treating it as wasted (instead of as composed)

**Axiom check (5-axiom rubric):**
- **Pri.** Layout is no longer a neutral container — it carries the brand
- **Cont.** Centered-hero default rejected; asymmetric explicit replacement
- **Coh.** Every section follows the same left-edge commitment
- **Ten.** δ ("triángulo huérfano") preserved as motivation
- **Ev.** DESIGN.md v0.3.0 spacing section + `.page-container` rules

## Alternative rejected

Centered hero with the triangle moved to a clearly-non-orphaned position (e.g., as a corner glyph, top-left of the page). Considered safer. Rejected because it would have preserved the symmetric layout that produced the orphan in the first place — moving the triangle doesn't fix the system.

## References

- `web/src/styles/tokens.css` — `.page-container` margins
- `web/src/components/Features.astro` — asymmetric grid ratios
- `web/src/components/Hero.astro` — data-block offset
- Variation 02 brutalist-dev-craft (chosen at Gate 1, 2026-05-12)
