# ADR-10 — Mapping conceptual MNEMA ↔ SEELE

**Estado**: Aceptado · 2026-05-10
**Decisión**: MNEMA mapea su modelo de Counsel + Skills + Decisiones al schema de SEELE de la siguiente manera. Este ADR fija el contrato para que la primera integración no requiera re-modelado.
**Cierra**: observación [ALTO] de Cloven en review post-arquitectura — *"no hay traducción entre los dos vocabularios"*.

## Contexto

SEELE adopta el vocabulario de ENGRAM (sessions, observations, types tipo `decision/architecture/bugfix/...`, topic_keys con family heuristics de coding agents). MNEMA tiene su propio vocabulario (Counsel pattern: 5 advisors + 5 reviewers + verdict; kinds tipo `decision/skill/advisor_output/review/verdict/memory`; conceptos como `verdict_id`, `advisor_role`, `blind_id`, `axiomatic`, `core`, `earn_score`).

Sin un mapping explícito, el primer sprint donde MNEMA consuma SEELE (sprint-04 BE de MNEMA, según el plan original) va a generar mapping ad-hoc inconsistente. Mejor cerrarlo ahora.

## Mapping

### Capa 1 — Counsel session ↔ SEELE session

Una **counsel completa** de MNEMA = una **session** de SEELE.

| MNEMA Counsel | SEELE session field | Notas |
|---|---|---|
| Counsel ID (ej: `counsel_01HX...`) | `sessions.id` (ULID) | Mismo ULID |
| Pregunta inicial / título | `sessions.summary` (al cierre) | Se completa al `seele_session_end` |
| Project / dominio (ej: `dev-zen`) | `sessions.project` | Direct mapping |
| Working directory (si aplica) | `sessions.directory` | Opcional |
| Inicio de Recall phase | `sessions.started_at` | Timestamp |
| Cierre de Encode phase | `sessions.ended_at` | Timestamp |
| Estado (en curso / cerrado / abortado) | `sessions.status` | enum |

**Concretamente**: cuando MNEMA arranca una counsel, llama `seele_session_start({id: counsel_01HX..., project: "dev-zen", directory: cwd})`. Cuando emite verdict, llama `seele_session_end({id: counsel_01HX..., summary: "..."})`.

### Capa 2 — Outputs intermedios ↔ SEELE observations

Cada output del Counsel (advisor outputs, reviews, prompts) es una **observation** de SEELE, linkeada a la session vía `session_id`.

| MNEMA concept | SEELE `observations` row | Type field | Metadata JSON |
|---|---|---|---|
| Pregunta del User | row con `type='memory'`, `tool_name='user_prompt'` | `memory` | `{kind: 'user_prompt'}` |
| Output de un advisor | row con `type='advisor_output'`, content del JSON output | `advisor_output` | `{kind: 'advisor_output', advisor_role: 'contrarian\|first-principles\|expansionist\|outsider\|executor', model: 'claude-opus-4-7', tokens_used: ..., counsel_id: 'counsel_01HX...'}` |
| Output de un reviewer ciego | row con `type='review'`, content del JSON | `review` | `{kind: 'review', blind_id: 'blind_xyz', advisor_role_reviewed: '...', score_breakdown: {...}, model: ..., tokens_used: ..., counsel_id: ...}` |
| Verdict final | row con `type='verdict'`, content del verdict razonado | `verdict` | `{kind: 'verdict', counsel_id: 'counsel_01HX...', accepted_advisor_roles: [...], dissent: '...', model: ..., tokens_used: ...}` |
| Skill emitido | row con `type='skill'`, content del skill document | `skill` | `{kind: 'skill', skill_id: 'skl_01HY...', derived_from_verdict: 'vrd_01HX...', earn_score: 1.0, axiomatic: false}` |
| Decisión persistida (post-verdict) | row con `type='decision'`, body del razonamiento decidido | `decision` | `{kind: 'decision', verdict_id: 'vrd_01HX...', earn_score: 1.0, axiomatic: false, core: false, last_referenced_at: ts}` |
| Memoria genérica (no parte de counsel) | row con `type='memory'` | `memory` | `{kind: 'memory', earn_score: 1.0}` |

**Notas clave**:
- El `type` field de SEELE acepta el set extendido `decision | architecture | bugfix | pattern | config | discovery | learning | memory | skill | advisor_output | review | verdict`. Los primeros 7 son los de ENGRAM (coding agents); los últimos 5 son los de MNEMA. SEELE no enforce un enum cerrado en el schema — el `type` es TEXT libre, pero los family heuristics sí filtran por estos sets.
- `kind` sigue dentro del JSON `metadata` para compatibilidad con consumers que usen schema MNEMA-específico (muchos campos de MNEMA viven solo en metadata).
- `counsel_id` en metadata permite query "todos los outputs de tal counsel" sin join contra `sessions` cuando ya tenés el ID.

### Capa 3 — Tags / domain / project

| MNEMA | SEELE | Notas |
|---|---|---|
| `tags: ["api", "rate-limit"]` | metadata JSON `{tags: [...]}` | Multi-tag flexible |
| Tag principal / canonical | `observations.topic_key` | Singular; usado para upserts |
| `domain: "dev-zen"` | `observations.project` | Mapping directo |

### Capa 4 — Lifecycle metadata

Estos campos viven en `metadata` JSON en SEELE, indexados via virtual generated columns (ver ADR-02).

| MNEMA campo | SEELE metadata path | Virtual column |
|---|---|---|
| `earn_score: 1.0` | `metadata.earn_score` | `meta_score` |
| `axiomatic: true/false` | `metadata.axiomatic` | `meta_axiomatic` |
| `core: true/false` | `metadata.core` | (sin virtual col por default; consumer puede agregar) |
| `last_referenced_at` | `metadata.last_referenced_at` | (sin virtual col; o usar `last_seen_at` que es column nativa de observations) |
| `blind_id` | `metadata.blind_id` | (sin virtual col por default) |
| `model` (LLM model usado) | `metadata.model` | (sin virtual col) |
| `tokens_used` | `metadata.tokens_used` | (sin virtual col) |

MNEMA puede agregar virtual columns custom via `seele schema add-virtual-col mnema_blind_id $.blind_id` si emerge demanda de filter eficiente.

### Capa 5 — Relations / links / supersede

MNEMA tiene `linked_to: [...]` y conceptos de `supersedes`, `derives_from`, `evidence_for`. SEELE tiene **dos** mecanismos relacionales:

- **`memory_relations`**: para conflict / judgment lifecycle (heredado de ENGRAM). Estados: pending / judged / orphaned / ignored. Relations: `supersedes / conflicts_with / scoped / related / compatible / not_conflict`.
- **`links`**: tabla general purpose para grafos de memoria sin lifecycle de juicio. Relations libres: `derives_from / supersedes / related_to / contradicts / evidence_for / part_of_verdict / etc.`

| MNEMA concept | SEELE table | link_type / relation |
|---|---|---|
| `derives_from <skill_old>` | `links` | `derives_from` |
| `supersedes <skill_old>` (skill evolutiva) | `links` | `supersedes` |
| `related_to <vrd_X>` | `links` | `related_to` |
| `evidence_for <vrd_X>` | `links` | `evidence_for` |
| `part_of_verdict <vrd_X>` (advisor output asociado) | `links` | `part_of_verdict` |
| `contradicts <vrd_old>` (con razón explícita) | `memory_relations` con relation=`conflicts_with`, status=`judged` | El judgment lifecycle protege la decisión |
| `superseded-by <vrd_NEW>` | `memory_relations` con relation=`supersedes`, status=`judged` | Mismo motivo |

**Regla práctica**: usar `links` para relaciones derivativas/explicativas que no requieren resolución (graph). Usar `memory_relations` cuando la relación implica que **una decisión invalida a otra** (necesita auditoría, rollback, supersede formal).

### Capa 6 — Domain / project plurality

MNEMA opera sobre múltiples dominios (`dev-zen`, `mnema`, otros proyectos del User). SEELE soporta esto natívamente via `observations.project`.

- **MNEMA decide en qué project guarda** cada observation. Ej: una decisión de arquitectura del propio MNEMA se guarda con `project='mnema'`. Una skill de un cliente externo se guarda con `project='client-acme'`.
- **`scope: project | personal`**: MNEMA usa `personal` para skills cross-domain (heurísticas universales del User) y `project` para todo lo project-specific.

### Capa 7 — Topic keys MNEMA-specific

MNEMA define su propio set de family heuristics, registrado via `~/.seele/topic-families.toml`:

```toml
[families.mnema]
prefix_patterns = [
    "verdict/*",          # ej: verdict/architecture-v3
    "skill/*",            # ej: skill/counsel-orchestration
    "axiomatica/*",       # ej: axiomatica/local-first-storage
    "disenso/*",          # ej: disenso/postgres-vs-sqlite-decision
    "decision/*",         # heredado de coding agents (compatible)
    "memory/*",           # heredado
]
suggest_function = "mnema_suggest_topic_key"  # opcional, función custom
```

SEELE expone API para registrarlas:

```bash
seele topic-keys add-family --prefix "verdict/*" --consumer mnema
seele topic-keys list
```

Y MCP tool `seele_suggest_topic_key` consulta este registry antes de fallback al set default.

## API integration de MNEMA

El backend de MNEMA (TS+Bun) tiene un wrapper sobre el HTTP/MCP de SEELE:

```typescript
// MNEMA backend — ejemplo conceptual
import { SeeleClient } from '@mnema/seele-client';

const seele = new SeeleClient({ url: 'http://localhost:7437', token: process.env.SEELE_TOKEN });

async function startCounsel(counselId: string, project: string) {
  await seele.session_start({ id: counselId, project, directory: process.cwd() });
}

async function persistAdvisorOutput(counselId: string, advisorRole: string, output: string, model: string, tokens: number) {
  return seele.save({
    body: output,
    metadata: {
      kind: 'advisor_output',
      advisor_role: advisorRole,
      counsel_id: counselId,
      model,
      tokens_used: tokens,
    },
    type: 'advisor_output',
    title: `${advisorRole} output for ${counselId}`,
    project: getProjectFromCounsel(counselId),
    scope: 'project',
  });
}

async function persistVerdict(counselId: string, verdict: string, acceptedRoles: string[], dissent: string) {
  const verdictId = `vrd_${ulid()}`;
  const obs = await seele.save({
    body: verdict,
    metadata: {
      kind: 'verdict',
      counsel_id: counselId,
      verdict_id: verdictId,
      accepted_advisor_roles: acceptedRoles,
      dissent,
      earn_score: 1.0,
      axiomatic: false,
    },
    type: 'verdict',
    title: `Verdict for ${counselId}`,
  });

  // Link verdict to all advisor outputs of this counsel
  const advisorOutputs = await seele.list({ filters: { counsel_id: counselId, type: 'advisor_output' } });
  for (const out of advisorOutputs) {
    await seele.link({ from_id: out.id, to_id: obs.id, link_type: 'part_of_verdict' });
  }

  await seele.session_end({ id: counselId, summary: verdict.slice(0, 200) });
  return { observation_id: obs.id, verdict_id: verdictId };
}
```

## Casos de uso end-to-end

### Caso 1 — Counsel para una decisión de arquitectura

```
MNEMA Counsel start
  → SEELE session_start (id=counsel_01HX, project='dev-zen')
  → 5 advisors generan outputs en paralelo
    → 5x SEELE save (type='advisor_output', metadata.advisor_role + counsel_id)
  → 5 reviewers ciegos generan reviews
    → 5x SEELE save (type='review', metadata.blind_id + advisor_role_reviewed + counsel_id)
  → 1 verdict
    → SEELE save (type='verdict', metadata.verdict_id + dissent + accepted_roles)
    → SEELE link 5 advisor_outputs → verdict (link_type='part_of_verdict')
    → SEELE link 5 reviews → verdict (link_type='part_of_verdict')
  → MNEMA Encode phase
    → Si emite skill: SEELE save (type='skill', metadata.derived_from_verdict)
    → Si emite decision persistida: SEELE save (type='decision', metadata.verdict_id)
  → SEELE session_end (summary truncado del verdict)
```

### Caso 2 — Recall pre-counsel

```
MNEMA Recall phase para nueva pregunta
  → SEELE search (query=pregunta, filters: { kind: 'decision', project: 'dev-zen' })
  → SEELE search (query=pregunta, filters: { kind: 'skill' })
  → SEELE list (filters: { kind: 'verdict', project: 'dev-zen', axiomatic: true })
  → MNEMA consolida resultados como contexto del counsel
```

### Caso 3 — Supersede de una decisión vieja

```
MNEMA Verdict en counsel-NEW invalida verdict-OLD
  → SEELE save verdict-NEW (type='verdict')
  → SEELE memory_relations insert: 
       source_id=vrd_OLD, target_id=vrd_NEW, 
       relation='supersedes', judgment_status='judged',
       reason='New evidence X', confidence=0.95,
       marked_by_actor='counsel', marked_by_kind='verdict_emission'
  → MNEMA UI muestra annotation `superseded_by: vrd_NEW` cuando alguien busca vrd_OLD
```

## Validación del mapping

El mapping se valida con un test E2E al cierre del sprint-01 BE de SEELE + sprint-04 BE de MNEMA (cuando MNEMA empiece a consumir SEELE):

1. **Test de sesión completa**: arrancar counsel, persistir 5 advisors + 5 reviews + 1 verdict, cerrar sesión. Verificar que el query `SELECT * FROM observations WHERE session_id = ?` devuelve 11 rows con types correctos.
2. **Test de supersede**: emitir vrd_OLD con axiomatic=false. Después emitir vrd_NEW que supersede. Verificar que `seele search` con filter `kind: 'verdict'` muestra annotation correcta.
3. **Test de skill evolution**: emitir skill v1, después skill v2 con `derives_from` link. Verificar que ambos están en `links` table y query "skills relacionadas" devuelve ambos.
4. **Test de Recall**: simular query Recall con 10 decisiones del project, verificar que la respuesta orden ed por earn_score * cosine_similarity.

## Consecuencias

### Positivas

- MNEMA puede integrar SEELE sin re-modelar conceptos.
- SEELE no necesita conocer nada de Counsel pattern — el contrato es a través de metadata libre.
- Otros consumers (no MNEMA) pueden seguir el mismo patrón con sus propios kinds en metadata.
- Mapeo bidireccional documentado evita drift entre los dos vocabularios.

### Negativas

- Algunos campos de MNEMA viven solo en metadata JSON, no en columns nativas. Significa que queries específicas de MNEMA (ej: filtrar por `blind_id`) requieren virtual generated column adicional. Mitigación: SEELE expone CLI `seele schema add-virtual-col` para que MNEMA agregue las que necesite.
- Doble mecanismo relacional (`memory_relations` vs `links`) puede confundir. Mitigación: regla explícita en este ADR ("links para graph derivativo, memory_relations para invalidación con auditoría").

## Referencias

- ADR-02 — Schema SQLite (memories/observations/relations/links).
- MNEMA `MNEMA-PROTOCOL.md` — definición de kinds y conceptos.
- MNEMA `guides/memory-engine.md` — schema MNEMA del JSON metadata.
- ENGRAM `/docs/ARCHITECTURE.md` — schema original que SEELE adopta.
- Cloven review 2026-05-10 — observación [ALTO] sobre falta de mapping.
