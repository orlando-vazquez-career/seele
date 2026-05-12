# SEELE web

Landing page for SEELE — Astro 5.x static site, deployed to GitHub Pages.

## Develop

```bash
cd web
npm install
npm run dev
```

Open http://localhost:4321/seele/ — the `/seele` base path matches production.

## Build

```bash
npm run build
```

Output lands in `web/dist/`. Deploy is automated via `.github/workflows/deploy-web.yml` on push to `main`.

## Stack

- **Astro 5.x** — static output, zero JS by default, islands for the donate widget.
- **Vanilla CSS** with tokens from `src/styles/tokens.css`. No Tailwind. No webfonts.
- **Vanilla JS** for the donate island. No SDK. No tracking.

## Files

- `src/pages/index.astro` — landing single page.
- `src/layouts/Layout.astro` — html shell.
- `src/components/` — Hero, Features, Install, Support, DonateButtons, Footer.
- `src/styles/tokens.css` — design tokens, semantic + primitive layers.

## Design system

Visual contract lives at the repo root in [`/DESIGN.md`](../DESIGN.md). Bump that file's semver when changing primitives or semantics.

## Perf budget

LCP <1.0s · INP <100ms · CLS <0.05 · Total transfer <50 KB. Lighthouse target 100/100/100/100.
