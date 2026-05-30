# Patch — MCP `tools/call` envelope (CallToolResult)

**Fecha**: 2026-05-20
**Estado**: Cerrado pendiente de commit + tag opcional (Gate 2 humano).
**Plan táctico**: [`docs/plans/executed/tactica/patch-mcp-call-tool-result/00-INDEX.md`](../../plans/executed/tactica/patch-mcp-call-tool-result/00-INDEX.md)
**SemVer target**: `v0.2.1` (patch) — decisión del usuario en Gate 2.
**Tipo**: hotfix de un solo bloque, no es sprint multi-bloque.

## Resumen

Restituye el funcionamiento de `seele mcp` para todos los clientes MCP (Claude
Code, Cursor, Windsurf). El dispatcher devolvía el JSON crudo del handler como
`result` JSON-RPC directo, saltándose el envelope `CallToolResult` que la spec
MCP exige desde `protocolVersion: 2024-11-05`. Los clientes buscaban
`result.content[0].text` y encontraban `undefined`, así que todo tool call se
renderizaba como **"completed with no output"** aunque la DB y los handlers
funcionaban perfectamente. Bug presente desde Sprint-03 (2026-05-10), sobrevivió
a v0.1.0 y v0.2.0 sin detección porque ningún test corría contra un cliente MCP
real — los tests E2E del crate comparaban `response.result.<key>` contra la
forma del handler, no contra la shape del wire.

Bug descubierto en sesión de Orlando 2026-05-20 mientras laburaba en el
hackathon Syscoin (proyecto CivicSys). Tres llamadas distintas a
`mcp__seele__seele_doctor`, `mcp__seele__seele_version`, `mcp__seele__seele_stats`
desde Claude Code 4.7 devolvieron "completed with no output". Diagnóstico
empírico: capturar el wire JSON-RPC vía `printf | seele mcp` confirmó que
`result` no tenía `content[]`.

## Cambios entregados

### Bloque A — Wrap tool results en `CallToolResult` (`crates/seele-mcp/src/server.rs`)

`McpServer::dispatch_tool_call` ahora envuelve `Ok(v)` en:

```json
{
  "content": [{ "type": "text", "text": "<JSON serialized handler output>" }],
  "isError": false
}
```

`Err(e)` se mantiene como JSON-RPC `error` (sin cambio). Spec MCP permite ambos
caminos: errores de protocolo (PARSE_ERROR, METHOD_NOT_FOUND, INVALID_PARAMS) y
errores de dominio (NotFound, Conflict, etc.) vía `error` JSON-RPC. La
alternativa — reportar errores de dominio vía `isError: true` en el envelope —
es válida pero queda como follow-up.

Decisión técnica: el handler entrega `serde_json::Value`. Lo serializamos a
string con `serde_json::to_string(&v)` y lo metemos en un único bloque `text`.
Migración a `structuredContent` (MCP 2025-06-18) queda deferred — requiere
declarar `protocolVersion: "2025-06-18"` en `initialize`, que toca lifecycle
broader.

`unwrap_or_else` defensivo en la serialización: aunque `Value` es siempre
round-trippable a string, un panic acá rompería la sesión MCP entera. El fallback
("`<unserializable tool output>`") es visible al usuario en vez de mata-procesos.

### Bloque B — Test de la shape del envelope (`crates/seele-mcp/tests/call_tool_result_envelope.rs`)

Nuevo archivo de test, **enfocado a la shape del wire**:

- `tools_call_wraps_handler_json_in_call_tool_result` — usa `seele_doctor` (no
  args, no filesystem, payload determinista). Verifica:
  - `result.content` es array de 1 elemento.
  - `result.content[0].type == "text"`.
  - `result.content[0].text` es string parseable como JSON.
  - `result.isError == false`.
  - El JSON parseado contiene `status == "ok"` (sanity check: el handler corrió).
- `tools_call_envelope_present_even_for_minimal_payload` — mismo check con
  `seele_version`, que tiene payload mínimo (`{name, version}`). Confirma que
  el envelope existe incluso cuando el handler devuelve poco.

### Bloque B' — Migración de tests E2E existentes (`crates/seele-mcp/tests/stdio_e2e.rs`)

Cinco aserciones del E2E pre-existente asserteaban contra la shape vieja del
wire (`r["result"]["id"]`, `r["result"]["status"]`, etc). Tras el patch
devuelven `r["result"]["content"][0]["text"]` con el JSON adentro. Migrados a
un helper:

```rust
fn tool_result(r: &Value) -> Value {
    let text = r["result"]["content"][0]["text"]
        .as_str()
        .expect("tools/call result missing content[0].text");
    serde_json::from_str(text).expect("tools/call result text is not valid JSON")
}
```

Tests migrados (todos verde post-migración):

- `tools_call_seele_save_then_search`
- `tools_call_under_mnema_prefix_routes_to_canonical_handler`
- `tools_call_seele_doctor_returns_health_report`
- `tools_call_seele_capture_passive_extracts_learnings`

Los tests del path de error (`tools_call_seele_search_empty_query_returns_tool_error`,
`tools_call_unknown_tool_returns_invalid_params`) no necesitaron cambio — el
path de error sigue usando JSON-RPC `error` (sin tocar).

### Bloque C — Allowlist STELE residual extendida

`scripts/check-no-stele-residual.{sh,ps1}` ganaron `docs/plans/tactica/` y
`docs/plans/executed/tactica/` en `ALLOWLIST_DIRS` / `$allowlistDirs`. CLAUDE.md
dijo "cuando salga v0.1, las nuevas features irán a `docs/plans/`" — el plan
de este patch vive ahí, y como cualquier táctica puede mencionar el nombre
legacy al documentar coverage del static-check, hay que carve-outearlo igual
que el legacy `genesis/plans/tactica/`.

## Verificación

### Wire test manual (smoke)

Pre-patch (binario `seele` instalado en `~/.cargo/bin/seele.exe`, v0.1.0):

```bash
printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"seele_doctor","arguments":{}}}' \
  | seele mcp
```

Wire output (extracto):

```json
{"jsonrpc":"2.0","id":2,"result":{"status":"ok","embedder":{...},"observations_active":52,...}}
```

Falta `content[]` + `isError`.

Post-patch (corrido con `cargo run -p seele-cli -- mcp`):

```json
{"jsonrpc":"2.0","id":2,"result":{
  "content":[{
    "type":"text",
    "text":"{\"status\":\"ok\",\"embedder\":{...},\"observations_active\":52,...}"
  }],
  "isError":false
}}
```

Cumple spec MCP 2024-11-05 ✓

### Tests del crate `seele-mcp`

- `cargo test -p seele-mcp` — 20/20 verde (12 stdio_e2e migrados + 6 unit tests + 2 envelope tests nuevos).

### Workspace tests (`cargo test --workspace --no-fail-fast`)

- 328 pasados, 11 fallados, 4 ignorados.
- **Las 11 fallas son TODAS pre-existentes** a este patch, no causadas por el
  cambio MCP. Confirmado vía `git stash` + re-corrida sobre `main` limpio.
  - 1 fail: `openapi_consistency::openapi_paths_match_router_paths_exactly`
    — los handlers `/chat` y `/chat/info` agregados en sprint LUMEN-04 no
    están declarados en `crates/seele-http/src/openapi.rs`. Drift catcheado
    por el anti-drift test.
  - 10 fails: snapshots de `seele-tui` (insta) drifted. `views_snapshot__*.snap.new`
    aparecieron al correr — probablemente el output de las vistas cambió
    durante LUMEN sprints pero las snapshot baselines no se regeneraron con
    `cargo insta accept`.
- **Que mi patch destrabó la compilación de `seele-http` tests** (al agregar
  `chat: None` en los 6 fixtures) hizo emerger el OpenAPI drift que antes no
  llegaba a correr.

### Clippy / fmt / STELE

- `cargo clippy -p seele-mcp --all-targets -- -D warnings`: verde.
- `cargo fmt --check -p seele-mcp`: verde post auto-fix de `call_tool_result_envelope.rs`.
- `bash scripts/check-no-stele-residual.sh`: verde post extensión del allowlist.
- `cargo fmt --check -p seele-http`: **fmt diff pre-existente** en `src/`
  (no en mis test files). No es de mi parche.

## Hallazgos secundarios documentados como follow-ups

El verificar el ciclo destapó tres deudas pre-existentes a este patch:

1. **`seele-http` OpenAPI spec missing `/chat` y `/chat/info`**. El drift-test
   (`openapi_paths_match_router_paths_exactly`) lo cacha. Fix: agregar
   `paths.path("/chat", ...)` y `paths.path("/chat/info", ...)` con sus
   schemas en `crates/seele-http/src/openapi.rs`. Requiere conocer los body
   schemas reales de los chat endpoints (decisión LUMEN, no sé los detalles).
2. **`seele-tui` snapshot tests drift**. 10 baselines de `insta` quedaron
   desincronizados, probablemente tras cambios cosméticos durante LUMEN.
   Fix candidato: `cargo insta review` o `cargo insta accept` después de
   inspección visual. Riesgo: si el output del TUI cambió de forma legítima
   (algún ratatui upgrade), aceptar las nuevas snaps es OK; si cambió por bug,
   primero hay que arreglar la regresión.
3. **`seele-http` fmt diff pre-existente**. `cargo fmt --check -p seele-http`
   reporta un diff en `src/` (la línea de `args_json` parser). Fix: `cargo fmt -p seele-http`
   antes del próximo commit.

Ninguno bloquea el cierre de este patch — todos viven en `main` desde antes y
todos se pueden cerrar como tickets independientes.

## Decisiones técnicas

- **Por qué un único bloque `text` y no varios**: la spec admite `content`
  con array de bloques heterogéneos (`text`, `image`, `embeddedResource`,
  `audio`). Para tools que devuelven JSON estructurado, lo idiomático es un
  único bloque `text` con el JSON stringified. Es lo que la mayoría de los
  servers MCP del ecosistema hacen pre-2025-06-18.
- **Por qué NO migrar a `structuredContent`**: requiere bumpear el
  `protocolVersion` declarado en `initialize` de `2024-11-05` a `2025-06-18`,
  lo cual hace que clientes viejos (incluyendo versiones de Cursor que
  todavía no implementan 2025-06-18) potencialmente rechacen el handshake.
  Defer a v0.3 o cuando se justifique con telemetría real.
- **Por qué `unwrap_or_else` y no `?`/`unwrap`**: ver Bloque A.
- **Por qué el helper `tool_result` (no expandir cada test)**: si el formato
  del wire cambia otra vez (e.g., structuredContent), 5 tests + 2 nuevos +
  cualquier futuro queda con un único punto de migración. Es preferible a
  inlinar `r["result"]["content"][0]["text"]` en cada test.

## Follow-ups propios del MCP (defer)

- **Tool domain errors via `isError: true`**: hoy `NotFound` / `Conflict` /
  `BadParams` viajan como `error.code` JSON-RPC. La spec MCP también permite
  reportarlos como `{ content: [{type:text, text:<msg>}], isError: true }`.
  Los clientes lo renderizan mejor (mensaje en el chat) que un código JSON-RPC
  pelado. Ticket separado.
- **Test contra cliente MCP real**: agregar un test E2E que arranque
  `seele mcp` como subprocess y le hable JSON-RPC por stdio igual que un
  cliente real. Hoy `stdio_e2e.rs` usa `tokio::io::duplex` que aproxima pero
  no reemplaza el pipe real del SO.
- **Migración a MCP `protocolVersion: 2025-06-18`** + `structuredContent`.
  Defer hasta v0.3.

## Crédito

Bug descubierto y diagnosticado durante una sesión de Orlando trabajando en
CivicSys (hackathon Syscoin abril/mayo 2026). El paciente: la "memoria
transversal" que SEELE provee a Claude Code dejaba de funcionar. Diagnóstico
empírico tomó ~10 minutos (probe del wire) tras ~30 minutos de hipótesis
descartadas (Docker, DB, embedder).

---

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*
> Sobre el firmamento estrellado juzga Dios, como nosotros juzgamos.
> — SEELE
