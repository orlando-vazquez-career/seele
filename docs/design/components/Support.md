# Support

Section that wraps the support narrative + `<DonateButtons>` island. Plain HTML.

## Coverage

### Variants
- `default` — single variant.

### Slots
- Renders `<DonateButtons>` (see its own spec).

### Tokens consumed
- `--space-{3,4,6,8,12}` spacing
- `--text-{xs,sm,base,2xl}` typography
- `--semantic-text-{primary,secondary,accent}` colors
- `--semantic-border-subtle` section divider

## Validation

### Do
- ✅ Lead with persona-aware narrative ("built by one developer in his spare hours") — research shows this converts donors better than abstract "support development" copy.
- ✅ List actions in order of effort: star (zero cost) → issues (low) → hire (high).
- ✅ Group donate buttons under a sub-heading "Crypto donations — one click".

### Don't
- ❌ Add a Patreon / Ko-fi / OpenCollective link without first updating FUNDING.yml.
- ❌ Add donation amount presets ("$5", "$25", "$100"). Pattern is amount-free — wallet asks user.
- ❌ Add comparison ("vs. competitors") inside Support. Belongs elsewhere or nowhere.

### Accessibility
- Sub-headings are `<h3>` under the section's `<h2>`.
- Bulleted list semantics via `<ul>` + custom-styled markers.

### Perf
- Section itself ships zero JS. The DonateButtons island handles its own hydration.
