## 17. Web Landing & Observability — `web/`

### 17.1 Purpose and place in the system

`web/` is a self-contained Astro static site — the public marketing landing for SEELE plus a live "observability" page. The `astro.config.mjs` header comment calls it "Astro 5.x static", but `package.json` pins `astro: ^6.0.0`, so the running framework is Astro 6. It is **not** a Rust crate and does not appear in the workspace dependency graph; it ships separately to GitHub Pages and has no compile-time coupling to the binary. Its only runtime coupling is over HTTP: three of its client components (`SeeleStatus`, `Observability`, `ChatPanel`) talk to a *user-owned* `seele serve` instance on `localhost:7777`. The hosted site has no database and no server of its own — every dynamic feature degrades to static sample data or a "configure your key" prompt when no local server is reachable. This is the central design contract of the page: "all paths lead to localhost" (the footer copy line, `Footer.astro:62`). It was produced under the **LUMEN protocol** (a frontend analogue of AEGIS), sprints LUMEN-01..03, with the design system frozen in `DESIGN.md` v0.3.0.

The site has a strict, unusual design system: **Brutalist dev-craft**. Three colors, one monospace family, 2px solid borders as the only hierarchy primitive, incomplete borders as deliberate "visual syntax", and zero motion (`transition: none`). The pitch — that the target persona (platform/infra/ML engineers) rewards technical credibility over visual warmth — is captured by the front-matter tagline "credibility before warmth" (`DESIGN.md:4`) and argued in the "Why this direction" prose (`DESIGN.md:156-162`, in Spanish: "la audiencia premia credibilidad técnica antes que calidez visual").

### 17.2 Build configuration

`web/package.json`: name `seele-web`, version `0.1.0`, `type: module`, `private: true`, license MIT, `engines.node: >=20.0.0`. The single runtime dependency is `astro: ^6.0.0` (note: the package `description` field says "Astro 5 static site" but the pin is `^6`). The lone dev dependency is `@playwright/test: ^1.60.0`; the screenshot tool actually imports the bundled `playwright` package directly (`import { chromium } from 'playwright'`). Scripts: `dev`/`build`/`preview`/`check` are stock Astro (`astro dev|build|preview|check`); `visual` runs `node scripts/visual-critique.mjs`; `visual:preview` adds `--url http://localhost:4321/seele`.

`web/astro.config.mjs` is small but load-bearing:

```js
// web/astro.config.mjs:11-25
export default defineConfig({
  site: 'https://orlando-vazquez-career.github.io',
  base: '/seele',
  trailingSlash: 'never',
  output: 'static',
  build: { inlineStylesheets: 'always', assets: 'assets' },
  vite: { build: { cssCodeSplit: false } },
});
```

| Option | Value | Why |
|---|---|---|
| `site` + `base: '/seele'` | GH Pages subpath | Dev preview and built paths line up with the `/seele/` subdirectory served from GH Pages (the config comment notes the `gh-pages` branch source) |
| `trailingSlash: 'never'` | no `/` suffix | `Header.astro` link logic strips trailing slashes to honor this |
| `output: 'static'` | pure static | No SSR; everything is prerendered, all dynamic behavior is client JS |
| `inlineStylesheets: 'always'` + `cssCodeSplit: false` | single inlined CSS | Brutalist perf budget: zero render-blocking CSS requests, no webfont downloads |

Because `base` is `/seele`, components consume `import.meta.env.BASE_URL` for internal links rather than hard-coding paths (e.g. `Header.astro:4`, `Layout.astro:14`).

### 17.3 Deployment — `deploy-web.yml`

`.github/workflows/deploy-web.yml` triggers on `push` to `main` filtered to `web/**` or the workflow file itself, plus `workflow_dispatch`. Permissions `contents: read`, `pages: write`, `id-token: write`; a `concurrency` group `pages` with `cancel-in-progress: false`. Two jobs:

1. **build** (`ubuntu-latest`): `checkout@v4` → `setup-node@v4` (node 20, npm cache keyed on `web/package-lock.json`) → `actions/configure-pages@v5` → `npm ci` (cwd `web`) → `npm run build` (cwd `web`) → `upload-pages-artifact@v3` with `path: web/dist`.
2. **deploy** (`ubuntu-latest`): `needs: build`, environment `github-pages` (url from `steps.deployment.outputs.page_url`), uses `actions/deploy-pages@v4`.

The `configure-pages` step carries a comment that it flips the Pages site to `build_type: workflow` automatically, otherwise `deploy-pages` fails against a site initialised with a branch source (the manual `gh-pages` fallback).

### 17.4 The design system — `DESIGN.md` + `tokens.css`

`DESIGN.md` is YAML-front-matter + prose, version `0.3.0`, scope `web/`, direction `brutalist-dev-craft`, sprint `LUMEN-02`. It is the authoritative visual contract; `tokens.css` is its machine-readable projection. Key axioms:

- **Three colors, period** (`DESIGN.md:164-171`; raw values in front-matter `DESIGN.md:17-19`): `--bg: oklch(0.08 0 0)`, `--fg: oklch(0.94 0 0)`, `--accent: oklch(0.65 0.18 50)` (electric orange). A fourth color is "prohibido". Derived greys `--fg-muted` (oklch 0.65) and `--fg-faint` (oklch 0.42) are explicitly framed as opacity/mix variations of `fg`, not new colors. `--danger`/`--success` are feedback-only (success is documented as "solo para SeeleStatus detected"); the five network brand hexes (`--network-btc` `#f7931a` etc.) are reserved literally for the donate widget.
- **Single family**: `--font-mono` is a `ui-monospace`→JetBrains Mono→Cascadia Code→Menlo→Monaco→Consolas→Liberation Mono→Courier New→`monospace` fallback chain. No webfont download. Inter/Roboto/Helvetica/Arial and "any non-monospace family" are explicitly banned (`DESIGN.md:39-48`). Hierarchy emerges from weight contrast (800 hero vs 400 body) + an abrupt size jump.
- **Borders as the hierarchy primitive**: `--border-hair: 1px`, `--border-base: 2px`, `--border-bold: 3px`. All `box-shadow` is `none`, all `border-radius` is `0` (sole exception `--rounded-pill: 999px` for status pills), and gradients/filters/`backdrop-filter` are banned (`DESIGN.md:218-234`).
- **Incomplete borders as visual syntax** (`DESIGN.md:187-196`): a 3-sided border (top+right+bottom, no left) is a deliberate "connector open to the left margin", not a bug. `tokens.css` ships utility classes `.border-3-sides`, `.border-2-sides-tl`, `.border-left-only`, `.border-top-only` (lines 320–350) encoding this. Note: the Hero data block and the Install post-install `<details>` re-implement the 3-sided pattern with *inline* CSS rather than these utility classes (`Hero.astro:107-117`, `Install.astro:123-131`).
- **Motion: zero**: `tokens.css` sets `transition: none` on `*,*::before,*::after` (line 165). The only animation in the whole site is a `@keyframes pulse` opacity loop on status dots (SeeleStatus pill, DonateButtons detect pill, Observability dot). Hover states are instant color/background swaps. `prefers-reduced-motion: reduce` is honored by clamping every animation/transition duration to `0.01ms !important` (lines 272–280), disabling even the pulse.
- **Asymmetric layout**: `.page-container` (lines 291–307) has `max-width: 1100px`, `margin-inline-start: clamp(var(--space-3), 4vw, var(--space-14))` and a far larger `margin-inline-end: clamp(var(--space-3), 12vw, var(--space-50))` — "left commit, right breathe". Above 1400px it flips to a centered `max-width: 1300px` with symmetric `margin-inline: auto` (asymmetry past that point is "wasted space"). `.page-container-centered` is the symmetric variant used by the footer and the observability page.

`tokens.css` carries a header version of v0.3.0 but documents a **scale rebalance v0.4.1** (lines 34–44): small sizes bumped +1px for readability, large end cut ~25%. The actual shipped sizes are `--text-meta: 13px`, `--text-xs: 15px`, `--text-sm: 16px`, `--text-base: 18px`, `--text-lg: 22px`, `--text-2xl: 28px`, `--text-hero: 64px` — the comment annotates the cut as `hero 84→64, lg 24→22, 2xl 32→28`. (These differ from the `DESIGN.md` front-matter sizes, which still list the older scale — meta 11px, base 15px, hero 72px — so `tokens.css` is ahead of the prose spec here.) `tokens.css` also defines weights (300/400/500/700/800), leadings, trackings (`--tracking-caps: 0.12em` for ALL-CAPS labels), a 12-step spacing scale (4..200px), z-index tokens, and a full semantic token layer (`--semantic-*`) that components consume instead of raw primitives. Light mode is supported two ways: a `@media (prefers-color-scheme: light)` block scoped to `:root:not([data-theme="dark"])`, plus manual `:root[data-theme="light"|"dark"]` overrides driven by the header toggle.

`docs/design/` holds the LUMEN plan trail under `docs/design/plans/executed/`: personas (`lens/01/personas/marisol-platform-dev.md`), JTBD (`lens/01/jtbd/eval-and-donate.md`), scaffold wireframes/sitemap/flows (`scaffold/01/`), four aesthetic variations (`material/02/variations/01-brushed-metal-editorial` … `04-editorial-warm`; variation 2 "brutalist-dev-craft" was chosen), five "wow" ADRs (`material/02/adr/wow-01-monospace-only` … `wow-05-motion-zero`), and a visual-critique doc. Plus a11y/perf/heuristic evidence reports under `docs/design/evidence/{01,02}/`, three LUMEN devlogs under `docs/design/devlogs/`, and per-component specs under `docs/design/components/` (Header, Hero, Features, Install, Support, Footer, DonateButtons).

### 17.5 Internationalization mechanism

The site is bilingual EN/ES with **no routing or build duplication**. Every translatable element renders *both* languages as sibling inline spans, e.g. `<span lang="en">features</span><span lang="es">características</span>`. CSS hides the inactive one based on the `<html lang>` attribute (`tokens.css:268-269`): `html[lang="en"] [lang="es"]{display:none}` and the mirror `html[lang="es"] [lang="en"]{display:none}`. `<span lang="la">` (the Latin footer motto) is intentionally unmatched and always visible. The active language (and theme) are resolved before paint by an `is:inline` script in `Header.astro` (lines 79–106): localStorage `seele-theme`/`seele-lang` → `prefers-color-scheme`/`navigator.language` → defaults `dark`/`en`. A second module script (lines 108–160) wires the toggle buttons, persists choices to localStorage, updates icon glyphs and `data-placeholder-en/es` inputs. The lang-toggle icon shows the language a click switches *to* (`updateLangIcons`, `Header.astro:143-149`).

### 17.6 File-by-file map

**Pages & layout.** `src/layouts/Layout.astro` is the HTML shell: `<head>` with charset/viewport, description, `theme-color #141414`, SVG favicon (base-prefixed via `${base}/favicon.svg`), canonical URL (`https://orlando-vazquez-career.github.io/seele/`), `og:type`/`og:title`/`og:description` and a `twitter:card` summary meta (no OG/Twitter image), a default title (`SEELE — memory engine for serious Rust devs`) and description (both overridable via `Props { title?, description? }`), and a single `<slot/>`; it imports `tokens.css` once. `src/pages/index.astro` composes the landing: `Header → main(Hero, Features, Install, Support) → Footer`. `src/pages/observability.astro` is the second route (`/observability`): it sets a custom title/description, renders a `// LOCAL-ONLY` banner explaining the localhost contract (setup copy: `cargo install --path crates/seele-cli`, then `seele serve --cors-allow {Astro.url.origin}`), a bilingual page header, then `<Observability/>` and `<ChatPanel/>`. Its scoped `<style>` defines the page-only banner/header/lead classes.

**Header / brand / footer.** `Header.astro` is sticky, bordered-bottom, contains the DevZen brand (Triangle + "DevZen"/"SEELE" stack), a context-aware nav (different links on the landing vs observability page, computed from `import.meta.env.BASE_URL` and `Astro.url.pathname` with trailing slashes stripped to honor `trailingSlash:'never'`), a GitHub link, and the lang/theme toggle buttons. It owns the anti-FOUC inline script and the toggle module script. `Triangle.astro` renders the DevZen motif as inline SVG; `Props { size=24, variant: 'framed'|'outline', class='', decorative=true }`. The `framed` variant draws a 22×22 `rect` (x/y=1, viewBox `0 0 24 24`) + a `polygon` triangle, both `fill="none" stroke="currentColor" stroke-width="2"`; `decorative` toggles `aria-hidden`/`role`. Its doc-comment says it is used at 24px in the brand mark, but the header actually instantiates it at `size={28}` (`Header.astro:30`). `Footer.astro` is a 5-column grid (brand / links / inspired-by / connect / privacy) collapsing `5 → 3 (≤1100px) → 2 (≤720px) → 1 (≤540px)` columns by breakpoint, with a bottom row carrying `© {year} DevZen SpA · all paths lead to localhost` and the Latin motto `Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.` (with trailing period). The "inspired by" column credits `Gentleman-Programming/engram` with a separate `MIT · clean-room` line.

**Hero.** `Hero.astro` renders a meta line (`// SEELE` + `[v0.2.0]` as two spans), the H1 (bilingual, `font-size: clamp(28px, 5.5vw, var(--text-hero))`), a subline naming the stack, a 3-sided-border **data block** (`latency < 5ms p99`, `backend Rust 1.85+`, `tests 322 green`, `status ALPHA` in accent, `license MIT`), two CTAs (`INSTALL NOW` primary linking `#install`, `GITHUB ↗` secondary), and an embedded `<SeeleStatus/>`. Note these stats are hard-coded marketing copy, not live values.

**Features.** `Features.astro` defines a local `features` array (4 entries: CLI/MCP/HTTP/TUI) with bilingual blurbs, a `meta` command snippet, and a `href`, and maps each to `<FeatureCard/>` in an asymmetric grid (`grid-template-columns: 1.5fr 1.25fr 1.25fr 1fr` — "CLI dominates", collapsing to `1fr 1fr` ≤960px and `1fr` ≤540px). The copy states "17 subcommands", "19 tools under `seele_*`", "18 paths + OpenAPI 3.1", and "5 panes" for the TUI. `FeatureCard.astro` is a typed link card: `Props { num, title, blurbEn, blurbEs, meta, href }`; full-height flex column with number/title/blurb/`<pre>` meta/`view →` link; hover inverts to fg-background. `http`-prefixed `href`s open in a new tab.

**Install.** `Install.astro` defines `installPaths` (3 entries: install script, `cargo install --git`, build from source), the first marked `recommended: true`, and renders `<InstallCard {...p}/>` plus a `<details>` "after install" example (save + search + setup). `InstallCard.astro` (`Props` with a single `notes?` string *and* `notesEn?`/`notesEs?` bilingual variants, plus `recommended?`) shows num/badge/title/description, a `<pre>` command prefixed `$ ` with an absolutely-positioned **COPY** button. The copy script uses `navigator.clipboard.writeText`, flips the label to `COPIED` (adding `.is-copied`) for 1500ms, and fails silently in a `catch`. The recommended card gets an accent border.

**Support / donate.** `Support.astro` is the human-funding section: attribution to Orlando Nahuel Vazquez Gonzalez under DevZen SpA, an ordered list of three "ways" (star/share, open issues, hire/consult), then a crypto-donations block (labelled `// 04 — crypto donations`) hosting `<DonateButtons/>` and the tagline "EIP-1193 for EVM. BIP-21 for Bitcoin. Solana Pay for Phantom. No WalletConnect, no tracking."

### 17.7 SeeleStatus — the localhost probe

`SeeleStatus.astro` (`Props { endpoint = 'http://localhost:7777/health' }`) is a small pill that tells the visitor whether SEELE is running on their machine. Constants: `TIMEOUT_MS = 1500`, `CACHE_TTL_MS = 30_000`, `STORAGE_KEY = 'seele:status-cache:v1'`. The state machine is `type Status = 'checking' | 'detected' | 'not-detected' | 'blocked'`. On load it checks a `sessionStorage` cache (30s TTL); on miss it runs `detect()`:

```ts
// web/src/components/SeeleStatus.astro:61-75
async function detect(endpoint: string): Promise<Status> {
  if (location.protocol === 'https:' && endpoint.startsWith('http:')) {
    return 'blocked';
  }
  try {
    await fetch(endpoint, { mode: 'no-cors', cache: 'no-store',
      signal: AbortSignal.timeout(TIMEOUT_MS) });
    return 'detected';
  } catch { return 'not-detected'; }
}
```

Two gotchas worth flagging for downstream improvement: (1) on the hosted HTTPS site, an `http://localhost` probe is **mixed content** and returns `'blocked'` immediately without ever attempting the fetch — so the landing's status pill shows "auto-detect needs local hosting" in production. (2) The probe is `mode: 'no-cors'`, an *opaque* fetch — it resolves as long as the server responds at all, so `'detected'` means "something answered on :7777", not "SEELE specifically answered". Messages are localized at apply time by reading `<html lang>`. The CTA (`install →`) is hidden only when status is `detected` (`cta.hidden = status === 'detected'`). The dot pulses while `checking`; `detected` turns the border/dot `--success` green.

### 17.8 Observability — the live panel

`Observability.astro` (`Props { endpoint = 'http://localhost:7777' }`) is the largest component. It reads directly from a local `seele serve` over CORS and renders, in the `online` state, six visible sections (Stats, By type, By project, Search, Recent, Sparkline) plus a seventh **detail panel** that stays `hidden` until an item is clicked; if anything fails it degrades gracefully. Its TS state machine is `type Status = 'probing' | 'offline' | 'cors-blocked' | 'online'`, surfaced via `data-state` + `[data-show-when]` blocks. Constants: `TIMEOUT_MS = 1500`, `POLL_INTERVAL_MS = 15_000`, `RECENT_LIMIT = 10`, `SPARK_DAYS = 14`, `SPARK_BLOCKS = '▁▂▃▄▅▆▇█'` (8 levels).

The **probe** is the clever part: it first does a readable cross-origin `GET /health`. If that succeeds and is 2xx → `online`. If it throws, it can't tell "server down" from "CORS rejected", so it retries with `mode: 'no-cors'`; success there means the server is up but blocking CORS → `cors-blocked`, failure → `offline` (`Observability.astro:317-344`). Only `online` proceeds to `refreshAll` + a 15s polling interval + `bindSearch`.

The `offline` and `cors-blocked` states render **identical hard-coded sample data** (total 142, by-type/by-project bars, three fake recent items, a 14-char sparkline `▁▂▁▃▂▄▃▅▄▆▅▇▆█`) behind a preview banner at `opacity: 0.72`, so the panel "feels alive" even with no server. The `cors-blocked` banner additionally prints the fix as `$ seele serve --cors-allow {endpoint}` (with a trailing slash stripped from `endpoint`).

When `online`, JS hydrates the live sections from three endpoints:

| Section | Endpoint | Shape consumed |
|---|---|---|
| Stats (total = `active`, projects) + By-type bars | `GET /stats` | `StatsResponse.observations { active, projects, by_type[] }` |
| Recent list, last-save age, By-project bars, sparkline | `GET /memories?limit=10` | `MemoryRecord[]` (`id, title, type, project, scope, topic_key, content, created_at, updated_at, …`) |
| Search | `POST /search` `{query, limit:8}` | `{ results: [{ memory, score? }] }` |

By-project counts and the saves/day sparkline are derived **client-side** from the `/memories` array (`renderProjectBars`, `renderSparkline`), not separate endpoints. `fetchJson<T>` swallows all errors and non-2xx into `null`, and `refreshAll` uses `Promise.allSettled` over `refreshStats` + `refreshRecent`, so a single failing endpoint never blanks the whole panel. Bar charts render the top 6 items, each scaled to the max (floored at 4% width). Search is debounced 220ms, min query length 2, and aborts the previous in-flight request via `AbortController`. Clicking or Enter/Space on a recent or result item opens the inline **detail panel** showing id/type/project/topic/created (ISO) + full content. All injected strings pass through `escapeHtml()` (defence against XSS from memory content); titles are truncated to 60 chars via `truncate()`. The state shapes (`MemoryRecord`, `StatsResponse`) mirror the seele-http response types, which is the contract this component depends on.

### 17.9 ChatPanel — chat-with-your-DB

`ChatPanel.astro` (`Props { endpoint = 'http://localhost:7777' }`) is a chat widget for the observability page. Per its header doc-comment, it POSTs to SEELE's local `/chat`, which runs a **server-side tool-use loop**: the model decides when to call `seele_search`, the server executes it and feeds results back, and only the final assistant text returns. The user's AI API key lives in this browser's `localStorage` (`SETTINGS_KEY = 'seele-chat-settings'`) and is forwarded only to the local SEELE server — "never reaches a third party from this page".

States: `'probing' | 'needs-config' | 'enabled'`. On init it `GET /chat/info` (3s timeout) and reads localStorage. It becomes `enabled` if **either** the server reports chat config (`ChatInfo.enabled`) **or** the browser has saved settings — browser settings win at request time (`ChatPanel.astro:221-232`). Otherwise `needs-config` shows an `[ OPEN SETTINGS ]` CTA. The settings `<dialog>` (provider select, model text, password `api_key`, optional endpoint override) persists to localStorage; submitting with an empty `api_key` deliberately **clears** stored credentials and reloads. The provider list (`minimax, openai, anthropic, openrouter, together, groq, deepseek`) and `DEFAULT_MODELS` map exactly mirror the seele-http chat backend (verified against `crates/seele-http/src/handlers.rs`: `default_model_for` and `default_endpoint_for` route the same provider names to their `/chat/completions` base URLs, e.g. `minimax → MiniMax-M2`, `anthropic → claude-haiku-4-5-20251001`, `groq → llama-3.3-70b-versatile`).

Sending: the client keeps a `Message[]` history, POSTs `{ messages, provider?, api_key?, model?, endpoint? }` with a **60s** `AbortSignal.timeout`, and the server returns the *full* history. The client diffs with `data.messages.slice(history.length + 1)` — the `+1` skips the system prompt the server prepends (`ChatPanel.astro:389`) — and renders only the new turns. Rendering hardening worth noting: `stripThinking()` removes `<think>`, `<thinking>`, and `<|thinking|>` reasoning traces (its comment cites Minimax M2 / DeepSeek-R1 / Qwen QwQ, older Claude, and Together/vLLM variants); `renderAssistantBody()` escapes first, then applies a tiny markdown subset (`**bold**`, `` `code` ``) on the escaped string ("no markdown can ever inject HTML"); tool turns are collapsed to "N results" (from the parsed `count` field); system turns are never displayed; assistant turns that strip to empty with no tool calls are dropped entirely. Enter submits, Shift+Enter newlines, IME composition is respected (`e.isComposing`). The log scrolls internally only (`log.scrollTop = log.scrollHeight`) — never the page.

### 17.10 DonateButtons — real wallet integration

`DonateButtons.astro` is the most logic-heavy client component. Its `targets` array holds five chains (BTC, ETH, Base, Syscoin, Solana) with addresses, `method` (`bip21|evm|solana-pay`), `uri`, `install` URL, `walletName`, and `chainId`. The three methods:

- **EVM** (ETH chainId 1 / Base 8453 / Syscoin 57): discovers providers via **EIP-6963** (dispatches `eip6963:requestProvider`, listens for `eip6963:announceProvider`), preferring rdns in order `io.metamask`, `io.metamask.flask`, `io.pali`, `io.paliwallet`, `app.phantom`, else the first announced provider, else legacy `window.ethereum`. On click it `eth_requestAccounts`, ensures the chain via `wallet_switchEthereumChain` with a `4902 → wallet_addEthereumChain` fallback (Base/Syscoin carry full `add` configs incl. RPC + explorer; chain 1 has no `add`), then `eth_sendTransaction` with `value: '0x0'`. Error codes are mapped: `4001 → REJECTED`, `-32002 → CHECK WALLET`, else `ERROR`.
- **Solana** (`solana-pay`): if a Phantom/`window.solana` provider exists, navigates to the `solana:` URI; after 700ms, if the document is still visible (no app intercepted), it copies the address and shows `COPIED`.
- **Bitcoin** (`bip21`): prefers a browser extension — UniSat (`sendBitcoin(addr, 1000)` sats), or Xverse / Leather via `sats-connect`-style `request('sendTransfer', …)` (Xverse: `recipients:[{address, amount:1000}]`; Leather: `{address, amount:'0.00001'}`). Otherwise navigates to the `bitcoin:` URI; after 700ms-still-visible it opens the install page and copies as last resort.

A `data-detect` pill (`scanning → detected/none`) scans wallet globals 200ms after load and re-runs on every `eip6963:announceProvider`, and tags each button's wallet label `data-available='yes'` when its method is satisfiable. Every method shares a fallback: no wallet → open the official extension install page + copy the address. A **bug to flag**: the status CSS references `--warning` (`DonateButtons.astro:546` and `Observability.astro:672`) but no `--warning` token is defined in `tokens.css` — the `pending`/`cors-blocked` dot color silently falls back. The Observability rule uses `var(--warning, var(--accent))` so it degrades to accent; the donate `pending` rule (`color: var(--warning)`) has no fallback and inherits instead. This is a real, low-severity defect in the improvement surface.

### 17.11 Visual-critique tooling

`web/scripts/visual-critique.mjs` is a Playwright (`chromium`, headless) screenshot harness for the LUMEN "Visual Critique Multi-Resolution" phase. It captures the full matrix: 2 pages (`home` = `''`, `observability` = `/observability`) × 5 viewports (320/768/1024/1440/1920 px) × 2 themes (dark/light) × 2 langs (en/es) = **40 shots**, written as `{page}-{lang}-{theme}-{viewport-label}.png` under `test-results/visual/` (the viewport label is e.g. `320-mobile-narrow`). Per shot it opens a fresh context with `reducedMotion: 'reduce'` and the matching `colorScheme`, injects `localStorage` `seele-theme`/`seele-lang` via `addInitScript` (so the anti-FOUC script picks them up), navigates with `waitUntil:'networkidle'` (15s cap), waits 800ms, then waits for the Observability `[data-obs]` to leave `data-state="probing"` (3s cap, captures anyway on timeout), waits another 200ms, then takes a `fullPage` PNG. `--url` overrides the base (default `http://localhost:4321/seele`, trailing slashes stripped). Exit code 1 if any shot fails. It produces artifacts only — it does no automated assertion or diffing; the "critique" is done by a human/agent reviewing the PNGs.

### 17.12 Cross-system boundary summary

The web subsystem touches the rest of SEELE only at runtime, only over HTTP, and only against a user-local server:

| Component | Calls | seele-http endpoint(s) | Notes |
|---|---|---|---|
| SeeleStatus | `fetch` no-cors | `GET /health` | opaque probe; returns `blocked` under HTTPS mixed-content |
| Observability | `fetch` (readable then no-cors) | `GET /health`, `GET /stats`, `GET /memories?limit=10`, `POST /search` | needs `seele serve --cors-allow <origin>` |
| ChatPanel | `fetch` | `GET /chat/info`, `POST /chat` | forwards user-held API key to local server only |
| DonateButtons | none against SEELE | — | talks to browser wallet extensions via EIP-6963/EIP-1193/sats-connect, not SEELE |

The CORS contract surfaced by these components corresponds to the server side: `seele serve --cors-allow <origin>` populates `cors_origins`, and `seele-http/src/server.rs:105-117` builds the CORS layer in two branches — when `cors_origins` is **empty** it is a bare `CorsLayer::new()` (no cross-origin exposure), and when **any** origin is requested it builds a permissive `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)` (with an in-code note that a per-origin allowlist is "Block D" future refinement, and that `Any` methods/headers are required so a JSON `POST` clears preflight). The privacy posture advertised in the footer ("no tracking / no analytics / no cookies / no SDKs") is backed by the actual implementation: no analytics scripts, no webfonts, all state in localStorage/sessionStorage, and every network call pointed at the user's own machine.
