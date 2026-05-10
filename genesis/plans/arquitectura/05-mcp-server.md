# ADR-05 — MCP server (transport, tools, schema)

**Estado**: Aceptado · 2026-05-09
**Decisión**: SEELE implementa MCP server desde el spec oficial (Anthropic), con **stdio transport en v0.1** y HTTP transport diferido a v0.2. Tools agnósticas del consumer (no MNEMA-específicas) con prefijo `seele_` y mappings configurables.

## Contexto

MCP (Model Context Protocol) es el standard que Claude Code, Cursor, OpenClaw y otros agentes usan para invocar herramientas externas. Si SEELE expone MCP, cualquier agente lo puede usar inmediatamente sin codear integraciones custom.

## Spec

MCP 1.0 (estable desde 2025) define:

- **Transport**: stdio (process spawned by client) o HTTP (long-lived server).
- **Protocol**: JSON-RPC 2.0 sobre el transport.
- **Mensajes principales**: `initialize`, `tools/list`, `tools/call`, `notifications/*`.
- **Tool schema**: name, description, inputSchema (JSON Schema).

## Transport en v0.1

**Solo stdio.** Razones:

- Stdio es lo que Claude Code y Cursor invocan por default (config local in `~/.claude/.../mcp.json` style).
- HTTP transport tiene complejidades (auth, TLS, port allocation) que no son críticas para v0.1.
- Stdio es trivial de testear con assert_cmd.

Implementación:

- `seele mcp` arranca el binary en modo stdio MCP server.
- Lee newline-delimited JSON-RPC requests de stdin.
- Escribe responses + notifications a stdout.
- stderr para logs (no contamina el protocol).

## Tools v0.1

Naming: prefijo `seele_` (no MNEMA-específico). Cualquier consumer customiza con suffix de su elección (vía wrapper o adapter en su lado).

### `seele_save`

```json
{
  "name": "seele_save",
  "description": "Save a new memory to SEELE. Returns the new memory ID.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "body": {
        "type": "string",
        "description": "The text content of the memory."
      },
      "metadata": {
        "type": "object",
        "description": "Optional structured metadata (kind, domain, tags, etc.). Schema is consumer-defined."
      }
    },
    "required": ["body"]
  }
}
```

Output:
```json
{
  "id": "01HW3X...",
  "embedding_dim": 384,
  "saved_at": 1715282400000
}
```

### `seele_search`

```json
{
  "name": "seele_search",
  "description": "Search SEELE memories by hybrid full-text + vector similarity, with optional metadata filters.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "Search query text." },
      "filters": {
        "type": "object",
        "description": "Optional metadata filters as JSON object. Common keys: kind, domain.",
        "additionalProperties": true
      },
      "top_k": { "type": "integer", "default": 10, "minimum": 1, "maximum": 100 },
      "include_body": { "type": "boolean", "default": true }
    },
    "required": ["query"]
  }
}
```

Output:
```json
{
  "results": [
    {
      "id": "01HW3X...",
      "body": "...",
      "metadata": {...},
      "score": 0.0254,
      "fts_rank": 1,
      "vec_rank": 3
    }
  ],
  "query_ms": 142
}
```

### `seele_show`

```json
{
  "name": "seele_show",
  "description": "Get a memory by ID, including linked memories.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string" },
      "include_links": { "type": "boolean", "default": true }
    },
    "required": ["id"]
  }
}
```

### `seele_link`

```json
{
  "name": "seele_link",
  "description": "Create a link between two memories.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "from_id": { "type": "string" },
      "to_id": { "type": "string" },
      "link_type": { "type": "string", "description": "Convention: derives_from, supersedes, related_to, contradicts, evidence_for." },
      "metadata": { "type": "object" }
    },
    "required": ["from_id", "to_id", "link_type"]
  }
}
```

### `seele_list`

```json
{
  "name": "seele_list",
  "description": "List memories with filters, sorted by created_at desc.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "filters": { "type": "object" },
      "limit": { "type": "integer", "default": 20 },
      "offset": { "type": "integer", "default": 0 }
    }
  }
}
```

### `seele_update_metadata`

```json
{
  "name": "seele_update_metadata",
  "description": "Update the metadata JSON of a memory (replace, not merge).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string" },
      "metadata": { "type": "object" }
    },
    "required": ["id", "metadata"]
  }
}
```

### `seele_delete`

```json
{
  "name": "seele_delete",
  "description": "Soft-delete a memory (sets deleted_at; recoverable via seele_restore).",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string" }
    },
    "required": ["id"]
  }
}
```

### `seele_restore`

```json
{
  "name": "seele_restore",
  "description": "Restore a soft-deleted memory.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string" }
    },
    "required": ["id"]
  }
}
```

### `seele_stats`

```json
{
  "name": "seele_stats",
  "description": "Return DB statistics: total memories, active, deleted, by kind/domain.",
  "inputSchema": { "type": "object" }
}
```

## Naming convention y customización

SEELE expone tools genéricas. Si MNEMA quiere wrappers con su naming (`mnema_recall`, `mnema_encode`, etc), lo hace en su capa de orchestrator wrapping las llamadas:

```typescript
// MNEMA backend wrapper
async function mnema_recall(query, kind?, domain?) {
  return seele_search(query, { kind, domain });
}
```

Esto mantiene SEELE agnóstico y reusable.

## Stdio transport details

```
┌─────────────────┐      stdin (JSON-RPC)      ┌─────────────────┐
│                 │ ──────────────────────────► │                 │
│  Claude Code    │                             │  SEELE process  │
│  (MCP client)   │ ◄────────────────────────── │  (mcp mode)     │
│                 │      stdout (JSON-RPC)      │                 │
└─────────────────┘                             └─────────────────┘
                                                        │
                                                        ▼
                                                  stderr (logs)
```

Server reads line-by-line. Cada request es un JSON-RPC objeto:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"seele_search","arguments":{"query":"rate limit"}}}
```

Response:

```json
{"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"...search results JSON..."}]}}
```

## Errores

Errores se devuelven via JSON-RPC error response:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"Invalid params: missing 'query'"}}
```

Códigos:
- -32600 Invalid Request
- -32601 Method not found
- -32602 Invalid params
- -32603 Internal error
- 1001+ SEELE-specific (DB error, embedder error, etc).

## Configuración del consumer (Claude Code)

```json
{
  "mcpServers": {
    "seele": {
      "command": "seele",
      "args": ["mcp"],
      "env": {
        "SEELE_DB": "/Users/orlando/.seele/seele.db"
      }
    }
  }
}
```

## Out of scope v0.1

- **HTTP transport**: v0.2.
- **Server-side tools registration dinámica**: el set de tools es hardcoded en v0.1. v0.2 puede agregar plugins.
- **Authentication**: stdio transport no necesita auth (process trust). HTTP necesitará bearer en v0.2.
- **Streaming responses**: search devuelve resultado completo. Streaming para large results en v0.3.

## Testing

- Unit tests del MCP request/response parsing.
- Integration test: spawn `seele mcp` desde un test, escribir requests, leer responses, validar.
- Smoke test contra Claude Code real (manual durante release QA).

## Referencias

- MCP spec: https://modelcontextprotocol.io/specification
- Reference implementation TypeScript SDK (Anthropic).
- JSON-RPC 2.0 spec: https://www.jsonrpc.org/specification
