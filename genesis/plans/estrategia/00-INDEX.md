# SEELE Genesis — Estrategia

**Fecha**: 2026-05-09
**Estado**: en ejecución
**Protocolo de creación**: AEGIS v2.0.0 aplicado al diseño de un memory engine clean-room.
**Pedido humano**: el User — "configuremos la memoria a largo plazo de MNEMA... clean room back engineering, sacar la lógica de negocio útil de ENGRAM y crear una solución nueva en Rust con otro nombre pero referenciando al ENGRAM de Gentleman-Programming".

## Documentos de la fase

- `01-overview.md` — qué es SEELE, qué problema resuelve, qué relación tiene con MNEMA y con ENGRAM. Epígrafe latino + metáfora "alma del agente".
- `02-reimplementacion-inspirada.md` — postura legal/ética: ENGRAM es MIT, leemos código y damos crédito. Auditoría detallada de qué tomamos y qué NO. Salvaguardas operativas.
- `03-naming-options.md` — proceso de naming. Decisión final SEELE (alemán "alma") con disclaim sobre Evangelion.
- `04-scope-mvp.md` — qué entra en v0.1 (incluye features heredadas de ENGRAM + diferenciadores propios) y qué se difiere.
- `05-engram-feature-audit.md` — auditoría profunda archivo por archivo de ENGRAM con feature parity check vs SEELE plan.

## Salida esperada

Un memory engine en **Rust** instalable como binary independiente (`seele`), reusable fuera del ecosistema MNEMA (cualquier agente Claude Code / Cursor / OpenClaw lo puede usar como MCP server). Output: `C:/dev/tools/SEELE/` y repositorio público bajo `orlando-vazquez-career/seele`.

## Próximas fases (después de Estrategia)

- **Arquitectura** — ADRs sobre Rust, SQLite + FTS5 + sqlite-vec, embedder runtime, MCP transport, HTTP framework, CLI framework, schema, repo layout, distribution, license + credit.
- **Táctica** — bloques de sprint-01 BE.
- **Ejecución** — código real Rust del MVP.
- **LUMEN sprint** — UX del CLI/TUI tras la versión funcional del engine.
