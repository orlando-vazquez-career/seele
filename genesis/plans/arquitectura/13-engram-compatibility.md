# ADR-13 — ENGRAM compatibility (migration tool + tool-prefix shim)

**Estado**: Aceptado · 2026-05-10
**Decisión**: SEELE provee dos mecanismos para que consumidores que usan ENGRAM (incluido MNEMA) puedan migrar a SEELE sin re-implementar su capa de wrapping:
1. Un comando CLI `seele import --from-engram <path>` que migra una DB de ENGRAM al schema de SEELE.
2. Un flag opcional en MCP/HTTP que expone las tools/endpoints con prefijo configurable (`mnema_*`, `engram_*`, etc) para drop-in compatibility.

Este ADR cierra el riesgo de timeline alto que Cloven detectó (2026-05-10): MNEMA hoy depende de ENGRAM, y la migración naive a SEELE rompe queries SQL directos del usuario.

## Contexto

ENGRAM (Gentleman-Programming, Go) es el memory engine de referencia que SEELE re-implementa en Rust. Hoy **MNEMA usa ENGRAM** como su capa de persistencia ([detalle](https://github.com/Gentleman-Programming/engram)):

- DB: `~/.mnema/mnema.db` (SQLite + FTS5 + sqlite-vec).
- Tabla: `memories`.
- Virtual columns: `mnema_kind`, `mnema_domain`, `mnema_advisor_role`, `mnema_verdict_id`, `mnema_earn_score`, `mnema_axiomatic`.
- Indexes: `idx_mnema_*`.
- MCP tools: `mnema_recall`, `mnema_save`, `mnema_link`, etc.

SEELE tiene un schema **parecido pero distinto**:

- Tabla: `observations`.
- Virtual columns: `meta_kind`, `meta_domain`, `meta_axiomatic`, `meta_score`, `meta_context_mode`.
- Indexes: `idx_obs_meta_*`.
- MCP tools: `seele_save`, `seele_search`, `seele_link`, etc.

El contrato superficial (HTTP 7777, MCP stdio, SQLite local, embedder local) coincide. Los detalles del schema y nombres de tools NO.

Si MNEMA migra naive (cambiar el binary `engram` → `seele` y nada más):

- Las queries SQL directas del bootstrap (`WHERE mnema_kind = ?`) se rompen.
- Los wrappers MNEMA que llaman `mnema_save` via MCP se rompen.
- La DB existente con datos no es leíble — SEELE no tiene `memories` table.

## Decisión 1 — Migration tool

CLI: `seele import --from-engram <engram-db-path> [--target <seele-db-path>] [--re-embed]`.

Comportamiento:

1. Abre el DB ENGRAM en read-only.
2. Verifica que la tabla `memories` exista y tenga el schema esperado.
3. Si SEELE target no existe, lo inicializa (corre migrations V001 + V002 si aplica).
4. Por cada row de `memories`:
   - Convierte `id` (ULID o cuid) al formato `SeeleId`. Si el ID original ya es ULID válido, se preserva.
   - Mapea campos:
     - `memories.body` → `observations.content`.
     - `memories.metadata` → `observations.metadata` (JSON preservado as-is).
     - `memories.created_at` (epoch ms o ISO string) → `observations.created_at`.
     - `metadata.kind` queda en JSON; SEELE virtual column `meta_kind` lo expone.
     - `metadata.domain` → SEELE no tiene `domain` first-class; queda en JSON, virtual column `meta_domain` lo expone.
     - `metadata.advisor_role`, `metadata.blind_id`, `metadata.verdict_id`, `metadata.earn_score`, `metadata.axiomatic` → preservados en JSON. SEELE expone `meta_axiomatic` y `meta_score` como virtual cols.
     - `metadata.linked_to: ["mem_X", ...]` → migra a la tabla `links` con `link_type = "derives_from"` o `"related_to"` (decisión: usar `related_to` por default, documentar en el output).
   - Inserta en `observations` via `ObservationStore::save_raw` (método nuevo que bypassa privacy strip + dedup — la data ya está auditada).
5. Si `--re-embed`, calcula embeddings nuevos con el modelo SEELE (`all-MiniLM-L6-v2` quantized) y los inserta en `observations_vec`. Si no, deja la tabla vacía (search vec branch retornará 0 hits hasta reindex).
6. Verifica idempotencia: re-correr el comando sobre el mismo source no duplica (cada row se identifica por su ULID original).
7. Reporta: total migrado, total skipped (idempotency), embeddings calculados, errores recoverable.

Implementación: **Sprint-04 Bloque CLI** (cuando se construya el CLI completo con clap).

## Decisión 2 — Tool prefix shim

Por default, MCP server expone tools con prefijo `seele_*` y HTTP API en `/memories`, `/sessions`, etc.

Flag opcional para drop-in compatibility:

```bash
# MCP mode: tools expuestas como mnema_save, mnema_recall, ...
seele mcp --tool-prefix mnema

# HTTP mode: endpoints con paths legacy ENGRAM
seele serve --legacy-engram-paths
```

Comportamiento `--tool-prefix mnema`:

- `seele_save` → `mnema_save` (mismo handler subyacente).
- `seele_search` → `mnema_recall` (rename específico — ENGRAM usaba "recall" en lugar de "search").
- `seele_link` → `mnema_link`.
- `seele_show` → `mnema_show`.
- Resto: `seele_*` → `mnema_*` literal.

El handler interno es el mismo. Solo cambia el nombre que aparece en `tools/list` de MCP.

Comportamiento `--legacy-engram-paths`:

- `POST /memories` también responde a `POST /save` (ENGRAM original).
- `POST /search` queda igual (mismo nombre).
- `GET /memories/{id}` también responde a `GET /show/{id}`.
- Endpoints SEELE-específicos (`/sessions`, `/relations`, etc) NO se exponen como legacy — son features nuevas que ENGRAM no tenía.

Implementación: **Sprint-03 Bloques E (MCP) y D (HTTP)** — los handlers ya son thin wrappers sobre `SeeleService`, solo se duplican las rutas con names alternativos. Costo bajo (~20-40 LOC de aliasing).

## Decisión 3 — Schema virtual columns: NO renombrar a `mnema_*`

Razón: SEELE es agnóstico del consumer. `meta_*` es el naming canónico. Si MNEMA quiere `mnema_kind`, lo puede crear como **virtual column adicional** en su wrapping layer:

```sql
ALTER TABLE observations ADD COLUMN mnema_kind TEXT
  GENERATED ALWAYS AS (json_extract(metadata, '$.kind')) VIRTUAL;
```

Esto vive en MNEMA, no en SEELE. SEELE expone `meta_kind`; MNEMA agrega `mnema_kind` como alias si lo necesita. Tradeoff: MNEMA tiene que correr una migración SQL en su `seele init` script al primer uso.

**Alternativa rechazada**: que SEELE expose `mnema_*` virtual columns nativamente. Esto contamina SEELE con dependencias del consumer y no escala a otros consumers (Cursor, OpenCode, etc).

## Out of scope

- Conversión bidireccional: SEELE → ENGRAM. No la implementamos. La migración es one-way.
- Sync continuo entre ENGRAM y SEELE corriendo en paralelo. Tampoco. Es una migración puntual.
- Soporte para múltiples versiones de schema ENGRAM. Soportamos la versión que MNEMA usa hoy (2026-05). Si Gentleman bumpea ENGRAM y cambia schema, actualizamos el importer en un sprint dedicado.

## Plan de implementación

### Sprint-03 (en curso)

- Bloque D (auth+openapi): agregar `--legacy-engram-paths` como flag de `ServerConfig` + test que verifica que `POST /save` responde igual que `POST /memories`.
- Bloque E (mcp): agregar `--tool-prefix` como flag al binary `seele mcp` + test que verifica `tools/list` con prefix `mnema` retorna `mnema_save` en lugar de `seele_save`.

### Sprint-04 (siguiente)

- Bloque CLI: implementar `seele import --from-engram <path>` con flag `--re-embed`.
- Bloque setup: actualizar wizard para detectar `~/.mnema/mnema.db` existente y ofrecer importar automáticamente.
- Bloque docs: agregar `docs/ENGRAM-MIGRATION.md` con paso a paso.

### Sprint-05 (release)

- Tests E2E que validan: arranca con ENGRAM DB, importa, queries `mnema_*` retornan resultados consistentes vs `seele_*`.
- Documentación de migración linkeada desde el README.

## Consecuencias

### Positivas

- MNEMA puede migrar de ENGRAM a SEELE sin tirar datos.
- Otros consumers que usaban ENGRAM (si emergen) pueden migrar usando el mismo path.
- SEELE queda agnóstico del consumer en su naming canonical, sin contaminar con `mnema_*`.

### Negativas

- ~150-250 LOC adicionales en Sprint-03 Bloques D + E (aliasing).
- ~400-600 LOC en Sprint-04 CLI (import command + tests + edge cases).
- ~50 LOC de docs.

Total ~600-900 LOC adicionales. Aceptable vs el costo de migración manual (semana-hombre).

### Riesgos mitigados

- Cloven 2026-05-10 [ALTO/RIESGO timeline]: MNEMA→SEELE migration sin tooling. Cerrado por este ADR.

## Referencias

- ENGRAM upstream: https://github.com/Gentleman-Programming/engram
- MNEMA memory-engine guide (consumer side): `C:/dev/protocols/MNEMA/guides/memory-engine.md`
- MNEMA install-engram (legacy): `C:/dev/protocols/MNEMA/guides/install-engram.md`
- SEELE ADR-02 schema: `genesis/plans/arquitectura/02-schema-sqlite.md`
- SEELE ADR-05 MCP tools: `genesis/plans/arquitectura/05-mcp-server.md`
- Cloven review post Bloque C.1 (2026-05-10): timeline sight del problema.
