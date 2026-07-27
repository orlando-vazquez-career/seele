# 01 — Indexación de codebase (símbolos, blast-radius, watchers)

**Estado**: fuera de charter v0.4 · **Origen**: análisis de `DeusData/codebase-memory-mcp` (2026-07-26).

## Qué es

Un índice estructural del código: funciones, clases, edges de llamada, rutas HTTP, imports — extraído con tree-sitter, consultable como grafo, con herramientas derivadas como *blast-radius* (git diff → símbolos tocados → BFS sobre callers) y watchers que re-indexan incrementalmente.

## Evidencia (lo que codebase-memory-mcp hace bien)

- Extracción AST con un solo cursor walk + scope stack (`internal/cbm/extract_unified.c`), resolutores de tipos "Hybrid LSP" para 10 lenguajes.
- Re-index incremental: `file_hashes(project, path, sha256, mtime_ns, size)` — stat primero, hash solo si mtime/size cambió; los nodos de archivos cambiados se borran (edges en cascada) y se re-parsean (`pipeline_incremental.c:87-135`).
- **Ledger de cobertura/staleness**: cada archivo no cubierto queda registrado con kind (`parse_partial`/`skipped`) y rangos de línea; *freshness* (mtime match) se reporta separado de *coverage* (`store.c:280-306`). Degradación honesta: "ausencia de flag NO es garantía de completitud".
- `detect_changes`: seeds = símbolos definidos en archivos del diff, un solo BFS multi-fuente sobre callers, rollup por módulo (`mcp.c:9532-9564`).
- Watcher por git-poll con firma de dirty-state: HEAD hash + porcelain signature — re-indexa una vez por estado *distinto*, baseline solo tras reindex exitoso (`watcher.c:1-22`).

## Por qué es valioso

La memoria del ecosistema hoy guarda *decisiones sobre* el código, no el código. Un índice estructural permitiría recalls tipo "¿qué rompe este cambio?" directo desde SEELE, y la hidratación de KAIROS (`<slug>/meta/seele-sync`) podría comparar drift a nivel símbolo en vez de mtime de docs.

## Por qué quedó fuera

- **Charter**: SEELE es memoria de agentes, no motor de código. codebase-memory-mcp ya existe, es MIT y hace exactamente esto — duplicarlo es peor que integrarlo.
- **Costo real**: 158 gramáticas tree-sitter vendored + resolutores por lenguaje + watcher + pipeline incremental ≈ decenas de miles de líneas. codebase-memory-mcp gasta ~170k LOC en esto.
- **Mantenimiento**: un dev solo; cada lenguaje es un grifo de edge cases.

## Gatillo de adopción

Que SEELE decida crecer hacia "memoria del código" como feature de producto (no como consumidor), o que un segundo consumidor del ecosistema lo requiera y codebase-memory-mcp no cubra el caso (p.ej. necesidad de edges unidos a `memory_relations`).

## Boceto de integración (si algún día entra)

Camino recomendado: **integración, no construcción**. SEELE consume el índice de codebase-memory-mcp como fuente: un `seele import from-cbm <project>` que lee su SQLite y persiste observaciones tipo `code_structure` con provenance. Si se construyera nativo: crate `seele-code` separado (nunca en core), tabla `file_hashes` + mtime fast path tal cual la evidencia, coverage ledger obligatorio desde el día uno, y watcher git-poll (no fsnotify — ver las lecciones de su `watcher.c`).
