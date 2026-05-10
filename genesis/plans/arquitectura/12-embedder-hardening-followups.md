# ADR-12 — Embedder hardening follow-ups (post Sprint-02)

**Estado**: Aceptado · 2026-05-10
**Decisión**: documentar 2 follow-ups sobre el embedder que quedaron como decisiones conscientes durante Sprint-02 y deben revisarse antes de v0.1.0 release.

## Contexto

Sprint-02 cerró `seele-embedder` con polish completo según plan estrategia/04-scope-mvp y ADR-04. Dos decisiones quedaron explícitamente como gaps conscientes:

1. **`TRUSTED_HASHES` table vacía**. La infra de SHA256 verification existe pero no hay hashes registrados al v0.1. El behavior por default es "no listado → log debug + proceed".
2. **INT8 quantized fallback silencioso con warn**. Si HF retira temporalmente el modelo quantized, el embedder cae a full precision con `tracing::warn` y sigue funcionando.

Cloven (2026-05-10) flagueó ambas como [NIT] aceptables pero rastreables. Este ADR las hace seguibles.

## Follow-up 1: poblar `TRUSTED_HASHES` al primer release tagged

**Cuándo**: en Sprint-05 cuando se prepare el release pipeline.

**Cómo**:

1. Correr una build clean del embedder en CI release.
2. Para cada artifact descargado de HF:
   ```bash
   sha256sum ~/.seele/embedder/sentence-transformers/all-MiniLM-L6-v2/blobs/<hash>
   ```
   (Los blobs reales viven en el cache de hf-hub.)
3. Registrar los 3 hashes en `crates/seele-embedder/src/onnx.rs::TRUSTED_HASHES`:
   ```rust
   const TRUSTED_HASHES: &[(&str, &str, &str)] = &[
       ("sentence-transformers/all-MiniLM-L6-v2", "onnx/model.onnx", "<hex>"),
       ("sentence-transformers/all-MiniLM-L6-v2", "onnx/model_quantized.onnx", "<hex>"),
       ("sentence-transformers/all-MiniLM-L6-v2", "tokenizer.json", "<hex>"),
   ];
   ```
4. Documentar el procedimiento de bump en `CHANGELOG.md` sección "Histórico de bumps" (parecido al sqlite-vec vendored).
5. Test `embedder_artifacts_match_trusted_hashes` con `#[ignore]` corre `OnnxEmbedder::new()` y verifica que NO retorna `HashMismatch`.

**Riesgo de seguir sin esto**: usuario con cache comprometido podría cargar un modelo modificado y no enterarse. Probabilidad baja (requiere acceso al filesystem del usuario o tampering en HF), pero el costo de la mitigación es minutos.

## Follow-up 2: opt-in strict quantized mode

**Cuándo**: cuando emerja un consumer que quiera detección temprana del fallback.

**Cómo**:

1. Agregar a `OnnxConfig`:
   ```rust
   /// When true and `quantized=true`, return `EmbedderError::QuantizedNotAvailable`
   /// instead of falling back to full precision on HF resolution failure.
   /// Default: false (preserva fallback con warn).
   pub strict_quantized: bool,
   ```
2. Nuevo error variant:
   ```rust
   #[error("INT8 quantized model not available for {model}; HF returned: {hf_error}")]
   QuantizedNotAvailable { model: String, hf_error: String },
   ```
3. En `resolve_model_files`, si `quantized && strict_quantized && quantized_get_fails`, retornar el error sin fallback.
4. CLI flag (Sprint-04): `seele embedder install --strict-quantized` setea el config.
5. Test que verifica el error path.

**Riesgo de seguir sin esto**: operador no se entera que el server consume CPU 30% más por estar en full precision. Detectable por logs si se monitorea `tracing::warn`. Aceptable para v0.1 sin servers de producción.

## Out of scope

Este ADR no cubre:
- Cosine similarity verification (test de regression contra fixture de embeddings esperados) — pertenece a Sprint-05 polish.
- CUDA/Metal/DirectML support — sigue v0.2 (ADR-04).
- Embedder swap CLI — Sprint-04.

## Tracking

- **Follow-up 1**: bloqueante para `v0.1.0` release. Owner: el sprint que prepare el release pipeline (Sprint-05).
- **Follow-up 2**: nice-to-have antes de v0.1.0. Owner: cualquier sprint que tenga slack; si no se incluye, va a `CHANGELOG.md` v0.2 backlog.

## Referencias

- Sprint-02 devlog: `docs/aegis/devlogs/2026-05-10-sprint-02-embedder-search.md`
- ADR-04 embedder ONNX: `genesis/plans/arquitectura/04-embedder-onnx.md`
- Cloven review post Sprint-02 (2026-05-10): este ADR cierra los [NIT] que dejó como rastreables.
