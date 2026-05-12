# Variation 1 — Brushed-metal editorial

## Paleta
- `surface-canvas`: oklch(0.08, 0.004, 230) — negro con tono azul-acero frío, base de todo
- `surface-raised`: oklch(0.13, 0.006, 225) — plano elevado, tarjetas y panels
- `surface-edge`: oklch(0.22, 0.005, 220) — borde físico, como canto de metal cepillado
- `fg-primary`: oklch(0.91, 0.008, 215) — blanco frío con ligero tinte plateado
- `fg-muted`: oklch(0.52, 0.005, 220) — texto secundario, metálico apagado
- `accent-warm`: oklch(0.76, 0.13, 85) — dorado cálido, solo en titulares y CTAs
- `accent-cool`: oklch(0.78, 0.05, 195) — cyan-plata, para código inline y etiquetas técnicas
- `accent-danger`: oklch(0.62, 0.18, 25) — cobre oxidado, solo para estados de error

## Typography
- **Display**: Playfair Display Italic — 72px hero, 48px section headers — el serif generoso con itálica fuerte hace todo el trabajo emocional
- **Body**: Inter — 15px/1.65 — neutro, funcional, no compite con display
- **Mono**: JetBrains Mono — 13px — comandos, hashes, version strings
- Ratio display:body = 4.8:1
- Tracking display: -0.02em. Tracking mono: 0.04em. Body: 0.

## Composition
```
│ col1 │ col2 col3 col4 │ col5 col6 col7 │
│      │                │                │
│      │  SEELE         │  [triangle]    │
│      │  ─────────     │  logo framed   │
│      │  Memory engine │  top-right     │
│      │  for serious   │                │
│      │  Rust devs.    │  v0.1.0-alpha  │
│      │                │  ──────────    │
│   ▶  │  [install cmd] │  "cargo add"   │
│  rot │                │                │
│  90° │                │                │
```
Col1 vacía excepto por caption rotada 90° ("SEELE MEMORY ENGINE / 2026"). El headline serif-italic ocupa cols 2-4. El triángulo-logo vive cols 6-7, pequeño, enmarcado en borde `surface-edge`, nunca mayor a 48px.

## Motion vocabulary
- **Entrada de contenido**: fade + translate-Y 6px, 220ms ease-out. Solo en scroll trigger. Nunca en load.
- **Hover en CTA**: border-color transition 90ms linear. Sin scale, sin translate.
- **Hover en tiles**: background shift de `surface-raised` a `surface-edge`, 90ms. Nada más.
- **NO animar**: triángulo-logo, tipografía, nada en above-the-fold inicial.

## Texture & motifs
La superficie tiene ruido SVG sutil (feTurbulence baseFrequency 0.65, opacity 0.04) simulando el grano del metal cepillado — perceptible solo en pantallas de alta densidad. Los bordes de cards son 1px sólido `surface-edge` — sin blur, sin sombra, como corte preciso. El triángulo es geométrico perfecto, enmarcado en box 1px, esquina superior derecha del hero — funciona como marca editorial, no como decoración.

## El detalle distintivo
Las secciones se separan por **reglas horizontales de 1px** con el hash de commit actual embebido como texto `fg-muted` alineado a la derecha — `// a3f9d12`. Solo Brushed-metal editorial usa artefactos de proceso (commits, timestamps) como elementos de diseño funcional-decorativo. Las otras variaciones nunca mostrarían el intestino del repo en el layout.

## Cuándo elegir esta dirección
Cuando SEELE necesita comunicar que es una herramienta de craftsmen — no un producto de startup, no un framework académico. Si Marisol tiene que sentir que está mirando el panel de control de algo real y serio antes de leer una sola palabra, esta es la dirección.
