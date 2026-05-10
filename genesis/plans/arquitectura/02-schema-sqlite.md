# ADR-02 — Schema SQLite (memories, FTS5, vectores, virtual columns)

**Estado**: Aceptado · 2026-05-09
**Decisión**: Schema canónico con tabla principal `memories`, FTS5 virtual table sobre `body`, vec0 virtual table para embeddings, virtual generated columns para campos de metadata, índices B-tree sobre las virtual columns.

## Contexto

El schema es el contrato más estable del engine — cambiarlo después requiere migrations. Tiene que soportar:
- Búsqueda full-text (FTS5).
- Búsqueda vectorial (sqlite-vec).
- Filtrado eficiente por campos JSON metadata (kind, domain, tags, custom...).
- Soft delete.
- Auditoría temporal (created_at, updated_at, deleted_at).

## Schema canónico

### Tabla principal `memories`

```sql
CREATE TABLE memories (
    id          TEXT PRIMARY KEY,         -- ULID o UUIDv7 (sortable)
    body        TEXT NOT NULL,            -- contenido principal de la memoria
    metadata    TEXT NOT NULL DEFAULT '{}', -- JSON string, validado a serde_json::Value
    created_at  INTEGER NOT NULL,         -- Unix epoch milliseconds
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER                   -- NULL si activa, timestamp si soft-deleted
) WITHOUT ROWID;
```

Notas:
- `WITHOUT ROWID` porque el `id` es PK textual y queremos lookup eficiente sin rowid mapping.
- `metadata` como TEXT con JSON, no `BLOB` ni columnas explícitas. Trade-off: flexibilidad por consumidor (MNEMA define su schema; otros pueden usar SEELE con su propio schema).

### FTS5 virtual table

```sql
CREATE VIRTUAL TABLE memories_fts USING fts5(
    body,
    content='memories',
    content_rowid='id',
    tokenize='porter unicode61 remove_diacritics 2'
);
```

Tokenizer:
- `porter` para stemming inglés.
- `unicode61` para soporte unicode general.
- `remove_diacritics 2` para normalizar acentos en español/latino (búsqueda "telefono" matchea "teléfono").

Triggers para mantener sincronizada la FTS al insertar/update/delete en `memories`:

```sql
CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, body) VALUES (new.id, new.body);
END;

CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, body) VALUES('delete', old.id, old.body);
END;

CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, body) VALUES('delete', old.id, old.body);
    INSERT INTO memories_fts(rowid, body) VALUES (new.id, new.body);
END;
```

### vec0 virtual table (sqlite-vec)

```sql
CREATE VIRTUAL TABLE memories_vec USING vec0(
    embedding FLOAT[384]
);
```

Notas:
- Dimensión 384 = output de `all-MiniLM-L6-v2`. Si más adelante soportamos otros modelos, esa decisión va a ADR separado.
- Se inserta manualmente (no trigger) porque embedding requiere cómputo del embedder — el caller es quien sabe cuándo está disponible.
- `rowid` de `memories_vec` mapea al `id` de `memories` (cast text→int64 vía hash o uso de ULID timestamp+rand).
- La extensión `vec0` está vendorizada en `crates/seele-storage/vendor/sqlite-vec/` (binarios precompilados upstream para 5 targets, embebidos vía `include_bytes!`). Detalles completos en ADR-11.

**Decisión clave**: usar un campo numérico `int_id` derivado del ULID para mapear `memories.id` ↔ `memories_vec.rowid`:

```sql
ALTER TABLE memories ADD COLUMN int_id INTEGER UNIQUE
    GENERATED ALWAYS AS (CAST(SUBSTR(id, 1, 16) AS INTEGER)) VIRTUAL;
CREATE INDEX idx_memories_int_id ON memories(int_id);
```

Esto da un mapeo determinístico y único (los primeros 48 bits de ULID son timestamp + random — colisiones astronómicamente improbables).

### Virtual generated columns para metadata

ENGRAM (y la propuesta original de ADR-008 en MNEMA) usa virtual columns para extraer campos JSON sin escribir en disco:

```sql
ALTER TABLE memories ADD COLUMN meta_kind TEXT
    GENERATED ALWAYS AS (JSON_EXTRACT(metadata, '$.kind')) VIRTUAL;
ALTER TABLE memories ADD COLUMN meta_domain TEXT
    GENERATED ALWAYS AS (JSON_EXTRACT(metadata, '$.domain')) VIRTUAL;
ALTER TABLE memories ADD COLUMN meta_axiomatic INTEGER
    GENERATED ALWAYS AS (JSON_EXTRACT(metadata, '$.axiomatic')) VIRTUAL;
ALTER TABLE memories ADD COLUMN meta_score REAL
    GENERATED ALWAYS AS (JSON_EXTRACT(metadata, '$.score')) VIRTUAL;
ALTER TABLE memories ADD COLUMN meta_context_mode TEXT
    GENERATED ALWAYS AS (JSON_EXTRACT(metadata, '$.context_mode')) VIRTUAL;
```

**¿Por qué estos 5 campos predefinidos?**

SEELE NO conoce el schema completo del consumidor (MNEMA define `kind`, `domain`, `axiomatic`, `earn_score`, `context_mode` — pero otro consumidor podría usar otros). Hay 5 que se ganaron espacio canónico por uso transversal:

- **`kind`** — clasificación (decision/skill/advisor_output/review/verdict/memory). Universal.
- **`domain`** — proyecto/dominio. Universal para multi-project.
- **`axiomatic`** — flag de inmunidad a decay. Universal en patterns tipo MNEMA.
- **`score`** — earn_score / priority. Universal.
- **`context_mode`** — purist | contextual. Específico de patrones tipo Counsel pero crítico para Recall correcto (ver ADR-10 capa 4.5). Sin este virtual col, MNEMA tendría que filtrar por JSON_EXTRACT en cada Recall — ineficiente a escala.

Otros campos van en metadata sin virtual col por default; consumer puede agregar via `seele schema add-virtual-col <name> <jsonpath>`.

Esto da:
- Filtros eficientes out-of-the-box para casos comunes.
- Extensibilidad sin recompilar.

### Índices

```sql
CREATE INDEX idx_memories_kind ON memories(meta_kind) WHERE meta_kind IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_memories_domain ON memories(meta_domain) WHERE meta_domain IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_memories_kind_domain ON memories(meta_kind, meta_domain) WHERE deleted_at IS NULL;
CREATE INDEX idx_memories_axiomatic ON memories(meta_axiomatic) WHERE meta_axiomatic = 1;
CREATE INDEX idx_memories_score ON memories(meta_score) WHERE deleted_at IS NULL;
CREATE INDEX idx_memories_context_mode ON memories(meta_context_mode) WHERE meta_context_mode IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_memories_created_at ON memories(created_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_memories_deleted_at ON memories(deleted_at) WHERE deleted_at IS NOT NULL;
```

Partial indexes (WHERE clause) para minimizar tamaño — solo indexar memorias activas para filtros que excluyen soft-deletes.

### Tabla `links` (relaciones entre memorias)

```sql
CREATE TABLE links (
    id          TEXT PRIMARY KEY,
    from_id     TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    to_id       TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
    link_type   TEXT NOT NULL,            -- 'derives_from', 'supersedes', 'related_to', 'contradicts', etc
    metadata    TEXT NOT NULL DEFAULT '{}',
    created_at  INTEGER NOT NULL,
    UNIQUE(from_id, to_id, link_type)
) WITHOUT ROWID;

CREATE INDEX idx_links_from ON links(from_id);
CREATE INDEX idx_links_to ON links(to_id);
CREATE INDEX idx_links_type ON links(link_type);
```

Decisión: `link_type` es libre TEXT. SEELE define convenciones recomendadas pero no las enforce.

### Tabla `schema_version`

```sql
CREATE TABLE schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  INTEGER NOT NULL,
    description TEXT NOT NULL
);

INSERT INTO schema_version (version, applied_at, description)
VALUES (1, strftime('%s', 'now') * 1000, 'initial schema v0.1');
```

Migrations versionadas via `refinery` (Rust). Cada migration up-only (sin downgrade en v0.1).

## Decisiones secundarias

### IDs: ULID vs UUIDv7 vs autoincrement

**ULID** (Universally Unique Lexicographically Sortable Identifier).

Razones:
- Sortable por timestamp (queries `ORDER BY id` ≈ `ORDER BY created_at`, sin índice extra).
- 26 chars Crockford-base32 (más cortos que UUID, copy-pasteables).
- Random en últimos 80 bits (no enumera).

UUIDv7 sería equivalente pero más verboso (36 chars con guiones).

Crate Rust: `ulid` 1.x.

### JSON encoding: serde_json vs sonic-rs

`serde_json` por simplicidad. `sonic-rs` (más rápido) si benchmarks muestran bottleneck.

### Encoding de embeddings en vec0

sqlite-vec almacena `FLOAT[N]` como blob little-endian sin compresión. Para 384 floats = 1.5 KB por memoria. 10K memorias = 15 MB. Aceptable.

Para optimización futura (v0.2+): cuantización int8 (4x menos espacio, ~5% pérdida de precision), feature flag.

## Consecuencias

### Positivas
- Schema flexible (cualquier consumer define su metadata) + eficiente (virtual cols + indexes).
- FTS5 + vec0 + B-tree indexes en una sola DB SQLite — single-file portable.
- Soft delete + audit timestamps cubren lifecycle básico.

### Negativas
- Virtual columns dependen del JSON path — si el consumer cambia el shape, las columns devuelven NULL silenciosamente. Mitigación: validación de schema en CLI/HTTP layer.
- Embeddings llenan disco proporcional a memorias. 100K memorias ≈ 150 MB solo de embeddings. Aceptable para v0.1.

### Migraciones futuras

- v0.2: agregar tabla `embeddings_meta` (modelo usado, dim, fecha) si soportamos múltiples embedders.
- v0.3: agregar `tags` como tabla separada para queries multi-tag eficientes.

## Referencias

- SQLite FTS5 docs: https://www.sqlite.org/fts5.html
- sqlite-vec README: https://github.com/asg017/sqlite-vec
- ULID spec: https://github.com/ulid/spec
- Schema decisión MNEMA ADR-008 (virtual cols + indexes) que inspiró este: `MNEMA/docs/aegis/plans/executed/arquitectura/sprint-01/01-indexes-sqlite-virtual-columns.md`.
