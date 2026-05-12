# Journey as-is — descubrir SEELE en 2026-05

## Paso 1 — Discovery

**Canal**: HN post / Twitter mention / Reddit r/rust.
**Actor**: lector como Marisol.
**Estado actual**: README en GitHub aparece de primero. Sin landing dedicada.

**Pain**: README es excelente para devs profundos pero la primera pantalla pide scroll para entender. No hay narrativa de "para quién es esto" antes de la API.

**Oportunidad**: una landing actúa como entry point con narrativa antes del README.

## Paso 2 — Comprensión

**Acción**: lee la primera mitad del README.
**Estado actual**: encuentra título, blockquote latino, badges, status v0.1.0, quick start.
**Tiempo a primer "ah, ya entendí"**: ~40s leyendo.

**Pain**: el primer párrafo descriptivo es técnico ("Rust + SQLite + FTS5 + sqlite-vec + ONNX embeddings + hybrid search via RRF") — denso para alguien que aún no decidió si le interesa.

**Oportunidad**: hero con 1 línea humana ("Memoria local para tus agentes de IA") + sub-línea técnica.

## Paso 3 — Decisión de instalar

**Acción**: copia el comando `curl … install.sh | bash`.
**Estado actual**: visible en línea 24 del README. Requiere scroll mínimo.

**Pain**: para usuarios que no quieren ejecutar `curl | bash`, las alternativas (cargo install, build from source) están en líneas 67-75 más abajo.

**Oportunidad**: landing presenta los 3 paths de install lado a lado con tabs o cards.

## Paso 4 — Decisión de donar

**Acción**: scrollea al "Support / Apoyar".
**Estado actual**: 5 badges con direcciones. Para donar, copia address → abre wallet manualmente → pega → ingresa monto → firma.

**Pain**: 5+ pasos manuales. Si la persona estaba en un browser desktop con MetaMask instalado, no se aprovecha la integración.

**Oportunidad**: clic en el botón → URI scheme abre wallet preferido con destino pre-rellenado. Fallback a copy si no hay wallet. Esto es **el principal valor** del sprint.

## Resumen pain points

| # | Paso | Pain | Severidad | Oportunidad |
|---|---|---|---|---|
| 1 | Discovery | README no es entry point ideal para newcomers | Media | Landing dedicada |
| 2 | Comprensión | Primer párrafo técnico-denso | Baja | Hero con tagline humana |
| 3 | Install | Paths alternativos enterrados | Baja | Sección Install con cards lado a lado |
| 4 | Donate | 5+ pasos manuales, sin integración wallet | **Alta** | Donate widget con URI scheme |

Los pain 1-3 son nice-to-have. **El pain 4 justifica el sprint por sí solo.**
