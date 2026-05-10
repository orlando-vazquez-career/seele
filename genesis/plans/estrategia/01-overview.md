# SEELE — Overview

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*
> Sobre el firmamento estrellado juzga Dios, como nosotros juzgamos.

## Qué problema resuelve

MNEMA es un protocolo de persistencia + criterio + análisis. Pero un protocolo no escribe a disco — necesita un **substrato físico** que guarde memorias, las indexe, las busque por similaridad semántica, las exponga via MCP a los agentes.

[ENGRAM](https://github.com/Gentleman-Programming/engram) (binary Go de Gentleman-Programming, MIT) es el referente del nicho — single-binary, SQLite + FTS5, MCP server, CLI, HTTP API, TUI, git sync de chunks comprimidos, conflict detection con LLM judging. Lo que **no** hace ENGRAM: embeddings vectoriales para semantic similarity (solo FTS5).

Usar ENGRAM upstream tiene tres limitaciones para el ecosistema del User:

1. **No tenemos control del roadmap.** Si MNEMA necesita un tipo de query custom (ej: filtrado por `axiomatic`, `kind`, `domain` con índices virtuales) y el upstream no lo prioriza, dependemos de PRs aceptados.
2. **No conocemos el código por dentro.** Adoptar un binary de terceros para el corazón de la persistencia significa que cuando algo rompa, debugamos un binario externo. Para un componente tan crítico, eso es deuda técnica desde el día uno.
3. **El stack del ecosistema.** AEGIS / LUMEN / MNEMA están en TS + Python + Astro. Sumar Go al ecosistema solo para el engine es un lenguaje más de mantener. Rust es la elección del User para componentes de sistema (binarios, performance, FFI), entonces el engine va en Rust.

**SEELE** (alemán: alma, espíritu, psique) resuelve esto: un memory engine **diseñado desde cero en Rust**, agnóstico del protocolo MNEMA pero con afinidad nativa, distribuido como binary, reusable por cualquier agente que entienda MCP. La metáfora — la memoria es lo que constituye el alma de un agente — encaja con MNEMA (substrato anímico) y le agrega valor diferencial sobre ENGRAM con embeddings vectoriales y search híbrido FTS+vector.

## Postura frente a ENGRAM

ENGRAM está bajo licencia **MIT**, lo que significa:

- Podemos **leer su código línea por línea**.
- Podemos **extraer lógica de negocio** y reimplementarla con nuestras propias decisiones técnicas.
- **Debemos dar crédito** al autor en README, CREDITS y release notes (es la condición de la licencia MIT).

Por lo tanto: **SEELE es una reimplementación inspirada en ENGRAM bajo licencia MIT**, no un fork ni un clean-room estricto. Detalle exhaustivo en `02-reimplementacion-inspirada.md`.

## Por qué un repo nuevo y no parte de MNEMA

Tres razones:

1. **Reuso fuera de MNEMA.** Un memory engine es útil para cualquier agente — no solo para el counsel pattern. Con SEELE separado, otro proyecto puede usarlo sin importar todo MNEMA.
2. **Cadencia distinta.** SEELE evoluciona con el motor de search/embeddings/storage; MNEMA evoluciona con el counsel pattern y schema de decisiones. Versionados independientes.
3. **Disciplina del binary.** El binary es el contrato entre MNEMA y el storage. Si MNEMA importara el código del engine, esa frontera se diluye. Con un binary separado el contrato es CLI / HTTP / MCP — claro y testeable.

## Alcance

SEELE cubre (heredando de ENGRAM lo aplicable + agregando lo nuestro):

### Heredado de ENGRAM (con nuestra implementación Rust)

- **Sessions** — first-class entity con start/end/summary lifecycle.
- **Observations** (memorias en SEELE) con: `type`, `title`, `content`, `tool_name`, `project`, `scope` (`project|personal`), `topic_key`.
- **Topic key upserts** — memorias evolutivas en lugar de duplicar (revision_count).
- **Normalized hash dedup** — mismas memorias en ventana temporal incrementan duplicate_count.
- **User prompts** como entidad separada (linked a sessions).
- **Memory relations** con judgment lifecycle (`pending|judged|orphaned|ignored`) — `supersedes`, `conflicts_with`, `related`, `compatible`, `scoped`.
- **Project detection** — algoritmo de 5 casos (config.json / git remote / git root / child scan / dir basename).
- **Privacy stripping** — `<private>...</private>` se elimina antes de save.
- **Capture passive** — extraer learnings de output text con `## Key Learnings:` patterns.
- **Git sync** — chunks comprimidos para multi-machine sin merge conflicts.
- **Agent setup wizard** — installs hooks/configs para Claude Code, Cursor, VS Code, OpenCode, Gemini CLI, Codex.
- **Doctor / Stats / Export / Import / Timeline / Context** — operacional completo.
- **TUI** — para inspección visual.

### Diferenciador propio de SEELE

- **Embeddings vectoriales** — embedder local ONNX (`all-MiniLM-L6-v2`, dim 384), persistidos en sqlite-vec.
- **Search híbrido FTS+vector con RRF** (Reciprocal Rank Fusion) — ENGRAM solo FTS, SEELE además semantic similarity.
- **Virtual generated columns + B-tree indexes** sobre campos JSON metadata — sub-10ms filtering aún a 100K+ memorias.
- **ULID IDs** sortables por timestamp.
- **Rust** — single-binary multi-OS, performance predictible, memoria segura.
- **TUI con ratatui+crossterm** vs bubbletea (Go) de ENGRAM.

### Diferido a v0.2+

- **Cloud replication** (Postgres + dashboard servido). v0.1 es local-first puro. Tailscale alcanza para multi-machine personal.
- **Conflict detection con LLM judging** — el storage de relations sí está en v0.1 (tabla + status). El LLM-based scan es v0.2.
- **Semantic conflict scan** (`engram conflicts scan --semantic`) — v0.2.
- **Obsidian export** — v0.2.
- **Setup automation** completa para todos los agentes — v0.1 cubre Claude Code + un genérico; los demás v0.2.

## Quién opera SEELE

Cualquier agente Claude Code, Cursor, OpenCode, Codex, OpenClaw o usuario humano via CLI / TUI. Default install: localhost; remote via Tailscale o VPS según fase de hosting (ver MNEMA `guides/hosting-options.md`).

## Output esperado

`C:/dev/tools/SEELE/` con:

```
SEELE/
├── README.md
├── LICENSE                     # MIT, copyright DevZen SpA
├── CREDITS.md                  # crédito explícito a Gentleman-Programming/ENGRAM
├── Cargo.toml                  # workspace root
├── crates/
│   ├── seele-core/             # tipos, schema, traits
│   ├── seele-storage/          # SQLite + FTS5 + vec + sessions/observations/relations/sync_chunks
│   ├── seele-embedder/         # ONNX runtime + modelos
│   ├── seele-search/           # query engine híbrido
│   ├── seele-mcp/              # MCP server (stdio v0.1, HTTP v0.2)
│   ├── seele-http/             # HTTP REST API
│   ├── seele-tui/              # TUI ratatui
│   ├── seele-sync/             # git sync chunks
│   ├── seele-setup/            # agent setup wizard
│   ├── seele-project/          # project detection
│   └── seele-cli/              # binary `seele` (clap)
├── docs/
│   └── aegis/                  # planes y devlogs bajo AEGIS
└── tests/                      # integration tests
```

Y release binario bajo `orlando-vazquez-career/seele` con cobertura mínima de tests + CI.
