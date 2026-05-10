# Sprint-02 Bloque A — Embedder polish

**Tema**: Cerrar los gaps del `seele-embedder` heredado del commit `8d67f48` para alinearlo con el plan estrategia/04-scope-mvp + ADR-04: INT8 quantization, cache path `~/.seele/embedder/`, SHA256 verification, singleton global.

**Pre-requisitos**: Sprint-01 cerrado.

## Tareas atómicas

### A.1 — Cache path controlado por SEELE

**Inputs**: `crates/seele-embedder/src/onnx.rs::resolve_model_files`, dependencia `dirs`.

**Cambios**:
- Reemplazar `Api::new()` plano por construcción explicita con cache dir override.
- Cache dir: resolver con `dirs::cache_dir().map(|d| d.join("seele/embedder")).ok_or(EmbedderError::CacheDirUnresolvable)`.
- Override por env: `SEELE_EMBEDDER_DIR` toma prioridad si está seteado.
- Crear el directorio idempotente con `std::fs::create_dir_all`.
- Pasar el cache dir a `hf-hub::api::sync::ApiBuilder::with_cache_dir(...)`.

**Criterio de done**:
- Test unit `cache_dir_resolution_uses_env_when_set` con `SEELE_EMBEDDER_DIR=/tmp/foo`.
- Test unit `cache_dir_resolution_falls_back_to_dirs` que verifica el path por default.
- `OnnxEmbedder::new()` usa el path resuelto en vez del default de hf-hub.

### A.2 — INT8 quantized model como default

**Inputs**: ADR-04, `OnnxConfig::ONNX_PATH_IN_REPO = "onnx/model.onnx"`.

**Cambios**:
- Verificar existencia del path `onnx/model_quantized.onnx` en el repo HF mediante `api.model(repo).get(path)` con `Result` handling — si NO existe en HF para `all-MiniLM-L6-v2`, fallback a `onnx/model.onnx` con tracing::warn.
- Agregar campo a `OnnxConfig`:
  ```rust
  pub quantized: bool, // default true
  ```
- `resolve_model_files` selecciona `model_quantized.onnx` si `quantized=true`.
- Si el modelo quantized no está disponible y `quantized=true`, retornar `EmbedderError::QuantizedNotAvailable { model: String }` con suggested fix.

**Criterio de done**:
- `OnnxConfig::default()` tiene `quantized: true`.
- Test `#[ignore]` `embed_with_quantized_returns_unit_vector` (descarga real, run local).
- Doc-comment en `OnnxConfig` explica el trade-off (~30% faster, ~2% drop).

### A.3 — SHA256 verification del modelo descargado

**Inputs**: ADR-04 ("Verificación SHA256 hardcodeada en SEELE para detectar tampering").

**Cambios**:
- Const tabla en `onnx.rs`:
  ```rust
  /// Known SHA256s for trusted models. Update when bumping versions.
  /// Format: (model_repo, file_path_in_repo) -> sha256_hex.
  const TRUSTED_HASHES: &[(&str, &str, &str)] = &[
      ("sentence-transformers/all-MiniLM-L6-v2", "onnx/model.onnx", "<HASH>"),
      ("sentence-transformers/all-MiniLM-L6-v2", "onnx/model_quantized.onnx", "<HASH>"),
      ("sentence-transformers/all-MiniLM-L6-v2", "tokenizer.json", "<HASH>"),
  ];
  ```
- Después de `api.model(repo).get(path)`, computar SHA256 con `sha2::Sha256::digest` sobre `std::fs::read(&path)`. Comparar contra `TRUSTED_HASHES`.
- Si no hay match en la tabla → `tracing::warn!` "no trusted hash for {model}/{file}". No es error fatal — el usuario puede cargar modelos no listados.
- Si hay match en la tabla y NO coincide → `EmbedderError::HashMismatch { expected, got }` con suggested fix "delete `~/.seele/embedder/<model>/` and retry to redownload".

**Criterio de done**:
- Test unit `hash_mismatch_returns_error` con un archivo modificado en TempDir.
- Test unit `hash_match_proceeds` con un archivo cuyo hash está en la tabla.
- Hashes reales para `all-MiniLM-L6-v2` v3 calculados y agregados a `TRUSTED_HASHES`. Documentar el comando bash usado para calcular cada uno (`sha256sum onnx/model.onnx`).

### A.4 — Singleton global

**Inputs**: ADR-04 sección "Singleton runtime", uso esperado en HTTP/MCP servers.

**Cambios**:
- Nuevo módulo `crates/seele-embedder/src/singleton.rs`:
  ```rust
  use once_cell::sync::OnceCell;
  use std::sync::Arc;

  static GLOBAL_EMBEDDER: OnceCell<Arc<dyn Embedder>> = OnceCell::new();

  /// Initialize the global embedder. Idempotent — second call returns Err
  /// if already initialized.
  pub fn init_global(embedder: Arc<dyn Embedder>) -> Result<()>;

  /// Get the global embedder. Panics if not initialized.
  pub fn global() -> Arc<dyn Embedder>;

  /// Get the global embedder without panic. Returns None if not init.
  pub fn try_global() -> Option<Arc<dyn Embedder>>;
  ```
- Re-exports en `lib.rs`: `pub use singleton::{init_global, global, try_global}`.

**Criterio de done**:
- Test unit `init_global_is_idempotent_and_second_call_errors`.
- Test unit `try_global_returns_none_before_init`.
- Doc-comment explica cuándo usarlo (servers de larga vida) vs cuándo no (tests, scripts CLI cortos).

### A.5 — Trait `Embedder::expected_sha256()` para audit

**Inputs**: necesidad de detectar embedder swap en sprint-04.

**Cambios**:
- Agregar al trait:
  ```rust
  /// Hex-encoded SHA256 identifier of the model weights, if available.
  /// `None` for fakes or models without trusted hashes.
  fn expected_sha256(&self) -> Option<&str> { None }
  ```
- `OnnxEmbedder::expected_sha256()` retorna el hash de la tabla `TRUSTED_HASHES` para el modelo cargado (o None si no listado).
- `FakeEmbedder::expected_sha256()` retorna `None` (default).

**Criterio de done**:
- Test unit que verifica el hash retornado para `OnnxEmbedder::with_config(default)` matches `TRUSTED_HASHES`.

### A.6 — Doc + CHANGELOG

**Cambios**:
- Doc-comment del crate: explicar el flow `init → embed → singleton (server)`.
- `CHANGELOG.md` Unreleased / Added: enumerar cache path, INT8 quantized default, SHA256 verify, singleton.

## Tests del bloque A

Total esperado: ~10 nuevos.

- `cache_dir_resolution_uses_env_when_set`
- `cache_dir_resolution_falls_back_to_dirs`
- `cache_dir_create_idempotent`
- `quantized_default_is_true`
- `quantized_falls_back_when_unavailable_with_warn` (con mock Api)
- `hash_mismatch_returns_error`
- `hash_match_proceeds`
- `hash_unlisted_model_warns_but_continues`
- `init_global_is_idempotent_and_second_call_errors`
- `try_global_returns_none_before_init`
- `expected_sha256_matches_trusted_table`

(Tests `#[ignore]` con descarga real: `embed_with_quantized_returns_unit_vector`.)

## Criterios de aceptación del bloque A

1. `cargo build -p seele-embedder` verde.
2. `cargo test -p seele-embedder` verde con los nuevos tests (~16 total).
3. `cargo clippy -p seele-embedder -- -D warnings` verde.
4. `OnnxEmbedder::new()` resuelve cache a `~/.seele/embedder/` (o env override).
5. INT8 quantized es default; full-precision opt-in via `OnnxConfig { quantized: false, .. }`.
6. SHA256 mismatch falla fast con error tipado y suggested fix.
7. Singleton global utilizable desde HTTP/MCP en sprint-03.

## Commit del bloque A

```
git add -A
git commit -m "sprint-02 bloque-A — embedder polish (cache path + int8 + sha256 + singleton)"
git push
```
