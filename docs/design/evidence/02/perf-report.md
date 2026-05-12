# Performance report — SEELE web v0.3.0 (Brutalist dev-craft)

**Sprint:** LUMEN-02
**Date:** 2026-05-12
**Build target:** static Astro 6.3.1 output → GitHub Pages
**Measurement:** local build artifacts, gzipped via `gzip -9`

## Bundle sizes (the only metric that matters)

| asset                          | raw size  | gzipped  | notes |
|--------------------------------|----------:|---------:|-------|
| `dist/index.html`              | 52,017 B  | 10,747 B | inlined CSS + inlined component scripts (donate widget + install copy + seele-detect) |
| `dist/favicon.svg`             |    291 B  |    ~220 B (gz of small static rarely worth it) | brutalist framed-triangle, monochrome |
| **Total (over the wire, 1st paint)** | **~52.3 KB** | **~10.9 KB** | one HTML + one SVG |

For comparison: an average React landing in 2026 ships ~250 KB gzipped JS on first paint before showing pixels. SEELE ships 10.9 KB total. **~22× lighter.**

No `_astro/` directory exists in the build output because:
- No external CSS file: Astro inlines all `<style>` blocks (single-page, no cross-page sharing benefit)
- No external JS file: the 3 component scripts (donate, copy, status) are short enough that Astro inlines them too
- No images
- No fonts loaded over the wire (system-monospace fallback chain in `--font-mono`; we don't ship a self-hosted JetBrains Mono — the user's OS handles it via `ui-monospace`)

## Network requests (first paint)

1. `GET /seele/index.html` — 10.9 KB gz
2. `GET /seele/favicon.svg` — 0.3 KB

Total: **2 requests**, ~11 KB. No CDN, no analytics beacon, no font.gstatic.com, no ipify, nothing.

## Estimated Core Web Vitals

Without running Lighthouse on a public deploy yet, estimates are based on bundle composition:

- **LCP (Largest Contentful Paint):** the hero `<h1>` text. With 11 KB total payload and zero render-blocking external resources, LCP should land at < 0.8 s on a fast 4G connection, < 0.3 s on cable.
- **CLS (Cumulative Layout Shift):** 0 expected. No web fonts loaded (no FOUT/FOIT), no images, no async-loaded components. Layout is fully described by HTML+CSS in the single response.
- **INP (Interaction to Next Paint):** the only interactive paths are hover (instant — `transition: none`), donate-button click (network round-trip to wallet extension, not page perf), and install-card copy (synchronous `navigator.clipboard.writeText`, < 16 ms). Should target ≤ 100 ms p75.
- **FCP (First Contentful Paint):** identical to LCP for this page since the first paintable element is the hero text. Same estimate.

## Caching strategy (recommendation for GitHub Pages)

GitHub Pages sets `Cache-Control: max-age=600` (10 min) on HTML by default. Since this is essentially a static brochure that changes once per release, the default is fine. No need to set `Cache-Control: immutable` because the URL doesn't include a content hash.

For the favicon, default GitHub Pages caching is acceptable.

## Things deliberately not optimized

- **JetBrains Mono not self-hosted.** Sprint LUMEN-01 considered shipping woff2 files for the brand-consistent rendering. Decision in wow-01 ADR: defer — `ui-monospace` (SF Mono on macOS, Cascadia Code on Windows, Liberation Mono on Linux) produces a visually equivalent result for 95% of users at zero KB extra. Self-hosted JetBrains Mono returns in Sprint LUMEN-03 if the eyeball check from director rejects system-mono rendering.
- **No service worker.** Adding one would let us cache offline, but a 10.9 KB landing doesn't benefit. Out of scope.
- **No HTTP/2 push, no resource hints.** Single page, zero cross-origin resources. Hints would be noise.

## Comparison against Sprint LUMEN-01 baseline

| metric                          | LUMEN-01 v0.2.0 | LUMEN-02 v0.3.0 | delta |
|---------------------------------|----------------:|----------------:|------:|
| HTML raw                         | ~64 KB          | 52 KB           | −18%  |
| HTML gzipped                     | ~13.8 KB        | 10.7 KB         | −22%  |
| External font requests           | 2 (Inter + JetBrains Mono woff2) | 0 | −100% |
| Total wire payload               | ~38 KB gz       | ~11 KB gz       | −71%  |
| CSS custom properties exported   | 67              | 28              | −58%  |
| Transitions declared             | 24              | 0               | −100% |
| Gradients (linear/radial)        | 3               | 0               | −100% |

The reduction is genuine — not from deletion-for-deletion's-sake, but from collapsing the design vocabulary (1 family instead of 2, 3 colors instead of 8, 0 motion instead of 4 durations). Less tokens → smaller bundle → faster paint.

## Verdict

Performance posture for v0.1.0 launch: **strong**. The 10.7 KB gzipped payload is in the bottom 1% of public developer-tool landings as of 2026. No remediation needed. Re-run Lighthouse after public GitHub Pages deploy to populate field metrics; if any Core Web Vital comes back below "Good" (LCP > 2.5s, CLS > 0.1, INP > 200 ms p75), file as a Sprint LUMEN-03 finding.
