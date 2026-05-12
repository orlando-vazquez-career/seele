# Heuristic eval — Sprint LUMEN-01

Nielsen 10 + Don Norman 6 applied to the SEELE landing after Bloque C. Verde = passes, amarillo = aceptable con caveat, rojo = pain real.

## Nielsen 10

| # | Heurística | Estado | Notas |
|---|---|---|---|
| 1 | Visibilidad del estado del sistema | 🟢 | Status dot + "v0.1.0 released · 322 tests green · MIT license" en hero. Donate button states (opening/copied) son feedback inmediato. |
| 2 | Match con mundo real | 🟢 | Lenguaje técnico apropiado para audiencia dev. Tagline en español (mercado LATAM), términos en inglés para conceptos técnicos universales (FTS, MCP, vec). |
| 3 | Control y libertad del usuario | 🟢 | Sin pop-ups, sin redirects forzados, sin auto-play. `<details>` collapsible para snippet opcional. URI-scheme click respeta el wallet picker del OS. |
| 4 | Consistencia y estándares | 🟢 | Tokens unificados (semantic layer). Hover states consistentes (border highlight + lift). Buttons follow OS conventions. |
| 5 | Prevención de errores | 🟢 | Address copyable a clipboard como fallback evita errores de transcripción manual. CTAs primary/secondary visualmente diferenciados. |
| 6 | Reconocer vs recordar | 🟢 | Comandos visibles en cards de Install (no requiere recordar sintaxis). Donate buttons usan glyphs Unicode reconocibles + label texto. |
| 7 | Flexibilidad y eficiencia | 🟢 | Power users: anchor links directos `#install`, `#support`. Newcomers: scroll lineal narrativo. |
| 8 | Diseño estético y minimalista | 🟢 | Cero ruido. Single accent purple. Tipografía monospaciada coherente. Densidad apropiada para audiencia técnica. |
| 9 | Reconocer/recuperarse de errores | 🟡 | Donate fallback (clipboard si no hay wallet) es elegante. Sin embargo, si `navigator.clipboard` rechaza, el feedback es muy sutil ("first 8 chars") — recomendación v0.2: mostrar la address completa como tooltip o expandir el botón. |
| 10 | Ayuda y documentación | 🟢 | `notes` en InstallCard explica edge cases (Windows variant, MSRV). Footer links a CHANGELOG y GitHub para detalles. |

## Don Norman 6 (psicológicos)

| # | Principio | Estado | Notas |
|---|---|---|---|
| 1 | Affordance | 🟢 | Buttons se ven clickeables (hover state + cursor). Anchor links subrayados. Code blocks tienen botón "copy" visible. |
| 2 | Signifier | 🟢 | Brand colors en donate buttons señalan "esta es la red X". Glyph + label refuerzan. |
| 3 | Mapping | 🟢 | Anchor links del header mapean directamente a secciones. Orden de navegación = orden de scroll. |
| 4 | Feedback | 🟢 | "opening…" durante el click, "copied" en verde para confirmación. Visualmente instantáneo. |
| 5 | Constraints | 🟢 | Solo 5 redes en donate (no infinite scroll de opciones). 3 install paths (no 7 variantes Linux). Choice architecture limpia. |
| 6 | Conceptual model | 🟢 | El modelo es claro: "esta es una landing → mirá qué hace → instalalo → si te gustó, dejá una propina". |

## Patrones validados

- ✅ Hero con tagline humana arriba + subline técnica abajo (research-validated).
- ✅ Code snippet visible en primer scroll (en Install cards, no hero — variant aceptable).
- ✅ Donate buttons con brand colors oficiales (pattern matching contra UI de wallets).
- ✅ Footer con bandwidth disclosure honesto.
- ✅ Latin signature como cierre poético (matchea con README).

## Antipatrones evitados (audit pre-sprint check)

- ✅ No hay newsletter signup.
- ✅ No hay cookie banner (no usamos cookies).
- ✅ No hay video autoplay.
- ✅ No hay "Get Started" que lleva a signup (lleva a comandos copiables).
- ✅ No hay comparison tables.
- ✅ No hay marketing speak.
- ✅ No hay carousels.

## Issues encontrados

| # | Severidad | Descripción | Plan |
|---|---|---|---|
| 1 | Baja | Si clipboard falla (raro), feedback muestra solo "first 8 chars" — usuario debe inferir | Documentado, mejorar en v0.2 con tooltip expandible |
| 2 | Baja | Sin opt-in para light mode | Decisión consciente (DESIGN.md) — agregar es MINOR semver bump |
| 3 | Info | Internal anchor scroll usa native CSS `scroll-behavior: smooth` — algunos usuarios prefieren instant | Respetado vía `prefers-reduced-motion: reduce` |

## Overall

**Heuristic eval: PASS** — 9/10 verdes en Nielsen, 6/6 verdes en Norman. 1 amarillo en heurística 9 documentado y deferred a v0.2.
