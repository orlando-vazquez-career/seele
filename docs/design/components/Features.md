# Features + FeatureCard

`<Features>` is the section container. `<FeatureCard>` is the reusable card primitive (4 instances in v0.1: CLI, MCP, HTTP, TUI).

## Coverage

### Variants
- `Features.default` — grid 4 columns desktop, 2 columns tablet, 1 column mobile.
- `FeatureCard.default` — single variant. Auto-sizes to grid cell.

### Slots
- `FeatureCard` props: `icon`, `title`, `blurb`, `meta`, `href`.

### Tokens consumed
- `--semantic-surface-elevated` card bg
- `--semantic-border-subtle/strong` borders + hover
- `--semantic-text-{primary,secondary,tertiary,accent}` text colors
- `--space-{1,2,4,6,8,12}` spacing
- `--rounded-md` corners
- `--text-{xs,sm,base,xl,2xl}` typography
- `--duration-fast`, `--easing-standard` motion

## Validation

### Do
- ✅ Keep blurb to 1-2 sentences. Reader scans 4 cards in <10s.
- ✅ Use `meta` for the canonical command (monospace, secondary color).
- ✅ Cards link to docs/ section in GitHub, not to internal pages (v0.1 has no internal docs).

### Don't
- ❌ Add screenshots or animated GIFs to cards. Defeats the static-fast goal.
- ❌ Add a 5th feature without considering the 4-column grid wrap-down.
- ❌ Use brand colors here. Cards stay neutral; brand is reserved for donate.

### Accessibility
- Cards are `<a href>` — fully keyboard reachable.
- Icon is `aria-hidden="true"`.
- `<h3>` per card; section is preceded by `<h2>`.

### Perf
- Zero JS. CSS grid. No external assets.
