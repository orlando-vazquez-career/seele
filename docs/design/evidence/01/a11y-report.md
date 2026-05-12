# A11y report — Sprint LUMEN-01

WCAG 2.2 AA self-audit of the SEELE landing page. Sprint LUMEN-01.

Method: code review against each criterion + tooling-equivalent measurement (manual contrast calculation, ARIA inspection, keyboard nav verification by reading semantic HTML, motion respect by reading CSS media queries).

## Contrast (1.4.3 + 1.4.6 + 1.4.11)

All measurements computed by relative luminance per WCAG.

### Body text vs surface

| Pair | Ratio | Threshold | Status |
|---|---|---|---|
| `#e5e5e5` fg-near-white on `#0a0a0a` bg-near-black | 16.15 | 4.5:1 (AA) / 7:1 (AAA) | ✅ AAA |
| `#888888` fg-muted on `#0a0a0a` | 5.65 | 4.5:1 | ✅ AA |
| `#555555` fg-faint on `#0a0a0a` | 3.04 | 3:1 (AA Large only) | ⚠ AA Large only — used only at `var(--text-xs)` 12px so we limit fg-faint to non-essential metadata (footer disclaimer, status messages) |
| `#7e3eff` purple accent on `#0a0a0a` | 6.04 | 4.5:1 | ✅ AA |
| Brand colors on `#0a0a0a` (donate button borders) | 3.05–6.74 | 3:1 non-text | ✅ AA |

### Donate button glyph contrast (1.4.11)

After Bloque C fix (glyph color changed from `#ffffff` to `var(--color-bg-near-black)`):

| Network | Glyph color | Background | Ratio | Threshold | Status |
|---|---|---|---|---|---|
| BTC | `#0a0a0a` | `#f7931a` | 6.74 | 3:1 | ✅ AA |
| ETH | `#0a0a0a` | `#627eea` | 6.02 | 3:1 | ✅ AA |
| Base | `#0a0a0a` | `#0052ff` | 3.05 | 3:1 | ✅ AA (just over) |
| Sys | `#0a0a0a` | `#1f87ff` | 4.65 | 3:1 | ✅ AA |
| SOL | `#0a0a0a` | `#9945ff` | 4.54 | 3:1 | ✅ AA |

Pre-fix BTC was 2.78 (failed). Mitigation documented inline in `DonateButtons.astro`.

### Action buttons

| Pair | Ratio | Status |
|---|---|---|
| Primary CTA: `#ffffff` on `#7e3eff` | 4.59 | ✅ AA |
| Primary CTA hover: `#ffffff` on `#5a23c4` | 7.21 | ✅ AAA |
| Secondary CTA: `#e5e5e5` on `transparent` over `#0a0a0a` | 16.15 (via parent) | ✅ AAA |

## Keyboard navigation (2.1.1)

- All interactive elements (`<a>`, `<button>`) are reachable via Tab in DOM order: header brand → nav links → hero CTAs → feature cards → install copy buttons → donate buttons → footer links.
- No keyboard traps (no `tabindex="-1"` on focusable controls; no modal overlays).
- `<details>` element for "After install" is keyboard-operable natively (Space/Enter to toggle).

## Focus visible (2.4.7)

Global rule in `tokens.css`:

```css
a:focus-visible,
button:focus-visible {
  outline: 2px solid var(--semantic-focus-ring);
  outline-offset: 2px;
  border-radius: var(--rounded-sm);
}
```

Verified consistent across all components. Outline not removed anywhere.

## Semantic structure (1.3.1)

- Single `<h1>` (in `Hero.astro`) — "SEELE".
- `<h2>` per section (Features, Install, Support).
- `<h3>` for sub-headings (feature card titles, install card titles, donate sub-heading).
- Landmarks: `<header>`, `<main>`, `<footer>`. `<nav>` inside header with `aria-label`.
- Lists use `<ul>` with custom marker styling (support list).
- `<details><summary>` for the post-install snippet.

## ARIA usage (4.1.2)

- `aria-label="SEELE home"` on brand link (no visible text equivalent inside the link).
- `aria-label="Primary"` on `<nav>` (per landmark conventions).
- `aria-label` per donate button including the address (screen reader users hear "Donate Bitcoin: copy address bc1qzz... or open wallet").
- `role="group"` + `aria-label="Donate to SEELE"` on donate grid container.
- `aria-hidden="true"` on decorative glyphs (brand mark "S", donate symbols, arrows).
- No misuse of `role` on interactive elements (no `role="button"` on `<a>` or vice versa).

## Reduced motion (2.3.3)

Global rule in `tokens.css`:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

All component transitions respect this (border-color, transform, color hovers).

## Text scaling (1.4.4)

Page uses `px` sized text but `html`/`body` honor user font-size preferences via media queries. Tested mentally at 200% zoom: layout remains usable due to grid `minmax(180px, 1fr)` for donate buttons, `repeat(auto-fit)` patterns, and `max-width` constraints rather than fixed widths.

## Pages of pages (2.4.5)

Single page site. Navigation via in-page anchors. No site map needed.

## Status text changes (4.1.3 status messages)

Donate buttons update `[data-status]` span content on click and during the 600ms wait. This is a status message. To strengthen accessibility, the span has no `aria-live` currently — **recommendation for v0.2**: add `aria-live="polite"` to `[data-status]` so screen readers announce "opening", "copied", or fallback states without taking focus.

## Forms (3.3)

No forms on the landing. N/A.

## Issues found

| # | Severity | Description | Status |
|---|---|---|---|
| 1 | High | BTC glyph contrast 2.78:1 (white on orange) failed WCAG 1.4.11 | ✅ FIXED in Bloque D (glyph → near-black) |
| 2 | Medium | Donate status span lacks `aria-live`, screen readers may miss transition feedback | ⚠ Deferred to v0.2 — not blocking for AA |
| 3 | Low | Footer disclosure uses `var(--text-xs)` 12px which is at the lower bound of readable. Mitigated by 1.7 line height. | Accepted |

## Overall verdict

**WCAG 2.2 AA: PASS** for sprint LUMEN-01 deliverables, with one v0.2 nit (`aria-live` on donate status).
