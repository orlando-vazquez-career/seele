# 02 — Cuantización int8 de vectores

**Estado**: fuera de v0.4 por prematuro · **Origen**: análisis de `DeusData/codebase-memory-mcp` (2026-07-26).

## Qué es

Guardar una copia cuantizada a int8 de cada embedding (384-dim float32 → 384 int8) y hacer el primer paso de similitud coseno con aritmética entera; re-rankear el top-K en float.

## Evidencia

- codebase-memory-mcp cuantiza todos sus vectores a int8 (`semantic.c:58-60`) y computa coseno entero en SQL (`cbm_cosine_i8`, `store.c:7664-7686`): 4× menos memoria, dot product solo-enteros, pérdida de recall despreciable en su escala.
- SEELE hoy guarda float32 en sqlite-vec (vec0). 384 dims × 4 bytes = 1.536 B por observación; 100k observaciones ≈ 150 MB en vectores (más el índice ANN).

## Por qué es valioso

Corte 4× de disco y RAM del índice vectorial, scans más rápidos (enteros caben en SIMD trivial), y el patrón coarse-int8 → rerank-float es compatible con sqlite-vec sin cambiar el modelo.

## Por qué quedó fuera

- **SEELE ya tiene ANN** (sqlite-vec): el escaneo bruto que int8 acelera no es el cuello de botella a esta escala. El beneficio grande de int8 es en brute-force; con ANN el ahorro es de footprint, no de latencia.
- **Costo**: columna paralela + migración + re-cuantización en cada write + un paso de rerank — y un nuevo modo de fallo silencioso si float e int8 se desincronizan (el mismo riesgo que hoy tiene el vector faltante, WS1).
- A ~100k observaciones el ahorro absoluto son ~110 MB: no mueve la aguja.

## Gatillo de adopción

>500k observaciones, o presión real de disco/RAM medida (p.ej. el índice supera la mitad del tamaño de la DB), o perfiles de `seele-eval`/perf que muestren el scan vectorial como cuello.

## Boceto de integración (si algún día entra)

Tabla lateral `observations_vec_i8(int_id PK, vec BLOB)` escrita en el mismo commit que el float (eliminar el best-effort post-commit primero — WS1), cuantización min-max por vector, coarse pass `ORDER BY int_dot(vec_i8, ?) LIMIT 4K` y rerank float sobre esos 4K. Migración: `reembed-all` gana un flag `--quantize` que la rellena. Medir recall@k contra float puro antes de activar el camino int8 por defecto.
