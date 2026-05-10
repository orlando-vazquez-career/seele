# ADR-11 — sqlite-vec vendorizado en `seele-storage`

**Estado**: Aceptado · 2026-05-10
**Decisión**: los binarios precompilados de `sqlite-vec` (`vec0.so`, `vec0.dylib`, `vec0.dll`) para los 5 targets soportados se incluyen como archivos en `crates/seele-storage/vendor/sqlite-vec/<target>/` y se embeben en el ejecutable final de SEELE via `include_bytes!`. En runtime, `seele-storage` los escribe a un path en cache local (`~/.cache/seele/vec0-<sha>...`) idempotentemente y los carga via `rusqlite::Connection::load_extension`.

## Contexto

`sqlite-vec` (de Alex Garcia, MIT) es la dependencia core para el lado vectorial de SEELE. No existe binding Rust mantenido en estado estable a fecha de génesis (2026-05). Las opciones evaluadas:

1. **Build.rs con descarga**: el `build.rs` baja el binario correcto al compilar.
   - Contras: requiere conectividad de red en build time, rompe `cargo install seele` offline, agrega complejidad al CI, potencial flakiness por upstream releases caídos.
2. **Crate Rust upstream**: depender de un wrapper Rust del proyecto upstream.
   - Contras: a 2026-05 los crates existentes están medio abandonados o desfasados respecto al upstream nativo. Riesgo de bug-driven divergence.
3. **Vendored**: incluir los `.so/.dylib/.dll` directamente en el repo y embebirlos via `include_bytes!`.
   - Pro: `cargo install seele` funciona out-of-the-box sin red.
   - Pro: control determinista del binary que termina en el ejecutable final.
   - Pro: trivial de auditar (los archivos están en el repo + su `checksums.txt` upstream).
   - Contra: ~880 KB sumados al `crates/seele-storage/`. **Aceptado por el User el 2026-05-10**.

Decisión tomada en review de Cloven 2026-05-10: el User eligió la opción 3 (Vendored) explícitamente con racional "soluciones completas".

## Implementación

### Layout en repo

```
crates/seele-storage/
├── src/
│   ├── lib.rs
│   └── vec0_loader.rs        # include_bytes! por target + helpers
└── vendor/
    └── sqlite-vec/
        ├── README.md          # provenance, version, license, update steps
        ├── CHECKSUMS-upstream.txt  # checksums.txt del release upstream
        ├── linux-x86_64/vec0.so
        ├── linux-aarch64/vec0.so
        ├── macos-x86_64/vec0.dylib
        ├── macos-aarch64/vec0.dylib
        └── windows-x86_64/vec0.dll
```

### API expuesta por `vec0_loader`

```rust
/// Bytes vendorizados para el target actual; None si no soportado.
pub fn vec0_bytes() -> Option<&'static [u8]>;

/// Sufijo de archivo para la plataforma (.so / .dylib / .dll).
pub fn vec0_extension_suffix() -> &'static str;
```

### Flujo de carga en runtime (sprint-01 bloque-C)

`pool::init_pool` llama un helper `ensure_vec0_extension_path()`:

1. Lee `vec0_loader::vec0_bytes()`.
2. Si `None`: retorna `StorageError::VecNotSupportedTarget(target_str)`.
3. Si `Some(bytes)`:
   - Calcula `sha256(bytes)[..16]` para hash estable.
   - Path destino: `~/.cache/seele/vec0-<sha><suffix>`.
   - Si ya existe y SHA coincide: usa el path existente.
   - Si no existe: crea el dir, escribe bytes con permisos 0644, sync.
   - Retorna `PathBuf`.
4. En `with_init` del `SqliteConnectionManager`: `conn.load_extension(&path, None)`.

Override por env var:

```bash
SEELE_VEC_PATH=/usr/local/lib/vec0.so seele server
```

— si está seteada y el archivo existe, ese path se usa en vez del vendored.
Esto cubre: targets no soportados (`linux-arm-musl`, BSDs), debugging,
distros que ya proveen el shared lib oficial.

## Targets soportados v0.1

| Target triple                  | Vendored           |
| ------------------------------ | ------------------ |
| `x86_64-unknown-linux-gnu`     | sí                 |
| `aarch64-unknown-linux-gnu`    | sí                 |
| `x86_64-apple-darwin`          | sí                 |
| `aarch64-apple-darwin`         | sí                 |
| `x86_64-pc-windows-msvc`       | sí                 |
| Otros                          | requiere SEELE_VEC_PATH |

Estos 5 targets coinciden con los `release` builds de SEELE (ADR-09 distribución).

## Versión upstream rastreada

`v0.1.9` al 2026-05-10. El proceso de bump está documentado en
`crates/seele-storage/vendor/sqlite-vec/README.md` (5 pasos: download,
extract, replace, replace checksums, test, commit). El `CHANGELOG.md`
raíz registra cada bump como entrada explícita.

Dependabot puede sugerir nuevos releases vía `.github/dependabot.yml`
(monitoreo Cargo + GitHub Actions), pero el bump del binary es manual.

## Crédito y licencia

`sqlite-vec` es **MIT** © 2024 Alex Garcia. Aviso de copyright + texto
de la licencia se incluyen en:

- `README.md` raíz de SEELE (bloque "Crédito").
- `CREDITS.md` con cita completa.
- `crates/seele-storage/vendor/sqlite-vec/README.md` referenciando upstream.

No re-empaquetamos el código fuente — sólo los binarios públicos del
release. Esto es uso permitido bajo MIT con preservación de aviso.

## Riesgos asumidos

1. **Tamaño del repo**: +880 KB. Decisión consciente del User.
2. **Bump manual**: si upstream saca v0.2.0 con fixes de seguridad,
   tenemos que bumpear nosotros. Mitigación: dependabot config + alert
   manual semanal.
3. **Falsos positivos en escáner de seguridad**: algunos sistemas marcan
   binarios `.so/.dll` en repos como sospechosos. Mitigación: `vendor/`
   está documentado en README + checksums verificables.
4. **Discrepancia con build local**: si un dev compila desde main en
   un target no soportado, falla con error claro pidiendo `SEELE_VEC_PATH`.

## Alternativas descartadas (referencia)

- **Build.rs + curl**: rompe offline, agrega flakiness CI.
- **Crate Rust wrapper**: ningún wrapper estable a fecha de génesis.
- **Compilar desde fuente en build.rs**: requiere C toolchain en CI Windows
  + macOS, agrega 5-10 min a cada build, y el upstream no garantiza
  compilación cross-platform sin tweaks específicos.
- **Submódulo git de sqlite-vec**: el upstream distribuye bins, no fuente
  fácil de compilar; submódulo + build.rs nos llevaría al peor de varios
  mundos.

## Consecuencias

- `cargo install seele` funciona en linux/mac/win sin red ni toolchain extra.
- El binary final de SEELE pesa ~880 KB más (4-5 binarios embebidos).
- Bumping de `sqlite-vec` es un PR humano cada vez que sale release upstream.
- Para targets exóticos, el usuario setea `SEELE_VEC_PATH` o compila el
  loadable desde el upstream.
