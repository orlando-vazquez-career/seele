# 03 — Chunking de contenido largo

**Estado**: fuera de v0.4 (requiere ADR propio) · **Origen**: re-auditoría SEELE (2026-07-26).

## Qué es

Hoy cualquier observación más larga que `DEFAULT_MAX_LEN = 256` tokens (`crates/seele-embedder/src/onnx.rs:32`) se embebe **truncada en silencio**: el resto del contenido existe en la DB pero es invisible al vec search. Chunking = dividir el contenido largo en piezas embebibles y componer su identidad vectorial.

## Evidencia

- El truncado silencioso está confirmado: no hay chunking de texto (el `chunks.rs` del repo es dedup de sync, no de texto) ni warning al persistir.
- GRAIL resuelve el caso equivalente con `TextUnit` chunks con back-pointers a documento (`grail/schemas.py`) y rankea text_units por entity-overlap — pero paga el costo de una segunda entidad que mantener.
- Mitigación interina ya incluida en v0.4-WS0: documentar el límite en el help de `save`.

## Por qué es valioso

Las observaciones largas son exactamente las de más valor (ADRs, veredictos, post-mortems). Que su segunda mitad sea semánticamente invisible es una pérdida real de recall que hoy no se manifiesta en ningún métrico.

## Por qué quedó fuera

- **Identidad**: ¿qué es una observación — el documento o el chunk? Rompe la simetría `1 fila = 1 memoria = 1 vector`, complica upsert por topic-key (¿qué pasa con los chunks viejos al re-escribir?), dedup, y soft-delete (cascada).
- **Costo**: tabla `observation_chunks` + re-embed en cada write + cambios en el contrato de retrieval (¿devolver chunk o padre?) + migración. Es la feature con más blast radius del parque.
- **Medición primero**: sin `seele-eval` (v0.3) no hay forma de saber cuánto recall se pierde hoy; si el corpus real es 95% observaciones <256 tokens, la feature no se justifica todavía.

## Gatillo de adopción

`seele-eval` muestra recall semántico degradado en la categoría de contenido largo (o >20% del corpus real supera 256 tokens).

## Boceto de integración (si algún día entra)

Opción preferida — **chunk fantasma**: la observación madre conserva identidad y topic-key; los chunks viven en `observation_chunks(int_id, seq, text, vec)` y se regeneran atómicamente en el mismo write (nunca best-effort — ver WS1). Retrieval: el vec search apunta a chunks, el resultado se re-componde al padre (dedup por int_id, score = max de sus chunks). FTS5 queda sobre el documento completo (no se toca). ADR obligatorio: formato de split (por párrafos con overlap ~15%), límite de chunks por doc, y comportamiento del upsert.
