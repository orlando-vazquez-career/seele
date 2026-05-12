# Heuristic evaluation — SEELE web v0.3.0 (Brutalist dev-craft)

**Sprint:** LUMEN-02
**Date:** 2026-05-12
**Framework:** Nielsen's 10 Usability Heuristics + LUMEN v0.10.0 5-axiom rubric
**Evaluator:** static analysis on built `dist/index.html` + source components

Scoring: **pass** / **partial** / **fail**, with notes that explain when a brutalist choice deliberately trades a heuristic for an axiomatic gain.

## H1 — Visibility of system status

- **`<SeeleStatus>`** announces 4 states (checking / detected / not-detected / blocked) with a pulse dot in the `checking` state. Status is visible without click.
- **Donate buttons** show per-action status: opening / sent / rejected / copied / install / error / pending. Each transitions to the next state within the same button.
- **Install card COPY button** toggles to "COPIED" for 1.5s after a click.
- **`<DonateButtons>` detection banner** updates from "scanning…" to either "wallets detected: <names>" or "no wallet extension detected".

**Verdict: pass.** System status is communicated at three scales (page-level for SEELE detect, section-level for wallet detect, per-element for copy/sent). No silent failures.

## H2 — Match between system and the real world

- Vocabulary is *engineer-real-world*: "binary", "WAL", "MCP", "EIP-1193", "BIP-21", "OpenAPI 3.1", "cargo install".
- This is correct for SEELE's audience (developers running CLI tools on their laptops). It is **wrong** for a non-technical visitor — but a non-technical visitor is explicitly not the audience.

**Verdict: pass for audience.** Brutalist + monospace amplifies the in-group vocabulary; non-developers will bounce immediately, which is the intended filter.

## H3 — User control and freedom

- No flows trap the user. No modal dialogs. No multi-step forms.
- `<details>` post-install accordion: keyboard-native, closeable.
- Donate-button click states (opening / pending / rejected) all clear themselves after a timeout — the user never gets stuck on a permanent intermediate state.
- The **rejected** path is preserved as a first-class state (not hidden, not retried) — user rejecting a wallet popup gets clear feedback.

**Verdict: pass.** One minor note: there is no in-page "undo" for the install-COPY (it's a one-way clipboard write). Acceptable because the clipboard's prior content was outside our scope.

## H4 — Consistency and standards

- Every component uses the same 3 CSS tokens (`--bg`, `--fg`, `--accent`) for chrome.
- Every `// 0X` eyebrow follows the same format (slash-slash, two digits, uppercase if needed).
- Every interactive hover follows the same rule: background inverts to `--accent`, text inverts to `--bg`, no transition.
- Every `<button>` uses the same monospace family inherited from `<body>`.

**Verdict: pass.** Internal consistency is very high — the rigid token system makes per-component drift hard to introduce.

## H5 — Error prevention

- Donate buttons require a 2-step confirmation **at the wallet level** (the wallet extension shows its own popup before any tx is sent). We do not initiate transactions silently.
- The `eth_sendTransaction` call uses `value: '0x0'` — the user is never tricked into sending a non-zero amount. They must edit the amount in their wallet popup. (This is the "intentional donations only" pattern from Sprint LUMEN-01.)
- No destructive UI actions on the page. Clicking install COPY only writes to clipboard; nothing executes.

**Verdict: pass.**

## H6 — Recognition rather than recall

- Three install paths are presented side-by-side as cards with full commands visible. The user does not have to remember which one they wanted.
- Each donate button shows the truncated address (`bc1qz…7kte`) so the user can verify visually before clicking.
- The detection banner names which wallet extension was found (MetaMask / Pali / Phantom) so the user knows which popup to expect.

**Verdict: pass.**

## H7 — Flexibility and efficiency of use

- The 3-install-path grid is a built-in efficiency tool: experts pick `cargo install`, beginners pick the install-script, contributors pick `git clone`.
- COPY buttons remove the need to manually select-and-copy.
- Keyboard navigation supports all interactive elements.

**Verdict: pass.**

## H8 — Aesthetic and minimalist design

- 3-color palette, 1 typeface family, zero gradients, zero shadows, zero decorative SVGs (except 1 brand mark).
- Every visible token earns its place — there is no "decorative chrome" by design.
- Tension: brutalism can read as *too* minimal to some users — they expect more visual reassurance. This is the explicit aesthetic decision (wow-02, wow-03), not an oversight.

**Verdict: pass with intentional friction.** The minimalism is the brand, not a void.

## H9 — Help users recognize, diagnose, and recover from errors

- Donate-button error states show `rejected` / `error` / `check wallet` / `install →` — each names what happened and (where relevant) what to do next.
- `<SeeleStatus>` blocked state ("auto-detect needs local hosting") explains *why* the localhost probe failed when served over https.
- No error 404 / error 500 paths in scope — single-page static.

**Verdict: pass.**

## H10 — Help and documentation

- Hero CTAs point to `#install` (in-page) and the GitHub repo (out-of-page authoritative docs).
- "Full matrix in docs/INSTALLATION.md" link in the install section connects to the canonical doc.
- Footer links to changelog, discussions, and license.
- No in-page tutorial or onboarding tooltips. Acceptable: the audience reads `--help` output and READMEs, not in-page tooltips.

**Verdict: pass for audience.**

## LUMEN v0.10.0 5-axiom rubric (parallel pass)

Repeated from `material/02/visual-critique.md` for completeness:

- **Pri.** Color, typography, layout all carry brand load. **Pass.**
- **Cont.** Tailwind defaults rejected and replaced. **Pass.**
- **Coh.** Single token system; no per-component overrides. **Pass.**
- **Ten.** δ critiques named in ADRs, not hidden. **Pass.**
- **Ev.** 5 ADRs + 1 visual-critique + 3 evidence reports + 3 persisted variations. **Pass.**

## Summary

| heuristic | verdict |
|-----------|---------|
| H1 visibility of system status      | pass |
| H2 match real world                 | pass for audience |
| H3 user control and freedom         | pass |
| H4 consistency and standards        | pass |
| H5 error prevention                 | pass |
| H6 recognition over recall          | pass |
| H7 flexibility and efficiency       | pass |
| H8 aesthetic minimalism             | pass with intentional friction |
| H9 error recognition and recovery   | pass |
| H10 help and documentation          | pass for audience |

**Overall verdict:** No remediation required. Brutalism imposes intentional friction on H2 / H8 / H10 (vocabulary, minimalism, no in-page help) — those are not regressions, they are filters that match the audience. Public launch can proceed.
