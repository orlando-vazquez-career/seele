# Sprint LUMEN-03 — scope

**Fecha**: 2026-05-12
**Protocolo**: LUMEN v0.10.0
**Tema**: Observability panel + responsive fix
**Modo**: lightweight sprint (extiende baseline brutalista, no re-diseña)

## Trigger

Director eyeball pass de LUMEN-02 (Round 3) identificó dos issues que no son ajustes menores sino scope nuevo:

1. **Responsive whitespace** — A 1920×1080 viewport, `.page-container` deja ~700px de whitespace lateral a la derecha. El director lo describió como "las proporciones están mal optimizadas para esta resolución". El estilo brutalista en sí pasa el eyeball, pero el comportamiento de layout a viewports anchos no.
2. **Falta observabilidad** — "monitoreo real del estado de memorias de SEELE, quiero que se pueda gestionar desde este frontend o por lo menos observar que se está guardando y cómo". El director pidió "fijate en los repositorios de ENGRAM como manejaron los monitoreos y observabilidad, inspírate no copies".

Investigación ENGRAM: tienen TUI con Dashboard panel (stats counts + recent observations + project list), Search panel, Recent panel, Detail panel, Timeline panel. No tienen web frontend. Sus endpoints HTTP equivalentes a SEELE: `/stats`, `/observations/recent`, `/search`, `/observations/{id}`.

## Decisión de estructura

Sprint LUMEN-02 cerró limpio con el baseline brutalista (commit `cf857b2`, push remote). Sprint LUMEN-03 se abre inmediato sin extender LUMEN-02 — boundaries AEGIS limpias.

## Objetivos

1. **Responsive fix** — eliminar el whitespace lateral derecho en viewports >1400px. Soluciones evaluadas:
   - A) Elevar `max-width` de page-container desde 1100px hacia ~1400px en viewports grandes
   - B) Cambiar a layout de 2 columnas en viewports anchos (content izq + observability derecha sticky)
   - C) Centrar el page-container con margenes simétricos arriba de cierto viewport
   - **Decisión: B** — resuelve los dos problemas a la vez (responsive + observability vive en el espacio recuperado)

2. **Observability panel (scope: maximum)** — director eligió scope maximum sobre minimal/medium. Componentes:
   - **Live stats card** — total memories, projects count, ratio active/deleted, last save timestamp
   - **By-type bar chart** — top N tipos (advisor_output, decision, etc) con barras horizontales
   - **Recent saves list** — últimas 10 con title + type pill + project + age relativo ("3min ago")
   - **Search input** — pega /search?q= → renderiza results inline (no nueva pantalla)
   - **Detail expanded panel** — click en recent/result → muestra full content + metadata abajo
   - **Sparkline saves/day** — últimos 14 días, fetch /memories aggregando client-side (acceptable para <1000 memories)
   - **Per-project breakdown** — counts por project (mnema, seele, default, etc) — derivado de /stats

3. **Connection state** — toda la observabilidad solo se renderiza si `/health` responde OK. Otherwise muestra CTA "Run `seele serve` to enable live observability" con copy de instrucción.

4. **No backend changes** — todo client-side fetch contra el HTTP server local que SEELE ya expone.

## Alcance fuera (deferred a LUMEN-04)

- Chat con Kimi K2 (user decision a request de Sprint LUMEN-04 con scope dedicado)
- Edit/delete memories desde el panel
- Project-name migration desde panel
- Auth header / token handling (opcional, SEELE actualmente accept sin auth para local)

## Métricas de éxito

- Sin whitespace lateral excedente a 1920×1080 (verificado por director eyeball)
- Panel funciona contra SEELE local (verificado: /health → /stats → /memories → render)
- Bundle no excede 20 KB gzipped (current 10.7 → permitir +9 KB para observability JS)
- A11y: panel keyboard-navegable + screen-reader anuncia stats live (aria-live="polite")
- Performance: observability fetch no bloquea paint inicial (lazy)

## Riesgos

- **CORS** — browser fetch a localhost:7777 desde localhost:4321 (dev) o github.io (prod) puede chocar con CORS. SEELE HTTP server tiene que enviar `Access-Control-Allow-Origin`. Verify primero.
- **Large DBs** — el sparkline client-side aggregation se vuelve lento >1000 memories. Mitigation: paginate /memories fetch (limit=50) + fallback "data sample" disclaimer si DB > N.
- **No data state** — DB vacío produce gráficos vacíos. Diseño tiene que manejar gracefully.

## Plan de bloques

- Bloque A: responsive fix + observability scaffold (Observability.astro + tokens responsive)
- Bloque B: data fetching + render (stats + recent + by-type + per-project)
- Bloque C: search + detail expand
- Bloque D: sparkline + finishing
- Bloque E: critique loop + evidence + commit

Estimate: ~3-4 horas wall-clock con Opus 4.7.
