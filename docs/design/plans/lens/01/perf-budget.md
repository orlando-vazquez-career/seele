# Perf budget — Sprint LUMEN-01

Métricas target a cumplir en el deployment final (gh-pages). Medición con Lighthouse 12 desktop + mobile, conexión 4G simulada, Moto G4 emulado.

## Core Web Vitals 2026

| Métrica | Target | Razón |
|---|---|---|
| **LCP** (Largest Contentful Paint) | <1.0s | Marisol cierra tab a 2.5s. Apuntamos a la mitad. |
| **INP** (Interaction to Next Paint) | <100ms | Donate buttons deben sentirse instantáneos. |
| **CLS** (Cumulative Layout Shift) | <0.05 | Hero estático + sin font-swap visible |
| **TTFB** | <200ms | GH Pages CDN cubre esto naturalmente |

## Bundle budget

| Asset | Target gzip | Hard limit |
|---|---|---|
| HTML | <5 KB | 8 KB |
| CSS (todos los archivos) | <8 KB | 12 KB |
| JS (donate widget island) | <3 KB | 6 KB |
| Fontfile | 0 KB (system fonts) | 15 KB si Inter incluido |
| Imágenes (hero/icons) | <10 KB total (SVG inline preferido) | 25 KB |
| **Total transfer** | **<25 KB** | **50 KB** |

## Restricciones de implementación

- ❌ No Tailwind (CSS escrito a mano).
- ❌ No web fonts pesados (system stack `ui-monospace, monospace` + `ui-sans-serif, system-ui` permite cero fontfile descargado).
- ❌ No fuentes Google (privacy + perf).
- ❌ No analytics (privacy + perf).
- ❌ No service worker / PWA (sobrecarga sin valor).
- ✅ SVG inline para iconos (zero request extra).
- ✅ `image/avif` o `image/webp` si llega a haber raster.
- ✅ `<link rel="preload">` solo para el font crítico si se decide incluir.

## Lighthouse score target

- Performance: 100
- Accessibility: 100
- Best Practices: 100
- SEO: 100

Si algún score baja de 95, sprint no cierra Bloque D.

## Medición plan

1. Lighthouse local en `web/dist` servido con `npx serve`.
2. Lighthouse remoto via PageSpeed Insights sobre URL `https://orlando-vazquez-career.github.io/seele/` post-deploy.
3. WebPageTest fallback si discrepancia.

Resultados se persisten en `docs/design/evidence/01/perf-report.md` durante Bloque D.

## Por qué este budget es realista

Investigación 2026 muestra que Astro sin islands hidratados produce páginas de ~11 KB. Con un único island (donate widget) de ~2.5 KB JS gzip + un CSS hand-written de ~6 KB + HTML de ~4 KB, el total estimado es **~23 KB**, bien dentro del soft target. El hard limit de 50 KB deja margen para añadir 1-2 SVGs o un icono favicon razonable.
