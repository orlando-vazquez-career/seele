# Sprint-03 Bloque F — Tests E2E binary + state-sync

**Tema**: Tests E2E spawn del binary real + cierre AEGIS del Sprint-03 (devlog + executed + CHANGELOG + INDEX + CLAUDE.md + memoria + tag).

**Pre-requisitos**: Bloques A/B/C/D/E cerrados.

## Tareas atómicas

### F.1 — Tests E2E con binary spawn

`tests/binary_e2e.rs` (workspace-level, en `tools/SEELE/tests/`):

```rust
use std::process::{Command, Stdio};
use std::io::{Write, BufRead, BufReader};

#[test]
fn binary_version_works() {
    let out = Command::new(env!("CARGO_BIN_EXE_seele"))
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("seele"));
}

#[test]
fn binary_mcp_responds_to_tools_list() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_seele"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().unwrap();
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    writeln!(stdin, "{req}").unwrap();
    drop(stdin); // close stdin so server exits after responding
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
    assert!(!lines.is_empty());
    let first: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    let tools = first["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 19);
    child.wait().unwrap();
}

#[test]
fn binary_serve_health_returns_200() {
    // Spawn `seele serve --port 0` (Sprint-04 agrega flag real; aquí asumimos
    // que serve usa SEELE_PORT env). Pegar parser de port desde stderr.
    // Esperar listening message + curl GET /health.
}
```

### F.2 — Devlog Sprint-03

`docs/aegis/devlogs/2026-05-XX-sprint-03-interfaces.md` con secciones canónicas:

- Resumen del flow HTTP + MCP.
- Cambios entregados por bloque (A/B/C/D/E/F).
- Decisiones técnicas (service layer compartido, anti-empty-query gate, auth opt-in, etc).
- Incidentes durante la ejecución.
- Cómo reproducir: `cargo test --workspace`, `seele serve` + curl, `seele mcp` + JSON-RPC.
- Pendiente: HTTP transport MCP (v0.2), rate limiting (v0.2), CLI completa (Sprint-04).
- Uso y costo (con flag estimated).
- Referencias por commit.

### F.3 — Plan a executed/

```bash
git mv genesis/plans/tactica/sprint-03 genesis/plans/executed/tactica/sprint-03
```

### F.4 — Update genesis/plans/tactica/00-INDEX.md

Marcar Sprint-03 como ✅ ejecutado con link al devlog.

### F.5 — Update CHANGELOG.md

Sección Unreleased / Added: seele-http (~25 endpoints + auth bearer + OpenAPI), seele-mcp (stdio + 19 tools), binary CLI mínimo (seele [mcp|serve|--version]).
Sección Changed: Sprint-03 BE Interfaces cerrado.

### F.6 — Update CLAUDE.md

Sección "Estado actual": Sprint-03 cerrado. Update count tests, update sprints pendientes.

### F.7 — Update docs/INDEX.md

Agregar entry al devlog Sprint-03 + entry al plan executed.

### F.8 — Update memoria persistente

`C:/Users/Orlando/.claude/projects/C--dev/memory/project_seele.md`: Sprint-03 cerrado.

### F.9 — Append cost-ledger.jsonl

2 entries para Sprint-03 (ejecucion + state-sync) con `"estimated": true`.

### F.10 — Commit + tag

```bash
git commit -m "sprint-03 state-sync — devlog + executed + CHANGELOG + INDEX + memoria"
git tag -a sprint-03-interfaces -m "Sprint-03 BE Interfaces (HTTP axum + MCP stdio) completed"
git push origin main && git push --tags
```

### F.11 — Invocar /cloven

Review externo antes de Sprint-04.

## Criterios de aceptación del bloque F

1. Tests E2E del binary verde (`cargo test --workspace`).
2. Devlog escrito con 8 secciones canónicas.
3. Plan movido a executed/.
4. CHANGELOG, INDEX, CLAUDE.md, memoria persistente, cost-ledger actualizados.
5. Commit + tag + push autorizado.
6. Cloven invocado y respondió aprobado / con observaciones documentadas.
