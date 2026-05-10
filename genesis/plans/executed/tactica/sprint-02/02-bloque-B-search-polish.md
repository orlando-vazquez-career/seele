# Sprint-02 Bloque B — Search polish

**Tema**: Cerrar los gaps de `seele-search` heredados del commit `8d67f48`: boost por metadata score, empty-query path, annotation lines de relations, max-distance threshold.

**Pre-requisitos**: Sprint-01 cerrado. Independiente de Bloque A (puede ejecutarse en paralelo en el mismo turno).

## Tareas atómicas

### B.1 — Boost por metadata score

**Inputs**: ADR-03 capa 5 (`final_score = rrf_score * (1.0 + 0.1 * meta_score.unwrap_or(1.0))`).

**Cambios**:
- Agregar a `SearchQuery`:
  ```rust
  /// Multiplier per unit of `meta_score`. Default 0.0 = boost off.
  /// E.g. 0.1 = +10% score per unit (ADR-03 reference).
  pub score_boost_multiplier: f64,
  ```
- Default: `0.0` para preservar comportamiento actual.
- En `search()`, después de `rrf::combine` y antes de truncar a `final_limit`:
  - Hidratar `meta_score` virtual column desde `observations` para los hit IDs.
  - Para cada `RrfHit`, si `meta_score.is_some()` y `score_boost_multiplier > 0.0`: `score *= 1.0 + boost * meta_score.unwrap_or(1.0)`.
  - Re-sort por score desc.
  - Truncar a `final_limit`.

**Criterio de done**:
- Test unit en `engine.rs::tests`:
  ```rust
  #[test]
  fn boost_promotes_high_meta_score_doc()
  ```
  Crear dos observations con texto idéntico ("cache invalidation"), una con `meta_score=10.0`, otra sin score. Sin boost → orden indefinido (depende de ULID). Con `score_boost_multiplier=0.1` → la high-score gana.
- Test unit `boost_zero_preserves_rrf_order`.

### B.2 — Empty-query path (list por created_at desc)

**Inputs**: ADR-03 sección "Casos edge / Query vacía + filtros solo".

**Cambios**:
- En `search()`, antes del check de empty:
  ```rust
  if query.text.trim().is_empty() {
      return self.list_by_filters(&query);
  }
  ```
  (En vez de retornar `InvalidInput`.)
- Nuevo método privado `fn list_by_filters(&self, query: &SearchQuery) -> Result<Vec<SearchHit>>`:
  - SQL: `SELECT … FROM observations WHERE deleted_at IS NULL [+ filters] ORDER BY created_at DESC LIMIT ?`.
  - Construye `SearchHit` con `score=0.0`, `fts_rank=None`, `vec_rank=None`, `annotations=Vec::new()`.
  - Aplica filtros project/scope/kind/include_purist igual que `fts_query`/`vec_query`.

**Criterio de done**:
- Test integration en `hybrid_search.rs`:
  ```rust
  #[test]
  fn empty_query_with_filters_lists_recent_in_project()
  ```
  Crear 5 observations en project="p" con `created_at` distintos, 5 más en otro project. Search con `text=""`, `project=Some("p")`, `limit=Some(3)` → 3 más recientes de "p" en orden created_at desc.
- Test integration `empty_query_excludes_purist_by_default_and_deleted`.
- **Reemplazar** test existente `empty_query_returns_invalid_input_error` por `whitespace_only_query_treated_as_empty_query`.

### B.3 — Annotation lines de relations

**Inputs**: estrategia/04-scope-mvp.md ("Annotation lines en results: supersedes:, superseded_by:, conflicts:, conflict: contested by").

**Cambios**:
- Agregar a `SearchQuery`:
  ```rust
  /// Include relation annotations on each hit. Costs an extra query per hit.
  /// Default false to preserve sub-300ms target on big queries.
  pub include_annotations: bool,
  ```
- Agregar a `SearchHit`:
  ```rust
  pub annotations: Vec<RelationAnnotation>,
  ```
- Nuevo type:
  ```rust
  #[derive(Debug, Clone)]
  pub struct RelationAnnotation {
      pub kind: AnnotationKind,
      pub other_id: SeeleId,
      pub other_title: Option<String>,
      pub reason: Option<String>,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum AnnotationKind {
      Supersedes,         // self supersedes other
      SupersededBy,       // other supersedes self
      ConflictsWith,      // peer-level conflict, judgment pending
      ContestedBy,        // judgment_status='judged', other won
  }
  ```
- En `search()`, si `include_annotations=true`, después de hidratar observations y antes de retornar:
  - Una sola query: `SELECT id, source_id, target_id, relation, judgment_status FROM memory_relations WHERE source_id IN (?…) OR target_id IN (?…)`.
  - Mapear cada relation a annotation correspondiente según source/target.
  - Adjuntar a cada `SearchHit`.

**Criterio de done**:
- Test integration `annotations_supersedes_attaches_to_winner_and_loser`.
- Test integration `annotations_off_by_default`.
- Test unit que verifica el mapping `RelationKind::ConflictsWith + judgment_status='judged'` → `AnnotationKind::ContestedBy`.

### B.4 — Max-distance threshold (vec)

**Inputs**: ADR-03 sección "Vector sin matches" → "filtra opcionalmente por threshold `--max-distance`".

**Cambios**:
- Agregar a `SearchQuery`:
  ```rust
  /// If set, drop vec hits with distance > max_distance from the ranking.
  /// Useful when vec0 returns top-K regardless of similarity.
  pub max_vec_distance: Option<f64>,
  ```
- En `vec_query()`, agregar al SQL: `AND vec.distance <= ?` cuando `max_vec_distance` es Some.

**Criterio de done**:
- Test unit `max_distance_drops_far_hits`.

### B.5 — Doc + CHANGELOG

**Cambios**:
- Doc-comment del crate explicando el flow: pre-filter (filters) → fts + vec en paralelo → RRF → boost (opcional) → annotations (opcional) → truncate.
- `CHANGELOG.md` Unreleased / Added: enumerar boost, empty-query, annotations, max-distance. Changed: `empty query no longer InvalidInput → returns recent list`.

## Tests del bloque B

Total esperado: ~9 nuevos.

- `boost_promotes_high_meta_score_doc`
- `boost_zero_preserves_rrf_order`
- `empty_query_with_filters_lists_recent_in_project`
- `empty_query_excludes_purist_by_default_and_deleted`
- `whitespace_only_query_treated_as_empty_query` (reemplaza el `_returns_invalid_input_error`)
- `annotations_supersedes_attaches_to_winner_and_loser`
- `annotations_off_by_default`
- `annotations_kind_mapping_unit`
- `max_distance_drops_far_hits`

## Criterios de aceptación del bloque B

1. `cargo build -p seele-search` verde.
2. `cargo test -p seele-search` verde con los nuevos tests.
3. `cargo clippy -p seele-search -- -D warnings` verde.
4. `SearchQuery::default()` mantiene comportamiento Sprint-01 (boost=0, no annotations, no max_distance).
5. Empty query con filtros funciona como list ordenado por created_at desc.
6. Annotations opt-in agregan info útil sin afectar el ranking RRF.
7. Boost por meta_score documentado con ejemplo de uso.

## Commit del bloque B

```
git add -A
git commit -m "sprint-02 bloque-B — search polish (boost + empty-query + annotations + max-distance)"
git push
```
