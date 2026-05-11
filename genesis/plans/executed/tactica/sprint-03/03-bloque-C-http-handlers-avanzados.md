# Sprint-03 Bloque C — HTTP handlers avanzados

**Tema**: Handlers para sessions, links, relations, stats, schema. Aprox 12-14 endpoints adicionales.

**Pre-requisitos**: Bloque B cerrado.

## Endpoints

### Sessions (5)

- `POST /sessions` — start (body: `{project, directory?}`).
- `GET /sessions` — list (query: `?project=&status=&limit=`).
- `GET /sessions/{id}` — show.
- `PUT /sessions/{id}/end` — end (body: `{summary?}`).
- `PUT /sessions/{id}/abort` — abort.

### Links (3)

- `POST /links` — create (body: `{from_id, to_id, link_type, metadata?}`).
- `GET /memories/{id}/links` — list links from/to memory.
- `DELETE /links/{id}` — remove.

### Relations (4)

- `POST /relations` — create (body: `{sync_id, source_id, target_id, relation, reason?, evidence?, confidence?}`).
- `GET /relations` — list (query: `?source_id=&target_id=&status=`).
- `PUT /relations/{id}/judge` — judge (body: `{status, reason?, evidence?, confidence?}`).
- `GET /conflicts` — list conflicts pending (status='pending', relation='conflicts_with').

### Stats + meta (2)

- `GET /stats` — total observations/sessions/projects + breakdown por type/scope.
- `GET /embedder` — current model + dim + expected_sha256.

## Tareas atómicas

### C.1 — Service methods

Agregar a `SeeleService` los métodos (8-12 nuevos):

```rust
impl SeeleService {
    // Sessions
    pub fn start_session(&self, project: &str, directory: Option<&str>) -> Result<Session>;
    pub fn list_sessions(&self, filter: SessionFilter) -> Result<Vec<Session>>;
    pub fn get_session(&self, id: SeeleId) -> Result<Option<Session>>;
    pub fn end_session(&self, id: SeeleId, summary: Option<String>) -> Result<()>;
    pub fn abort_session(&self, id: SeeleId) -> Result<()>;

    // Links
    pub fn create_link(&self, input: LinkInput) -> Result<Link>;
    pub fn list_links_for(&self, observation_id: SeeleId) -> Result<Vec<Link>>;
    pub fn delete_link(&self, id: SeeleId) -> Result<()>;

    // Relations
    pub fn create_relation(&self, input: RelationInput) -> Result<MemoryRelation>;
    pub fn list_relations(&self, q: RelationQuery) -> Result<Vec<MemoryRelation>>;
    pub fn judge_relation(&self, id: SeeleId, judgment: JudgmentInput) -> Result<()>;
    pub fn list_conflicts_pending(&self, limit: u32) -> Result<Vec<MemoryRelation>>;

    // Stats
    pub fn stats(&self) -> Result<StatsResponse>;
}
```

### C.2 — Handlers HTTP

Agrupados por dominio en `src/handlers/sessions.rs`, `links.rs`, `relations.rs`, `stats.rs`.

### C.3 — Wire en router

Agregar las ~14 nuevas rutas en `server.rs::router()`.

### C.4 — Tests E2E

`crates/seele-http/tests/handlers_avanzados.rs`:

Aprox 10-15 tests cubriendo:
- Session lifecycle start → list → end.
- Session abort.
- Link create → list-for-memory → delete.
- Relation create → list → judge.
- Conflict pending listing excluye judged.
- Stats retorna counts.

## Criterios de aceptación del bloque C

1. `cargo test -p seele-http` verde con tests acumulados (~22-25 total).
2. Todos los endpoints retornan JSON con shape consistente.
3. Errores 404 cuando ID no existe; 400 cuando input inválido.

## Commit del bloque C

```
git add -A
git commit -m "sprint-03 bloque-C — http handlers avanzados (sessions + links + relations + stats)"
```
