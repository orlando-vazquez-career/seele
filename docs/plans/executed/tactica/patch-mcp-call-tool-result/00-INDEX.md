# Patch — MCP `tools/call` devuelve `result` sin envelope `CallToolResult`

**Fecha**: 2026-05-20
**Estado**: ✅ Ejecutado 2026-05-20. Cierre pendiente de Gate 2 (commit + tag opcional `v0.2.1` por el usuario). Devlog: [`../../../../aegis/devlogs/2026-05-20-patch-mcp-call-tool-result.md`](../../../../aegis/devlogs/2026-05-20-patch-mcp-call-tool-result.md).
**Pre-requisitos**: v0.2.0 released (commit `d28aad1`). Working tree limpio antes de arrancar.
**SemVer target**: `v0.2.1` (patch).
**Naturaleza**: hotfix de un solo bloque — no es un sprint multi-bloque.

---

## Diagnóstico

`seele mcp` (stdio JSON-RPC 2.0) responde a `tools/call` con un `result` que **es el JSON crudo del handler**, no el envelope `CallToolResult` que exige la spec MCP. Captura empírica del wire enviando un `tools/call` con `name=seele_doctor`:

```json
{"jsonrpc":"2.0","id":2,"result":{
  "status":"ok",
  "embedder":{"model_id":"seele/fake-embedder","dim":384,"expected_sha256":null},
  "observations_active":52,
  "sessions_total":1,
  "schema_version":"0.1.0"
}}
```

Lo que la spec MCP (`protocolVersion >= 2024-11-05`) exige:

```json
{"jsonrpc":"2.0","id":2,"result":{
  "content":[{"type":"text","text":"{\"status\":\"ok\",...}"}],
  "isError":false
}}
```

Como falta el `content[]`, los clientes MCP (Claude Code, Cursor, Windsurf) buscan `result.content[0].text` y encuentran `undefined` → renderizan "completed with no output" → la memoria transversal queda inutilizable aunque la DB y los handlers funcionan correctamente.

### Causa raíz exacta

`crates/seele-mcp/src/server.rs:137-141`:

```rust
match (tool.handler)(&self.service, arguments) {
    Ok(v) => Response::ok(id, v),                  // ← devuelve el JSON crudo
    Err(e) => Response::err(id, e.to_jsonrpc()),
}
```

`Response::ok(id, v)` setea `result: Some(v)` directo. No hay paso intermedio que envuelva en `CallToolResult`.

### Por qué los tests no lo agarraron

Los tests de `seele-mcp` (`crates/seele-mcp/tests/`) validan el dispatch interno (request → handler → serialización), pero **no verifican la forma del wire response contra la spec MCP**. Comparan `response.result` contra el JSON esperado del handler, no contra `{ content: [...], isError: ... }`. En consecuencia el integration test pasa verde mientras los clientes reales rompen silencio.

### Impacto

- **Producción**: cero adoption útil en agents (Claude Code / Cursor / Windsurf). La CLI y HTTP siguen funcionando — solo MCP rompe.
- **Severidad**: Alta. SEELE se vende como memory engine para AI agents; el transporte MCP es el camino primario para los tres targets oficiales.
- **Antigüedad del bug**: presente desde Sprint-03 (cuando se introdujo `seele-mcp`). Sobrevivió a v0.1.0 y v0.2.0 sin detección. Implica que ningún test de extremo a extremo desde un cliente MCP real corrió en CI ni en aceptación humana.

---

## Objetivo

Restablecer la integración MCP. Tras el patch, un `tools/call` a cualquiera de los 19 tools devuelve un `result` que cumple la shape `CallToolResult` y los clientes Claude Code / Cursor / Windsurf renderizan el output del tool sin "completed with no output".

Tag SemVer `v0.2.1` opcional al cierre — el usuario decide si quiere release inmediato o agrupar con otros fixes pendientes.

---

## Out of scope (defer)

- **Tool errors via `isError: true`**: la spec MCP permite reportar errores de dominio (NotFound, BadParams, Conflict) vía `{ content: [...error message...], isError: true }` en vez de `error` JSON-RPC. El comportamiento actual usa `Response::err` (JSON-RPC error), que también es válido per spec. Este patch **no cambia el path de errores**; solo arregla el path de éxito. Refactor a `isError` queda para v0.3.
- **Structured content / `structuredContent`** (MCP 2025-06-18): el server declara `protocolVersion: "2024-11-05"` en su handshake, así que no entrega ni promete structured content. Migrar a `2025-06-18` queda fuera de scope.
- **Resources / Prompts capabilities**: no se tocan.
- **Auditoría exhaustiva de los otros métodos** (`initialize`, `tools/list`): se inspeccionan visualmente y se confirma que ya tienen la shape correcta (initialize devuelve `serverInfo`/`protocolVersion`/`capabilities`; tools/list devuelve `{tools: [...]}`). Si alguno también está roto, se trata como un sub-finding y se anota — pero el scope nominal es `tools/call`.

---

## Bloque A — Envolver tool results en `CallToolResult`

### Cambio de código

`crates/seele-mcp/src/server.rs::McpServer::dispatch_tool_call`:

```rust
match (tool.handler)(&self.service, arguments) {
    Ok(v) => {
        // MCP spec (2024-11-05+) exige que tools/call devuelva un
        // CallToolResult { content: [...], isError? }. El handler nos
        // entrega un JSON arbitrario; lo serializamos a string y lo
        // ponemos en un único bloque `text`.
        let text = serde_json::to_string(&v).unwrap_or_else(|_| "<unserializable>".into());
        Response::ok(id, serde_json::json!({
            "content": [{"type": "text", "text": text}],
            "isError": false,
        }))
    }
    Err(e) => Response::err(id, e.to_jsonrpc()),
}
```

Notas:

- `serde_json::to_string(&v)` no debería fallar para un `Value` (todos los `Value` son round-trippables), pero usamos `unwrap_or_else` defensivamente. Si pasa, devolvemos un sentinel que es visible para el usuario en vez de cortar la sesión.
- `unwrap_or_else` es preferible a `unwrap` o `?` — un panic acá rompería toda la sesión MCP, y el `?` exigiría mapear `serde_json::Error` a `ErrorObject` para ese camino imposible.
- **Decisión técnica**: se envuelve **toda** la salida en un único bloque `text`. La alternativa (devolver el JSON en `structuredContent`) requiere `protocolVersion: 2025-06-18`, que está fuera de scope. Un único bloque `text` con el JSON serializado es el camino estándar pre-2025-06.

### Localización

- Archivo: `crates/seele-mcp/src/server.rs`
- Función: `dispatch_tool_call`
- Líneas: 137-141 (actuales)

### LOC estimado

- Source: ~8 líneas modificadas (de 3 a ~10).
- Tests: ~30 líneas (un test nuevo, ver Bloque B).

---

## Bloque B — Test del envelope MCP en `crates/seele-mcp/tests/`

### Test nuevo

`crates/seele-mcp/tests/call_tool_result_envelope.rs` — un test de integración que:

1. Construye un `McpServer` con un `SeeleService` in-memory + `FakeEmbedder` (mismo helper que ya usan los otros tests del crate).
2. Llama `server.handle_line(...)` con un request `tools/call` para `seele_doctor` (no requiere argumentos, no toca filesystem).
3. Verifica:
   - `response.result.is_some()`.
   - `result["content"]` es un array de 1 elemento.
   - `result["content"][0]["type"] == "text"`.
   - `result["content"][0]["text"]` es un string parseable como JSON.
   - El JSON parseado contiene la key `"status"` con valor `"ok"` (sanity check: el handler corrió de verdad).
   - `result["isError"] == false`.

### Por qué `seele_doctor`

- No requiere parámetros (`empty_object_schema`).
- No toca filesystem ni red.
- Su payload de éxito es estable (`status`, `embedder`, `observations_active`, `sessions_total`, `schema_version`) — fácil de assertar sin acoplar al estado de la DB.

### Tests existentes — auditoría rápida

Antes de cerrar el bloque, recorrer `crates/seele-mcp/tests/*.rs` y `crates/seele-mcp/src/` para identificar tests que asserteaban contra `result.<algo>` directo (la shape vieja). Si los hay, ajustarlos a `result.content[0].text` parseado. Espera-baja-confianza pero hay que mirar: los tests podrían estar comparando `response.result["status"]` o similar.

### LOC estimado

- ~50-70 LOC (test nuevo + ajustes a tests existentes si los hay).

---

## Bloque C — Verificación + state-sync AEGIS

1. **Build**: `cargo build --workspace`. Verde.
2. **Tests**: `cargo test --workspace`. Todos los pre-existentes + el nuevo. Verde.
3. **Clippy**: `cargo clippy --workspace --all-targets -- -D warnings`. Verde.
4. **Fmt**: `cargo fmt --all -- --check`. Verde.
5. **STELE residual**: `pwsh scripts/check-no-stele-residual.ps1` (Windows). Verde.
6. **Smoke manual del wire**: `printf '...' | seele mcp` con `tools/call seele_doctor`, verificar que `result.content[0].text` es un JSON parseable con `"status":"ok"`.
7. **CHANGELOG**: append a sección `[Unreleased]` un `### Fixed` con el bug y el fix.
8. **Devlog**: `docs/aegis/devlogs/2026-05-20-patch-mcp-call-tool-result.md` con resumen, evidencia del wire pre/post, bloques cumplidos, tests añadidos, decisiones técnicas, follow-ups (incluyendo el `isError` refactor deferido).
9. **Cost ledger**: append a `docs/aegis/devlogs/cost-ledger.jsonl`.
10. **Mover plan**: `docs/plans/tactica/patch-mcp-call-tool-result/` → `docs/plans/executed/tactica/patch-mcp-call-tool-result/`.

**NO se commitea ni se pushea automáticamente**. Gate 2 humano: el usuario revisa el diff completo, decide commit message + si quiere tag `v0.2.1`, y ejecuta los `git add`/`commit`/`push`/`tag` manualmente.

---

## Criterios de salida (Definition of Done)

- [ ] `dispatch_tool_call` envuelve `Ok(v)` en `{content: [{type: "text", text: stringified}], isError: false}`.
- [ ] Test nuevo `call_tool_result_envelope.rs` pasa.
- [ ] Tests pre-existentes del workspace siguen verdes (no regresiones).
- [ ] Clippy + fmt + STELE residual verdes.
- [ ] Smoke manual del wire confirma envelope correcto.
- [ ] CHANGELOG `[Unreleased]` actualizado.
- [ ] Devlog escrito en `docs/aegis/devlogs/2026-05-20-patch-mcp-call-tool-result.md`.
- [ ] Cost ledger appended.
- [ ] Plan movido a `executed/`.
- [ ] Tag git pendiente — decisión del usuario en Gate 2.

---

## Follow-ups identificados (no bloquean el cierre)

- **MCP integration test contra un cliente real**: hoy ningún test corre un cliente MCP de verdad contra `seele mcp`. Considerar agregar uno usando el SDK Rust de `mcp` (si existe estable) o un script de smoke en CI que haga lo mismo que el wire-probe manual.
- **`isError: true` para errores de dominio** (NotFound / Conflict / BadParams): hoy se reportan como JSON-RPC error. Spec MCP también permite reportarlos vía `isError: true` en el content envelope — los clientes lo manejan mejor que un JSON-RPC error genérico.
- **Migración a `protocolVersion: 2025-06-18`**: gana `structuredContent` (devolver el JSON estructurado en vez de stringified), `_meta` campos, y mejores capabilities. Defer a v0.3.
- **Lint sobre el archivo de CLAUDE.md del repo**: dice "v0.1" en varios lugares pero ya está v0.2.0 released. Update separado.

---

## Referencias

- Spec MCP (tools/call): https://modelcontextprotocol.io/specification/2024-11-05/server/tools.
- Issue raíz observado en CivicSys (hackathon Syscoin) — sesión Orlando 2026-05-20.
- Repo SEELE: `C:/dev/tools/SEELE` (este).
- Hand-off file producido por el debugging: respuestas vacías al llamar `mcp__seele__seele_doctor`, `mcp__seele__seele_version`, `mcp__seele__seele_stats` desde Claude Code 4.7.
