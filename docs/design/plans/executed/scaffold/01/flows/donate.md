# Flow — donate

## Trigger

Usuario hace clic en uno de los 5 botones del DonateButtons grid en la sección `#support`.

## Path A — wallet instalado en el browser/OS (caso favorable)

1. Click en botón "Ethereum" (ejemplo).
2. JS construye URI: `ethereum:0x0f47273B14118EDD15b40A5CfDBFc4A7891F08D8@1`.
3. `window.open(uri, '_self')` ejecuta navegación al URI scheme.
4. El OS encuentra el handler registrado (MetaMask Mobile, Pali, etc.).
5. El wallet abre con la transacción pre-rellenada (destino, optionally chainId).
6. El usuario elige el amount y firma.
7. El sitio queda en la página original; el usuario puede volver vía task switcher / cierre del modal del wallet.

**Métrica**: tiempo total target <60s desde clic.

## Path B — sin wallet instalado (fallback)

1. Click en botón.
2. JS construye URI y ejecuta `window.open`.
3. El OS no encuentra handler → la navegación falla silenciosamente (browser no muestra error; el usuario percibe "nada pasó").
4. JS detecta `setTimeout(600ms)` + `!document.hidden` (la página no fue ocultada).
5. JS copia la address al clipboard via `navigator.clipboard.writeText(addr)`.
6. UI feedback: el span dentro del botón cambia a "copied" (verde) durante 2s.
7. El usuario percibe el éxito del fallback. Decide a) instalar wallet via link sugerido o b) pegar address en wallet existente fuera del browser.

**Métrica**: el feedback "copied" debe aparecer <800ms desde el clic.

## Path C — wallet rechaza el URI scheme

Edge case raro: algunos wallets antiguos o configuraciones del OS no respetan el URI scheme aunque estén instalados.

Tratamiento: idéntico al Path B (fallback a clipboard). El usuario verá "copied" y procederá manualmente. No es una falla del frontend, es un edge del wallet o configuración del OS.

## Errores explícitos

| Caso | Detección | UX |
|---|---|---|
| Clipboard API no disponible (`navigator.clipboard` ausente) | try/catch en el `writeText` | Silent fail, no feedback (raro en browsers modernos 2026; aceptable) |
| URI scheme con sintaxis inválida | Validación en build time (construcción estática) | N/A — no debería ocurrir si tests pasan |
| User en private/incognito mode con Clipboard API restringido | catch en `writeText` | Silent fail |

## Edge cases

- **Mobile**: el URI scheme es manejado por el OS (iOS/Android). Si MetaMask Mobile / Phantom Mobile / BlueWallet están instalados, el comportamiento es idéntico al desktop. Si no, fallback a clipboard funciona también (mobile Safari/Chrome soportan Clipboard API).
- **Multi-wallet**: si el usuario tiene varias wallets instaladas para el mismo scheme, el OS muestra un picker. Esto NO es responsabilidad del sitio — es comportamiento OS estándar.
- **Pali + MetaMask coexistiendo**: ambos inyectan `window.ethereum`. Como NO usamos provider injection (solo URI scheme), no hay conflicto. El user elige en el picker del OS.

## Hand-offs

- Después del clic, el sitio **no espera ni verifica** la transacción. No hay tracking, no hay confirmación. Es una propina one-shot.
- Si el user quiere on-chain verification, va al explorador (los links de los badges del README llevan a mempool.space/etherscan/etc.).
