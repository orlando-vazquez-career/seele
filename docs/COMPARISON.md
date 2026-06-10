# SEELE vs ENGRAM vs GRAIL — comparación honesta

**Fecha**: 2026-06-10 · **Versiones comparadas**: SEELE `main` post-v0.2.0
(sprint GRAIL-H1) · [ENGRAM](https://github.com/Gentleman-Programming/engram)
(upstream, inspiración de SEELE — ver [CREDITS](../CREDITS.md)) ·
[GRAIL](https://github.com/CAMARA-CHILENA-INTELIGENCIA-ARTIFICIAL/GRAIL)
v0.1.4.

Regla de esta página: cada número de SEELE sale de un artefacto
reproducible (`seele eval --suite coding-memory`); lo que no podemos
verificar se marca como tal. La comparación desactualizada es peor que
ninguna — está fechada y entra al checklist de release
([RELEASING.md](./RELEASING.md)).

## Las tres apuestas

| Eje | ENGRAM | SEELE | GRAIL |
|---|---|---|---|
| Stack / runtime | Go, binario | Rust, binario estático único | Python ≥3.10, pip/uv + venv |
| Modelo de datos | SQLite + FTS5 | SQLite + FTS5 + vec0 (sqlite-vec vendorizado) | Parquet inmutable + FAISS + NetworkX en memoria |
| Retrieval | FTS5 | Híbrido: FTS5 estricta + FTS5 bag-of-words + KNN vectorial, fusión RRF (k=60) | 6 modos (local/cascade/global/document/agent/recall) sobre grafo + embeddings |
| ¿LLM en el camino core? | No | **Nunca** — embeddings ONNX locales (all-MiniLM-L6-v2); todo LLM es opt-in (`seele-chat`) | Sí — extracción de entidades, dedup-judge y reports son llamadas LLM (su *memory mode* funciona sin LLM) |
| Grafo | — | Tabla `links` direccional tipada + `memory_relations` con lifecycle de judgment | Entidades/relaciones/comunidades Leiden first-class |
| Embeddings | — | Locales, CPU-only, auto-descarga (~90 MB), provenance en `embeddings_meta` | Por API (OpenAI-compat, 10 endpoints) o locales |
| Superficie de agente | MCP (origen de las 19 tools) | MCP stdio (19 tools) + CLI `--json` con envelope `{ok,data,warnings}` + HTTP/OpenAPI + TUI | Skill portable (21 scripts) + CLI + chat web |
| Eval de calidad | — | `seele-eval`: recall@5/10, MRR, nDCG@10 por categoría, determinístico, sin costo por corrida | Benchmark LLM-as-judge de 6 sistemas (30 preguntas legales); los números insignia fueron juzgados manualmente |
| Distribución | binario Go | `cargo install --git` hoy; pipeline de 5 targets reparado en GRAIL-H1, primer release binario pendiente | PyPI (`graphgrail`), 5 releases en su primera semana |
| Footprint | chico (Go) | ~42,55 MB exe release (medición local ADR-15, Windows; sin release oficial que medir aún) | decenas de MB de deps Python + runtime |
| Licencia | MIT | MIT | MIT |

## Donde cada uno gana

**ENGRAM** definió la categoría: schema de 9 tablas, semántica de las 19
tools, detección de proyecto, topic-keys, privacy strip. SEELE es una
reimplementación clean-room de esas ideas (Rust, MIT, sin código Go
copiado) — sin ENGRAM no hay SEELE.

**SEELE** gana en: core sin LLM (cero costo y cero red en save/search),
un solo binario multiplataforma, migraciones SQL transaccionales
versionadas, búsqueda híbrida medida por harness propio, CI de código en
3 OS (~370 tests), y auditabilidad (baseline de eval versionado,
`embeddings_meta`, `--explain`).

**GRAIL** gana en: razonamiento multi-hop estructural sobre corpora densos
(comunidades Leiden + 6 modos de retrieval), documentación pública
navegable (Docusaurus bilingüe), loop de adopción funcionando (releases →
sitio → funnel → PR externo en días), y visualización (graph viewer D3
standalone).

## El punto débil de SEELE, con números propios

Nuestro harness marcó paraphrase y multi-hop como las categorías débiles
del RRF puro. El sprint GRAIL-H1 (2026-06-10) atacó paraphrase con un
tercer path RRF bag-of-words, gateado por eval (ONNX real, suite
coding-memory v2, n=34):

| Categoría | r@5 antes | r@5 después | MRR antes | MRR después |
|---|---|---|---|---|
| paraphrase (n=15) | 0.733 | **0.867** | 0.546 | **0.707** |
| multi-hop (n=10) | 0.900 | **1.000** | 0.720 | **0.833** |
| TOTAL (n=34) | 0.853 | **0.941** | 0.657 | **0.788** |

Honestidad estructural: en un corpus denso en entidades interconectadas,
un GraphRAG como GRAIL sigue teniendo ventaja arquitectónica en multi-hop
(la información conectada se recupera junta). La respuesta de SEELE en el
roadmap v0.3 es la expansión 1-hop por `links` post-RRF — grafo en
retrieval sin LLM. Reproducí los números: `cargo run --bin seele -- eval
--suite coding-memory` (los artefactos del sprint viven junto al plan
táctico en `docs/plans/`).

## Cuál usar

- Memoria local de agentes sin costos por operación, un binario, MCP
  directo: **SEELE**.
- Knowledge-base sobre un corpus documental grande y denso, con
  presupuesto LLM y Python en el stack: **GRAIL**.
- Referencia mínima de la categoría en Go: **ENGRAM**.
