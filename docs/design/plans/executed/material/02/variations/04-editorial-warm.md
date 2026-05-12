# Variation 4 — Editorial monumental warm

## Paleta (paper-tone warm)

- **Surface / paper**: `oklch(0.94 0.012 80)` — cream suave, ligeramente dorado, como papel de libro bien envejecido
- **Ink dark**: `oklch(0.18 0.005 25)` — casi negro, con undertone tierra, nunca fríamente neutral
- **Red editorial**: `oklch(0.50 0.18 25)` — el único color con carga cromática; reservado para folios, drop caps, y numerales de sección
- **Tint mid**: `oklch(0.75 0.008 70)` — gris-beige para hairlines, captions secundarias, marginalia
- **Ghost warm**: `oklch(0.88 0.018 75)` — segundo tono de superficie, columnas alternadas, pull-quote backgrounds

---

## Typography (serif italic dominant + body serif)

- **Display**: Cormorant Garamond Italic (o Freight Display Italic) — 72-120px para hero, tracking `-0.02em`, ligaduras activadas. Protagonista absoluto, nunca decoración.
- **Body**: Spectral Regular + Spectral Italic — 17-18px, `line-height: 1.75`, columnas de `48-58ch`. Lectura densa pero generosa en espacio vertical.
- **Captions / marginalia**: misma familia, 12-13px, `letter-spacing: 0.08em`, uppercase en casos de numeración
- **Code blocks**: JetBrains Mono — 13px, superficie `ghost warm`, sin border-radius, con un único hairline left-border en red editorial

---

## Magazine layout (columns + marginalia)

- **Hero**: full-bleed type sobre surface cream. Cero imagen. Typography ocupa el espacio que una fotografía ocuparía.
- **Features**: grid 3-col con col central (content) + col derecha estrecha (marginalia). Marginalia: notas breves en `tint mid`, rotadas `-90deg` en desktop, inline en mobile.
- **Install**: columna única `52ch` centrada, con pull quote extraído del comando en display italic como si fuera una cita memorable.
- **Support**: dos columnas asimétricas (2:1). Izquierda: cuerpo de texto. Derecha: índice numerado con hairlines separando items.
- **Footer**: folio con número de "página" en red editorial, centrado, tratado como colofón impreso.

---

## Composition (hero como página editorial)

El titular SEELE ocupa dos tercios de altura de viewport en display italic. Debajo, en body serif, un subtítulo de 15-20 palabras en dos líneas. El CTA es texto corrido — "Ver instalación →" — sin botón-caja. Un hairline horizontal 0.5px divide el hero del body como separador de sección en revista. Todo sobre cream. Cero imagen. Cero shape decorativo.

---

## Motion vocabulary (gentle reveals)

Scroll-intersect con `IntersectionObserver`: opacity `0 → 1` + `translateY(24px → 0)`, duration `700ms`, easing `cubic-bezier(0.16, 1, 0.3, 1)`. Las marginalia hacen reveal con `150ms` de delay extra. Pull quotes: clip-path vertical reveal `0% → 100%` en `900ms`. Nada se mueve antes de ser visto.

---

## El detalle distintivo

El **folio editorial** vivo: en cada sección, el número de sección aparece en red editorial como pie de imprenta corrido — "§ 02 / Features" — posicionado en margin exterior, visible pero nunca compitiendo con el contenido. Ninguna otra dirección del sprint tiene este artefacto editorial honesto.

---

## Cuándo elegir esta dirección

Cuando Marisol necesita sentir que el equipo detrás de SEELE *edita* su trabajo con tanto cuidado como ella edita su código. Cuando "parece serio" significa cultura impresa, no dashboard SaaS.

## El riesgo de esta dirección

Un dev tool con tipografía de revista puede señalizar "proyecto de autor" antes que "infraestructura confiable" — el riesgo real es que Marisol admire la dirección sin instalar nada.
