# Install + InstallCard

`<Install>` is the section container with 3 install paths + a collapsible "After install" block. `<InstallCard>` is the path primitive with a copy-to-clipboard command.

## Coverage

### Variants
- `Install.default` — grid 3 columns desktop, 1 column mobile.
- `InstallCard.default` — neutral border.
- `InstallCard.recommended` — accent border + "recomendado" badge top-left.

### Slots
- `InstallCard` props: `title`, `description`, `command` (string or multi-line), `recommended?: boolean`, `notes?: string`.

### Tokens consumed
- `--semantic-surface-{base,elevated}` containers
- `--semantic-border-{subtle,strong}` cards + code blocks
- `--semantic-text-{primary,secondary,tertiary,accent,success}` text
- `--semantic-action-primary-bg` badge bg
- `--space-{1,2,3,4,6,8,12}` spacing
- `--text-{xs,sm,base,2xl}` typography
- `--rounded-{sm,md}` corners
- `--duration-fast`, `--easing-standard` motion

## Validation

### Do
- ✅ One card flagged `recommended` per page. Currently: install script.
- ✅ Command is the copyable string, no trailing prompt characters (`$`, `>`).
- ✅ `notes` for platform-specific gotchas (Windows variant, MSRV, etc).
- ✅ Multi-line commands use `\n` in the command prop — `<pre>` preserves whitespace.

### Don't
- ❌ Hide the command behind a tab or accordion. Visible on first scroll.
- ❌ Add a "verify checksum" step inline — link to docs/INSTALLATION.md instead.
- ❌ Add a 4th install path without UX reconsideration (3 fits the grid cleanly).

### Accessibility
- Copy button has `aria-label` describing the path.
- `<pre>` + `<code>` semantics for the command block.
- `<details><summary>` for the post-install snippet (collapsed by default).
- Copy button focus-visible styled.

### Perf
- Tiny inline script for copy-to-clipboard (~150 bytes). Shared across all 3 cards via class selector — no re-binding cost.
