# Plan de mejoras: lo que GRAIL hace bien, aplicado a SEELE

**Fecha**: 2026-06-09
**Fuente**: [Análisis](2026-06-09-analisis-grail-vs-seele.md) comparativo de 10 dimensiones, verificado claim por claim contra el código de ambos repos y el compendio fact-checkeado de GRAIL (`C:/dev/tools/GRAIL/docs/COMPENDIUM.md` + `docs/compendium/01..16-*.md`).

---

## 1. Principios del plan

1. **Filosofía SEELE intacta e innegociable**: el camino core de guardado/búsqueda NUNCA requiere un LLM. Todas las propuestas de este plan son determinísticas (Rust puro, SQLite, embeddings ONNX locales ya computados, strsim) o estrictamente opt-in detrás de flags explícitos. El propio GRAIL valida que esto es posible: su *memory mode* (compendio 07) corre la capa de grafo completa sin LLM ni embeddings.
2. **Cada mejora mapea a una fortaleza verificada de GRAIL**: ninguna propuesta nace de moda; todas citan el doc del compendio (370 claims chequeados) y el archivo/línea de GRAIL que la valida — y, donde GRAIL falló (sus derivas documentadas), adoptamos la lección negativa como anti-patrón explícito.
3. **Eval-first**: todo cambio que toque retrieval se gatea con `seele eval` contra el baseline (`baseline-v0.2.json`: paraphrase r@5 0.667 / MRR 0.611; multi-hop r@5 0.667 / MRR 0.5). El número decide, no la intuición.
4. **Correcciones de verificación incorporadas**: este plan ya aplica las correcciones de los verificadores sobre el material original — umbrales fieles a GRAIL (JW ≥ 0.92, coseno ≥ 0.93 como señal; 0.85 como piso de confianza), unidades de distancia correctas (`observations_vec` usa **L2** por default, no coseno), esfuerzos recalibrados (S→M donde correspondía), citas de structs/archivos corregidas (`Tool`, no `ToolDef`; 42.55 MB, no 44), y prerequisitos reales presupuestados (el harness eval NO soporta links hoy; `clientInfo` del initialize MCP hoy se descarta).
5. **Consolidación de duplicados**: las 40 propuestas supervivientes de las 10 dimensiones (cero invalidadas) se consolidan en **37**, porque tres pares convergieron de forma independiente desde dimensiones distintas — señal fuerte de que valen:
   - Expansión 1-hop por links post-RRF (dimensiones Grafo + Retrieval) → **T1**.
   - Cablear `embeddings_meta` + guard anti-mezcla (dimensiones Provenance + Capa LLM) → **Q5**.
   - Historial append-only de revisiones del upsert (dimensiones Consolidación + Provenance) → **E4**.
6. **Nota AEGIS — este plan es PROPUESTA**: ningún bloque se ejecuta directo desde este documento. Cada propuesta (o grupo coherente de propuestas) requiere su plan táctico en `docs/plans/tactica/` y el **Gate 1 humano** antes de Ejecución, conforme AEGIS v2.0.0. El cierre de cada bloque exige devlog, CHANGELOG `[Unreleased]`, y los gates del repo (tests + clippy + fmt + STELE residual).

---

## 2. Matriz impacto × esfuerzo

37 propuestas consolidadas. IDs: `Q` = Horizonte 1 (quick wins), `T` = Horizonte 2 (v0.3), `E` = Horizonte 3 (v0.4+).

| ID | Propuesta | Dimensión origen | Impacto | Esfuerzo | Horizonte |
|----|-----------|------------------|---------|----------|-----------|
| Q1 | Reparar release pipeline + v0.2.1 + runbook | Docs/DX | Alto | M (*) | H1 |
| Q2 | Envelope JSON uniforme + contrato de error en CLI | Skill | Alto | S | H1 |
| Q3 | Tercer path RRF: FTS bag-of-words | Retrieval | Alto | S | H1 |
| Q4 | `near_duplicates` en el save path | Consolidación | Alto | S | H1 |
| Q5 | Cablear `embeddings_meta` + guard anti-mezcla | Provenance + LLM | Alto | S/M | H1 |
| Q6 | `find_similar` determinístico + `seele_compare` modo suggest | Grafo | Medio | S (recortado) | H1 |
| Q7 | nDCG@10 + `--suite-file` + engordar suites débiles | Benchmarks | Medio | S | H1 |
| Q8 | SearchTrace: `seele search --explain` | Retrieval | Medio | S/M | H1 |
| Q9 | Endurecer ChatProvider (timeout/retry/strip/dispatch) | Capa LLM | Medio | S/M | H1 |
| Q10 | `topic-families.toml` real | Prompts | Medio | S/M | H1 |
| Q11 | COMPARISON.md + mermaid en README | Docs/DX | Medio | S | H1 |
| T1 | Expansión 1-hop por links post-RRF | Grafo + Retrieval | Alto | M | H2 |
| T2 | Filtros componibles estilo RecallFilter (modo recall) | Retrieval | Alto | M | H2 |
| T3 | Ablation matrix: fts-only / vec-only / hybrid | Benchmarks | Alto | M | H2 |
| T4 | `seele reindex` + ledger `op_runs` | Provenance | Alto | M | H2 |
| T5 | `seele probe` (session_start de SEELE) | Skill | Medio | S | H2 |
| T6 | Docs-site navegable con Starlight | Docs/DX | Alto | M | H2 |
| T7 | Funnel de contribución right-sized | Docs/DX | Medio | S | H2 |
| T8 | Usage y costo honesto en POST /chat | Capa LLM | Medio | M | H2 |
| E1 | Entidades livianas como observaciones tipadas + `mentions` | Grafo | Alto | M | H3 (v0.4) |
| E2 | `seele merge` determinístico con reglas por campo | Consolidación | Alto | M | H3 (v0.4) |
| E3 | `seele consolidate`: proposals revisables en SQLite | Consolidación | Alto | L | H3 (v0.4) |
| E4 | Historial append-only de revisiones | Consolidación + Provenance | Medio | M | H3 (v0.4) |
| E5 | Provenance de actor en observations | Provenance | Medio | M | H3 (v0.4) |
| E6 | Registry único de providers de chat | Capa LLM | Medio | S | H3 (v0.4) |
| E7 | Instructions MCP + descripciones de tools overridables | Prompts | Alto | M | H3 (v0.4) |
| E8 | Registry de prompts versionado | Prompts | Medio | M | H3 (v0.4) |
| E9 | `seele prompts list/show` + provenance en eval | Prompts | Medio | S | H3 (v0.4) |
| E10 | Skill SEELE portable embebido en el binario | Skill | Alto | M | H3 (v0.4) |
| E11 | Retirar los 5 skeletons → instrucciones per-agent | Skill | Medio | S | H3 (v0.4) |
| E12 | GraphSnapshot API (degree + componentes conexas) | Viz | Alto | S | H3 (v0.4) |
| E13 | Crate `seele-viz` + `seele viz` HTML standalone | Viz | Alto | M | H3 (v0.4) |
| E14 | GET /graph + página /graph en Astro | Viz | Medio | S | H3 (v0.4) |
| E15 | Demo graph.html publicado + screenshot README | Viz | Medio | S | H3 (post-E13) |
| E16 | Eval-judge opt-in sobre chat-with-DB | Benchmarks | Medio | L | H3 (v0.4) |
| E17 | Página /benchmarks en el sitio | Benchmarks | Medio | S | H3 (v0.4) |
| E18 | `seele graph doctor` (análisis estructural) | Grafo | Medio | M | H3 (v1.0) |

(*) Q1 es la única excepción de esfuerzo del Horizonte 1: es M, pero va primera porque desbloquea la distribución de todo lo demás (skill, Homebrew, demo, adopción).

---

## 3. Horizonte 1 — Quick wins

Criterio: esfuerzo S (o S/M), impacto alto/medio, sin dependencias estructurales entre sí. Orden sugerido: Q1 → Q2 → Q5 → Q3/Q7 (par eval) → resto.

### Q1. Reparar release pipeline + shippear v0.2.1 + runbook de release

- **Qué construir**: (1) arreglar `.github/workflows/release.yml`: diagnosticar los 4 build jobs que mueren en <15s, `timeout-minutes: 60` para el job macOS (uno quedó colgado 24h), y el job crates.io: agregar clave `version` a TODOS los path-deps del workspace, completar la lista de 12→14 crates (faltan `seele-chat` y `seele-eval`), aplicar el plan ADR-09 nunca implementado (`publish = false` default). Adoptar el check "tag matchea versión del manifest" de GRAIL (`publish.yml:65-74`) como step previo. (2) Tagear **v0.2.1** con los dos fixes críticos varados en `[Unreleased]`: envelope MCP `CallToolResult` (sin él todo tool call se ve "completed with no output" en Claude Code/Cursor/Windsurf) y `hf-hub` 0.5 (first-run del modelo ONNX). (3) `docs/RELEASING.md` calcado del patrón de dos runbooks de GRAIL (`release_planning.md` el "antes", `releasing.md` el "durante" — su sección de fallos comunes es prosa con fixes, no tabla). (4) Limpiar deriva: CHANGELOG link-refs (`[0.2.0]` faltante, compare `[Unreleased]` aún `v0.1.0...HEAD`, doble heading `### Fixed` en líneas 15 y 53) y CLAUDE.md §Estado actual.
- **Por qué GRAIL lo valida**: compendio 01 y 14 §6.2 — 5 releases (v0.1.0→v0.1.4) en una semana, PyPI publicado, releases auto-generados. GRAIL demuestra que el orden importa: primero releases que funcionan, después sitio, después funnel — y eso le produjo un PR externo en días.
- **Criterio de éxito**: GitHub Release v0.2.1 con assets binarios para los 5 targets; `install.sh`/`install.ps1` funcionan end-to-end desde una máquina limpia. Hoy: 0 releases, 0/2 runs exitosos.
- **Riesgos**: iterar el workflow requiere tags rc (`v0.2.1-rc.N`, ya se hizo con `v0.1.0-rc.1`); cross-compile aarch64-linux puede pedir ajustes; los nombres en crates.io son para siempre — verificar disponibilidad y considerar publicar solo `seele-cli` en el primer round.

### Q2. Envelope JSON uniforme + contrato de error en seele-cli

- **Qué construir**: en `seele-cli`: (1) con `--json` activo, error → stdout `{"ok":false,"error":"<display>","kind":"<variante>"}` y exit 1 (hoy `main.rs:34-37` imprime texto libre a stderr aun con `--json`); (2) éxitos envueltos en `{"ok":true,"data":{...},"warnings":[]}` vía `output::emit_split`, eliminando los dos JSON hand-rolleados de `delete.rs:27`/`restore.rs:27`; (3) `warnings` con señales reales: `"fake-embedder-fallback"` cuando `pick_embedder` degrada ONNX→Fake. Matiz del verificador: `seele doctor --json` YA expone `fake_embedder_warning` — lo que falta es la señal **in-band** en el comando que degrada (`save`/`search`), no una señal inexistente. (4) Documentar el envelope como breaking change de los shapes `--json` en CHANGELOG (aceptable: 0 releases publicados). Incluye actualizar los E2E existentes, que parsean los shapes actuales vía `std::process::Command` + `CARGO_BIN_EXE_seele` (no assert_cmd).
- **Por qué GRAIL lo valida**: compendio 13 — el envelope `Reply {ok, data, warnings, next_steps, error}` + decorador `run(fn)` (`_common.py:25-82`) garantiza que el agente jamás parsea stderr ni ve tracebacks; el exit code queda acoplado a `ok`. GRAIL tuvo que construir 21 scripts wrapper para imponer ese contrato; SEELE lo obtiene gratis sobre su binario.
- **Criterio de éxito**: todo subcomando con `--json` emite envelope parseable incluyendo errores; E2E verdes; es el prerequisito de contrato del skill (E10).
- **Riesgos**: bajo. Decidir envolver bajo `"data"` (recomendado) para no colisionar claves.

### Q3. Tercer path RRF: FTS bag-of-words como rescate de paraphrase

- **Qué construir**: en `crates/seele-search/src/engine.rs`, `fts_query_loose()`: tokenizar la query, escapar cada token con comillas y unir con OR (`"tok1" OR "tok2"` en FTS5), mismos filtros inline. Pasarla como tercera lista a `rrf::combine` — el combinador YA es genérico sobre N fuentes (`engine.rs:107-110`); cambio mínimo. Extender `unpack_per_source` y `SearchHit` con `fts_loose_rank: Option<usize>`. Hoy `escape_fts` envuelve TODA la query en comillas (phrase-match estricto): en queries naturales largas la pata FTS devuelve 0 y el ranking queda vec-only — causa plausible directa del paraphrase débil.
- **Por qué GRAIL lo valida**: compendio 06 — el experimento multipath (5 paths × 8 estrategias de fusión) y el rescate textual bag-of-words de cascade (`cascade_search.py:218-231`). Ironía estratégica: SEELE tiene en producción el RRF multipath genérico que GRAIL solo tiene en un script de experimento.
- **Criterio de éxito**: `seele eval --suite coding-memory` antes/después: paraphrase r@5 > 0.667 y MRR > 0.611 sin degradar single-fact/temporal. Merge solo si el eval lo valida.
- **Riesgos**: OR-de-términos puede inundar candidatos en queries cortas — RRF pondera por rank (no score crudo) y `per_method_limit=50` acota; el harness decide.

### Q4. `near_duplicates` en el save path

- **Qué construir**: en el post-save del service (`seele-http/src/service.rs:96-107`, donde el embedding YA está computado): KNN top-3 contra `observations_vec` restringido a mismo project+scope, `deleted_at IS NULL`, excluyendo el id recién guardado. Si la distancia cae bajo umbral, la respuesta de save gana `near_duplicates: [{id, title, topic_key, distance}]` — informacional puro, no cambia el outcome. **Corrección crítica de unidades (verificador)**: `observations_vec` se crea sin `distance_metric` (V001:120-122), por lo que sqlite-vec usa **L2** por default — el umbral inicial debe ser **~0.37 L2** (≈ coseno 0.93 con vectores unit-norm), o recomputar coseno en Rust tras leer el vector; un "0.07" aplicado como L2 no detectaría prácticamente nada. Requiere `fn knn_by_vector(&[f32], filters, k)` en seele-search. Superficies: campo en SaveResponse (HTTP + OpenAPI) y en el resultado de `seele_save` MCP con hint textual estilo `next_steps`. Bonus barato: `seele_suggest_topic_key` sugiere el topic_key del vecino más cercano en vez del `<family>/auto` fijo.
- **Por qué GRAIL lo valida**: compendio 07 — `find_similar_entity` como chequeo de identidad pre-escritura (precisión del verificador: tres señales en paralelo — exact / Jaro-Winkler 0.85 / coseno 0.7 — cuyos candidatos se dedupean y rankean top_k, no cascada con short-circuit). El skill de GRAIL lo llama antes de crear entidades nuevas; SEELE lo da automáticamente en cada save.
- **Criterio de éxito**: casos near-dup agregados a la suite de seele-eval para calibrar el umbral; detección de los casos sembrados con 0 falsos positivos sobre el corpus actual. Ataca por higiene de datos la misma debilidad paraphrase que A1 ataca por ranking.
- **Riesgos**: calibración empírica con MiniLM-L6 384-dim; falso positivo inofensivo (solo informativo); filas importadas por sync/engram-import no tienen vector y quedan invisibles hasta T4 (reindex).

### Q5. Cablear `embeddings_meta` en escritura + guard anti-mezcla [fusión Provenance + Capa LLM]

- **Qué construir — fase 1 (este horizonte)**: cambiar la firma de `ObservationStore::set_embedding` a `set_embedding(&self, id, embedding, meta: EmbeddingMeta)` con `EmbeddingMeta {model_id, dim, contextualized}` y hacer el INSERT OR REPLACE en `embeddings_meta` en la misma operación que `observations_vec`. El trait Embedder ya expone `model_id()`/`dim()`. Guard en doctor: combos `model_id/dim` + conteo de observations activas sin vector — en las DOS superficies (CLI `commands/doctor.rs` Y MCP `tool_impls/meta.rs`, implementaciones independientes). Alcance realista per verificador: **~6-7 archivos** (3 producción, 2 doctor dual, 2 helpers de test de seele-search), no 3 — borde S/M. Sin migración: V002 ya existe; semántica best-effort con warn, consistente con el path actual.
- **Qué construir — fase 2 (aterriza con A2 en v0.3)**: guard en `SearchEngine` antes de la pata vec comparando `model_id` del embedder activo contra el dominante en `embeddings_meta`; ante mismatch, `SearchError::EmbedderMismatch` tipado con mensaje de remediación. Precisión del verificador: el `DimensionMismatch` existente (`error.rs:17-18`) solo chequea **auto-consistencia** del embedder activo, y un swap a dim distinta falla ruidoso en vec0 (columna `float[384]` tipada) — el riesgo genuinamente silencioso es el swap **mismo-dim/distinto-modelo**, que es exactamente el escenario A2 (paraphrase-multilingual-MiniLM-L12-v2 también es 384-d). El guard debe comparar `model_id`, no solo dim.
- **Por qué GRAIL lo valida**: compendio 05 — guard de dimensión en `grail/query/retrieval.py:136-153` con ValueError e instrucciones de remediación. SEELE dejó la tabla declarada como prerequisito de A2/B3 en el header de V002 ("heterogeneous vectors must never be mixed silently") pero nada la escribe: el guard prometido es inoperante hoy.
- **Criterio de éxito**: 100% de saves nuevos con fila en `embeddings_meta`; test que siembra DB mixta sintética y verifica que doctor la detecta; A2 queda desbloqueado con seguridad.
- **Riesgos**: cambio de firma pública (callers internos, contenidos en el workspace); cachear el estado en el engine para no pagar 1 query por búsqueda; definir semántica mid-reindex (recomendado: filtrar la pata vec al modelo activo + warn).

### Q6. `find_similar` determinístico + `seele_compare` que de verdad compare

- **Qué construir (v1 recortada a S, per verificador)**: (1) método `SeeleService::find_similar(id_or_text, top_k)` combinando similitud de título vía crate `strsim` (pure Rust) + KNN vec0 reutilizando el embedding ya almacenado (para una observación existente, SELECT del vector por `int_id` — sin re-embeber); (2) modo **suggest** en `seele_compare` (hoy crea un `conflicts_with` pending hardcodeado sin mirar contenido): devuelve candidatos `{id, title, score, signal: exact|title|vector}` y, si el caller confirma, crea la `memory_relation` con `confidence = score` y `marked_by_kind = 'heuristic'`. La tool dedicada `seele_find_similar` y el endpoint `POST /memories/{id}/similar` se **difieren a v0.4** (junto a E2/E3) para mantener el esfuerzo en S. **Umbrales corregidos, fieles a GRAIL**: JW ≥ 0.92 y coseno ≥ **0.93** como umbrales de señal (`alias_min_embedding_cosine: 0.93`, `config.py:346`); 0.85 es el piso de confianza del ciclo suggest→confirm, no la señal. Y en config — como hace el propio GRAIL con `MemoryConfig`, que NO hardcodea sus umbrales.
- **Por qué GRAIL lo valida**: compendio 07 — AliasDetect con sesgo conservador ("false merges are worse than missed merges"), señales same-type-only.
- **Criterio de éxito**: `seele_compare` en modo suggest devuelve candidatos correctos sobre fixtures; cero relaciones creadas a ciegas. Es el guard-rail de calidad de E1 (entidades).
- **Riesgos**: falsos positivos en títulos cortos/templados — exigir mismo type y mismo project; el coseno MiniLM es la señal débil conocida, por eso JW va primero.

### Q7. nDCG@10 + suites custom por archivo + engordar paraphrase/multi-hop

- **Qué construir**: (1) nDCG@10 en `Metrics`/`Acc` de `seele-eval` (prometido en ADR-14, hoy ausente; con relevancia binaria es DCG/IDCG directo, ~30 líneas + tests); (2) `seele eval --suite-file <path.json>` llamando al `load_suite(path)` que YA existe (`lib.rs:98`) sin caller CLI — habilita corpora propios de terceros y MNEMA sin recompilar; (3) crecer coding-memory en las categorías débiles: paraphrase 6→~15 queries, multi-hop 3→~10, con campo opcional `rationale` por query (documental, ignorado por el runner) imitando el diseño GRAIL. Versionar como suite v2 y recapturar baseline (invalida comparación directa con baseline-v0.2.json — esperado y correcto).
- **Por qué GRAIL lo valida**: compendio 15 — 30 preguntas en 7 categorías donde cada una aísla UNA capacidad, con gold + rationale. Sin esto, el gate A1/A2 de SEELE decide sobre n=3 en multi-hop: ruido estadístico.
- **Criterio de éxito**: gate A1/A2 decide sobre n≥10 por categoría débil.
- **Riesgos**: teaching-to-the-test — autorear las queries nuevas a ciegas contra el corpus, no contra los resultados del baseline; documentar nDCG como métrica secundaria hasta tener labels graduados.

### Q8. SearchTrace: `seele search --explain`

- **Qué construir (v1 mínima = S; HTTP/timings empujan a M — recortar)**: campo `explain: bool` en SearchQuery; v1 CLI-only con trace mínimo: query original, `fts_query` escapada (¡hace visible el phrase-quoting!), counts de candidatos por path (el `RrfHit.per_source` ya los trackea), distances de vec (ya se SELECTean y se descartan en `engine.rs:243` — drift conocido vs ADR-03), parámetros efectivos (`rrf_k`, `per_method_limit`, `boost_multiplier`, `max_vec_distance`), y model_id/dim del embedder (primer consumer real de `embeddings_meta`, sinergia Q5). Struct versionado. HTTP `"explain": true` y timings por etapa: v0.3 si hace falta.
- **Por qué GRAIL lo valida**: compendio 06 — QueryTracer (`--trace <dir>`, dump JSON del pipeline completo por query). La versión SEELE es determinística: sin LLM que capturar, el trace modela la fusión.
- **Criterio de éxito**: `--explain` sobre una query de paraphrase conocida muestra `fts_candidates: 0` — el síntoma exacto del bug queda visible al primer uso; herramienta interna para tunear Q3 y T1 contra el harness.
- **Riesgos**: bajo (read-only, opt-in). No volcar contenido completo de candidatos (privacy + payload): solo ids/títulos truncados/ranks/distances.

### Q9. Endurecer ChatProvider: timeout, retry/429, strip `<think>` server-side, dispatch por nombre de tool

- **Qué construir**: en `seele-chat`: (1) `reqwest::Client::builder().timeout(120s).connect_timeout(10s)` en ambos providers (hoy `Client::new()` sin timeout en `lib.rs:137,239`). Matiz del verificador: cuando el browser aborta, axum/hyper cancela el future en cascada — el daño real son los consumers que NO abortan (curl, llamadas programáticas a POST /chat) y los recursos retenidos hasta el abort. (2) Retry en `complete()`: máx 3 intentos, espera fija 5s, sobre timeout/connect/429/5xx, honrando `Retry-After` — versión mínima del tenacity de GRAIL, sin dependencia nueva. (3) `strip_thinking()` server-side antes de pushear a history (regex estilo `privacy.rs`: `<think>`/`<thinking>`/variantes) — corta la inflación de tokens en turnos siguientes y limpia la salida para consumers no-web. (4) En `seele-http/handlers.rs:318`: rutear tool calls por `tc.function.name` y responder `(tool error) unknown tool` a calls alucinados (hoy TODO call se rutea al parser de `seele_search`). (5) Bootstrap de la suite de tests de seele-chat (hoy CERO tests): mock server (httpmock/wiremock) con escenarios 429-then-success, timeout, `<think>` — esto empuja el paquete a S/M, asumido.
- **Por qué GRAIL lo valida**: compendio 05 — `wrapper.py:261-269` (retries), `:193-198` (`_strip_thinking` server-side), `:357-408` (tolerancia a fallos).
- **Criterio de éxito**: 3 tests mock verdes; consumers no-web reciben contenido limpio; el panel deja de poder colgar conexiones.
- **Riesgos**: retry sobre POST no-idempotente puede duplicar una llamada (aceptable en chat: peor caso tokens duplicados, sin efecto en la DB).

### Q10. `topic-families.toml` real: cerrar el vaporware de CLAUDE.md

- **Qué construir**: módulo `seele_core::families` (dep `toml`+serde): `TopicFamily {name, keywords}` + loader con resolución en capas `.seele/topic-families.toml` (proyecto) > `~/.seele/topic-families.toml` (usuario) > 7 familias ENGRAM builtin. Cablear `suggest_topic_key` (`seele-mcp/tool_impls/meta.rs:44-98`) vía SeeleService (resolución una vez al boot); campo `source: builtin|user|project` en la respuesta. TOML inválido = warn + fallback builtin, nunca panic. CLAUDE.md promete este archivo desde genesis y hoy hay cero hits en `crates/` — deriva doc-vs-código confirmada por el compendio propio.
- **Por qué GRAIL lo valida**: compendio 08 — resolución `custom_paths → builtin`, primer directorio gana, config de 2 campos.
- **Criterio de éxito**: E2E MCP con familia custom; unit tests del loader. Borde S/M por el cableado a través de seele-core/http/mcp.
- **Riesgos**: el matching por substring sigue naive (esto configura keywords, no mejora el algoritmo); validación mínima (nombre no vacío, ≥1 keyword).

### Q11. COMPARISON.md honesta + mermaid de arquitectura en README

- **Qué construir**: (1) `docs/COMPARISON.md` estilo `comparison.md` de GRAIL (323 líneas), tabla por dimensión: stack/runtime (Go vs Rust vs Python), apuesta de retrieval (FTS+LLM-judge vs FTS5+vec0+RRF vs GraphRAG+Leiden), requisito de LLM (core vs nunca-en-core vs LLM-céntrico), footprint, superficies — y la parte honesta: multi-hop citando el baseline propio (r@5 0.667, donde un GraphRAG estructuralmente gana) + evidencia reproducible (`seele eval --suite`). **Corrección del verificador sobre cifras**: el binario local medido en ADR-15 es 42.55 MB (exe release stripped, build local) — marcarlo como medición local hasta que exista release oficial; el target lite < ~12 MB es hipótesis, no dato. Fechar la tabla y citar versiones exactas (GRAIL v0.1.4, ENGRAM commit); agregarla al checklist del runbook Q1. (2) Bloque mermaid en README (hoy cero diagramas): save→strip→hash→upsert/dedup→FTS5+vec0 y search→FTS+vec→RRF→hits, con las 4 superficies sobre el único SeeleService.
- **Por qué GRAIL lo valida**: compendio 16 — la página de comparación honesta como activo de adopción; GitHub renderiza mermaid nativo a costo casi cero.
- **Criterio de éxito**: README con diagrama; comparison linkeada desde README y futuro docs-site.
- **Riesgos**: comparación desactualizada es peor que ninguna (GRAIL releasea semanalmente) — fechar, versionar, e incluir en el checklist de release.

---

## 4. Horizonte 2 — v0.3 calidad de memoria

Alineado al sprint eval-first ya abierto (ADR-14, gate A1 reranking / A2 embedder swap) y a las dos debilidades medidas del baseline: **paraphrase** (r@5 0.667 / MRR 0.611) y **multi-hop** (r@5 0.667 / MRR 0.5, el peor MRR de todas las categorías).

### T1. Expansión 1-hop por links post-RRF [fusión Grafo + Retrieval]

- **Qué construir**: campo `expand_links: bool` (default **off**) en SearchQuery. Tras `rrf::combine` + boost: tomar los top-5 seeds, correr UNA query SQL batched sobre `links` en ambas direcciones (`WHERE from_id IN (...) OR to_id IN (...)`, aprovechando `idx_links_from/to` existentes) + `memory_relations` (supersedes/related), recolectar vecinos a 1 salto no presentes e inyectarlos con `score = seed.score * 0.5` (decay fijo v1, constante con test), cap de 3 vecinos por seed, re-sort antes del truncado. `SearchHit` gana `graph_via: Option<SeeleId>` + `link_type` para que el agente vea POR QUÉ llegó ese hit. Vecinos hidratados con `deleted_at IS NULL`. Superficies: HTTP (`SearchRequest`), MCP (schema de `seele_search`), CLI (`--expand-links`). **Prerequisito presupuestado dentro del M (corrección del verificador)**: el harness eval NO soporta links hoy — `CorpusItem` es solo `{id, body, metadata}` y `ingest()` jamás toca LinkStore. Hay que extender el contrato Suite/CorpusItem con `links: [{from,to,type}]`, crear los links en el ingest (`LinkStore::create_in_tx` existe, `links.rs:69`), y sumar 5-8 queries multi-hop con aristas reales. Hoy la categoría multi-hop mide multi-hop sin darle al engine ninguna herramienta para resolverlo.
- **Por qué GRAIL lo valida**: compendio 06 (patrón rescue de cascade: inyectar candidatos fuera del camino principal, `cascade_search.py:222-231`; path C graph-walk del experimento multipath) + compendio 07 (memory mode: el grafo rinde sin LLM porque la información conectada se recupera junta). **Convergencia**: las dimensiones Grafo y Retrieval propusieron esto de forma independiente — la señal más fuerte del análisis.
- **Criterio de éxito**: multi-hop r@5 > 0.667 / MRR > 0.5 con `expand_links` on vs off, sobre la suite ampliada (n≥10). Cero LLM, una query SQL extra, sub-300ms intacto por default.
- **Riesgos**: con pocos links en la DB la pata no aporta (el feature rinde cuando el agente linkea — sinergia con E1); hubs muy linkeados contaminan (cap + decay agresivo); tuning del decay sin sobreajustar a la suite; default off hasta validación para no cambiar ranking de consumers.

### T2. Filtros componibles estilo RecallFilter: temporal + topic_prefix + session (modo recall SEELE)

- **Qué construir**: en SearchQuery: `created_after`/`created_before` (epoch ms), `topic_prefix` (`topic_key LIKE ?||'%'` — las families son jerárquicas: `bug/`, `decision/`), `session_id`. Aplicados inline en `fts_query`, `vec_query` Y `list_by_filters`. Superficies: HTTP acepta `since`/`before` como ISO-8601 o relativo (`'7d'`, `'2h'`) parseado por un `normalize_time` determinístico en seele-core — función pura, **estricta** (rechaza ambigüedad, no adivina); MCP suma params al schema; CLI gana `--since/--before/--topic/--session`. **Correcciones del verificador**: (a) la superficie de LISTADO ya filtra `topic_key` exacto + `session_id` (`ListQuery`, `seele_list`) — reusar/integrar ListQuery para no duplicar superficie; la novedad real es el filtro **temporal** (inexistente en todo el stack), el **prefijo** de topic y la **composición con búsqueda de texto**; (b) `topic_key` solo está en el índice compuesto `(project, scope, topic_key)` — un prefijo standalone es full-scan: agregar índice dedicado (migración trivial) si se quiere performante en DBs grandes. Bonus estructural: query vacía + filtros ya funciona sin embedder (`engine.rs:143`) — con esto SEELE gana el equivalente exacto del modo recall de GRAIL, 0 LLM, 0 embeddings.
- **Por qué GRAIL lo valida**: compendio 06 — RecallFilter como "cláusula WHERE componible" de 7 campos AND con tiempos relativos determinísticos (`recall_filter.py:55-79`) y el modo recall con `llm_calls=0` garantizado. Precisión del verificador: `apply_to_artifacts` pre-filtra local/cascade/document; global y recall lo consumen con lógica propia — el punto conceptual (filtros componibles desde la CLI) se sostiene.
- **Criterio de éxito**: "qué guardé esta semana sobre bugs de X" expresable en las 3 superficies — EL caso de uso de memoria agéntica que hoy no se puede expresar. Property/unit tests del parser de tiempos.
- **Riesgos**: 3 superficies + parser = superficie de tests amplia; filtros muy selectivos pueden vaciar la pata vec (KNN k=50 filtra post-distancia) — documentar que k aplica antes del filtro temporal en vec0.

### T3. Ablation matrix: fts-only / vec-only / hybrid head-to-head

- **Qué construir**: `pub enum SearchMode { Hybrid (default), FtsOnly, VecOnly }` en SearchQuery (`fts_query`/`vec_query` ya existen separadas — cambio chico), con `#[serde(default)]` y **sin exponer en HTTP/MCP** en esta fase. En seele-eval: `run_suite_matrix()` con UN solo ingest y las queries corridas 3 veces; `SuiteReport.variants: BTreeMap<String, Metrics>` por categoría. CLI: `seele eval --suite coding-memory --ablation` con tabla categoría × variante + columna delta (hybrid − mejor pata sola). Capturar `baseline-v0.3.json` con las 3 variantes.
- **Por qué GRAIL lo valida**: compendio 15 — la lección más incómoda de GRAIL: cuando arreglaron el baseline (mismo chunker/embeddings/LLM, única variable la capa de grafo), el RAG naive EMPATÓ o GANÓ (run 2026-05-26, −0.02) a todos los modos no-agente — y archivaron el resultado en vez de borrarlo. SEELE tiene la misma pregunta sin responder: ¿el RRF híbrido le gana a vec-only, dado el phrase-quoting de `escape_fts`? Si vec-only ya empata en paraphrase, el problema es el embedder (A2), no la fusión (A1).
- **Criterio de éxito**: atribución por pata, alimentando el gate A1/A2 con datos en vez de intuición; cuantifica exactamente cuán muerta está la pata FTS en paraphrase.
- **Riesgos**: conclusiones sobreajustadas a suites chicas — emparejar con Q7; regenerar baseline requiere ONNX real (corrida manual, como hoy).

### T4. `seele reindex` + ledger de operaciones batch (`op_runs`)

- **Qué construir**: subcomando `seele reindex [--missing-only|--all|--model <id>] [--project <p>] [--dry-run]`: selecciona observations activas sin fila en `observations_vec` (LEFT JOIN por `int_id`) o con `embeddings_meta.model_id` distinto del embedder vigente, re-embebe en batches y escribe vector + meta (sinergia Q5). Arregla el hueco real de hoy: **todo lo que entra por sync import o engram import es invisible para la pata vec del RRF** (`save_raw_in_tx` no embebe, `--re-embed` es no-op reconocido) y no hay forma de repararlo. Migration V003: `op_runs(id ULID, op, started_at, finished_at, ok, seele_version, embedder_model_id, embedder_dim, args JSON, counts JSON)` — el manifest de GRAIL pero relacional y barato; reindex, sync import y engram import escriben su fila **DENTRO de la misma transacción** que la operación (patrón ya probado en `sync_chunks`). Los ImportReport que hoy se imprimen y se pierden pasan a persistirse en `counts`. `seele doctor` muestra el último run por tipo.
- **Por qué GRAIL lo valida**: compendio 04 — run manifests con `operations[]`, snapshot de config, y persistencia **incluso en aborts** (`ok: False`, `core.py:311-326`): el fracaso también deja rastro. Y la lección negativa: la deriva #4 de GRAIL (docstring promete que append/edit/delete actualizan el manifest, solo `index()` lo hace) enseña que un ledger fuera del boundary transaccional de la operación, deriva — por eso acá va en la misma tx.
- **Criterio de éxito**: tras `engram import` + `reindex --missing-only`, 0 observaciones activas sin vector; una fila `op_runs` auditable por corrida que responde "cuándo se importó esto, con qué versión, cuántas filas quedaron sin embedding".
- **Riesgos**: re-embed lento en DBs grandes con ONNX local (batches + progreso + `--dry-run`); definir si corre con el server activo (WAL lo permite — documentar); crecimiento de `op_runs` (filas chicas; prune opcional).

### T5. `seele probe`: el session_start de SEELE en un solo comando

- **Qué construir**: `commands/probe.rs` componiendo los métodos REALES del service (corrección del verificador: no existen `SeeleService::doctor`/`get_stats`; la lógica doctor vive duplicada en CLI y MCP): `stats()` (service.rs:396), `list_projects()` (:229), `embedder_info()` y `list_pending_conflicts()` (:384). Un solo JSON con envelope Q2: version, db path + schema version, embedder efectivo real (onnx|fake, model, dim), counts, projects, conflictos pendientes, y `next_steps[]` directivos ("0 observaciones: proponé el primer save"; "N conflictos pendientes: revisá con seele_judge"; "DB no existe: se crea en el primer save"). No inicializar el runtime ONNX completo solo para reportar (caro en first-run): reportar config + cache presente.
- **Por qué GRAIL lo valida**: compendio 13 — `session_start.py` como probe único que subsume `setup.sh` + `list_grail_projects.py` (+ stats); GRAIL lo consolidó al descubrir que el agente desperdiciaba turnos probeando por partes.
- **Criterio de éxito**: 1 invocación reemplaza 3 (doctor+stats+projects) para un agente; primera llamada del skill E10.
- **Riesgos**: mínimo.

### T6. Docs-site navegable: Starlight sobre el Astro existente + link-check en CI

- **Qué construir**: integración `@astrojs/starlight` bajo `/docs` en el `web/` existente (NO migrar de framework), con content collection apuntando a `../docs` — los .md siguen siendo fuente de verdad única: una sola edición actualiza GitHub y el sitio. Contenido: quickstart, INSTALLATION, AGENT-SETUP, ENGRAM-MIGRATION, página de arquitectura (con el mermaid de Q11) y la comparison. Links rotos fallan el build (default de Starlight). De paso: corregir `web/README.md` (dice Astro 5.x, es 6.x) y nav "Docs" en Header.astro.
- **Por qué GRAIL lo valida**: compendio 16 — la separación de dos niveles (interno vs sitio público con search y sidebar) y el CI que eleva broken links a fallo de build. PERO invertida en cultura: GRAIL declara "may drift" para sus docs internos y el compendio le encontró 10+ derivas reales; SEELE adopta la separación SIN la licencia para derivar (misma fuente .md para ambos niveles).
- **Criterio de éxito**: guías navegables con search (Pagefind built-in); build falla con link roto.
- **Riesgos**: theme clash con el branding de la landing (subruta + override); disciplina de frontmatter (`title` obligatorio — validable por script en CI).

### T7. Funnel de contribución right-sized

- **Qué construir**: (1) CONTRIBUTING.md con los gates que CLAUDE.md ya documenta (tests, clippy -D warnings, fmt, STELE residual), convenciones, requisito de CHANGELOG, y nota de que AEGIS aplica solo a maintainers. (2) `.github/ISSUE_TEMPLATE/` con 4 forms YAML + `config.yml` (`blank_issues_enabled: false`): bug-report (campo obligatorio: output de `seele doctor` + superficie afectada), **agent-integration** (el killer: especifica el contrato — path del config del agente, shape `mcpServers`, `install_mcp_json`, outcomes, test E2E — espejo del template 06-vector-store de GRAIL que exige "subclase con los 7 métodos"), feature-request, docs-drift (SEELE ya trata la deriva como bug — darle canal). (3) PR template con checklist único. (4) **Corrección del verificador**: el repo tiene CERO issues hoy — el entregable incluye **abrir** los 5 issues de agent-integration (opencode/aider/cody/continue/zed) con label `good-first-issue`, no asumir que existen.
- **Por qué GRAIL lo valida**: compendio 14 §7-9 — el funnel estructurado le produjo su primer PR externo a días de publicarse. Sin el gating issue-first estricto (ver §6: eso espanta a los primeros contributors de un repo con 0 externos).
- **Criterio de éxito**: 5 issues abiertos e invitantes; blank issues off; primer PR externo como métrica aspiracional.
- **Riesgos**: templates sin tráfico son cosmética — el impacto depende de Q1 (nadie contribuye a lo que no puede instalar).

### T8. Usage y costo honesto en POST /chat: None nunca es 0.0

- **Qué construir**: (1) `ChatProvider::complete` devuelve `CompletionOutcome {message, usage: Option<Usage>}`; parsear usage de ambos wire formats (OpenAI `prompt/completion_tokens`, Anthropic `input/output_tokens`) — hoy serde los descarta. (2) `cost.rs` en seele-chat con la política exacta de GRAIL: PRICE_BOOK mínimo verificado a mano, `estimate_cost() -> Option<f64>`, `ChatUsage {.., cost_usd: Option<f64>, cost_resolved: bool}` — **None explícito cuando no hay tarifa, jamás 0.0 fingiendo gratis**; tarifas extra vía `~/.seele/chat-pricing.toml`. (3) `ChatResponse.usage` + OpenAPI. (4) ChatPanel muestra "tokens X in / Y out — $0.0012" o "costo: no verificado para <model>". (5) Test de parsing con mock en ambos formatos (infra de Q9).
- **Por qué GRAIL lo valida**: compendio 05 — `cost.py:103-106`: la versión anterior devolvía 0.0 para "gratis" y "desconocido" por igual, "which made the ledger lie about cost"; `cost_resolved: bool`, `pricing_status()`, book chico de 8 modelos + `extra_pricing` del usuario. SEELE tiene la auditabilidad como marca (cost-ledger AEGIS, baseline auditable) pero su única superficie LLM descarta los tokens — ni siquiera puede decir "no sé cuánto costó".
- **Criterio de éxito**: cada respuesta de chat reporta tokens reales; modelos sin tarifa → `cost_resolved: false` explícito.
- **Riesgos**: mantenimiento del book (chico por política: todo lo demás es Undefined honesto); cambio de firma del trait (hoy solo seele-http consume).

---

## 5. Horizonte 3 — v0.4+ estructurales

Features que requieren schema nuevo, crates nuevos, o que dependen de que H1/H2 hayan sembrado el terreno (links en uso, envelope CLI, releases funcionando).

### E1. Entidades livianas como observaciones tipadas + link `mentions` (v0.4)

- **Qué construir**: (1) variante `Entity` en `ObservationType`. **Corrección del verificador**: es data-compatible (el TEXT en SQLite y el wire serde no cambian: `'entity'` hoy parsea a `Other("entity")`, mañana a `Entity`) pero **API-breaking** sin `#[non_exhaustive]` — agregar `#[non_exhaustive]` al enum en el mismo cambio y documentar el bump en CHANGELOG (obligatorio por convención del repo; MNEMA es consumer declarado de seele-core). (2) Convención `topic_key = 'entity/<slug>'` (slug determinístico NFKD lowercase + property tests): el upsert existente por `(project, scope, topic_key)` da gratis el merge-by-name de GRAIL — re-guardar la misma entidad actualiza descripción + revision_count++. (3) link_type `mentions` sumado a las convenciones de `seele-core/src/link.rs`. (4) Tool MCP `seele_entity_upsert` que compone save (type=entity, topic_key por slugify) + links mentions, devolviendo insert|upsert. Las entidades quedan indexadas gratis en FTS+vec (son observaciones) y, con T1, actúan como **hubs**: query → hit entidad → expand_links trae todo lo que la menciona = el patrón multi-hop de GRAIL sin extractor LLM.
- **Por qué GRAIL lo valida**: compendio 07 — el memory mode ABANDONA el extractor LLM y hace que el agente aporte las entidades (merge determinístico en `_merge.py`); es la prueba de que la capa de entidades funciona 100% sin LLM.
- **Criterio de éxito**: queries multi-hop vía hub-entidad mejoran en la suite v2; AGENT-SETUP documenta la disciplina del agente consumidor.
- **Riesgos**: depende de la disciplina del agente (GRAIL lo resuelve con skills — sinergia E10); sin Q6, alias-explosion ("SQLite" vs "sqlite3").

### E2. `seele merge`: merge determinístico con reglas por campo (v0.4)

- **Qué construir**: `ObservationStore::merge_into(tx, source_id, target_id)` con reglas GRAIL-style adaptadas: target conserva su content/title; `duplicate_count` = suma + 1; `last_seen_at` = max; `revision_count` = max; metadata = unión con target ganando en conflicto; links y memory_relations del source reapuntados al target con dedup previo por `(from_id, to_id, link_type)` y drop de self-links resultantes (mismo patrón que seele-engram-import); source soft-deleteado con breadcrumb `metadata.merged_into=<ulid>` (reversible vía restore); embedding del target re-embebido best-effort fuera de la tx. Todo en UNA transacción. Superficies: `merge_observations` en SeeleService, tool `seele_merge {source_id, target_id, dry_run}`, CLI `seele merge <src> <dst> [--dry-run]`, HTTP `POST /memories/{id}/merge-into/{target}`.
- **Por qué GRAIL lo valida**: compendio 07 — el apply de `merge_aliases` (`project.py:1602-1681`): unión de referencias, reescritura de endpoints, drop de self-loops, dedup, invalidación de embedding. Nota: la regla de GRAIL "el incoming pisa la descripción" NO se adopta (ver §7) — en SEELE multi-vía, el canónico conserva.
- **Criterio de éxito**: property tests del reapuntado de aristas (self-link drop, dedup post-rewrite), como ya hace seele-storage; merge reversible vía restore.
- **Riesgos**: casos borde de aristas; el topic_key del source se libera vía soft-delete (el upsert ya filtra `deleted_at IS NULL`).

### E3. `seele consolidate`: ciclo periódico con proposals revisables en SQLite (v0.4)

- **Qué construir**: migration con tabla `merge_proposals`: id ULID, kind (`merge_near_duplicate`|`merge_topic_alias`|`stale_hash_duplicate`), source/target FK, detector, confidence, rationale, evidence, status pending/accepted/rejected con resolved_at/reason, `UNIQUE(source_id, target_id, kind)` para idempotencia entre corridas. Pasada **read-only** (como `consolidate()` de GRAIL, que se rehúsa a mutar): (a) near-dups por vectores YA guardados en `observations_vec` mismo project+scope — requiere `ObservationStore::get_embedding(id)` por `int_id`, hoy inexistente; umbral coseno 0.93 **recomputado en Rust** sobre el vector leído (o calibrado en L2 — misma corrección de unidades que Q4); (b) Jaro-Winkler ≥ 0.92 entre títulos de mismo `(project, scope)` con topic_key distinto (alias de topic keys); (c) `normalized_hash` idéntico FUERA de la ventana de 24h. Pisos de confianza por kind y umbrales **en config — como ya hace GRAIL en `MemoryConfig`** (corrección del verificador: GRAIL no los hardcodea; es el ejemplo a seguir, no el contraejemplo). Guard de corpus mínimo (~50 activas). Accept aplica `merge_into` (E2) en la misma tx que marca accepted; si el apply falla, status **vuelve a pending** con reason — el patrón crash-safe de `project.py:1452-1458`. Superficies: CLI `seele consolidate [--json]`, MCP `seele_proposals_list/accept/reject`, HTTP `GET /proposals` + `PUT /proposals/{id}/accept|reject`. Scan por KNN por fila (top-5 vecinos), no O(n²).
- **Por qué GRAIL lo valida**: compendio 07 — el ciclo proposal→accept/reject→apply completo, determinístico y crash-safe, con modelo de datos serio (`proposals.py:48-119`). La feature de GRAIL MÁS compatible con el core local-first: SEELE ya tiene el 70% (lifecycle de memory_relations = patrón proposal, embeddings computados, KNN en producción).
- **Criterio de éxito**: sobre DB sembrada con duplicados conocidos, proposals correctas con CERO auto-aplicación; cadena accept→merge auditable. Higiene de corpus = ataque estructural a paraphrase (dos filas casi idénticas parten la masa de rank del RRF).
- **Riesgos**: proposal-fatigue si los umbrales generan ruido (empezar 0.93+); filas sin vector quedan fuera de la señal (a) hasta T4 — el detector reporta cuántas excluyó; depende de E2 y T4.

### E4. Historial append-only de revisiones [fusión Consolidación + Provenance] (v0.4)

- **Qué construir**: tabla `observation_history` append-only (id autoincrement, observation_id FK ON DELETE CASCADE, event `revision`|`merged_into`|`merged_from`|`judged`, prev_title/content/metadata/type, actor, session_id, created_at), poblada dentro de `save_in_tx` en el branch upsert **ANTES** del UPDATE que pisa (observations.rs:470-497), en `merge_into` (E2) y en accept/reject de proposals (E3). **Diagnóstico corregido por el verificador**: el upsert PISA metadata/type/tool_name viejos con los nuevos y descarta solo el session_id nuevo; el dedup descarta metadata/session del save nuevo — pérdidas en direcciones opuestas, ambas cubiertas por el history. Retención: cap configurable (default 10 revisiones por observación) podado en la misma tx + `seele doctor --prune-history`. Lectura: `seele show <id> --history`, `GET /memories/{id}/revisions`, campo `history_count` en `seele_show` MCP. De paso: extender `JudgmentInput` con actor para que `judge()` registre quién juzgó. Las revisiones NO se indexan en FTS ni vec (solo la fila viva participa del retrieval); guardan contenido post-strip (`<private>` ya removido en el paso 1 de save_in_tx); NO viajan en sync chunks v1 (decidir local-only en ADR, como el ledger `sync_chunks`).
- **Por qué GRAIL lo valida**: compendio 07 (`_history.jsonl` append-only, `project.py:204-211`) + compendio 04 ("los runs viejos quedan para diff/rollback") — traducido al grano correcto de SEELE: la observación, no el run. Habilita undo manual y auditoría de cómo evolucionó `decision/auth` rev 1→4 — justo el caso de uso de un memory engine para agentes.
- **Criterio de éxito**: contenido de cualquier revisión del upsert recuperable; cero filas history en índices de retrieval.
- **Riesgos**: crecimiento de DB con contenidos largos (cap + prune + el dedup 24h ya absorbe re-saves idénticos).

### E5. Provenance de actor en observations (v0.4)

- **Qué construir**: migration `ALTER TABLE observations ADD COLUMN saved_by_actor TEXT, saved_by_kind TEXT` (valores `agent|tool|human|import|sync`, nullable para compat) — espejo deliberado del naming `marked_by_*` de memory_relations, cerrando la asimetría de auditabilidad sin justificación de diseño (las relations registran quién, las observations no). Campos en `SaveObservationInput`; HTTP acepta actor opcional; CLI `--actor`/`SEELE_ACTOR`; sync y engram-import estampan `sync-import`/`engram-import`. MCP: default = `clientInfo.name` del initialize — **corrección del verificador**: el server HOY descarta los params del initialize por completo (`server.rs:81-88` responde sin leer `req.params`); hay que parsear `clientInfo` y plumbearlo al contexto de los tool handlers (estado por conexión) — trabajo contabilizado en el M. Semántica del upsert: preservar el actor original, registrar el último en la revisión (sinergia E4). En el branch dedup, registrar el último actor visto en vez de descartar todo rastro.
- **Por qué GRAIL lo valida**: compendio 03/04 — provenance con semántica de propagación explícita y consistente; en el mundo multi-consumer que ADR-13 habilitó (Claude Code + Cursor + MNEMA sobre la misma DB), "qué agente escribió esta memoria" debe ser respondible.
- **Criterio de éxito**: provenance consultable en las 4 superficies; la suite completa del workspace verde tras tocar save_in_tx.
- **Riesgos**: el actor es self-reported (un cliente MCP puede mentir) — documentarlo como provenance, NO como seguridad.

### E6. Registry único de providers de chat servido al frontend (v0.4)

- **Qué construir**: `seele-chat/src/providers.rs`: `ProviderEndpoint {name, chat_url, default_model, wire: WireFormat::{OpenAICompat, Anthropic}}` + `BUILTIN_PROVIDERS` + `fn resolve(...) -> Box<dyn ChatProvider>` reemplazando el string match de `handlers.rs:304`. **Alcance corregido por el verificador**: la deuda real es `default_model_for` **triplicada** (handlers.rs:378-389, serve.rs:95-106, ChatPanel.astro:166-174) + `default_endpoint_for` en copia única (handlers.rs:391-401) — no "ambas triplicadas". `GET /chat/info` devuelve el catálogo `{name, default_model, requires_endpoint}` y el dropdown del panel se arma desde ahí. Opcional: providers custom vía `~/.seele/chat-providers.toml` mergeados sobre built-ins (útil para Ollama/vLLM/LM Studio locales).
- **Por qué GRAIL lo valida**: compendio 05 — registry de 11 endpoints como dataclass frozen + merge profundo de `endpoints.yaml` del usuario (`providers.py:36-44`, `config.py:472-485`): cero code paths por vendor.
- **Criterio de éxito**: agregar un provider = tocar 1 lugar; drift user-facing eliminado.
- **Riesgos**: cambio aditivo del shape de `/chat/info` (bajo); el TOML es superficie de config a documentar — opcional y merge-sobre-defaults.

### E7. Instructions MCP + descripciones de tools overridables: el quality lever de un memory server (v0.4)

- **Qué construir**: (a) campo `instructions` en el resultado del initialize MCP (existe en la spec 2024-11-05) con un builtin que enseñe CUÁNDO guardar (decisiones, bugs resueltos, learnings al cierre) y cuándo buscar antes de responder — hoy el agente solo ve 19 descripciones de una línea sin guía de comportamiento. (b) Descripciones overridables: cambiar `Tool.description` (**corrección del verificador**: el struct es `Tool`, `tools.rs:65`, no `ToolDef`; también su reflejo `ToolDescriptor.description`, `:73`) de `&'static str` a `Cow<'static, str>`, con overrides desde `~/.seele/prompts.toml` + `.seele/prompts.toml` (secciones `[mcp] instructions` y `[mcp.tools] seele_save = "..."`), compuesto con `--tool-prefix` (el override aplica al nombre canónico, el rename después — misma mentalidad que la tabla RENAMES de ADR-13). Un consumer como MNEMA pasa de renombrar tools a **re-prompterlas** para su dominio sin forkear.
- **Por qué GRAIL lo valida**: compendio 08 — el README de GRAIL declara los prompts "the biggest quality lever". Para un memory server MCP, las descripciones de tools + instructions son el equivalente funcional exacto: determinan si el agente guarda memorias de calidad y si busca antes de responder — impacto directo en la calidad de la memoria acumulada.
- **Criterio de éxito**: snapshot tests de `tools/list` con y sin overrides; E2E stdio con prompts.toml en HOME fake.
- **Riesgos**: descripciones de usuario kilométricas inflan el contexto de cada cliente (cap de longitud + warn en doctor + introspección E9); cache de tools/list en clientes (documentar reconexión).

### E8. Registry de prompts versionado para superficies LLM-opt-in (v0.4)

- **Qué construir**: `seele_core::prompts` compartiendo el loader TOML de Q10/E7: `PromptDef {name, version, required_params, template}` con sustitución plana `{param}` (sin lógica, a propósito — datos, no código). Dos decisiones correctas de GRAIL adoptadas: **validación AL CARGAR** (params requeridos ausentes en el template = error tipado al boot del serve, no 500 a mitad de request) y resolución per-request > proyecto > usuario > builtin. Builtin inicial: `chat.system` v1 (mover la constante de `seele-chat/lib.rs:104`). `ChatResponse` gana `prompt_name/version/source`; `GET /chat/info` reporta el prompt resuelto. Cuando lleguen rerank A1 (si es LLM-opt-in) o judge-assist, registran sus prompts acá — evita repetir el **Known Gap #1 de GRAIL** (corrección del verificador: el `AGENT_SYSTEM_PROMPT` inline está documentado en §5.3 del compendio como Known Gap, no entre las 8 derivas numeradas).
- **Por qué GRAIL lo valida**: compendio 08 — contrato runtime-checkeable validado al cargar (`loader.py:47-65`), resolución custom→builtin con cacheo.
- **Criterio de éxito**: prompt resuelto visible y trazable; boot falla tipado ante template inválido.
- **Riesgos**: over-engineering si nunca llegan más prompts LLM — mitigado: el loader es compartido con Q10/E7, el costo marginal del registry es chico.

### E9. `seele prompts list/show` + provenance de prompt en eval (v0.4)

- **Qué construir**: subcomando `seele prompts list` (nombre, versión, source builtin/user/project, params, superficie mcp/chat/families) y `seele prompts show <name> [--sample]` que renderiza con datos de muestra **pasando por la validación** de required_params — no esquivarla: la deriva 3 (de 8) del compendio 08 de GRAIL documenta exactamente ese bug (`cli/main.py:1013-1014` llama `build_messages` directo, enmascarando errores). Cubre los tres planos: prompts LLM (chat.system), textos de agente (instructions + 19 descripciones efectivas post-override) y heurísticas (familias resueltas). Además: cuando seele-eval incorpore un paso dependiente de prompt (rerank A1), el `_meta` del baseline registra prompt name+version+sha256 — paridad con la provenance que `embeddings_meta` ya da para el modelo.
- **Por qué GRAIL lo valida**: compendio 08 — `grail prompt list/show` como introspección (con su bug como lección de qué no repetir).
- **Criterio de éxito**: poder listar qué textos efectivos está usando el sistema — hoy imposible.
- **Riesgos**: depende de Q10/E7/E8 (sin registry no hay qué listar); secuenciar en el mismo sprint.

### E10. Skill SEELE portable embebido en el binario (v0.4)

- **Qué construir**: carpeta `skills/seele/` en el repo: SKILL.md (triggers: remember/recall/save this/"what did I"; conductas: correr `seele probe --json` una vez por sesión y cachear; buscar con `seele search --json` antes de responder preguntas que podrían vivir en memoria y citar ids; proponer save una sola vez para contenido save-worthy con topic_key sugerido; documentar `<private>`; tabla "cuándo NO usar SEELE") + `references/` (cli-reference.md con shapes del envelope Q2, topic-families.md, sync-workflow.md) + sidecar `agents/openai.yaml` para Codex. **CERO capa de scripts**: a diferencia de GRAIL (que necesitó 21 wrappers Python porque su producto es una librería), el binario estático con `--json` YA ES la capa de scripts — el skill solo documenta invocaciones. En seele-setup: `SkillInstall` embebiendo los archivos vía `include_str!`, instalación idempotente en `~/.claude/skills/seele/` y `~/.agents/skills/seele/`, reusando outcomes Created/Updated/Unchanged + backup + write atómico existentes; CLI `seele setup --skill [--dir <path>]`.
- **Por qué GRAIL lo valida**: compendio 13 — los tres contratos (datos/sesión/conducta) y la portabilidad real: la misma carpeta en Claude Code, Codex y Hermes. La vía skill escala O(1) por agente; el wizard MCP escala O(n) writers de config bespoke.
- **Criterio de éxito**: skill funcional en Claude Code y Codex con la misma carpeta; test en seele-cli que valida vía introspección de clap que cada subcomando documentado en el skill existe (patrón `openapi_consistency.rs` ya existe para HTTP). Depende de Q1 (binario instalable) y Q2 (envelope contract-grade).
- **Riesgos**: drift SKILL.md vs superficie clap (test lo mitiga); sin release pipeline arreglado, la fricción cae en `cargo install --git` — por eso Q1 va primero.

### E11. Retirar los 5 skeletons: NotImplemented → vía skill/instrucciones per-agent (v0.4)

- **Qué construir**: en `seele-setup/src/agents.rs`, los brazos opencode/aider/cody/continue/zed dejan de devolver `SetupError::NotImplemented` (`agents.rs:97`). `seele setup --agent <x>` emite el snippet de instrucciones listo para pegar en el archivo de convenciones de cada agente (OpenCode: AGENTS.md o skill directo; aider: CONVENTIONS.md; Continue: rules; Cody/Zed: instrucciones + nota MCP manual), instruyendo `seele --json` + `seele probe`. Outcome nuevo `InstructionsEmitted`; `--dry-run` muestra el snippet. Cierra el backlog completo de skeletons de CLAUDE.md sin escribir 5 mergers de config JSON bespoke — y varios de esos agentes (aider, cody) ni hablan MCP fluido, pero todos ejecutan shell.
- **Por qué GRAIL lo valida**: compendio 13 — la distribución por skill/instrucciones llega a cualquier agente que corra un comando; el único propósito de los skeletons era plomería de config per-agent.
- **Criterio de éxito**: `seele setup --agent aider` produce output útil hoy mismo; los 5 issues de T7 quedan como vía alternativa para quien quiera el writer MCP bespoke.
- **Riesgos**: expectativa de usuario — `setup --agent aider` ya no "instala", imprime instrucciones: comunicarlo claro en el output y en AGENT-SETUP.md.

### E12. GraphSnapshot API: dump + degree + componentes conexas deterministas (v0.4, diseñado para que retrieval lo consuma)

- **Qué construir**: `SeeleService::graph_snapshot(filter: GraphFilter) -> GraphSnapshot` (la lógica vive una vez; las superficies son shims), sobre lo que YA existe: `LinkStore::list` sin filtros, RelationStore, observations activas. Computa: degree por nodo y componentes conexas vía **union-find** (~80 líneas, sin deps — el análogo determinista de las comunidades Leiden). `GraphFilter`: project, linked_only, max_nodes con política de sampling propia de SEELE: **degree desc + last_seen_at desc, tie-break por id** (no top-N-by-degree puro: corrección del verificador — en GRAIL las entidades degree-0 son caso borde; en SEELE los links son opt-in y la mayoría de las observations tendrá degree 0 — el contraste es cuantitativo, no categórico) + truncation meta.
- **Por qué GRAIL lo valida**: compendio 12 — builder/sampling con subgrafo inducido determinista y mergesort para tie-breaks.
- **Criterio de éxito**: tests de determinismo del sampling + components correctos; consumido por E13 Y por la expansión de retrieval (T1 puede migrar a usarlo) — es la infraestructura compartida viz/retrieval.
- **Riesgos**: impacto alto condicionado a que retrieval lo consuma (si queda solo backend de viz, es medio); resistir scope-creep — snapshot plano + components, nada más.

### E13. Crate `seele-viz` + comando `seele viz` — HTML standalone offline (v0.4)

- **Qué construir**: crate #15 espejando la separación builder/exporter/sampling/template de GRAIL pero en Rust puro: payload.rs, sampling.rs, template.rs. Render: template.html + renderer.js vanilla (sin toolchain TS) + `d3.v7.min.js` VENDORIZADO (licencia ISC, entrada en CREDITS.md — mismo precedente que sqlite-vec), embebidos con `include_str!` — el HTML abre offline sin CDN, fiel a local-first. Modelo de payload SEELE: nodos = observations activas + nodos sintéticos `project:{name}`; edges = links (label=link_type, colores fijos para las 6 convenciones + hash_color determinista para tipos libres) + memory_relations con estilo por judgment_status (**conflicts_with pending en rojo punteado — los conflictos pendientes se VEN, feature que GRAIL no tiene**) + edge sintético observation→project. Doble color precomputado (projectColor/typeColor) para toggle instantáneo; seed determinista (randomLcg); aislados en anillo (isolatedRadius). Mostrar la truncación en el header — **corrección del verificador: GRAIL emite `meta.truncation` en el payload pero NINGÚN consumidor lo renderiza; SEELE mostrándola va un paso más allá, no copia**. CLI: `Command::Viz` + flags `--output/--project/--max-nodes/--linked-only/--open`. Presupuesto realista del renderer: el equivalente de GRAIL pesa ~1.5k líneas de TS — apuntar a <800 líneas vanilla y feature-gatear `viz` cuando aterrice el feature-gating de ADR-15 (binario hoy 42.55 MB medidos).
- **Por qué GRAIL lo valida**: compendio 12 — el HTML standalone offline es el artefacto más demo-able del proyecto (`examples/quickstart/graph.html`, ~1.07 MB, 529 entidades, pieza de marketing viviente).
- **Criterio de éxito**: golden test del payload contra DB seedeada + test estructural del HTML (payload re-parseable, html-escape) + fixture.json/dev.html para iterar sin DB; abre offline con doble click.
- **Riesgos**: d3 (~280 KB) infla el binario (feature-gate); renderer a mano sin type-check (mantener chico + harness fixture); grafo sparse puede verse vacío (anillo de aislados por default).

### E14. GET /graph en HTTP + página /graph en el sitio Astro (v0.4)

- **Qué construir**: handler `GET /graph` (bearer + OpenAPI, params project/max_nodes/linked_only) devolviendo el payload de E12; `web/src/pages/graph.astro` con el mismo renderer.js + d3 (fuente única en seele-viz), fetcheando el serve local con el patrón de discovery de ChatPanel (probing + estado disabled).
- **Por qué GRAIL lo valida**: compendio 12 — "un renderer, dos superficies" (el bundle compartido entre HTML standalone y la chat app).
- **Criterio de éxito**: payload idéntico CLI/HTTP; la página /observability gana un hermano visual.
- **Riesgos**: CORS Any/Any/Any es backlog reconocido (documentar uso local); no empezar por acá — sin el comando CLI primero, es una página que casi nadie ve. Depende de E12/E13.

### E15. Demo vendible: graph.html publicado en Pages + screenshot en README (inmediatamente post-E13)

- **Qué construir**: graph.html generado desde una DB seedeada con el corpus de coding-memory (22 observations embebidas, realistas, cero datos privados) + links sintéticos; publicado como `web/public/demo/graph.html` (deploy-web.yml ya es el único pipeline de distribución verde); screenshot + link en README. Regeneración: paso opcional en deploy-web.yml o manual versionada con el tag.
- **Por qué GRAIL lo valida**: compendio 12 — el quickstart graph.html como "prueba viviente" y único artefacto "probá sin instalar".
- **Criterio de éxito**: clickeable desde README, marcado con la versión que lo generó.
- **Riesgos**: el README ya promete cosas rotas (install scripts) — publicar SOLO cuando `seele viz` esté mergeado.

### E16. Eval-judge opt-in sobre el camino chat-with-DB (v0.4)

- **Qué construir**: `gold_answer: Option<String>` y `gold_keyphrases: Option<Vec<String>>` en EvalQuery. Dos jueces: (a) **determinístico keyword-judge** (default, CI-able, gratis): hits de gold_keyphrases en el contenido top-k recuperado; (b) **LLM-judge opt-in** detrás de flag explícito — **nombrarlo `--eval-judge` para evitar la colisión con la tool MCP `seele_judge`** (corrección del verificador: esa tool existe pero es judgment de conflictos entre memory_relations, semántica completamente distinta): corre cada query por `run_chat` de seele-chat (tool seele_search, presupuesto 5 iteraciones) y puntúa contra gold_answer con la rúbrica portada de GRAIL — pesos correctness 0.35 / completeness 0.25 / source_grounding 0.15 / no_hallucination 0.15 / coherence 0.10; precisión del verificador: `judge_prompt.py` define la rúbrica, el runner (`run_benchmark.py::judge_responses`) impone temperature 0 y `response_format json_object`. El lado LLM vive en seele-cli (ya async); seele-eval sigue sync y sin deps de red. El reporte registra judge_model/provider/costo.
- **Por qué GRAIL lo valida**: compendio 15 — rúbrica explícita ponderada con JSON estricto; y sus derivas #2/#3 (números oficiales sin judge_scores.json detrás) como anti-patrón a no repetir.
- **Criterio de éxito**: primer test end-to-end del único camino LLM de SEELE (hoy cero tests); keyword-judge entra a CI; el LLM-judge **NUNCA** gatea baselines ni CI.
- **Riesgos**: no-determinismo/costo del juez LLM (por eso nunca gate); destapará bugs de seele-chat (feature, no bug — pero infla el L); sesgo hacia respuestas verbosas (la rúbrica mitiga, no elimina); filosófico si dejara de ser opt-in — queda detrás de `--eval-judge` explícito.

### E17. Página /benchmarks en el sitio: SOLO artefactos del harness (v0.4)

- **Qué construir**: (1) convención de archivo de runs: `seele eval --ablation --json > docs/eval/runs/<fecha>-<commit>.json` commiteado como artefacto canónico con `_meta {model_id, commit, fecha, rrf_k}` — preservando también los runs malos, como el archive de GRAIL; (2) `web/src/pages/benchmarks.astro` importa el JSON más reciente en build-time: tabla categoría × variante + nota de metodología (suites chicas e indicativas, recall binario, qué aísla cada categoría) + link al JSON crudo; (3) regla escrita en devlog/ADR: **ningún número entra a la página que no salga byte-a-byte de un JSON emitido por el harness**.
- **Por qué GRAIL lo valida**: compendio 15 — por la positiva (publicar resultados como activo de adopción, archivar la cronología incluida la derrota del agente) y por la negativa (la trampa a evitar: sus números oficiales 4.80/4.14 viven solo en HTMLs ensamblados a mano, sin judge_scores.json). SEELE puede publicar números modestos con metodología impecable — eso suma más confianza que un 27-0-0.
- **Criterio de éxito**: página viva; el deploy falla ruidoso ante drift de schema JSON (validación en build de Astro).
- **Riesgos**: publicar r@5 0.667 expone debilidad — framing honesto + la serie temporal de mejora cuando A1/A2 aterricen ES la historia. Depende de T3.

### E18. `seele graph doctor`: análisis estructural con ciclo de proposals efímero (v1.0)

- **Qué construir**: subcomando `seele graph [stats|doctor]` + módulo graph.rs. Análisis 100% determinísticos: (a) componentes conexas sobre links vía union-find (reusa E12); (b) huérfanos: observaciones activas sin links ni topic_key ni relations; (c) candidatos a alias: batch de Q6 sobre pares mismo-project/mismo-type; (d) densidad y degree por project (los agrupadores naturales de SEELE — project/scope/family — juegan el rol del folder-as-community de GRAIL, sin clustering). Output: JSON de proposals `{kind, ids, confidence, rationale}` **efímero** (sin tabla nueva — la persistencia con lifecycle es E3 y es para merges de contenido; esto es higiene estructural), con pisos de confianza por kind; el humano o agente aplica con tools EXISTENTES (seele_link, seele_compare, seele_judge) — nada se auto-aplica. Guard de corpus mínimo (estilo `min_entities_for_consolidate=30` de GRAIL).
- **Por qué GRAIL lo valida**: compendio 07 — `consolidate()` con 4 análisis estructurales + accept/reject explícito, todo sin LLM.
- **Criterio de éxito**: sobre una DB con links reales (post T1/E1), reporte útil que no devuelva "todo huérfano".
- **Riesgos**: prematuro antes de T1/E1 (por eso v1.0: el valor llega cuando los agentes ya linkean y crean entidades); resistir el scope-creep hacia proposals persistidas.

---

## 6. Integración con el roadmap existente

Candidatas v0.2 listadas en CLAUDE.md, y cómo este plan las afecta:

| Candidata CLAUDE.md | Efecto del plan | Detalle |
|---------------------|-----------------|---------|
| **5 skeleton agents** (opencode/aider/cody/continue/zed) | **REEMPLAZADA** por E10 + E11 | El skill portable + instrucciones per-agent matan los 5 skeletons de un golpe: su único propósito era plomería de config MCP bespoke, y varios de esos agentes (aider, cody) ni hablan MCP fluido — pero todos ejecutan shell. El issue form de agent-integration (T7) deja la puerta abierta a contribuciones externas para quien igual quiera el writer MCP nativo. |
| **Homebrew tap** | **DESBLOQUEADA** por Q1 | La fórmula necesita assets binarios que hoy no existen (0 releases). Follow-on natural e inmediato del release pipeline reparado — ya anotado dentro de Q1. |
| **`claude mcp add` delegación** | **COMPATIBLE**, prioridad relativa baja | No hay conflicto. E7 (instructions + descripciones overridables) mejora la calidad de la integración MCP existente más que cualquier mecanismo de instalación adicional. |
| **Sync chunk splitter (~1 MB cap)** | **INDEPENDIENTE**, sinergia menor con T4 | Ningún overlap. Cuando se implemente, sus corridas de export/import quedan registradas gratis en el ledger `op_runs` de T4 — auditabilidad sin costo extra. |
| **TUI editing in-place** | **INDEPENDIENTE**, sinergia con E12 | Sin overlap. Bonus: GraphSnapshot (E12) y el trabajo de links (T1/E1) por fin habilitan la sección "Linked memories" del ADR-07 que la vista Detail de la TUI nunca renderizó. |
| **Project-detection wired en `seele save`** | **POTENCIADA** por Q4 y E5 | El project correcto automático sube el valor de los near-dups (Q4 restringe el KNN a mismo project+scope) y de la provenance de actor (E5). Recomendado secuenciarla junto a esos bloques. |

Relación con el gate v0.3-β ya declarado (A1 reranking / A2 embedder swap):

- **Q5 es prerequisito duro de A2** (el propio header de V002/ADR-14 lo declara): sin embeddings_meta escrita y sin guard de model_id, el swap multilingüe mismo-dim corrompe silenciosamente.
- **T3 (ablation) informa la decisión A1 vs A2**: si vec-only ya empata al híbrido en paraphrase, el problema es el embedder, no la fusión.
- **Q7 (suites engordadas) es prerequisito estadístico del gate**: decidir sobre n=3 en multi-hop es ruido.
- **A1 reranking NO gasta slot en este plan**: el qué ya está decidido en el roadmap SEELE (cross-encoder ONNX local, mismo stack ort+tokenizers); de GRAIL se adopta solo el patrón de integración cuando llegue — overfetch top_k×3 configurable + flag trivaluado off/on/config.

---

## 7. Lo que decidimos NO adoptar

Sección obligatoria. GRAIL hace muchas cosas bien *para su problema* (GraphRAG LLM-céntrico sobre corpora KB); estas no traducen a SEELE, y la razón importa tanto como la decisión.

### 7.1 Maquinaria de clustering y comunidades

| Qué | Por qué no |
|-----|-----------|
| **Leiden jerárquico + merge DBSCAN + comunidades persistidas** (compendio 04) | Escala y runtime equivocados: sin equivalente Rust maduro (vendorizar/bindear C contra un binario de 42.55 MB que ADR-15 quiere achicar), y el clustering paga en corpora densos de miles de entidades — el grafo de un proyecto SEELE tiene cientos de nodos ralos donde el propio GRAIL admite que las comunidades chicas producen basura. Project/scope/topic_key families ya dan el folder-as-community que el memory mode de GRAIL usa en lugar de Leiden. Union-find (E12/E18) cubre el 90% del valor con ~80 líneas. |
| **Comunidades como señal de color primaria de viz** | Requeriría el pipeline LLM completo. Sustituto determinista: color por project + componentes conexas. Se adopta la IDEA del doble color precomputado con toggle instantáneo, no la señal. |
| **Modelo de 5 node-kinds / 6 edge-kinds GraphRAG** (Document→Chunk→Entity→Community→Finding) | SEELE no tiene chunks ni communities ni findings: kinds vacíos = UI muerta. El modelo SEELE es honesto y distinto: observation + project como nodos, links tipados + relations con judgment como edges — y el conflicts_with pending visualizado es algo que GRAIL ni puede mostrar. |
| **Paleta DEFAULT_TYPE_PALETTE médica** (PERSON, DISEASE, DRUG...) | Dominio ajeno. Se adopta el mecanismo (hash_color determinista + asignación sorted estable), no el contenido. |
| **top_n_by_degree como única política de sampling** | En SEELE los links son opt-in y la mayoría de las observations tendrá degree 0 (en GRAIL es caso borde) — degeneraría a "solo lo linkeado". Política propia: degree + recencia + `--linked-only` + anillo de aislados (el isolatedRadius sí se adopta). |

### 7.2 LLM en el write path o en el core (violaciones directas de filosofía)

| Qué | Por qué no |
|-----|-----------|
| **Extracción de entidades/relaciones por LLM en el save** (compendio 03) | Una llamada LLM por save convierte el guardado local sub-milisegundo en operación con costo, latencia y API key. El propio GRAIL valida la alternativa: su memory mode ABANDONA el extractor y el agente aporta las entidades — esa variante ES la propuesta E1. |
| **Reportes narrativos de comunidad + reparación JSON en 3 pasadas** | Maquinaria 100% LLM para prosa que los consumers de SEELE (agentes con tool-use) no necesitan: leen filas crudas vía search/show. Si algún día se quiere "resumen de cluster", pertenece a seele-chat (ya opt-in), no al core. |
| **Juez LLM para dedup** (`entity_dedup._judge_batch`) | La mitad determinística (coseno + union-find + umbrales conservadores) SÍ se adopta (Q6/E3). El juez en SEELE ya existe y es mejor: el agente consumidor, vía suggest→confirm con provenance marked_by_* que registra quién decidió. |
| **Pack de prompts de indexing** (entity_relation, summarize, community_report, entity_dedup...) | El write path de SEELE es determinista por filosofía innegociable. Un juez LLM de duplicados contradice frontalmente el dedup por normalized_hash. |
| **json_correction (reparación LLM de JSON malformado)** | GRAIL parsea JSON generado por LLM en batch; en SEELE todo JSON estructurado lo genera serde. El único parseo de salida LLM (chat) ya degrada con `(tool error)` y sigue. |
| **Modo agent dentro del engine** (loop de tool-calling que elige modos) | El agent loop YA existe y vive del lado correcto: el cliente MCP ES el orquestador, y seele-chat ya implementa el tool-use loop opt-in del panel. Duplicarlo dentro del engine violaría core-sin-LLM y competiría con el consumer. |
| **Modo global map-reduce** (síntesis sobre community reports) | Requiere toda la cadena LLM-céntrica en indexado; N+1 llamadas por query, incompatible con sub-300ms. El equivalente funcional vive en seele-chat opt-in. |
| **LLM-as-judge como métrica primaria o gate de regresión** | recall@k/MRR/nDCG son determinísticos, gratis, reproducibles y CI-ables; el juez cuesta plata, no reproduce, y mide generación — que el core no hace. Solo audita el camino chat opt-in (E16). |

### 7.3 Implementaciones que serían retrocesos técnicos

| Qué | Por qué no |
|-----|-----------|
| **Cascade literal: embeber todos los chunks en query time** | GRAIL re-embebe el corpus por query porque no cachea embeddings de chunk; SEELE ya tiene todos los vectores persistidos en vec0 con KNN. Se adopta el CONCEPTO rescue (Q3, T1), no la implementación — copiar la mecánica sería un retroceso objetivo. |
| **RerankerClient HTTP remoto** (DeepInfra/Cohere) | Red + API key en el path de búsqueda rompe local-first. NO es "no hacer reranking": A1 ya está en el roadmap como cross-encoder ONNX local; de GRAIL se toma solo el patrón de integración (overfetch ×3, flag trivaluado). |
| **Grafo no dirigido con par canónico + weight promediado** | Correcto para co-ocurrencia estadística extraída por LLM; destruiría la semántica direccional de links (supersedes, derives_from) y el judgment lifecycle auditable. La dirección se maneja en query-time (T1 recorre ambos índices), no aplanando el schema. |
| **Migración lazy in-memory de schema** (`migrate_dataframe`) | Solución correcta para parquets inmutables, incorrecta para SQLite: disco en estado indeterminado, cada lector conoce los defaults. Refinery (transaccional, versionado, checksum) es estrictamente superior. Se importa solo la disciplina de documentar el porqué en el header de cada migration — que V002 ya practica. |
| **Proposals como YAML on-disk + audit log en archivos** (`_history.jsonl`) | GRAIL es file-first; SEELE es SQLite-first con 4 superficies leyendo la misma DB. Archivos bifurcarían la fuente de verdad y harían imposible el accept+apply atómico en una tx. Tabla SQL da lo mismo con mejor integridad (E3/E4). |
| **Scripts bash revisables para applies destructivos** + estado accepted-pending-manual | En GRAIL el apply mueve archivos en un working tree; en SEELE toda operación destructiva es una tx SQLite con soft-delete reversible. El equivalente correcto es `--dry-run` + soft-delete + breadcrumb — y shell scripts serían un problema de portabilidad en Windows, target first-class. |
| **Sobrescritura de description por el incoming en merges** ("el agente es dueño") | El write path de GRAIL es 100% agéntico con autoridad única; el save de SEELE llega por múltiples vías (CLI, MCP multi-agente, sync) sin autoridad única. Regla SEELE inversa: el canónico conserva, el merge preserva — y el reemplazo explícito ya existe (upsert por topic_key), ahora con red (E4). |
| **Carpetas de run completas con snapshot de artefactos** + current.json | GRAIL puede: su estado son parquets inmutables. El estado de SEELE es UNA DB viva; snapshotear por operación multiplica footprint y rompe la fuente única. `op_runs` relacional (T4) captura la auditabilidad a costo marginal. |
| **llm_calls.jsonl / ledger de costos LLM como manifest core** | El pipeline de GRAIL ES llamadas LLM; el core de SEELE no llama LLM — un ledger ahí no tiene nada que registrar. Si seele-chat algún día persiste conversaciones, será feature de seele-chat. |
| **Provenance agregativa observed_at=max / confidence=min** | GRAIL la necesita para agregados multi-fuente (chunks mixtos, entidades multi-chunk). La observación SEELE es atómica: UN origen, UN created_at. Sería schema sin consumidor; donde la confianza importa (relations), ya existe. |
| **Prompts como módulos de código ejecutable** (.py + importlib) | No traduce a Rust sin scripting embebido; ejecutar código de usuario para construir un string es superficie de ataque gratuita. SEELE usa DATOS (TOML + sustitución plana) — y la deriva 4 de GRAIL (DEFAULT_DELIMITERS exportado que el parser nunca lee) es un bug imposible con contrato declarativo. |
| **Cache JSON-on-disk de respuestas LLM** | Existe porque el indexado de GRAIL re-ejecuta miles de prompts deterministas; el chat de SEELE incluye la historia completa en la key → hit-rate ~0. Peor: persistir respuestas con contenido de memorias en JSON plano contradice el cuidado de privacidad existente. |
| **Protocolo único OpenAI-compat eliminando AnthropicProvider** | El provider nativo contra /v1/messages con tool_use bidireccional ya funciona en producción; tirarlo por pureza de protocolo es churn sin ganancia. El registry E6 absorbe ambos wires con un enum de 2 variantes. |
| **Semáforo de concurrencia + streaming forzado con include_usage** | Dimensionado para indexado masivo (15 concurrentes); el chat SEELE procesa 1 request por vez, y con `stream: false` el usage viene gratis en el body (T8 lo parsea sin streaming). |
| **CostTracker como ledger in-process por tag/session** | GRAIL lo necesita porque un `grail index` dispara miles de llamadas etiquetadas; cada POST /chat de SEELE es stateless. Sería estado sin consumidor — y no mezclar con el cost-ledger.jsonl de AEGIS, que es contabilidad del desarrollo, no telemetría runtime. |
| **TraceEntry orientado a LLM** (messages/responses completos) | El 90% del shape captura conversaciones que en el core de SEELE no existen. El trace SEELE (Q8) modela lo que SÍ pasa: candidatos por path, fusión, filtros, timings. |

### 7.4 Proceso, distribución y comunidad

| Qué | Por qué no |
|-----|-----------|
| **i18n bilingüe completo de docs** (ES fuente + EN espejo) | GRAIL tiene mandato institucional (CCHIA); para un solo dev duplica el costo de CADA edición y garantiza deriva (el CONTRIBUTING.es de GRAIL ya replica datos stale). SEELE ya tiene el balance correcto: toggle ES/EN a nivel landing, guías técnicas en inglés. Si hay demanda real: traducir solo el quickstart. |
| **Gating issue-first estricto** (status:approved antes de PR, 9 categorías, squash enforced) | Proceso para gestionar inflow que SEELE no tiene (0 contributors externos). Imponérselo a los primeros curiosos los espanta. Versión correcta invertida: issues pre-abiertos con good-first-issue que INVITEN al primer PR (T7). Gating cuando el volumen lo justifique. |
| **"Deriva por contrato"** (docs internos "may drift") | La decisión más cómoda y cara de GRAIL: 10+ derivas reales encontradas por su propio compendio. La marca SEELE es lo contrario — deriva tratada como bug, compendio fact-checkeado — y ese rigor es un diferenciador verificable. Separación pública/interna SÍ (T6); licencia para derivar NO: misma fuente .md. |
| **Migrar a Docusaurus + corpus de muestra pesado versionado** (17 MB PDFs + notebooks con outputs) | Astro 6 con deploy verde ya existe; Starlight da sidebar+search+link-check sin migración. El material ejecutable SEELE ya existe más barato y honesto: suites de eval embebidas, ejecutables con `seele eval` contra DB efímera. |
| **Proactividad agresiva del SKILL.md** (frontmatter "GRAIL IS INSTALLED" + releer el skill en CADA mensaje) | Fuerza bruta para compensar drop de contexto; quema contexto en cada turno de cada sesión. El skill SEELE usa triggers normales y confía en el mecanismo estándar. |
| **Instalación de runtime dentro del skill** (setup.sh con pip/uv, guard PEP 668) | GRAIL es una librería Python que se instala en el intérprete del host; SEELE es un binario estático. El probe (T5) chequea presencia del binario y apunta a docs; nada más. |
| **Registry ~/.grail/registry.json + discover_projects()** | GRAIL tiene proyectos dispersos en el filesystem que necesitan descubrimiento y reconciliación. SEELE tiene UNA SQLite con project como columna — `seele projects` ya enumera todo. Sería resolver un problema que la arquitectura no tiene. |
| **La capa de 21 scripts wrapper como artefactos del skill** | Existen porque GRAIL es un SDK sin CLI contract-grade (su typer rompía JSON con Rich y lo parchearon con print plano). La CLI de SEELE con el envelope Q2 ES esa capa, testeada E2E en CI. Duplicarla en scripts crearía drift. |
| **Skill como REEMPLAZO del MCP** | GRAIL nunca construyó MCP y lo vende como virtud. SEELE tiene 19 tools funcionando, y Cursor/Windsurf (2 de sus 3 integraciones implementadas) no soportan skills: MCP es el único transporte ahí, con schemas tipados que un SKILL.md no da. El skill complementa para la cola larga de agentes; SeeleService compartido garantiza paridad semántica. |
| **Toolchain TS + vite + dist/ prebuilt commiteado para el renderer** | GRAIL lo necesita (renderer grande, wheels sin node). SEELE arranca con renderer.js vanilla + d3 vendorizado vía include_str! — cero toolchain en el path de cargo build. Migrar a prebuilt es paso posterior legítimo SI el renderer crece. |
| **strict mode / prompt packs completos como feature inicial** | GRAIL lo justifica con 10 prompts builtin; SEELE arrancaría con 1 prompt LLM real. Over-engineering puro hoy; si aparece un consumer con pack completo, alcanza un warning de cobertura en `seele prompts list`. |
| **Juzgado manual con modelo frontier para números oficiales** | La deriva más grave del benchmark de GRAIL: los scores publicados no salen del pipeline (sin judge_scores.json detrás). Irreproducible y no auditable. SEELE ya tiene la disciplina correcta (baseline emitido por el harness); E17 la blinda como regla escrita. |
| **Head-to-head contra frameworks externos** (correr GRAIL/LlamaIndex sobre corpora SEELE) | Runtime distinto, problema distinto, mantenimiento altísimo (GRAIL necesitó 12 runs + 5 experimentos solo para que SU baseline fuera justo). El eje comparativo correcto es interno: ablations por pata (T3), reranker on/off y embedder A vs B cuando lleguen — misma fairness, una sola variable. |
| **Hacks para claims de marketing** (forced-synthesis "0 empty responses", redondeo de latencias) | Un harness que mide debe reportar el fallo, no maquillarlo. La credibilidad del eval de SEELE — su único activo comparativo real — depende de que el número publicado sea el medido. |

---

*Documento generado como salida del ciclo de análisis comparativo GRAIL→SEELE del 2026-06-09. Próximo paso AEGIS: seleccionar el primer bloque (recomendado: Q1 + Q2 como par de distribución/contrato), escribir su plan táctico en `docs/plans/tactica/` y pasar Gate 1.*
