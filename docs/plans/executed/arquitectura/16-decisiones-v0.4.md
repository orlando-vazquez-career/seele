# ADR-16 — Decisiones de la estrategia v0.4 (edges canónico, user_prompts, score_boost, alcance de detect)

**Estado**: Propuesto · 2026-07-27 · **Estrategia**: `docs/plans/estrategia/v0.4-dual-runtime-y-robustez/00-overview.md`

## Contexto

La estrategia v0.4 dejó cinco decisiones abiertas que bloquean workstreams. Esta ADR las resuelve antes de táctica (AEGIS: el Arquitecto decide antes de descomponer).

## Decisión 1 — Tabla de edges canónica: `links` para retrieval, `memory_relations` para juicio

**Decisión**: `links` es la tabla de edges para todo lo que WS3 construya (1-hop bonus en retrieval, merge rules de GRAIL, futura clusterización). `memory_relations` se queda con su rol actual: anotaciones de juicio (`supersedes`, `conflicts_with`) con ciclo de vida (`pending→judged`), consumido por WS2 para democión de suplantadas.

**Por qué**: `links` es ligera (`src`, `dst`, `link_type` libre), ya tiene endpoints y el verbo `seele link`; `memory_relations` carga estado de workflow que no tiene sentido como edge general. Unificarlas forzaría migración y mezclaría dos semánticas. El precio — dos tablas de edges — se paga con una regla escrita: **retrieval lee `links`; la higiene de memoria lee `memory_relations`**.

## Decisión 2 — `user_prompts` + `prompts_fts` + `PromptStore`: se borran

**Decisión**: eliminar la tabla, el FTS inerte y el store (write-dead en producción: instanciado en `service.rs:56` y jamás llamado fuera de tests).

**Por qué**: código muerto con costo de esquema y de lectura (parece feature y no lo es). Si algún día se quiere captura de prompts, se diseña de nuevo con su caso de uso (ver `docs/future-features/`), no se resucita un pipeline accidental. Migración: SQL que droppea ambas tablas; nota en CHANGELOG.

## Decisión 3 — Default canónico de `score_boost_multiplier`: **1.0**

**Decisión**: 1.0 en las cuatro superficies (HTTP `dto.rs`, CLI `search.rs`, TUI `app.rs`, `/chat` ya lo usa).

**Por qué**: un multiplicador cuyo neutro es 1.0 no puede tener default 0.0 — anula la señal que multiplica (hoy la metadata `score` no influye en nada fuera de `/chat`). 1.0 = "la metadata score pesa lo que el algoritmo dice que pesa"; quien quiera apagarla, la pasa a 0.0 explícitamente.

## Decisión 4 — `seele-project::detect` se wirea **solo al CLI**

**Decisión**: `seele save` sin `--project` llama `detect(&cwd)` (wiring en `crates/seele-cli/src/commands/save.rs:48-58`). En `serve` y `mcp` el `project` sigue siendo obligatorio-explicito del caller; `detect()` queda prohibido en procesos long-lived.

**Por qué**: `detect()` shella `git` con timeouts de 1.5 s — en un proceso servidor eso es latencia y superficie de fallo por request; en un CLI one-shot es exactamente su caso de uso documentado ("cuando un agente llama `seele save` sin pasar project"). La ambigüedad desaparece: servidores explícitos, CLI conveniente.

## Decisión 5 — Chunking de contenido largo: diferido con gatillo

**Decisión**: no entra en v0.4; documentado con gatillo de adopción y boceto en `docs/future-features/03-content-chunking.md`. Mitigación interina (WS0): el help de `save` declara el límite de 256 tokens del embedding.

**Por qué**: es la decisión de mayor blast radius del parque (identidad observación vs chunk) y no hay medición que la justifique todavía (disciplina v0.3 evaluation-first).

## Consecuencias

- WS3 queda desbloqueado sobre `links` (merge rules GRAIL: weight=avg, confidence=min, observed_at=max, dirección de autoría preservada).
- WS1 hereda la eliminación de `user_prompts` (tarea chica de migración SQL).
- Ninguna decisión toca el charter: local-first, offline-by-default, CPU-only, single-binary.
- Si `memory_relations` alguna vez necesita entrar a retrieval más allá de la democión, se revisita esta ADR — no se edita en caliente.
