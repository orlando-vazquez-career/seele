# Hero

First-fold hero with logotype, tagline, technical subline, two CTAs, and a status line. Static HTML, no JS.

## Coverage

### Variants
- `default` — single variant. Centered text, max-width 720px.

### Slots
None. Content is internal.

### Tokens consumed
- `--space-{2,3,4,6,8,16,24}` spacing
- `--text-{xs,sm,lg,xl,2xl,3xl}` typography
- `--font-mono` font family
- `--semantic-text-{primary,secondary,tertiary,accent,success}` colors
- `--semantic-surface-elevated` inline code bg
- `--semantic-action-{primary,secondary}-*` button tokens
- `--rounded-{sm,md,pill}` corners
- `--duration-fast`, `--easing-standard` motion

## Validation

### Do
- ✅ Keep tagline to ≤8 words. The 30s comprehension goal depends on it.
- ✅ Technical subline as supporting info, not primary message.
- ✅ Two CTAs only: primary (#install anchor) + secondary (GitHub external).
- ✅ Status line (green dot + version + tests + license) at the bottom — third-tier trust signal.

### Don't
- ❌ Add a third CTA. Cognitive load.
- ❌ Add video, hero image, or animated background. CLS and LCP risk.
- ❌ Marketing speak ("revolutionary", "enterprise-grade"). Banned per DESIGN.md.

### Accessibility
- `<h1>` is the logotype; tagline is `<p>`. Single h1 per page.
- CTAs are `<a>` with semantic href.
- Status dot has `aria-hidden="true"`.

### Perf
- Zero JS. Pure HTML + scoped CSS. No image. Renders <100ms LCP locally.

## Cross-protocol

Hero text changes are PATCH bumps to DESIGN.md (semver) unless the tagline itself is restructured (MINOR).
