# SEELE como reimplementación inspirada en ENGRAM (MIT)

## Postura legal y ética

ENGRAM está bajo licencia **MIT**. Esto cambia toda la postura comparado con un clean-room estricto:

- **Podemos leer su código línea por línea** sin contaminación legal.
- **Podemos extraer su lógica de negocio** y reimplementarla con nuestras propias decisiones técnicas en Rust.
- **Podemos copiar conceptos, schemas, algoritmos** — todo es legal bajo MIT.
- **Debemos dar crédito** al autor (Gentleman-Programming) en README, CREDITS y release notes — es la condición de la licencia MIT.
- **NO debemos copiar código fuente literal** y pasarlo como nuestro — eso violaría la licencia MIT (que requiere preservar copyright headers).

## Disciplina práctica

### Qué SÍ hacemos

1. **Leemos el código de ENGRAM** (Go) para entender cómo resolvió cada problema.
2. **Extraemos la lógica de negocio** (modelo de datos, algoritmos, edge cases manejados).
3. **Reimplementamos en Rust** desde cero — diferente lenguaje, diferente runtime, diferente ecosystem de crates.
4. **Documentamos qué inspiró qué** en CREDITS.md (transparencia).
5. **Damos crédito visible** en README, CREDITS y release notes.

### Qué NO hacemos

1. **No copiar literal** archivos `.go` para "traducir" a Rust — eso es una obra derivada que requeriría headers de copyright preservados.
2. **No copiar comentarios** de ENGRAM al pie de la letra.
3. **No reproducir exactamente** los nombres internos / structs si nuestro idiomatic Rust sugiere algo distinto.
4. **No reclamar como propio** lo que claramente es lógica de ENGRAM (ej: el algoritmo de project detection en 5 casos).

### En la práctica

Cuando una sesión esté trabajando en SEELE y necesite saber cómo ENGRAM resuelve algo:

1. Lee el archivo correspondiente en GitHub (ej: `internal/store/store.go`).
2. Entiende el patrón.
3. Cierra el archivo de ENGRAM.
4. Implementa en Rust con nuestras decisiones (tipos `Result<T, E>`, `tokio::spawn_blocking` para SQLite sync, etc).
5. Si una decisión específica viene directamente de ENGRAM, anótala en CREDITS.md.

## Lo que tomamos de ENGRAM

Auditoría completa archivo por archivo en `genesis/plans/estrategia/05-audit-engram.md` (o equivalente — ver INDEX). Resumen de alto nivel:

### Modelo de datos

- **`sessions` table**: `id` PK, `project`, `directory`, `started_at`, `ended_at`, `summary`, `status`. SEELE adopta este modelo.
- **`observations` table**: `id` PK autoincrement, `session_id` FK, `type`, `title`, `content`, `tool_name`, `project`, `scope`, `topic_key`, `normalized_hash`, `revision_count`, `duplicate_count`, `last_seen_at`, `created_at`, `updated_at`, `deleted_at`. SEELE adopta el shape (con ULID en vez de int autoincrement).
- **FTS5 virtual table** sincronizada con triggers sobre observations (title, content, tool_name, type, project). Adoptado.
- **`user_prompts` table** y `prompts_fts`. Adoptado.
- **`memory_relations` table** con `judgment_status (pending|judged|orphaned|ignored)`, `relation`, `confidence`, `marked_by_actor/kind/model`. Adoptado.
- **`sync_chunks` table** para git sync deduplication. Adoptado.
- **`sync_apply_deferred` table** para mutations diferidas. Adoptado parcial (v0.1 sin cloud, sin deferred logic full).

### Algoritmos

- **Project detection 5-case algorithm** (config.json / git remote / git root / git child / dir basename con timeout 200ms y skip de noise dirs). Adoptado.
- **Topic key family heuristics** (`architecture/*`, `bug/*`, `decision/*`, `pattern/*`, `config/*`, `discovery/*`, `learning/*`). Adoptado.
- **Normalized hash deduplication** (`hash + project + scope + type + title` en ventana temporal). Adoptado.
- **Privacy stripping en dos capas** (plugin layer + store layer `stripPrivateTags`). Adoptado.
- **Capture passive** — parser de `## Key Learnings:` con bullets/numbered. Adoptado.
- **Progressive disclosure 3-layer** (search → timeline → get_observation). Adoptado.

### Interfaces

- **CLI verbs** canónicos (`init`, `serve`, `mcp`, `tui`, `search`, `save`, `timeline`, `context`, `stats`, `export`, `import`, `sync`, `doctor`). Adoptados (son verbos universales tipo git/kubectl, no propios de ENGRAM).
- **MCP tools** (19 tools `mem_*`). Adoptamos el conjunto pero con prefijo `seele_*` (no `mem_*`) para diferenciar.
- **HTTP REST endpoints** sobre `/sessions`, `/observations`, `/search`, `/timeline`, `/context`, `/conflicts`, `/sync/*`. Adoptados.
- **Default port 7437** — heredado para compatibilidad de configs MCP existentes.
- **Default data dir `~/.seele/`** — análogo a `~/.engram/`.

### Lo que NO tomamos

- **Código Go literal** — cero líneas copiadas.
- **Catppuccin Mocha theme** — SEELE define su propio theme (más sobrio, sin tema "anime/pop"). Aunque podemos ofrecer Catppuccin como theme alternativo en v0.2.
- **Templ-based dashboard** — el cloud dashboard en HTMX/templ no entra en v0.1 (cloud diferido).
- **goreleaser** — usamos GitHub Actions custom (decisión técnica).
- **bubbletea** — usamos ratatui (Rust-native, sin overhead de Go runtime).

## Lo que SEELE agrega que ENGRAM no tiene

### 1. Embeddings vectoriales

ENGRAM usa solo FTS5 + LLM-based judging para conflicts. SEELE persiste embeddings de cada observation via `sqlite-vec`:

- Modelo default: `all-MiniLM-L6-v2` (dim 384, ONNX, CPU).
- Auto-download en primer init.
- Embedding por save (~5ms en CPU mid-range).
- Búsqueda híbrida FTS5 + cosine similarity.

### 2. Search híbrido con RRF

Reciprocal Rank Fusion sobre FTS rank + vector cosine distance + pre-filtro por metadata. Resultado: matches que ni FTS solo ni vector solo encontrarían.

### 3. Virtual generated columns

`meta_kind`, `meta_domain`, `meta_axiomatic`, `meta_score` extraídos del JSON metadata via virtual columns + B-tree indexes parciales. Sub-10ms filter incluso a 100K+ memorias.

### 4. ULID IDs

Sortables por timestamp, 26 chars Crockford-base32, sin enumeration risk. ENGRAM usa INTEGER autoincrement.

### 5. Rust ergonomics

- `Result<T, E>` con `thiserror` — error handling tipado.
- `tokio::spawn_blocking` para queries SQLite sync sin bloquear runtime.
- `r2d2` connection pool para HTTP concurrent.
- `ratatui` immediate-mode TUI en lugar de bubbletea event loop.
- Single-binary cross-compilable a 5 targets sin GC pauses.

## Crédito en SEELE

### `README.md` sección "Inspiration"

```markdown
## Inspiration

SEELE is a Rust reimplementation inspired by the [ENGRAM](https://github.com/Gentleman-Programming/engram) memory engine by [Gentleman-Programming](https://github.com/Gentleman-Programming). ENGRAM proved that local-first memory engines for AI agents are viable and demonstrated many of the patterns SEELE adopts: session lifecycle, project detection, topic key upserts, memory relations with judgment, git sync chunks, privacy stripping, and the MCP-as-primary-transport approach.

ENGRAM is licensed MIT (Copyright Gentleman-Programming). SEELE is also MIT (Copyright DevZen SpA). They are sibling tools in the same niche, with different stacks (Go vs Rust) and a different bet on retrieval (FTS+LLM-judge vs FTS+embeddings+RRF).

If SEELE is useful, please also try ENGRAM — the broader ecosystem benefits from multiple options.
```

### `CREDITS.md`

Más extenso. Estructura:

```markdown
# Credits

## Inspiration: ENGRAM

SEELE's architecture, data model, MCP tool set, project detection algorithm, topic key conventions, privacy stripping, capture passive parser, and many other design decisions are directly inspired by ENGRAM by [Gentleman-Programming](https://github.com/Gentleman-Programming).

ENGRAM is licensed under the MIT License. SEELE is a Rust reimplementation, also under MIT (DevZen SpA). The ENGRAM source code was studied as reference; no Go code was copied verbatim into SEELE.

What SEELE inherits from ENGRAM (with our own implementation):
- The 9-table SQLite schema (sessions, observations, observations_fts, user_prompts, prompts_fts, memory_relations, sync_chunks, sync_apply_deferred, schema_version).
- The 19 MCP tool semantics (renamed `seele_*` to differentiate).
- The HTTP REST API surface.
- The CLI verbs and subcommand layout.
- The 5-case project detection algorithm.
- The topic-key family heuristics.
- The privacy stripping in two layers.
- The capture passive parser pattern.
- The git-friendly compressed chunks sync.

What SEELE adds beyond ENGRAM:
- Vector embeddings via sqlite-vec (`all-MiniLM-L6-v2`).
- Hybrid search with Reciprocal Rank Fusion.
- Virtual generated columns + partial B-tree indexes.
- ULID IDs.

We thank Gentleman-Programming for proving the viability of this category and for choosing MIT as the license, enabling work like SEELE.

## Models

### all-MiniLM-L6-v2

Default embedder. By Sentence-Transformers (UKP Lab + Microsoft). Apache-2.0.

## Libraries

(Full list of Rust dependencies and their licenses.)
```

### Release notes v0.1

```markdown
## SEELE v0.1.0

A Rust memory engine for AI agents, inspired by ENGRAM (Gentleman-Programming, MIT).

This is the first release. SEELE adopts ENGRAM's data model and CLI/MCP/HTTP shape, and adds vector embeddings + hybrid search on top.

See CREDITS.md for full attribution.
```

## Salvaguardas operativas

Reglas para el resto del genesis y sprints:

1. **OK leer código de ENGRAM** desde GitHub. Sin necesidad de clonarlo localmente.
2. **OK extraer ideas, schemas, algoritmos**. Documentar el origen en CREDITS si es directo.
3. **NO copiar archivos .go** y "traducir" a Rust con copy-paste + edits. Eso es derivar, no inspirarse.
4. **NO replicar nombres internos** (variables, funciones) cuando idiomatic Rust sugiere otros.
5. **PRs con "tomado literal de ENGRAM" se rechazan**. PRs con "feature inspirada en ENGRAM" están bien.
6. **Cualquier mención a ENGRAM** en docs / commits / discusiones cita el repo original con link.
7. **Si SEELE descubre un mejor patrón** que ENGRAM (ej: nuestro RRF + embeddings), CREDITS.md menciona la divergencia con respeto.

## Contraste

| Atributo | Fork | Clean-room estricto | **Reimplementación inspirada (SEELE)** |
|---|---|---|---|
| Código compartido | Mucho | Cero, sin mirar el original | Cero líneas, pero **sí leemos** el original |
| Licencia | Heredada del upstream | Independiente | Independiente (MIT propio) |
| Crédito | Implícito por fork | Explícito generalizado | **Explícito y detallado por feature** |
| Decisiones técnicas | Acopladas | Propias sin contexto | Propias **con contexto del estado del arte** |
| Riesgo legal MIT | Cero (mismo origen) | Cero | Cero si no se copian copyright headers |
| Velocidad inicial | Rápido | Muy lento | Medio-rápido |
| Calidad final | Limitada | Variable | **Alta — combina best practice + nuestro stack** |

Esta postura es la que **SEELE adopta**. Justificada por:

1. ENGRAM es MIT — la licencia explícitamente permite leer y reusar.
2. Reescribir todo desde cero sin mirar es desperdiciar 6+ meses de aprendizajes operacionales del autor de ENGRAM.
3. El crédito es **honesto y proporcional** al beneficio que recibimos.
