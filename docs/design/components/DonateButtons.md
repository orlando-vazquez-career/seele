# DonateButtons

The only client-hydrated island in the SEELE landing. Renders a grid of 5 buttons (Bitcoin · Ethereum · Base · Syscoin NEVM · Solana) and on click attempts to open the user's preferred wallet via URI scheme, falling back to clipboard.

## Coverage

### Variants
- `default` — single layout, no variants. Buttons auto-size to fit content.

### States
- `idle` — initial render, no status text.
- `opening` — between click and 600ms timeout. Status text reads `opening…` in accent color.
- `copied` — fallback path: address was copied to clipboard. Status text `copied` in success green, auto-clears after 2000ms.
- `fallback` — clipboard API unavailable; status shows first 8 chars of address in muted color, auto-clears after 2000ms.
- `:hover` — `--brand` color border, lift transform `-1px`.
- `:focus-visible` — focus ring via global rule.

### Sizes
- Single size. Buttons grow to fill `minmax(180px, 1fr)` in autofit grid.

### Tokens consumed
- `--semantic-surface-elevated` — button bg
- `--semantic-border-strong` — button border
- `--semantic-text-primary/secondary/tertiary/accent/success` — text colors
- `--color-network-{btc,eth,base,sys,sol}` — assigned to `--brand` per button
- `--space-{2,3,4,6}`, `--rounded-{md,pill}`, `--text-{xs,sm,lg}`, `--font-mono`
- `--duration-fast`, `--easing-standard` — motion

### Slots
None. Targets are configured via the `targets[]` array in frontmatter (BTC/ETH/Base/Sys/SOL). To add a chain, append to the array.

## Validation

### Do
- ✅ Keep this as the only client-hydrated component on the landing.
- ✅ Use `window.location.href = uri` (not `window.open`) — research shows it's more reliable across mobile OS handlers.
- ✅ 600ms fallback delay — empirically the sweet spot per research (Mar-May 2026 OSS donate buttons).
- ✅ `document.hidden` check — only copy if the page is still visible (wallet didn't take over).
- ✅ Try/catch around `navigator.clipboard.writeText` — fails silently in restricted contexts (private mode, sandbox).
- ✅ ARIA label including the address — screen reader users hear the destination explicitly.
- ✅ `aria-hidden="true"` on the glyph — decorative.

### Don't
- ❌ Add WalletConnect, RainbowKit, wagmi, or any web3 SDK. Pattern is intentionally vanilla.
- ❌ Add analytics or tracking on click — violates DESIGN.md no-tracking rule.
- ❌ Add `https://` shim links or QR codes inside the button — keep one CTA per button.
- ❌ Reduce the fallback delay below 400ms — clipboard fires before wallet has time to open, double-feedback confuses user.
- ❌ Add EIP-1193 `eth_requestAccounts` connect flow — out of scope (donate is one-shot, not a dApp).
- ❌ Persist any state (cookies, localStorage) — privacy invariant.

### Accessibility
- All buttons are `<button type="button">`, keyboard reachable, `:focus-visible` styled.
- ARIA label on each button includes the destination address explicitly.
- Glyph + label both present; glyph has `aria-hidden`, label is readable.
- Container is `role="group"` with `aria-label="Donate to SEELE"`.
- Status text updates are perceivable: color + text both change.

### Perf
- Hydrate is bound globally on script load. No framework runtime.
- JS budget: ~1.5 KB gzipped including the script.
- No external requests on hydrate.

## Cross-protocol

This component is persisted to SEELE per LUMEN integration:

```bash
seele save --type pattern --topic-key design/component/DonateButton \
  "Vanilla URI-scheme donate button" \
  "Pattern: window.location.href = <uri>; 600ms setTimeout fallback to clipboard if !document.hidden. EIP-681 with @chainId for EVM, Solana Pay for SOL, BIP-21 for BTC. No SDK, no tracking, no WalletConnect."
```

## References

- EIP-681 — https://eips.ethereum.org/EIPS/eip-681
- Solana Pay spec — https://docs.solanapay.com/spec
- BIP-21 / BIP-321 — https://bips.dev/321/
- Research devlog: `docs/design/plans/scaffold/01/flows/donate.md`
