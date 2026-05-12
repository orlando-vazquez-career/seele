# Variation 3 — Studio Suizo cold-precision

## Paleta (neutros + 1 accent)
- `oklch(0.99 0.002 0)` — superficie primaria (near-white, nunca pure white)
- `oklch(0.92 0.003 0)` — superficie secundaria (cards, paneles de contraste mínimo)
- `oklch(0.55 0.008 0)` — texto secundario, labels, metadatos
- `oklch(0.12 0.004 0)` — texto primario (no pure black — disciplina suiza evita extremos absolutos)
- `oklch(0.55 0.22 25)` — accent único: rojo Helvetica, solo para CTAs primarios y estado activo

Regla absoluta: el accent aparece en ≤2 elementos por viewport. Ningún color secundario. Ningún color de "estado" salvo derivado del accent con cambio de lightness.

## Typography (sans geometric only)

Family única: **Helvetica Now Text** (fallback: Inter con ajuste de tracking manual).

Escala — ratio 1.333 (perfect fourth):

- `10.5px` — label / caption / metadata
- `14px` — body base
- `18.7px` — subheading / feature title
- `24.9px` — section heading
- `33.2px` — hero subline
- `44.3px` — hero heading

Line-height: 1.4 para body, 1.1 para headings. Letter-spacing: −0.01em en headings, 0.04em en labels uppercase. Ningún weight decorativo — solo Regular (400) y Medium (500). Bold reservado exclusivamente para el accent CTA.

## Grid system (12-col strict)

12 columnas. Gutter fijo: `24px`. Margin exterior: `80px` desktop, `40px` tablet, `20px` mobile.

Breakpoints: `1280px` (full 12-col) / `768px` (6-col con colapso) / `375px` (1-col lineal).

Sin asimetrías decorativas. Toda columna-span es múltiplo de 3 o 4. El contenido textual vive en cols 1–7, nunca centrado. Desequilibrio deliberado: la vacuidad de cols 8–12 es contenido.

## Composition (white-space dominante)

Hero: texto alineado a la izquierda estricta. Heading en cols 1–6, 44px. Subline directamente debajo, 33px, cols 1–5. Párrafo de apoyo en cols 1–4, 14px, máximo 3 líneas.

CTA en cols 1–2: botón izquierdo, fondo accent, texto blanco, sin border-radius (0px — la curva es ornamentación). Al lado del botón, una línea de texto mínima tipo "MIT license · Rust · open source".

Cols 7–12: vacío absoluto. No hay imagen, no hay ilustración, no hay gráfico. El espacio negativo activa la jerarquía. Marisol lee el texto porque no hay nada más.

## Motion vocabulary (lineal predecible)

`cubic-bezier(0.25, 0.1, 0.25, 1.0)` — un ease-in-out único para todo el sistema.

Duración: 150ms máximo para feedback de UI (hover, active). 250ms para transiciones de sección. Ninguna animación supera 300ms.

Springs prohibidos: el spring implica objeto físico con inercia. La Escuela Suiza comunica información, no simula materia. El movimiento debe ser predecible antes de ejecutarse.

## El detalle distintivo

Las otras tres variaciones *agregan* para diferenciarse: metal brushed agrega textura, Brutalist agrega peso tipográfico extremo, Editorial agrega calidez de color. Suiza se distingue por **sustracción sistemática**. El detalle distintivo es que cols 8–12 del hero permanecen vacías sin colapsar — el espacio negativo es decisión de diseño, no ausencia de contenido. Ninguna otra variación haría eso.

## Cuándo elegir esta dirección

Cuando la plataforma necesita credibilidad técnica inmediata ante developers que detectan rápido el "design theater". La austeridad suiza señala que el producto confía en su propio contenido.
