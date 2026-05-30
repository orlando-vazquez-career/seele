# ADR-15 — Tamaño del binario y footprint en disco (feature-gating)

**Estado**: Propuesto · 2026-05-29 · **Pendiente Gate 1**
**Continúa** la secuencia de ADRs del repo (01-14). Post-v0.1 → vive en
`docs/plans/arquitectura/` per la regla del `CLAUDE.md`.
**Origen**: concern del mantenedor (el footprint en disco frena la adopción) +
investigación empírica en sesión (Opus 4.8, consola). No estaba en la auditoría de
memory-engines (ADR-14) — es una brecha ortogonal a la calidad de retrieval.

## Decisión

1. SEELE adopta **feature-gating de los componentes pesados** para permitir builds
   "lite": el embedder ONNX (`ort`) y la superficie HTTP+Swagger pasan a ser **features
   de Cargo**, default-on (conserva el batteries-included actual) pero desactivables
   (`--no-default-features`) para un binario chico.
2. **Medir antes de cortar**: el split exacto por crate se obtiene con
   `cargo bloat --release --crates` y queda versionado (§Medición) **antes** de remover
   nada. Mismo principio que ADR-14 (eval-first): no optimizar a ciegas.
3. El binario oficial distribuido sigue siendo single-binary full; el feature-gating
   habilita variantes, no cambia el default.

## Contexto (medición inicial, en sesión)

- `seele.exe` release = **42.55 MB**, y **ya está stripped** (`Cargo.toml:94-98`:
  `strip=true`, `lto="thin"`, `codegen-units=1`). No son símbolos ni profile sin tunear.
- vec0 vendorizado: **cfg-gateado por target** (`vec0_loader.rs:11-31`); cada binario
  embebe solo su plataforma (~125-282 KB). **NO es la causa.**
- Causa estructural: deps nativas estáticas en **un** binario que carga **todos** los
  transportes a la vez:
  - `ort` (ONNX Runtime), default `download-binaries` → runtime ONNX **linkeado
    estático** (`Cargo.toml:54`). **Sospechado dominante.**
  - `utoipa-swagger-ui` v9 → dist de Swagger UI embebido en `seele-http`
    (`Cargo.toml:64`).
  - `rusqlite` `bundled` → SQLite compilado adentro (`Cargo.toml:45`).
- Runtime aparte: modelo `all-MiniLM` en `~/.seele` (~23 MB INT8 al primer uso) — no es
  el binario.
- `target/` dev = 3.92 GB (no se shippea; mitigable con `cargo clean`/sccache).

## Decisión detallada

### Feature `onnx` (embedder real)
`seele-embedder` expone feature `onnx` (default) que habilita `ort`/`tokenizers`/`hf-hub`.
Sin ella → solo `FakeEmbedder` o embedder remoto (vía `seele-chat`/endpoint local).
`seele-cli`/`seele-http` re-exportan el feature. Un build lite (FTS-only o embeddings
remotos) **no carga el runtime ONNX** → recorte esperado mayor (confirmar en §Medición).

### Feature `http` (servidor + Swagger)
`seele-http` + `utoipa-swagger-ui` detrás de feature `http`/`serve` (default). Usuarios
CLI/MCP-only no cargan axum + Swagger UI.

### Profile size (menor, gratis)
Evaluar `opt-level="s"|"z"`, `lto="fat"`, `panic="abort"` midiendo el delta.
`panic="abort"` cambia comportamiento (sin unwinding) — decidir si es aceptable para
CLI/server.

### `ort` load-dynamic (avanzado, trade-off)
Opción: `ort` con `load-dynamic` en vez de estático → binario mucho menor, pero agrega
dep de runtime (onnxruntime del sistema o sidecar). **Diferido**: solo si el
feature-gating no alcanza.

## Medición (cargo bloat, 2026-05-29)

Build release sin strip (`CARGO_PROFILE_RELEASE_STRIP=false`),
`cargo bloat --release --crates --bin seele`, Windows x86_64:

- **`seele.exe` = 42.5 MiB.** `.text` (código) = **21.7 MiB (51%)**; el **~49% restante
  (~20.8 MiB) es data/secciones no-`.text`** que `--crates` no desglosa.
- **No hay `onnxruntime.dll` en `target/release/`** → el runtime ONNX está **linkeado
  estático** dentro del `.exe` (no es un sidecar). Es el costo del single-binary
  batteries-included.
- **El `.text` no tiene un villano único** — se reparte en dos clusters opcionales:
  - **Embedder**: `tokenizers` (624K) + `ort`/onnxruntime (estático; gran parte cae en
    data/foreign, mal medido por cargo-bloat) + `hf-hub`→`reqwest`/`rustls`/`ring`/
    `webpki` (stack TLS solo para descargar el modelo) + tablas ICU/unicode (rodata).
  - **HTTP**: `axum` (421K) + `hyper`/`hyper_util` + `tower` + `utoipa-swagger-ui`
    (assets de Swagger embebidos vía `rust-embed`, en rodata).
  - Resto: `std` (1.3M), `clap` (314K), `ratatui` (TUI), `regex`, crates `seele_*`.
- **`strip` casi no mueve la aguja en Windows**: el debuginfo va a `seele.pdb` (128 MB,
  **no se shippea**), no al `.exe`. La palanca real es **quitar código/data
  (feature-gating)**, no `strip`/profile.

### Conclusión

Confirma la decisión: el peso son **los dos clusters opcionales**, no un profile mal
tuneado ni un bug de empaquetado (la hipótesis del `include_bytes!` x5 quedó falsada; la
de "ort dominante" se matiza: onnxruntime estático es el contribuyente individual más
grande, pero el resto está repartido). Quitar `onnx` saca onnxruntime + `tokenizers` + el
stack TLS de `hf-hub` + ICU; quitar `http` saca axum/hyper + los assets de Swagger.

### Pendiente (validación A/B)

El split exacto de onnxruntime no es medible con `cargo bloat` (código C++ foreign). Se
mide directo comparando `seele.exe` **con vs sin** el feature `onnx` una vez implementado
el gating — ése número fija el **target** (hipótesis: lite-CLI < ~12 MB, full ~42 MB).

## Alternativas rechazadas

- **Micro-optimizar sin medir** — rechazada (mismo fallo que la auditoría: optimizar sin
  baseline).
- **Quitar ONNX del todo** — rechazada: rompe el value-prop de embeddings on-device
  offline.
- **Comprimir el binario (UPX)** — rechazada: frágil, dispara antivirus, no ataca la
  causa.

## Riesgos

| Riesgo | Mitigación |
|---|---|
| La matriz de features explota en CI | Limitar a 2-3 combos soportados (full / lite-cli / lite-mcp) + verificarlos en CI |
| `load-dynamic` agrega fricción de install | Diferido; solo si feature-gating no basta |
| `panic="abort"` cambia semántica | Medir delta; adoptar solo si el recorte lo justifica y es seguro para el server |

## Consecuencias

- Usuarios CLI/MCP-only obtienen un binario chico → baja la barrera de adopción (el
  objetivo del concern).
- El build full no cambia para quien lo quiere.
- Habilita publicar tamaños por variante como dato (igual que el baseline de calidad de
  ADR-14).

## Referencias

- Medición en sesión: `seele.exe` = 42.55 MB; `Cargo.toml:45,54,64,94-98`;
  `vec0_loader.rs:11-31`.
- `ort` features (download-binaries vs load-dynamic): https://docs.rs/ort.
- cargo-bloat: https://github.com/RazrFalcon/cargo-bloat.
- ADR-11 (sqlite-vec vendorizado), ADR-04 (embedder), ADR-14 (eval-first, mismo
  principio "medir antes de optimizar").
