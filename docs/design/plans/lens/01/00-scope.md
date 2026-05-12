# Sprint LUMEN-01 — Scope

**Fecha**: 2026-05-11
**Tema**: Landing page + donate widget para SEELE v0.1.0
**Escala declarada**: **M-L** (vista nueva, primer frontend del proyecto)
**Mixto BE+FE**: sí — AEGIS supplemental cubre flip público + GH Action deploy

## Objetivo

Crear la primera presencia web del proyecto SEELE: una landing ultraligera que (a) comunique en <30s qué hace el engine y por qué importa, (b) permita instalar/probarlo en un par de comandos copy-paste, (c) facilite donaciones cripto one-click vía URI schemes (BTC/ETH/Base/Syscoin/Solana), con fallback a clipboard.

## Por qué ahora

- README hace su trabajo para devs que ya llegaron, pero no hay surface web para newcomers que descubren el proyecto via HN/Reddit/Twitter.
- Sin landing, las wallets en el README quedan inertes (no hay UX de donate).
- v0.1.0 está listo técnicamente. La landing es el último gating item antes del release publicitable.

## Entregables del sprint

1. `web/` Astro project en mismo repo (gitignored node_modules + dist).
2. `DESIGN.md` v0.1.0 con tokens primitivos + semánticos.
3. Landing single-page: Hero, Features (CLI/MCP/HTTP/TUI), Install, Support (donate grid), Footer.
4. `<DonateButtons>` island vanilla JS — 5 redes, URI scheme + clipboard fallback.
5. Deploy en GitHub Pages via Action a `gh-pages` (requiere flip público).
6. a11y WCAG 2.2 AA + Lighthouse 100/100/100/100.

## Fuera de scope

- Docs site (deferred a sprint LUMEN-02 si emerge demanda).
- Dashboard / panel interactivo del SEELE engine — solo TUI/CLI por ahora.
- i18n (solo EN inicial; ES considerar en LUMEN-02).
- Blog / changelog auto-generado.
- Custom domain (gh-pages.io subdomain por ahora; CNAME futura decisión).

## Stack ratificado en Gate 1

- **Astro 5.x** (ranking #1 en investigación 2026, Cloudflare-adquirida, OSS, islands gratis).
- **Vanilla JS** para donate widget (sin SDK web3, sin WalletConnect, sin tracking).
- **GitHub Pages** como hosting (gratis, repo público).
- **Sin Tailwind** inicial — CSS escrita a mano con tokens; reconsiderar en LUMEN-02 si la superficie crece.

## Decisiones humanas tomadas (Gate 1)

| Decisión | Elección | Alternativas descartadas |
|---|---|---|
| Stack | Astro 5 | Vanilla puro, Eleventy, Hono SSG |
| Ubicación | `web/` mismo repo | Repo separado `seele-web` |
| Hosting | GitHub Pages | Cloudflare Pages, Vercel |
| Donate widget | Vanilla URI scheme | WalletConnect, EIP-1193 connect flow |
