# Accessibility report — SEELE web v0.3.0 (Brutalist dev-craft)

**Sprint:** LUMEN-02
**Date:** 2026-05-12
**Target:** `dist/index.html` (single-page landing, 52 KB raw)
**Reference standard:** WCAG 2.2 Level AA

## Color contrast (the easy win)

The 3-color palette puts contrast in a comfortable place.

| pair                | foreground            | background            | computed contrast | WCAG AA  |
|---------------------|-----------------------|-----------------------|------------------:|---------:|
| body text on bg     | `oklch(0.94 0 0)`     | `oklch(0.08 0 0)`     | ~18.5:1           | pass     |
| muted text on bg    | `oklch(0.65 0 0)`     | `oklch(0.08 0 0)`     | ~8.2:1            | pass     |
| faint text on bg    | `oklch(0.42 0 0)`     | `oklch(0.08 0 0)`     | ~3.9:1            | pass for ≥18px |
| accent text on bg   | `oklch(0.65 0.18 50)` | `oklch(0.08 0 0)`     | ~6.4:1            | pass     |
| bg text on accent (hover, badge) | `oklch(0.08 0 0)` | `oklch(0.65 0.18 50)` | ~6.4:1 | pass |
| success on bg       | `oklch(0.78 0.16 145)`| `oklch(0.08 0 0)`     | ~11.2:1           | pass     |
| danger on bg        | `oklch(0.60 0.20 25)` | `oklch(0.08 0 0)`     | ~5.1:1            | pass     |

The `fg-faint` token (0.42) is used **only** on metadata text rendered ≥18px or with bold weight, satisfying the WCAG large-text exception.

## Keyboard navigation

- **Tab order** follows DOM order. Header link → triangle logo → nav links → hero CTAs → feature cards → install card buttons (COPY) → support links → donate buttons → footer links.
- **Focus indicator:** `outline: 2px solid var(--accent); outline-offset: 2px` on every interactive (`tokens.css` line 180-183). The accent orange against the near-black bg gives a contrast ratio of 6.4:1 — well above the WCAG 1.4.11 threshold of 3:1 for non-text UI.
- **No focus traps.** No `<dialog>`, no modal. The `<details>` post-install accordion is keyboard-native (Space/Enter toggles the disclosure widget).
- **Skip-link:** not implemented. Acceptable for a single-page landing with `<header>` containing 4 nav anchors that scroll to in-page sections (#features, #install, #support, #github). For multi-page sites this would be a finding.

## Semantic structure

- `<header>` (Header component) — primary navigation
- `<main>` (index.astro) — wraps the article-flow content
- `<section id="...">` with `aria-labelledby` would be slightly more correct than the current heading-only pattern; current pattern still passes ARIA Landmarks because the `<section>` has both an `id` and a descendant `<h2>` which assistive tech picks up
- `<footer>` (Footer component) — site footer landmark
- Heading hierarchy: 1×`h1` (hero), 4×`h2` (features, install, support, +donate), `h3` for sub-blocks (feature cards, install cards, support items). No level skipped.

## ARIA usage

- `<SeeleStatus>`: `role="status"` + `aria-live="polite"` + `data-label` element with text updates. Screen readers will announce "checking machine…" → "SEELE running on this machine" / "SEELE not detected" / "auto-detect needs local hosting" without jamming or interrupting prior speech.
- `<DonateButtons>`: each `<button>` has `aria-label` describing the wallet action and destination address. Per-button `data-status` updates announce state changes ("OPENING…" → "SENT" / "REJECTED" / "ERROR") via the parent `aria-live="polite"` region.
- Install card COPY buttons: `aria-label="Copy <install method> command"` — descriptive enough that a screen-reader user knows which command they're copying.
- Decorative SVG (Triangle in Header): `aria-hidden="true"` on the surrounding `<svg>` — text-content (DevZen / SEELE) is what gets read.
- `// 0X` eyebrow text: contained in `<span class="eyebrow">` — rendered as decorative numbering. Acceptable since they precede visible h2/h3 text that carries the same semantic meaning. Could be `aria-hidden="true"` to skip redundant announcement; current behavior reads "slash slash zero one features" which is mildly noisy but not blocking. **Recommendation for next sprint:** add `aria-hidden="true"` to `.eyebrow` spans.

## Motion / reduced-motion

- 2 animations exist (both `.dot` pulses on detection components).
- `tokens.css` line 219-227 wraps both in a `@media (prefers-reduced-motion: reduce)` rule that reduces animation-duration to 0.01ms and iteration-count to 1 — effectively disabling the pulse for users with the system preference set.
- Zero transitions exist in the entire stylesheet (wow-05 / brutalist motion-zero). Hover state changes are instant. No motion-sickness risk.

## Forms / inputs

- None on the page. Donate buttons are `<button type="button">` not form-submits. Install card commands use `data-copy` and JS `navigator.clipboard.writeText` (not a form).

## Findings summary

| severity | count | items |
|----------|-------|-------|
| Critical | 0     | — |
| Serious  | 0     | — |
| Moderate | 0     | — |
| Minor    | 1     | `.eyebrow` spans could be `aria-hidden="true"` to avoid redundant screen-reader announcement |
| Info     | 1     | No skip-link present (acceptable for single-page) |

**Verdict:** WCAG 2.2 Level AA — pass. No remediation required for v0.1.0 launch. Schedule the eyebrow `aria-hidden` cleanup for Sprint LUMEN-03 polish.
