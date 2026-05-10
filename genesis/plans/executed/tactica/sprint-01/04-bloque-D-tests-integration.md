# Sprint-01 Bloque D — Tests integración

**Tema**: Tests E2E del storage layer end-to-end + property tests + fixtures + smoke tests cross-platform en CI.

**Pre-requisitos**: bloques A + B + C cerrados.

## Estructura

```
tests/                                    # workspace-level (E2E)
├── storage_e2e.rs                        # full lifecycle test
├── migration_e2e.rs                      # apply migration → validate schema dump
└── fixtures/
    └── sample_observations.json          # 50 observations realistas

crates/seele-storage/tests/                # crate-level (unit + integration)
├── observations_crud.rs                  # ya en bloque C
├── observations_upsert.rs                # ya en bloque C
├── observations_fts.rs                   # ya en bloque C
├── sessions_crud.rs                      # ya en bloque C
├── links_crud.rs                         # ya en bloque C
├── relations_crud.rs                     # ya en bloque C
├── privacy.rs                            # ya en bloque C
└── property_tests.rs                     # nuevo en bloque D
```

## Tests workspace-level

### `tests/storage_e2e.rs`

Caso completo "vida de una observation":

1. Init DB temporal.
2. Start session.
3. Save 5 observations linkeadas a la session, varios types y projects.
4. Verify counts (`stats` aproximado).
5. Search por content keyword → match esperado.
6. Update una observation → updated_at cambia.
7. Soft delete → no aparece en list default.
8. Search excluye soft-deleted.
9. Restore → vuelve.
10. Hard delete + cascade DELETE de links asociados.
11. End session → status='ended'.
12. Cleanup.

Si esto pasa en Linux + macOS + Windows, el storage layer está sólido.

### `tests/migration_e2e.rs`

1. Apply V001.
2. Query `sqlite_master` para enumerar todas las tablas, indexes, triggers, virtual tables.
3. Compare against expected snapshot (insta).
4. Re-apply V001 → idempotent (no error, schema_version count sigue en 1).

### `tests/fixtures/sample_observations.json`

50 observations realistas con variedad de:
- 6 sessions (3 dev-zen, 2 mnema, 1 personal-scope).
- types mixtos (decision, architecture, bugfix, advisor_output, review, verdict, skill).
- topic_keys con family heuristics (architecture/api-rate-limit, decision/embedder-choice, etc).
- 10 con `metadata.context_mode='purist'` (advisor_outputs de Primeros Principios + Outsider).
- 20 con `metadata.axiomatic=true`.
- 5 soft-deleted.
- 30 con linked_to (para tests de links).
- 8 con memory_relations (judgment lifecycle: 5 pending, 3 judged).

## Property tests (bloque D)

### `crates/seele-storage/tests/property_tests.rs`

```rust
use proptest::prelude::*;
use seele_core::SeeleId;
use seele_storage::*;

proptest! {
    #[test]
    fn save_then_get_roundtrip(
        title in "\\PC{1,200}",
        content in "\\PC{1,5000}",
        project in proptest::option::of("[a-z][a-z0-9-]{2,30}"),
    ) {
        // Setup
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let pool = init_db(tmp.path()).unwrap();
        let store = ObservationStore::new(pool);

        // Save
        let outcome = store.save(SaveInput {
            title: title.clone(),
            content: content.clone(),
            project: project.clone(),
            ..Default::default()
        }).unwrap();

        let id = match outcome {
            SaveOutcome::Created(id) => id,
            _ => panic!("expected Created"),
        };

        // Get + assert roundtrip
        let got = store.get(id).unwrap().unwrap();
        prop_assert_eq!(got.title, title);
        prop_assert_eq!(got.content, content);
        prop_assert_eq!(got.project, project);
    }

    #[test]
    fn topic_key_upsert_increments_revision(
        topic in "[a-z]{3,20}/[a-z][a-z0-9-]{3,40}",
        title in "\\PC{1,100}",
    ) {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let pool = init_db(tmp.path()).unwrap();
        let store = ObservationStore::new(pool);

        let _ = store.save(SaveInput {
            title: title.clone(),
            content: "v1".into(),
            project: Some("test".into()),
            topic_key: Some(topic.clone()),
            ..Default::default()
        }).unwrap();

        let second = store.save(SaveInput {
            title,
            content: "v2".into(),
            project: Some("test".into()),
            topic_key: Some(topic),
            ..Default::default()
        }).unwrap();

        match second {
            SaveOutcome::UpsertedTopic { revision_count, .. } => {
                prop_assert_eq!(revision_count, 1);
            }
            other => prop_assert!(false, "expected UpsertedTopic, got {:?}", other),
        }
    }

    #[test]
    fn normalized_hash_dedup_increments_duplicate_count(
        content in "\\PC{20,200}",
    ) {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let pool = init_db(tmp.path()).unwrap();
        let store = ObservationStore::new(pool);

        let _ = store.save(SaveInput {
            title: "same title".into(),
            content: content.clone(),
            project: Some("test".into()),
            r#type: ObservationType::Memory,
            ..Default::default()
        }).unwrap();

        let second = store.save(SaveInput {
            title: "same title".into(),
            content,
            project: Some("test".into()),
            r#type: ObservationType::Memory,
            ..Default::default()
        }).unwrap();

        match second {
            SaveOutcome::DuplicateMerged { duplicate_count, .. } => {
                prop_assert_eq!(duplicate_count, 1);
            }
            other => prop_assert!(false, "expected DuplicateMerged, got {:?}", other),
        }
    }

    #[test]
    fn soft_delete_hides_from_list(
        n in 1u32..20u32,
    ) {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let pool = init_db(tmp.path()).unwrap();
        let store = ObservationStore::new(pool);

        let mut ids = Vec::new();
        for i in 0..n {
            let r = store.save(SaveInput {
                title: format!("obs {i}"),
                content: format!("content {i}"),
                project: Some("test".into()),
                ..Default::default()
            }).unwrap();
            if let SaveOutcome::Created(id) = r {
                ids.push(id);
            }
        }

        // Soft-delete half
        let to_delete = n as usize / 2;
        for id in ids.iter().take(to_delete) {
            store.soft_delete(*id).unwrap();
        }

        let visible = store.list(ObservationQuery::default()).unwrap();
        prop_assert_eq!(visible.len(), n as usize - to_delete);
    }
}
```

## Smoke tests CI cross-platform

CI workflow ya cubre: `cargo test --workspace --all-features` en Linux + macOS + Windows. La matriz en `.github/workflows/ci.yml` (creada en bloque A) corre los 3 OS.

Adicionalmente:

- `bash scripts/check-no-stele-residual.sh` (Linux/Mac).
- `powershell scripts/check-no-stele-residual.ps1` (Windows).
- Job `lint`: `cargo clippy --workspace -- -D warnings` + `cargo fmt --check`.
- Job `audit` (semanal, cron): `cargo audit` para vulnerabilities.

## Criterios de aceptación del bloque D

1. `cargo test --workspace --all-features` verde con todos los tests del sprint-01.
2. CI matrix verde en GitHub Actions (Linux + macOS + Windows).
3. Property tests corren 256 casos cada uno (`PROPTEST_CASES=256`).
4. Snapshot test del schema dump (insta) coincide con el schema esperado.
5. `cargo audit` sin vulnerabilidades críticas.
6. Cobertura de tests visible (no enforce mínimo en sprint-01, pero >70% paths del storage layer es razonable).

## Devlog del sprint

`docs/aegis/devlogs/2026-MM-DD-sprint-01-foundation.md`:

- Resumen de los 4 bloques cerrados.
- Tests verdes (count + lista por crate).
- Issues encontrados + cómo se resolvieron (cargo-deny licenses, sqlite-vec en Windows, etc).
- Tokens consumidos en cost-ledger.
- Pendiente para sprint-02: embedder + search.

## Cost-ledger append

`docs/aegis/devlogs/cost-ledger.jsonl`:

```jsonl
{"date":"2026-MM-DD","session_type":"sprint","session_id":"seele-sprint-01-foundation","status":"completed","models":["claude-opus-4-7"],"tokens_in":<X>,"tokens_out":<Y>,"usd_estimated":<Z>,"duration_seconds":<S>,"skills_generated":0,"skills_updated":0,"domain":"seele","notes":"Sprint-01 BE Foundation: Cargo workspace + seele-core + seele-storage. 11 crates skeleton + types + storage layer con FTS5 + vec0 + virtual cols + triggers + migrations + privacy + hash + upsert. ~30 tests verdes en CI Linux+Mac+Win.","bootstrap_estimate":false}
```

## Commit del bloque D

```
git add -A
git commit -m "sprint-01 bloque-D — tests integración + property tests + CI matrix verde"
git push
```

Y al cerrar el sprint completo:

```
git tag -a sprint-01-foundation -m "Sprint-01 BE Foundation completed"
git push --tags
```
