# Header

Sticky top navigation with brand mark + anchor nav + GitHub external link.

## Coverage

### Variants
- `default` — desktop layout, all nav items visible.
- Mobile (≤640px) — nav items collapse, only GitHub remains visible.

### Slots
None.

### Tokens consumed
- `--semantic-surface-base` (with `0.85` alpha + backdrop-filter blur)
- `--semantic-border-subtle` bottom border
- `--semantic-text-{primary,secondary}` text
- `--semantic-action-primary-bg/text` brand mark
- `--space-{2,3,4,6}` spacing
- `--rounded-sm` brand mark corner
- `--text-{base,sm}` typography
- `--z-sticky` stacking
- `--duration-fast`, `--easing-standard` motion

## Validation

### Do
- ✅ Sticky positioning — readers reference it during scroll.
- ✅ `backdrop-filter: blur(8px)` over translucent bg for the depth cue.
- ✅ GitHub link is always visible (mobile too) — it's the primary action when nav is collapsed.

### Don't
- ❌ Add a search input. Site is one-page.
- ❌ Add a dropdown menu. One level of nav, all visible.
- ❌ Add a logo image. Brand mark is text + colored tile — saves an HTTP request.
- ❌ Add a dark/light mode toggle. v0.1 is dark-only by design decision (DESIGN.md).

### Accessibility
- `<header>` landmark.
- `<nav aria-label="Primary">`.
- Brand link has `aria-label="SEELE home"`.
- GitHub link opens in new tab with `rel="noopener"` for security.

### Perf
- Zero JS. Pure CSS sticky + backdrop-filter (graceful degradation if unsupported).
