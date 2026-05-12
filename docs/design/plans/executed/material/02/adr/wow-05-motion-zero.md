# ADR wow-05 — Motion zero (transition: none)

**Sprint:** LUMEN-02
**Date:** 2026-05-12
**Status:** Adopted
**Decisión audaz #** 5 of 5
**Variation:** 02-brutalist-dev-craft

## Context

Sprint LUMEN-01 used the conventional easing-token set: `--duration-fast`, `--duration-base`, `--easing-standard`. Every hover triggered a 150ms transform/color transition. The donate widget had a `mix-blend-mode: overlay` sheen on hover with a 240ms ease.

All of those transitions communicated "this is a web app that cares about polish". None of them communicated "this is a tool that runs on your laptop and answers in 5ms".

SEELE's pitch is **"latency < 5ms p95 for first save"**. The landing was actively contradicting that pitch with 240ms hover animations.

## Decision

`transition: none` on every interactive element. `animation: none` except for one exception (the SEELE detection pulse + wallet detection pulse — both signal *live polling*, where motion is the information, not decoration).

Concretely in `tokens.css` v0.3.0:

```css
:root {
  --duration-none: 0ms;
  --duration-pulse: 1.4s;  /* the only allowed duration */
}

*,
*::before,
*::after {
  transition: none !important;
  /* no `animation: none` global — preserve the .dot pulse */
}
```

Hover states are *instant*. Click goes from `bg: bg` to `bg: accent` in zero milliseconds. Color inversion happens on the same paint frame as the cursor enter.

Pulse animation is kept on:
- `<SeeleStatus>` `.dot[data-status='checking']` — 1.4s ease-in-out infinite (information: "we're polling the localhost endpoint")
- `<DonateButtons>` `.detect-dot[data-state='scanning']` — same animation token

## Consequences

**Positive:**
- Landing feels physically faster — hovers respond at the same speed as the cursor
- Honest to the product: SEELE doesn't animate; queries return in ms; the landing should match
- Zero motion-sickness risk; `prefers-reduced-motion` is satisfied automatically (no preference media query needed)
- Removes ~6 easing-curve tokens, simplifies `tokens.css`

**Negative:**
- Hover changes look "abrupt" to viewers conditioned by 150ms ease-out defaults — some will read it as "broken/cheap"
- Removes the implicit "loading" affordance that a transition provides (the brain reads a brief transition as "thinking")
- Future contributors will instinctively add `transition: all 150ms` to new components — must be guarded by lint or convention

**Axiom check (5-axiom rubric):**
- **Pri.** Motion is no longer "polish"; it is information (pulse = polling)
- **Cont.** 150ms-ease-out default rejected; zero-duration explicit replacement
- **Coh.** Every interactive element follows the same instant-response rule
- **Ten.** "We have spent 240ms polishing a hover transition" tension preserved
- **Ev.** `tokens.css` `--duration-none` + `* { transition: none }` reset

## Alternative rejected

Keep transitions but shorten everything to ≤80ms. Considered. Rejected because 80ms is still 16× the SEELE p95 latency claim — the inconsistency between "we are fast" (product copy) and "we will pause briefly when you hover" (UI) remains.

## References

- `web/src/styles/tokens.css` — `--duration-none` + global reset
- `web/src/components/SeeleStatus.astro` — pulse exception
- `web/src/components/DonateButtons.astro` — scan-state pulse
- Variation 02 brutalist-dev-craft (chosen at Gate 1, 2026-05-12)
- LUMEN v0.10.0 banned moves: "polished defaults that fight the product claim"
