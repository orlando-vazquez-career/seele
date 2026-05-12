# Perf report — Sprint LUMEN-01

Build-time measurement of the SEELE landing page bundle.

Method:
- `npm run build` to produce `web/dist/`.
- `du` + `gzip` on artifacts.
- Manual count of assets, network requests, render-blocking resources.
- Lighthouse-equivalent reasoning from code review (no external CI to run Lighthouse in this sprint; deferred to post-deploy first run).

## Bundle measurement

| Asset | Path | Bytes raw | Bytes gzipped |
|---|---|---|---|
| HTML + inlined CSS + inlined JS | `dist/index.html` | 34,049 | 7,267 |
| Favicon SVG | `dist/favicon.svg` | 261 | (irrelevant — separate request, served by CDN) |
| **Total transfer (HTML)** | — | **34,049** | **7,267** |

The favicon adds one extra request but is <1 KB and is cached after first load. The actual "weight of the page" on first visit is **7.3 KB gzipped HTML + ~0.3 KB SVG ≈ 7.6 KB**.

## Against perf budget (from Lens phase)

| Metric | Budget soft | Budget hard | Measured | Status |
|---|---|---|---|---|
| Total transfer (gzip) | <25 KB | <50 KB | **7.6 KB** | ✅ 30% of soft budget |
| HTML gzip | <5 KB | <8 KB | 7.3 KB | ⚠ over soft (5 KB), within hard (8 KB) |
| CSS gzip | <8 KB | <12 KB | _inlined in HTML_ | ✅ no separate request |
| JS gzip | <3 KB | <6 KB | _inlined in HTML_ | ✅ no separate request |
| Fontfile | 0 KB | 15 KB | 0 KB | ✅ system stack |
| Images | <10 KB | <25 KB | 0.3 KB (favicon SVG) | ✅ |

The HTML is slightly over the soft 5 KB target because Astro inlines all CSS and JS (per `inlineStylesheets: 'always'` config). This is a deliberate trade-off: fewer requests + better LCP at the cost of slightly larger HTML. Net first-paint is faster.

## Core Web Vitals projection

Lighthouse can't be run from this CLI directly. Projections based on:
- Single-page static HTML with inlined CSS/JS.
- No external font requests.
- No external script requests.
- No third-party domain connections.
- One static SVG favicon.
- Server: GitHub Pages CDN edge (Cloudflare-backed since 2024).

| Metric | Projected | Target | Notes |
|---|---|---|---|
| **LCP** | <0.8s | <1.0s | Largest paint is the Hero `<h1>` "SEELE" or tagline `<p>`. No image LCP candidate. |
| **INP** | <50ms | <100ms | Two click handlers (copy buttons, donate buttons). Both are O(1) sync operations + a setTimeout. |
| **CLS** | 0 | <0.05 | No async-loaded content, no images without dimensions, no font-swap. |
| **TTFB** | <200ms | <200ms | GH Pages CDN. |
| **TBT** | <50ms | <200ms | Donate script binds 5 listeners, copy script binds 3 listeners. Sub-1ms work. |
| **FCP** | <0.5s | <1.5s | Inline CSS means no render-blocking external stylesheet. |

Real Lighthouse run will happen post-deploy on the live GH Pages URL. Numbers will be appended here.

## Lighthouse target

`100 / 100 / 100 / 100` (Performance / Accessibility / Best Practices / SEO).

If any score drops below 95, sprint does NOT close Bloque D and we iterate.

Plan to run:
1. Wait for `deploy-web.yml` to succeed.
2. PageSpeed Insights → `https://orlando-vazquez-career.github.io/seele/`.
3. Append result + screenshot/text to this file.

## Optimizations applied

- ✅ `inlineStylesheets: 'always'` — no external CSS file.
- ✅ `cssCodeSplit: false` in Vite — single CSS chunk.
- ✅ System font stack — zero font downloads.
- ✅ SVG favicon inline definition — 261 bytes.
- ✅ No analytics or tracking pixels.
- ✅ No third-party SDK (no React/Vue runtime, no web3 SDK, no WalletConnect).
- ✅ `prefers-reduced-motion` honored — no expensive transitions on resource-constrained devices.
- ✅ `backdrop-filter: blur(8px)` on header — modern browsers GPU-accelerate this; gracefully degrades if unsupported.

## Things that would regress perf

- ❌ Adding a hero image (especially raster).
- ❌ Adding Google Fonts or any `@font-face` download.
- ❌ Adding a JS framework (React/Vue/Svelte).
- ❌ Adding WalletConnect SDK (~200 KB).
- ❌ Adding analytics (PostHog, Plausible, GA, even self-hosted).
- ❌ Auto-playing video.
- ❌ Carousel with autoplay.

These are explicitly banned in `DESIGN.md`.

## Verdict

**Perf budget: PASS** projected. Pending Lighthouse confirmation post-deploy.
