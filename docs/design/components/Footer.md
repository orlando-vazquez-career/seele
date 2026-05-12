# Footer

Bottom strip with copyright, version link, ENGRAM attribution, GitHub link, and honest bandwidth disclosure.

## Coverage

### Variants
- `default` — single layout, wraps on mobile.

### Slots
None.

### Tokens consumed
- `--semantic-surface-base` bg
- `--semantic-border-subtle` top border
- `--semantic-text-{secondary,tertiary,accent}` text
- `--space-{2,4,6,8,12}` spacing
- `--text-xs` typography

## Validation

### Do
- ✅ Include the bandwidth disclosure ("This page weighs ~25 KB"). It's a trust signal in 2026.
- ✅ Credit ENGRAM here in addition to the README. Attribution is permanent.
- ✅ Year is dynamic (computed at build time).
- ✅ Latin tagline as a quiet signature — matches the README's blockquote.

### Don't
- ❌ Add social media icons. Project is dev-tool, not personal brand.
- ❌ Add a newsletter signup. Banned per DESIGN.md.
- ❌ Add "site map" if site stays one-page.

### Accessibility
- `<footer>` landmark.
- All external links have `rel="noopener"` and `target="_blank"`.

### Perf
- Zero JS. Date computed at SSR/build time — no runtime cost.
