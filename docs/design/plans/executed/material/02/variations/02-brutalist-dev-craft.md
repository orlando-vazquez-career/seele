# Variation 2 — Brutalist dev-craft

## Paleta (extrema minimalist)

- `oklch(0.08 0.00 0)` — background absoluto. Negro sin compensación, sin warm, sin cool.
- `oklch(0.94 0.00 0)` — texto primario y bordes. Off-white casi crudo, no puro.
- `oklch(0.65 0.18 50)` — accent único. Naranja eléctrico. Solo para CTAs y valores numéricos críticos (latencia, versión, monto de donación).
- Prohibido agregar un cuarto color. La restricción es la decisión.

## Typography (monospace-only)

**JetBrains Mono** — una sola familia, todas las variaciones de peso interno.

- Hero statement: 72px / weight 800 / tracking -0.02em. Texto como bloque gráfico.
- Subheadline / feature label: 13px / weight 400 / tracking 0.12em / ALL CAPS.
- Body y descripción: 15px / weight 400 / line-height 1.4.
- Hierarchy sin serif: peso extremo (800) vs extremo bajo (400), tamaño salto brusco 72→13px sin escalas intermedias, color accent solo en datos concretos.

Sin display. Sin serif. La jerarquía emerge del contraste de peso y tamaño, no de familias distintas.

## Composition (asimetría brutal)

```
|  SEELE                          [v0.1]  |
|                                         |
|  Memory engine                          |
|  for tools that                         |
|  need to remember.                      |
|                                         |
|        +--------------------------------|
|        | latency: 0.8ms                 |
|        | backend: Rust                  |
|        | status: ALPHA                  |
|        +--------------------------------|
|                                         |
|  [INSTALL NOW]        [DONATE $5]       |
```

El hero es left-heavy. El bloque de datos está offset a la derecha con borde superior-derecho solamente — bordes incompletos como decisión compositiva. Sin centrado. Margen izquierdo de 16px, derecho de 80px — desequilibrio intencional que fuerza la mirada.

## Motion vocabulary (restraint extremo)

`transition: none` en todo. Sin excepción.

El hover state es cambio de estado instantáneo: fondo naranja / texto negro, sin interpolación. El click es confirmación binaria, no performance visual. SEELE es una herramienta de infraestructura — el movimiento suave miente sobre su naturaleza.

Si hay un estado de carga, es un cursor parpadeante en texto. Eso es todo el motion budget.

## Borders & solidity (la herramienta jerárquica primaria)

Bordes 2px sólidos `oklch(0.94 0.00 0)` reemplazan toda shadow y todo gradient. La jerarquía de contenedores se lee por cuántos lados tiene borde: bloque principal (4 lados), elemento secundario (2 lados top+left), dato inline (1 lado left como indicador). Box-shadow: none. Border-radius: 0 en todo. El borde es información estructural, no decoración.

## El detalle distintivo

**Bordes incompletos como sintaxis visual.** Un contenedor con 3 bordes en lugar de 4 no es un error — es un conector abierto hacia el siguiente elemento. Crea flujo sin gaps ni flechas ni iconos. Las otras tres direcciones nunca harían esto porque parece "roto." En Brutalist dev-craft, parece exactamente como funciona Rust: estructura explícita, sin magia oculta.

## Cuándo elegir esta dirección

Cuando el público entiende que la herramienta es seria antes de que sea bonita. Cuando la credibilidad técnica importa más que la calidez editorial.
