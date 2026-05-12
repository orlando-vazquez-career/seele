# OOUX-ORCA — Sprint LUMEN-01

Aplicación mínima de Object-Oriented UX / ORCA (Sophia Prater). Esta landing tiene pocos objetos pero los explicito para que `<DonateButton>` quede reusable si en futuro hay docs/blog/dashboard.

## Objects

| Object | Definición |
|---|---|
| **Network** | Red cripto (Bitcoin, Ethereum, Base, Syscoin NEVM, Solana). |
| **DonateTarget** | Combinación (Network, Address). Lo que el botón ejecuta. |
| **Feature** | Capacidad de SEELE (CLI / MCP / HTTP / TUI). |
| **InstallPath** | Vía de instalación (script, cargo, source). |
| **Doc** | Página de documentación en docs/ (referenciable externamente). |

## Relationships

```
Network    1 ── N  DonateTarget    (una red puede tener N targets; aquí 1 por red excepto address compartido entre ETH/Base/Sys)
DonateTarget  has  URI scheme       (BIP-21 / EIP-681 / Solana Pay)
DonateTarget  has  Address          (texto canónico para clipboard fallback)
DonateTarget  has  Explorer URL     (link de verificación, para badges del README)
Feature       has  CTA              (link a docs/ relevante)
InstallPath   has  Command          (snippet copiable)
```

## CTAs (acciones primarias)

| Object | CTA | Comportamiento |
|---|---|---|
| `DonateTarget` | **Donate** (botón principal del widget) | Construye URI, `window.open(uri, '_self')`, fallback clipboard. |
| `DonateTarget` | _Copy address_ (acción secundaria implícita en fallback) | `navigator.clipboard.writeText(addr)`. |
| `Feature` | **View docs** | Link interno o a docs/ en GitHub. |
| `InstallPath` | **Copy command** | Copy-to-clipboard del snippet. |
| `Doc` | **Open** | Link externo a GitHub. |

## Attributes (campos)

### `Network`
- `id`: `btc | eth | base | sys | sol`
- `label`: display name
- `brandColor`: hex (para el badge background)
- `logoSvg`: opcional, inline SVG
- `chainId`: solo EVM (1 ETH, 8453 Base, 57 Syscoin)

### `DonateTarget`
- `network`: ref a `Network`
- `address`: string
- `uri`: string (computed: `<scheme>:<addr>[@<chainId>][?label=SEELE]`)
- `explorerUrl`: string (opcional, para link de verificación)

### `Feature`
- `id`: `cli | mcp | http | tui`
- `title`: string
- `description`: string (1-2 frases)
- `commandsSnippet`: string opcional
- `docLink`: string

### `InstallPath`
- `id`: `script | cargo | source`
- `title`: string
- `command`: string
- `notes`: string opcional

## Por qué ORCA mínimo (no full)

Esta landing tiene 5 objects con relationships simples. Un ORCA completo (con CTAs cross-objet, content references, etc.) sería overkill. La estructura acá es suficiente para que el componente `<DonateButton>` quede reusable y para que en LUMEN-02 sea trivial agregar `Doc` como tipo de superficie.

## Cross-protocol nota (AEGIS + LUMEN)

`DonateTarget` puede persistirse como `seele save --type pattern --topic-key design/component/DonateButton` para que sprints futuros encuentren la decisión sin re-inventar el patrón.
