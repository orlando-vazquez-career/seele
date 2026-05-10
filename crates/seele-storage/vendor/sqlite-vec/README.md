# vendor/sqlite-vec — vendored loadable extension

Binarios precompilados de [`sqlite-vec`](https://github.com/asg017/sqlite-vec)
embebidos en `seele-storage` via `include_bytes!`. Esto permite que
`cargo install seele` funcione out-of-the-box sin descargas de runtime ni
build steps adicionales.

## Versión actual

`v0.1.9` (release del upstream). Para ver la próxima versión planeada
de bump y el calendario, ver `CHANGELOG.md` raíz.

## Targets vendorizados

| Target triple                  | Archivo                       |
| ------------------------------ | ----------------------------- |
| `x86_64-unknown-linux-gnu`     | `linux-x86_64/vec0.so`        |
| `aarch64-unknown-linux-gnu`    | `linux-aarch64/vec0.so`       |
| `x86_64-apple-darwin`          | `macos-x86_64/vec0.dylib`     |
| `aarch64-apple-darwin`         | `macos-aarch64/vec0.dylib`    |
| `x86_64-pc-windows-msvc`       | `windows-x86_64/vec0.dll`     |

Targets fuera de esta lista (Android, iOS, 32-bit Linux, etc.) reciben
`None` desde `vec0_loader::vec0_bytes()` y deberían fallar con un mensaje
claro al iniciar SEELE en esa plataforma.

## Licencia

`sqlite-vec` está bajo **dual license Apache-2.0 OR MIT**, copyright
2024 Alex Garcia. Como esta carpeta redistribuye binarios upstream, ambas
licencias requieren preservar el aviso de copyright + texto de licencia.
Por eso esta carpeta incluye:

- `LICENSE-APACHE-upstream.txt` — copia textual de `LICENSE-APACHE` del upstream.
- `LICENSE-MIT-upstream.txt` — copia textual de `LICENSE-MIT` del upstream.

Crédito completo en `README.md` raíz de SEELE y `CREDITS.md`.

## Verificación de integridad

`CHECKSUMS-upstream.txt` es el archivo `checksums.txt` publicado por el
upstream para `v0.1.9`. Permite re-verificar manualmente que los bytes
embebidos no fueron alterados.

```bash
# Desde la raíz del repo
sha256sum crates/seele-storage/vendor/sqlite-vec/linux-x86_64/vec0.so
# comparar contra `sqlite-vec-0.1.9-loadable-linux-x86_64.tar.gz` extraído
```

## Cómo actualizar a una nueva versión upstream

1. Bumpear el número de versión en este README + en `CHANGELOG.md`.
2. Descargar todos los `loadable-{target}.tar.gz` desde
   `https://github.com/asg017/sqlite-vec/releases/download/vX.Y.Z/`.
3. Extraer cada `vec0.{so|dylib|dll}` al subdirectorio correspondiente
   (sobrescribiendo lo previo).
4. Reemplazar `CHECKSUMS-upstream.txt` con el nuevo `checksums.txt`.
5. Correr `cargo test -p seele-storage` para confirmar que el bump no
   rompe el contrato de carga.
6. Commit con mensaje `vendor: bump sqlite-vec to vX.Y.Z`.

## Por qué no usar build.rs ni un crate Rust upstream

- `build.rs` con descarga obligaría conectividad de red en build time,
  rompe `cargo install seele` offline, y suma complejidad al CI.
- Crates Rust de bindings a `sqlite-vec` no están consolidados a fecha
  de génesis (2026-05). Vendorizar el loadable da control determinista
  del binario que termina en el ejecutable final.

Compromiso aceptado: ~880 KB sumados a `crates/seele-storage/` (los 5
archivos vendorizados). Documentado en ADR-11 y aceptado por el User.
