# ADR wow-01 — Monospace-only typography

**Sprint:** LUMEN-02
**Date:** 2026-05-12
**Status:** Adopted
**Decisión audaz #** 1 of 5
**Variation:** 02-brutalist-dev-craft

## Context

The SEELE landing previously used a display-serif + sans-serif pair (Inter for body, Recoleta-like display). Functional, but visually indistinguishable from a thousand other developer-tool landings. The director's note from Sprint LUMEN-01 was explicit: *"ese diseño tan basico sacado directo de tailwind me da cosita"*. The Tailwind-default smell came from the typeface combo as much as from the layout.

SEELE is a tool that programmers run in a terminal. Its primary interfaces (CLI, MCP stdio, TUI) are monospace by definition. The landing has been treating those interfaces as content to *escape from* (decorating them with fancy sans-serif headings) rather than as the thing being sold.

## Decision

Use **JetBrains Mono** for every text element on the landing. Every. Single. One. No serif. No sans-serif. No system-ui except the OS-monospace fallback chain (`ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`) when JetBrains Mono fails to load.

- Hero headline: JetBrains Mono, 72px, weight 800
- Body copy: JetBrains Mono, 15px, weight 400
- Meta labels: JetBrains Mono, 11px, weight 700, letter-spacing 0.12em, uppercase
- Code blocks: same family — no visual switch from prose to code

The font is self-hosted from `/fonts/` (already in `tokens.css` v0.3.0). 4 weights bundled (400, 500, 700, 800). Total: ~120 KB woff2 across the wire.

## Consequences

**Positive:**
- Visual identity becomes inseparable from the product: the landing *is* the terminal
- No reader uncertainty about what's prose and what's a command — the typeface stops volunteering that distinction
- Eliminates ~30 KB of Inter that was loaded before
- Forces every heading/body decision to live within one rhythm — no escape hatch into "make it pretty with serif"

**Negative:**
- Less hierarchy from typeface alone — must lean harder on size, weight, color, and whitespace
- Monospace bodies at 15px feel slightly wider than proportional — copy must stay tight (max-width 60ch)
- Some users associate monospace with "code" not "marketing" — for SEELE that's the point; for a generic SaaS it would be hostile

**Axiom check (5-axiom rubric, LUMEN v0.10.0):**
- **Pri.** Type is no longer "neutral" — it carries the brand
- **Cont.** Mono-only sans/serif default replaced with explicit replacement
- **Coh.** All 7 components inherit from one family chain
- **Ten.** δ ("Tailwind genérico me da cosita") preserved as motivation
- **Ev.** This ADR + DESIGN.md v0.3.0 + tokens.css

## Alternative rejected

Mono + a single sans-serif accent for body (e.g., JetBrains Mono headlines + Inter 14px body). Considered safer. Rejected because the "Tailwind smell" emerges precisely from that pairing — any sans-serif body in 2026 reads as "default React landing".

## References

- `docs/design/DESIGN.md` v0.3.0 typography section
- `web/src/styles/tokens.css` — `--font-display`, `--font-body`, `--font-mono` collapsed into one value
- Variation 02 brutalist-dev-craft (chosen at Gate 1, 2026-05-12)
