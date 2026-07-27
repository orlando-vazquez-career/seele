# 04 — Resúmenes temáticos tipo GraphRAG (clusters + community reports)

**Estado**: fuera de v0.4 (depende de WS3) · **Origen**: análisis de `CCHIA/GRAIL` (2026-07-26).

## Qué es

Sobre el grafo de memorias: detectar comunidades temáticas (Leiden o componentes conectados por edges fuertes), generar un resumen por comunidad (LLM local vía `seele-chat`), y usar esos resúmenes como una capa de retrieval "global" (preguntas de síntesis: "¿qué sé yo sobre X?").

## Evidencia (lo que GRAIL hace bien)

- **Actualización incremental de comunidades** por ratio de cambio: changed/total < 0.3 → label propagation (heredar la comunidad del vecino más conectado); ≥ 0.3 → Leiden sobre el subgrafo afectado de 1-hop, mergeado de vuelta. Solo se regeneran los reports de las comunidades afectadas (`grail/indexing/incremental_community.py:47-79`).
- Búsqueda global por map-reduce sobre community reports (la capa que responde síntesis, no instancias).
- Consolidación por densidad de edges como detector de temas sin LLM (`grail/memory/analyses/edge_density.py:55-100`).

## Por qué es valioso

Es la única capa de retrieval que SEELE no puede emular hoy: preguntas de síntesis sobre todo un proyecto. Complementa FTS+vector (que encuentran *instancias*) con *temas*. Y reutiliza piezas que ya entran por WS3 (edges reales) y por el charter (`seele-chat` con endpoint local = LLM local opt-in, 100% offline).

## Por qué quedó fuera

- **Dependencia dura**: sin edges útiles en retrieval (WS3) no hay grafo sobre el cual clusterizar. Es fase 2 por construcción.
- **Stack ajeno**: GRAIL paga parquet + NetworkX + rewrites completos por escritura; la versión SEELE debe ser SQL puro (componentes conectados = CTE recursivo) o nada.
- **Costo operativo**: los reports son regeneraciones LLM caras; exige la disciplina incremental de GRAIL desde el día uno o se vuelve inmantenible.

## Gatillo de adopción

WS3 cerrado (edges + merge rules + 1-hop bonus en producción) **y** una necesidad real articulada de síntesis temática (p.ej. la hidratación de KAIROS pidiendo "resumen del proyecto" en vez de lista de observaciones).

## Boceto de integración (si algún día entra)

Tabla `communities(id, project, summary, member_count, updated_at)` + `community_members(memory_id, community_id)`. Clustering: componentes conectados sobre la tabla de edges canónica (post-ADR WS3) con peso mínimo, en SQL. Resumen: `seele-chat` apuntado a localhost (charter: LLM opt-in local). Regeneración: solo comunidades cuyo ratio de cambio ≥ 0.3 (regla GRAIL). Retrieval global: modo `seele search --global` que rankea reports en vez de observaciones. Nada de parquet ni NetworkX.
