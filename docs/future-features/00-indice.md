# Future features — SEELE

Ideas **valiosas pero costosas** que quedaron fuera de la estrategia v0.4 (ver `docs/plans/estrategia/v0.4-dual-runtime-y-robustez/00-overview.md`, sección "Alcance out"). Cada documento detalla qué es, de dónde salió la evidencia, por qué vale, por qué cuesta, el **gatillo de adopción** que la haría entrar, y un boceto de integración.

| # | Feature | Origen de la idea | Gatillo de adopción |
|---|---|---|---|
| 01 | [Indexación de codebase (símbolos, blast-radius, watchers)](./01-codebase-indexing.md) | codebase-memory-mcp | SEELE crece hacia "memoria del código" o un segundo consumidor la pide |
| 02 | [Cuantización int8 de vectores](./02-vector-quantization-int8.md) | codebase-memory-mcp | >500k observaciones o presión real de disco/RAM |
| 03 | [Chunking de contenido largo](./03-content-chunking.md) | auditoría SEELE (`DEFAULT_MAX_LEN=256`) | recall semántico pobre en observaciones largas, medido con `seele-eval` |
| 04 | [Resúmenes temáticos tipo GraphRAG](./04-community-summaries-graphrag.md) | GRAIL | WS3 (grafo) maduro + necesidad real de síntesis sobre clusters |

**Regla de la casa**: ninguna entra sin (a) su gatillo cumplido, (b) un número de `seele-eval` que la justifique, y (c) ADR propio. Están documentadas para no re-descubrir el terreno — no para construirse todavía.
