# Estrategia v0.4 — Membrana dual-runtime + robustez + calidad de retrieval

**Estado**: Borrador · 2026-07-27 · **Pendiente Gate 1 humano**
**Origen**: análisis profundo de tres sistemas el 2026-07-26/27 — `DeusData/codebase-memory-mcp` (C11, grafo estructural de código), `CCHIA/GRAIL` (fork GraphRAG, memoria con grafo tipado, benchmark legal-es) y re-auditoría del propio SEELE (verificación de 11 hallazgos previos + 9 nuevos).
**Relación con v0.3**: v0.3 (evaluation-first, pendiente de Gate 1) es **prerequisito de medición** para WS2/WS3 — las mejoras de ranking se mergean contra el baseline de `seele-eval`, no a ciegas. WS0/WS1/WS4 son independientes y pueden ir antes.
**Táctica**: pendiente (post Gate 1). **ADRs a producir**: WS3 (edges) y los puntos de "Decisiones abiertas".

## TL;DR de la decisión

SEELE es arquitectónicamente superior a ambos referentes donde importa (embeddings ONNX reales, sqlite-vec ANN, transaccional, single-binary sin daemon). Le faltan tres cosas que los referentes hacen bien: **robustez bajo concurrencia**, **integridad del índice vectorial**, y **disciplina de query-shape/provenance**. v0.4 cierra esas tres brechas, adapta SEELE a Kimi Code (el ecosistema ya es dual-runtime: AEGIS v2.20.0, LUMEN v0.17.0, MNEMA v0.9.5, KAIROS v0.4.0), y lo hace sin tocar el charter: local-first, offline-by-default, CPU-only, single-binary.

## Evidencia (verificada en disco, con file:line en los reportes de análisis)

**Robustez**: sin `PRAGMA busy_timeout` ni retry de SQLITE_BUSY (`crates/seele-storage/src/pool.rs:34-39`) — dos procesos escribiendo (MCP + CLI + serve) fallan al instante con "database is locked". Es el bug de mayor impacto real: la rutina matutina + cron + reinject ya comparten la DB.

**Integridad vectorial**: el embedding es post-commit best-effort (`service.rs:110-140`) y **no existe reindex** — `onnx.rs:135` documenta un `seele embedder reembed-all` que nunca se construyó. Filas sin vector son invisibles al vec search hasta hoy. `import --re-embed` es no-op con excusa obsoleta (el backend ONNX ya existe).

**Dual-runtime**: `seele setup` soporta claude-code/cursor/windsurf (`seele-setup/src/agents.rs:22-33`); el instalador `install_mcp_json` es genérico — agregar `kimi-code` (`~/.kimi-code/mcp.json`, clave `mcpServers`) son ~20 líneas + 1 test e2e.

**Query-shape** (de codebase-memory-mcp): FTS5 de SEELE puede perder el early-exit de WAND al filtrar/joinear en la misma query; la forma correcta es dos pasos (inner `ORDER BY bm25() LIMIT N`, outer filter). Sus boosts estructurales por label (`mcp.c:2756-2761`) y su output contract (`total`/`has_more`/`detail:"ids"`) son directamente portables.

**Provenance/grafo** (de GRAIL): los edges de SEELE (`links`, `memory_relations`) **nunca se leen en retrieval** — una memoria suplantada rankea igual que su reemplazo. GRAIL aporta: `retrieval_queries` horneadas en el texto del embedding (3× mejor entity matching, costo cero en query time), merge rules de edges (weight=avg, confidence=min, observed_at=max), RecallFilter componible con parser de tiempo relativo, y "cascade rescue" (hits FTS-only inyectados al pool final).

## Workstreams

### WS0 — Robustez y dual-runtime (independiente, ~2-3 días)

- `PRAGMA busy_timeout=5000` + retry-once en la capa storage, con test de contención real (2 procesos escribiendo).
- `kimi-code` en el registry de `seele setup` + test e2e sobre `~/.kimi-code/mcp.json`.
- Default de `score_boost_multiplier` unificado (hoy 0.0 en HTTP/CLI/TUI y 1.0 en `/chat` — elegir uno canónico).
- Bearer token: comparación constant-time (`subtle`) + indirección `$ENVVAR` (hoy el token viaja en argv visible en `ps`).
- `seele save --content -` (stdin) implementar o quitar la promesa del help.
- Pinear `TRUSTED_HASHES` del modelo por defecto (hoy descarga sin verificar).
- REST: `GET /projects` + `PATCH /memories/{id}` (los métodos de servicio ya existen — gap de transporte + OpenAPI).
- `import.rs`: quitar la advertencia obsoleta "until Sprint-05" (la feature ya existe o se implementa en WS1).

### WS1 — Integridad del índice vectorial (~2-3 días)

- `seele embedder reembed-all`: re-embedde filas con vector faltante o modelo distinto al vigente (usa `EmbeddingProvenance::mix_warning` de doctor como detector; batch con progreso; idempotente). Hace real `import --re-embed`. Prerequisito de confianza para cualquier cambio de ranking.
- Decidir el destino de `user_prompts`/`prompts_fts`: está write-dead en producción (PromptStore instanciado y jamás llamado) — wirear captura o borrar tabla+FTS+store.

### WS2 — Calidad de retrieval (gated por baseline v0.3, ~3-4 días)

- Two-step FTS5: inner top-N por bm25, outer con filtros — preserva WAND.
- `retrieval_queries` (col JSON opcional en observations): el texto embebido pasa a `"{topic_key}: {content} {q1} {q2}"`. Cero costo en query-time; portado de GRAIL.
- Cascade rescue: top-K hits FTS-only ausentes del pool vectorial se inyectan a la lista final (política de ranking, no infra nueva).
- Superseded-blindness: hits con relación `SupersededBy` se degradan o filtran en query time (primer uso real de edges en retrieval).
- Recency opt-in: término de ranking sobre `last_seen_at` (schema ya lo tiene), default off.
- Cada cambio entra con su número contra el baseline de `seele-eval` (disciplina v0.3).

### WS3 — Grafo útil (ADR requerido, ~3-4 días)

- ADR: resolver la superposición `links` vs `memory_relations` (una sola tabla de edges real) + merge rules de GRAIL (weight=avg, confidence=min, observed_at=max, dirección de autoría preservada).
- Bonus RRF a memorias a 1-hop de los top-K (segundo uso de edges en retrieval).
- `seele consolidate`: detección determinista de alias (Jaro-Winkler + coseno, crate `strsim`) + propuestas JSON con piso de confianza — **propone, nunca muta** (patrón GRAIL; es exactamente el PASO 7 de higiene SEELE de la rutina matutina, hoy manual).
- MinHash/LSH near-dup pre-insert (trigramas, tabla lateral) como complemento del topic-key upsert.

### WS4 — Operación y ergonomía (~2 días)

- `seele backup` (VACUUM INTO + destino) y `seele doctor` ganando chequeo de FTS optimize.
- Op-log `_history.jsonl` por proyecto (append en cada write: op, ts, payload) para replay/debug — portado de GRAIL.
- Import batch genérico JSONL (hoy solo `import from-engram`).
- Feature-gates: `tui`/`chat`/`eval` fuera del binario CLI por defecto (seele-cli hoy linkea los 13 crates, incl. ratatui).
- `capture_passive`: headings localizados o reemplazo por convención explícita en los agent configs que genera setup.

## Alcance (out / diferido)

- **Indexación de codebase** (tree-sitter, símbolos, blast-radius, file watching): fuera de charter — SEELE es memoria de agentes, no motor de código. codebase-memory-mcp ya lo hace mejor como herramienta separada.
- **int8 quantization** de vectores: ganancia real pero prematura a 384-dim con sqlite-vec ANN; revisitar >500k observaciones.
- **Chunking de contenido >256 tokens**: válido pero se diseña con ADR propio (afecta identidad de observaciones); mitigación interina: documentar el límite.
- **Leiden/parquet/community reports**: el stack GraphRAG no entra; SQLite transaccional es la ventaja a preservar.
- **Mover `seele-project::detect` al serve/MCP**: el crate (hoy dead code) se wirea **solo al CLI save**; en procesos long-lived los callers pasan `project` explícito (detect() shella git con timeouts — política a fijar en táctica).

## Decisiones abiertas (para el Arquitecto)

1. `links` vs `memory_relations`: ¿cuál es la tabla de edges canónica? (bloquea WS3).
2. `user_prompts`: ¿wirear o borrar? (WS1).
3. Default canónico de `score_boost_multiplier`: 0.0 (neutro) o 1.0 (chat).
4. Chunking de contenido largo: ¿identidad por chunk o por documento padre?
5. `detect()` en serve/MCP: ¿cache por cwd o prohibido fuera del CLI?

## Riesgos

- **Contaminar el baseline v0.3**: mitigado por el gate (WS2/WS3 solo con número contra baseline).
- **Scope creep hacia motor de código**: mitigado por el "out" explícito.
- **busy_timeout mal configurado** (retry infinito): retry-once acotado + test de contención.
- **Un dev solo**: cada WS cierra con docs + tests e2e, nada queda tribal.

## Criterios de éxito (medibles)

1. Test de contención: 2 procesos × 100 saves concurrentes, cero "database is locked".
2. `seele setup --agent kimi-code` escribe `~/.kimi-code/mcp.json` válido (e2e) y `seele doctor` OK en una sesión Kimi real.
3. Tras `reembed-all`, `doctor` reporta **0** vectores faltantes/mixtos en la DB real.
4. WS2: mejora de recall@k medible contra baseline v0.3 (sin regresión >2% en ninguna categoría del eval).
5. `consolidate` emite propuestas sobre la DB real y ninguna mutación ocurre sin aceptación explícita.

## Wagers propuestos (para registrar en KAIROS al pasar Gate 1)

1. `wgr_seele_v04_robustez_001` — "Tras WS0+WS1, `seele doctor` sobre la DB de producción reporta 0 vectores faltantes y el test de contención pasa en CI", p=0.8, resolve_by=+30 días.
2. `wgr_seele_v04_retrieval_001` — "Al menos una mejora de WS2 muestra recall@k superior al baseline v0.3 sin regresión >2% en ninguna categoría", p=0.6, resolve_by=+60 días.
3. `wgr_seele_v04_kimi_001` — "Una sesión real de Kimi Code usa `mcp__seele__*` vía `mcp.json` instalado por `seele setup` sin intervención manual", p=0.75, resolve_by=+14 días.
