# ADR-03 — Search híbrido FTS + vector + metadata

**Estado**: Aceptado · 2026-05-09
**Decisión**: Algoritmo de search híbrido en tres capas con **Reciprocal Rank Fusion (RRF)** como combinador de FTS5 y vector. Filtrado por metadata aplicado *antes* del scoring para eficiencia.

## Contexto

Una query típica del consumer:

```
search("rate limit retry policy", domain="dev-zen", kind="decision", top_k=10)
```

Qué tiene que hacer SEELE:

1. Filtrar memorias activas + matching `domain="dev-zen"` + `kind="decision"`.
2. Buscar texto "rate limit retry policy" full-text.
3. Buscar similar semánticamente al embedding de "rate limit retry policy".
4. Combinar ambos resultados.
5. Devolver top 10.

Las tres dimensiones (FTS, vector, metadata) tienen que jugar bien juntas.

## Algoritmo

### Capa 1 — Pre-filtro (índices B-tree sobre virtual columns)

```sql
-- Subselect que devuelve solo IDs candidatos según metadata
SELECT id, body, metadata
FROM memories
WHERE deleted_at IS NULL
  AND meta_domain = 'dev-zen'
  AND meta_kind = 'decision';
```

Usa `idx_memories_kind_domain` (B-tree). Con 100K memorias y filtro razonable, devuelve ~1K-10K candidates en sub-10ms.

### Capa 2 — FTS5 ranking sobre candidatos

```sql
SELECT m.id, bm25(memories_fts) AS fts_rank
FROM memories m
JOIN memories_fts ON m.id = memories_fts.rowid
WHERE m.id IN (<candidatos_capa_1>)
  AND memories_fts MATCH 'rate limit retry policy'
ORDER BY fts_rank
LIMIT 100;
```

`bm25()` es la función built-in de FTS5 que ranquea matches. Output: lista de `(id, fts_rank)` ordenada (lower rank = better match).

### Capa 3 — Vector similarity sobre los mismos candidatos

```sql
SELECT v.rowid, vec_distance_cosine(v.embedding, ?) AS vec_dist
FROM memories_vec v
WHERE v.rowid IN (<int_ids_capa_1>)
ORDER BY vec_dist
LIMIT 100;
```

`?` = embedding del query input (calculado por el embedder local antes de la query). `vec_distance_cosine` está en sqlite-vec.

Output: lista de `(rowid, vec_dist)` (lower distance = more similar).

### Capa 4 — Reciprocal Rank Fusion (RRF)

RRF combina dos rankings con la fórmula:

```
score(doc) = Σ (1 / (k + rank(doc, ranking_i)))
```

Donde `k` es una constante (default 60 según paper original). Para SEELE:

```
score(doc) = 1/(60 + rank_fts(doc)) + 1/(60 + rank_vec(doc))
```

Si un doc no aparece en uno de los rankings, su contribución de ese ranking es 0.

**Por qué RRF y no weighted sum**:
- Weighted sum (`α * score_fts + β * score_vec`) requiere normalización de scores que vienen de distribuciones distintas (BM25 unbounded vs cosine [0,2]).
- RRF es escala-invariante — solo importa el rank, no el valor.
- RRF es robusto al "outlier" (un doc que matchea perfectamente FTS pero pésimo en vector) — lo deja en top sin saturar.
- Empírico: RRF outperformea weighted sum en benchmarks BEIR / MS-MARCO.

**Implementación**:

```rust
pub struct SearchResult {
    pub id: String,
    pub body: String,
    pub metadata: serde_json::Value,
    pub score: f64,
    pub fts_rank: Option<usize>,
    pub vec_rank: Option<usize>,
    pub vec_distance: Option<f64>,
}

fn rrf_combine(
    fts_results: Vec<(String, f64)>,    // (id, bm25_rank)
    vec_results: Vec<(String, f64)>,    // (id, cosine_dist)
    top_k: usize,
    k_const: f64,                       // default 60.0
) -> Vec<SearchResult> {
    let mut score_map: HashMap<String, f64> = HashMap::new();

    for (rank, (id, _)) in fts_results.iter().enumerate() {
        *score_map.entry(id.clone()).or_insert(0.0) += 1.0 / (k_const + rank as f64 + 1.0);
    }

    for (rank, (id, _)) in vec_results.iter().enumerate() {
        *score_map.entry(id.clone()).or_insert(0.0) += 1.0 / (k_const + rank as f64 + 1.0);
    }

    let mut sorted: Vec<_> = score_map.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    sorted.truncate(top_k);

    // Fetch full memory data for top-k IDs
    fetch_memories(sorted)
}
```

### Capa 5 — Boost opcional por `score` metadata

Si el consumer guarda un campo `score` (ej: MNEMA usa `earn_score`) en metadata, SEELE multiplica el RRF score:

```rust
final_score = rrf_score * (1.0 + 0.1 * meta_score.unwrap_or(1.0))
```

Esto da ligero boost a memorias "valiosas" sin dominar el ranking. Configurable via flag `--boost-score-multiplier`.

## API

### CLI

```bash
seele search "rate limit retry policy" \
    --domain dev-zen \
    --kind decision \
    --top-k 10 \
    --json
```

### HTTP

```http
POST /search
Content-Type: application/json

{
  "query": "rate limit retry policy",
  "filters": {
    "kind": "decision",
    "domain": "dev-zen"
  },
  "top_k": 10,
  "include_distances": true,
  "rrf_k": 60
}
```

Response:

```json
{
  "results": [
    {
      "id": "01HW3X...",
      "body": "...",
      "metadata": { "kind": "decision", "domain": "dev-zen", ... },
      "score": 0.0254,
      "fts_rank": 1,
      "vec_rank": 3,
      "vec_distance": 0.342
    }
  ],
  "total_candidates": 47,
  "query_ms": 142
}
```

## Casos edge

### FTS sin matches

Si la query es muy específica y FTS devuelve 0 docs, el ranking es solo vector. Normal — RRF lo maneja sin código extra (la suma del ranking ausente es 0).

### Vector sin matches

Imposible en práctica — vector siempre devuelve top-k por distance, aunque la distance sea grande. Se filtra opcionalmente por threshold `--max-distance`.

### Query vacía + filtros solo

`search("", domain="X", top_k=10)` → lista los más recientes del dominio. SEELE lo soporta haciendo skip de FTS+vector y queryando solo capa 1 con `ORDER BY created_at DESC`.

### Filtros inválidos

Si el consumer pone `kind="nope"` y no hay matches, devolver array vacío sin error.

## Performance esperada

Con 10K memorias en CPU mid-range (Ryzen 5 / M2):

- Capa 1 (filtro): ~1-5ms.
- Capa 2 (FTS5): ~10-50ms para top 100.
- Capa 3 (vec0): ~30-100ms para top 100 (cosine sobre 384-dim).
- Capa 4 (RRF): ~1ms en memoria.
- **Total**: ~50-200ms.

Con 100K memorias: ~150-400ms. Aceptable para v0.1; optimización (sqlite-vec compilado con SIMD, tantivy en lugar de FTS5) si emerge bottleneck.

## Out of scope (v0.1)

- **Reranking por LLM**: meter un cross-encoder o LLM rerank después de RRF. Útil pero costoso. v0.2.
- **Personalization**: peso por user/dominio aprendido. Out of charter.
- **Query expansion**: sinónimos, traducciones automáticas. v0.3.

## Referencias

- *Reciprocal Rank Fusion outperforms Condorcet and individual Rank Learning Methods* — Cormack, Clarke, Buettcher, 2009.
- BEIR benchmarks suite (HuggingFace).
- sqlite-vec performance docs.
