# Táctica v0.4 — Dual-runtime y robustez (tareas atómicas)

**Estado**: Propuesta · 2026-07-27 · **Estrategia**: `docs/plans/estrategia/v0.4-dual-runtime-y-robustez/00-overview.md` · **ADR**: `docs/plans/arquitectura/16-decisiones-v0.4.md`
**Convención**: cada tarea es cerrada, verificable (done_cmd), aislada (files_touched disjuntos por oleada) y acotada. Oleadas de ≤3 despachos paralelos.

## Oleada 1 — Robustez (WS0)

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-01 | `PRAGMA busy_timeout=5000` al abrir conexiones + retry-once con backoff en errores SQLITE_BUSY, en la capa storage. Test de contención: 2 procesos × 50 saves concurrentes sin "database is locked". | `crates/seele-storage/src/pool.rs`, nuevo test | `cargo test -p seele-storage busy` |
| T-02 | Agente `kimi-code` en el registry de setup: variante + `as_str` + `all` + `is_implemented` + match arm a `install_mcp_json(~/.kimi-code/mcp.json, "mcpServers")`. Test e2e sobre HOME temporal. | `crates/seele-setup/src/agents.rs`, `crates/seele-setup/tests/` | `cargo test -p seele-setup kimi` |
| T-03 | Default `score_boost_multiplier` = 1.0 en HTTP dto, CLI search, TUI app (ADR-16 D3). Ajustar tests que asuman 0.0. | `crates/seele-http/src/dto.rs`, `crates/seele-cli/src/commands/search.rs`, `crates/seele-tui/src/app.rs` | `cargo test -p seele-http -p seele-cli score` |

## Oleada 2 — Seguridad y superficie (WS0 cont.)

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-04 | Bearer: comparación constant-time (`subtle::ConstantTimeEq`) + aceptar `--auth-bearer $ENVVAR` (leer del entorno si el valor empieza con `$`, patrón ya usado por `--chat-key`). | `crates/seele-http/src/auth.rs`, `crates/seele-cli/src/commands/serve.rs` | `cargo test -p seele-http auth` |
| T-05 | `seele save --content -` lee stdin; help declara límite de 256 tokens del embedding. `import.rs`: quitar advertencia obsoleta "until Sprint-05". | `crates/seele-cli/src/commands/save.rs`, `crates/seele-cli/src/commands/import.rs` | `cargo test -p seele-cli save` |
| T-06 | Pinear hash del modelo por defecto en `TRUSTED_HASHES` (obtener SHA-256 real del archivo ONNX actual y verificar descarga). | `crates/seele-embedder/src/onnx.rs` | `cargo test -p seele-embedder trusted` |

## Oleada 3 — Transporte e integridad (WS0 + WS1)

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-07 | REST: `GET /projects` y `PATCH /memories/{id}` (delegan en `SeeleService::list_projects` y `merge_observation_metadata`) + entradas OpenAPI (el test `openapi_consistency` las exige). | `crates/seele-http/src/server.rs`, handlers, `openapi.rs` | `cargo test -p seele-http openapi_consistency` |
| T-08 | `seele embedder reembed-all`: re-embedde filas sin vector o con modelo distinto al vigente; batch con progreso, idempotente; hace real `import --re-embed`. ✅ Ejecutada: método en `seele-http/src/service.rs` (el `SeeleService` vive ahí; seele-core es hoja del DAG), SQL en `seele-storage/src/observations.rs`. | `crates/seele-cli/src/commands/embedder.rs` (nuevo), `crates/seele-http/src/service.rs`, `crates/seele-storage/src/observations.rs`, `crates/seele-cli/src/commands/import.rs` | `cargo test -p seele-cli reembed` |
| T-09 | Borrar `user_prompts` + `prompts_fts` + `PromptStore` (ADR-16 D2): migración SQL drop, eliminar store y referencias. | migraciones, `crates/seele-core/src/service.rs`, `crates/seele-storage/` | `cargo test -p seele-core && cargo test -p seele-storage` |

## Oleada 4 — Wiring y operación (WS1 + WS4)

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-10 | `seele save` sin `--project` usa `seele_project::detect(&cwd)` (ADR-16 D4, solo CLI). | `crates/seele-cli/src/commands/save.rs`, `Cargo.toml` (use real) | `cargo test -p seele-cli project` |
| T-11 | `seele backup <destino>` (VACUUM INTO) + chequeo FTS optimize en `doctor`. | `crates/seele-cli/src/commands/`, `crates/seele-core/src/doctor.rs` | `cargo test -p seele-cli backup` |
| T-12 | Op-log `_history.jsonl` por proyecto: append en cada save/delete/restore (op, ts, id). ✅ Ejecutada como `<db>.history.jsonl` (un archivo por DB, slug `project` por línea), opt-in vía builder, wireado en el CLI. | `crates/seele-http/src/service.rs`, `crates/seele-cli/src/app.rs` | `cargo test -p seele-http history` |
| T-13 | Feature-gates `tui`/`chat`/`eval` fuera del binario por defecto de `seele-cli` (features cargo, CI compila ambas variantes). | `crates/seele-cli/Cargo.toml`, `crates/seele-cli/src/main.rs` | `cargo check -p seele-cli --no-default-features` |

## Oleada 5 — Ingesta (WS4)

| # | Tarea | files_touched | done_cmd |
|---|---|---|---|
| T-14 | Import batch genérico JSONL (`seele import from-jsonl <file>`): una observación por línea con el schema del save; dedup por topic-key; reporte de conteos. | `crates/seele-cli/src/commands/import.rs` | `cargo test -p seele-cli jsonl` |
| T-15 | `capture_passive`: aceptar headings localizados (`## Key Learnings:` + `## Aprendizajes:`) y documentar la convención en AGENT-SETUP. | `crates/seele-mcp/src/tool_impls/sessions.rs`, `docs/AGENT-SETUP.md` | `cargo test -p seele-mcp capture` |

## Gated (no se despachan hasta su condición)

- **WS2** (two-step FTS5, retrieval_queries, cascade rescue, superseded demotion, recency): condición = baseline `seele-eval` (estrategia v0.3) con número registrado.
- **WS3** (merge rules GRAIL sobre `links`, 1-hop bonus, consolidate, MinHash): condición = WS2 cerrado + ADR-16 D1 ratificada por el humano en Gate 2.

## Notas de ejecución

- Cada tarea: tests que fallan antes y pasan después (AEGIS Guardrails), análisis estático (`cargo clippy -- -D warnings`, `cargo fmt --check`) antes del handoff.
- `cargo test` completo en verde al cierre de cada oleada.
- Al terminar todas las oleadas: Gate 2 humano → state-sync (mover a `executed/`, devlog, CHANGELOG, version bump v0.4.0).
