# Naming — proceso de decisión y nombre final

## Contexto

El nombre fija el branding del binary, el repo, los env vars (`SEELE_DB`, `SEELE_PORT`...), los paths default (`~/.seele/`), los logs, los release artifacts. Cambiarlo después tiene costo.

## Criterios

Heredados del registro de naming del ecosistema (AEGIS, LUMEN, MNEMA):

1. **5-7 letras**, fácil de tipear.
2. **Pronunciación clara** en español latino y en inglés.
3. **Greco-latina o evocadora** — palabra del idioma común, no acrónimo forzado.
4. **No colisiona** con producto comercial relevante en 2026 en clase 9 (software).
5. **Encaja conceptualmente** con "substrato físico de la memoria" — piedra, libro, parche, registro, alma.
6. **El dominio `.dev` o `.io` está disponible** (preferible aunque no obligatorio).
7. **Slug no taken** en GitHub para `orlando-vazquez-career/<name>` (verificable).

## Proceso

En la primera iteración el Orquestador propuso STELE (griego στήλη, "lápida inscrita"). El User contraofreció **SEELE** (alemán "alma") con la racional: *"es exactamente eso, es el alma de todos los proyectos"*. Decisión final: **SEELE**.

## Opciones evaluadas

### Opción elegida — SEELE

**Origen**: alemán *Seele*, "alma, espíritu, psique". Palabra del idioma común alemán; no copyrightable como concepto.

**Pros**:
- Metáfora fortísima alineada al rol del engine: la memoria es lo que constituye el alma de un agente. Encaja con MNEMA (substrato anímico) sin pisar su metáfora.
- 5 letras. Pronunciable: ZAY-luh (alemán correcto) / SEE-leh (anglocéntrico común).
- Greco-germánico — distinto registro a AEGIS/LUMEN/MNEMA (greco-latinos), pero coherente con tradición filosófica europea.
- Permite branding visual: glyph minimalista, theme oscuro/contemplativo. Encaja con el epígrafe latino propuesto: *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus*.

**Contras**:
- Asociación inevitable con Evangelion (1995-1997) — SEELE es el nombre de la organización antagonista. Doble filo: atrae fans dev / puede sentirse "tomado" por terceros.
- Pronunciación dual (es/en) puede confundir en presentaciones.

**Riesgo trademark**:
- "Seele" como palabra alemana común NO es copyrightable.
- Khara/Gainax tienen marca registrada de "SEELE" probablemente en clases 41 (entretenimiento), 16 (impresos), 25 (ropa) — relacionadas a la franquicia Evangelion.
- **Clase 9 (software) no debería estar tomada** porque "Seele" es palabra común alemana. Riesgo bajo si:
  1. No registramos TM nuestro en clase 41 / 16 / 25.
  2. No usamos branding visual de Evangelion (no fonts SEELE-style, no colores rojo+blanco icónicos, no monolito).
  3. Disclaim explícito en README: "palabra alemana común, no relacionada con franquicias".

**Disponibilidad**: dominio `seele.dev` y `seele.io` no verificados; slug GitHub `seele` parcialmente tomado en otros orgs (verificable; en `orlando-vazquez-career` libre).

### Otras opciones evaluadas

#### STELE (propuesta inicial del Orquestador, no elegida)

Griego στήλη, "lápida inscrita en piedra". 5 letras, pronunciable, baja collision, encaja con MNEMA en registro. Razón por la que no se eligió: SEELE captura mejor la metáfora "alma del agente" que prefirió el User.

#### CODEX (descartada — collision)

Latín *codex*, "libro encuadernado". Alta collision: OpenAI Codex, otros. Descartada.

#### CALAMUS (descartada — menos directa)

Latín *calamus*, "pluma de escribir". Acto de inscribir vs resultado. 7 letras, baja collision. No elegida — SEELE es más conceptual.

#### VELLUM (descartada — collision)

Latín *vellum*, "pergamino". Collision con Vellum AI (prompt management). Descartada.

#### HEBB (descartada — riesgo de pronunciación ofensiva)

Donald Hebb, neurociencia. Descartada por riesgo de slur en pronunciación inglesa.

#### SCRIBA (descartada — collision media)

Latín *scriba*, "escriba". Múltiples productos open source con el nombre. Descartada.

## Decisión final

**SEELE** confirmado por el User el 2026-05-09 con racional explícito: *"es el alma de todos los proyectos"*.

Path locked-in: `C:\dev\tools\SEELE\` (separado de `C:\dev\protocols\` que aloja AEGIS/LUMEN/MNEMA — SEELE es herramienta, no protocolo).

Repo GitHub previsto: `orlando-vazquez-career/seele`.

Epígrafe oficial del proyecto:

> *Super stellatum firmamentum iudicat Deus, sicut nos iudicamus.*
> Sobre el firmamento estrellado juzga Dios, como nosotros juzgamos.

Aparece en:
- README.md (al inicio, latín + traducción) — primera impresión del proyecto.
- Cierre de CREDITS.md — donde queda como sello de cierre del documento.

NO aparece en el banner del CLI (un epígrafe de 12 palabras dentro de un ASCII art TUI puede leerse pretencioso o quedar feo en monoespaciado). El epígrafe pertenece al texto en prosa, no al output del binary.

## Disclaim oficial

En el README de SEELE incluir:

```markdown
## On the name

"Seele" is the German word for "soul" or "spirit". It is a common word
of the German language, not a trademark we claim and not a reference to
any specific franchise. We chose it because the metaphor — memory as
what constitutes the soul of an agent — captures what this engine is
for.
```

Esto preempta confusión con Evangelion sin demonizar la asociación (los fans del anime que lleguen al repo no se sentirán rechazados; los no-fans entenderán de inmediato que el nombre viene del idioma alemán, no de la franquicia).
