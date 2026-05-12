#!/usr/bin/env python3
"""Persist sleep-mode research findings to SEELE."""

import json
import urllib.request
import sys
import io

if sys.stdout.encoding and sys.stdout.encoding.lower() != "utf-8":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")

BASE = "http://localhost:7777"

OBSERVATIONS = [
    {
        "title": "Research — Claude Code 'Auto Dream' feature (mayo 2026)",
        "type": "discovery",
        "topic_key": "research/anthropic/auto-dream",
        "content": """# Claude Code Auto Dream — feature analysis

Anthropic's upcoming memory consolidation feature for Claude Code. Confirmed real, currently in silent rollout under feature flag `tengu_onyx_plover`.

## Status (mayo 2026)
- Official name: **Auto Dream** (not "sleep"). Manual trigger: `/dream` command.
- UI: embedded as menu entry "Auto-dream: off · never" in `/memory`.
- Rollout: gradual by cohorts, no public blog announcement yet (Anthropic "quietly shipping").
- Trigger: `minHours=24 AND minSessions=5` since last consolidation. Plus manual `/dream`.

## 4-phase pipeline
1. **Orient** — read `~/.claude/projects/<proj>/memory/` directory structure.
2. **Gather Signal** — targeted greps over JSONL transcripts, looking for: user corrections, explicit decisions, recurring patterns.
3. **Consolidate** — merge duplicated facts, normalize relative→absolute dates, eliminate contradicted facts, resolve conflicts.
4. **Prune & Index** — rewrite `MEMORY.md` as index ≤200 lines, demote verbose entries to topic files.

## Single anecdote of perf: "consolidated 913 sessions in ~8-9 minutes".

## Sources
- claudefa.st/blog/guide/mechanics/auto-dream
- dev.to/akari_iku/does-claude-code-need-sleep-inside-the-unreleased-auto-dream-feature
- decodethefuture.org/en/claude-code-auto-dream-explained
- letsdatascience.com/news/anthropic-introduces-dreaming-for-claude-agent-memory-consol
- github.com/grandamenium/dream-skill (OSS reimplementation)

## Implication for SEELE
Anthropic chose plain-text file consolidation (`MEMORY.md`) over graph architectures. SEELE with SQLite+FTS5+vec0 is in the same quadrant. Pipeline is portable. Naming convention `seele dream` would be familiar to Claude Code users.""",
        "metadata": {
            "kind": "discovery",
            "domain": "seele-features",
            "tags": ["research", "anthropic", "memory-consolidation", "auto-dream", "claude-code"],
            "model": "claude-opus-4-7",
            "context_mode": "contextual",
        }
    },
    {
        "title": "Research — Memory consolidation algorithms for SEELE",
        "type": "discovery",
        "topic_key": "research/memory-consolidation/algorithms",
        "content": """# Memory consolidation — top 5 algorithms for SEELE

Synthesized from: Mem0 (arxiv 2504.19413), Zep/Graphiti (arxiv 2501.13956), A-MEM (arxiv 2502.12110), Letta sleep-time compute (arxiv 2504.13171), Generative Agents (Park 2023), Memory in the Age of AI Agents survey (arxiv 2512.13564).

## Top 5 to implement (priority order)

### 1. Dedup híbrido — P0 MUST
- Exact hash on `normalized_hash` (already exists in SEELE).
- Semantic dedup: cosine > 0.85 between vec0 embeddings.
- For borderline 0.78-0.88: Sonnet 4.6 judge batches 10 pairs per call.
- Never delete: mark `superseded_by = winner_id`. Reversible.

### 2. Conflict resolution temporal — P0 MUST
- Bi-temporal model from Graphiti adapted to flat table.
- New fields: `valid_from`, `valid_until`.
- When new contradicts old (cosine > 0.75 + LLM judge says yes): set `valid_until = now()` on old + prefix `[ACTUALIZADO YYYY-MM-DD]`.
- Aprovecha el `revision_count` existente (topic-key upsert).

### 3. Cluster + summarize jerárquico — P1 MUST
- Embeddings already in vec0 (free).
- HDBSCAN clustering (rust crate `hdbscan = "0.12"`), allows noise.
- Per cluster n≥5: Sonnet generates summary, mark originals with `parent_summary_id`.
- Fold-only, never delete. `tree_depth` cap = 2 (anti-zombi).

### 4. Decay con earn-their-keep — P1 MUST
- Score: `earn = w1*log(1+access_count) + w2*exp(-Δt/60d) + w3*link_count + w4*importance`.
- `earn < 0.15 AND last_referenced_at > 90d` → cold storage (flag `archived_at`).
- `created_at > 30d ago` automatic bypass (Park's recency penalty mitigation).
- Reversible via `seele restore`.

### 5. Pattern → skill extraction — P2 NICE-TO-HAVE (REM-analog)
- Trigger: cluster with N≥7 obs of same topic family.
- Sonnet extracts abstract rule as new observation, `kind=skill`.
- Cap: 5-10 skills per session.
- Inspired by LangMem procedural memory + Generative Agents reflect.

## Discarded explicitly
- NREM/REM strict dichotomy: metaphor not load-bearing for SEELE.
- Cross-attention compression (SleepGate paper): requires trainable model, out of scope local-first.
- Full entity graph (Zep): SEELE stays flat+links, not KG.

## Cost per session (1000 memories, Sonnet 4.6 @ $3/$15 per Mtok)
- Dedup judge: ~$0.30
- Conflict: ~$0.20
- Cluster summary: ~$0.40
- Skill extraction: ~$0.25
- Total: **~$1.15 USD/session**

Monthly budget cap: $5 USD via `--max-llm-cost`.

Local-only mode: swap Sonnet for SmolLM2-1.7B via `candle`. Quality degrades for summaries/skills, OK for judges.""",
        "metadata": {
            "kind": "discovery",
            "domain": "seele-features",
            "tags": ["research", "memory-consolidation", "algorithms", "papers"],
            "model": "claude-opus-4-7",
            "context_mode": "contextual",
        }
    },
    {
        "title": "Plan táctico — Sprint SEELE-06 consolidate-foundation (v0.2.0)",
        "type": "architecture",
        "topic_key": "seele/sprint-06/plan",
        "content": """# Sprint SEELE-06 — consolidate-foundation (target v0.2.0)

## Scope
Implement `seele consolidate` (alias `seele dream`) — local sleep/consolidation mode.
Modelo target: **Sonnet 4.6** (NO Opus, per user request).
TUI gate humano per-proposal con batch accept.

## Crate nuevo: `seele-consolidate`

```text
crates/seele-consolidate/
  src/
    lib.rs           # ConsolidationEngine + Config
    plan.rs          # ProposalBatch JSON serde
    proposers/
      dedup.rs       # hash+cosine, LLM judge for borderline
      conflict.rs    # bi-temporal invalidation
      cluster.rs     # HDBSCAN + Sonnet summary
      decay.rs       # earn-score → archived
      skill.rs       # REM-analog, capped
    sonnet.rs        # Anthropic API client (batch judge)
    candle_local.rs  # SmolLM2 fallback (feature flag)
    gate.rs          # TUI integration (review iter)
  tests/integration.rs
```

## Schema migrations (refinery)
```sql
ALTER TABLE observations ADD COLUMN superseded_by TEXT REFERENCES observations(id);
ALTER TABLE observations ADD COLUMN parent_summary_id TEXT REFERENCES observations(id);
ALTER TABLE observations ADD COLUMN folded_at TIMESTAMP;
ALTER TABLE observations ADD COLUMN archived_at TIMESTAMP;
ALTER TABLE observations ADD COLUMN valid_until TIMESTAMP;
ALTER TABLE observations ADD COLUMN earn_score REAL;
ALTER TABLE observations ADD COLUMN consolidation_source TEXT;  -- anti-loop
CREATE TABLE consolidation_runs (...);
```

Search filter default: `WHERE folded_at IS NULL AND archived_at IS NULL AND (valid_until IS NULL OR valid_until > ?)`.

## Bloques

| # | Tema | Tiempo | Output |
|---|---|---|---|
| A | ADR + schema migrations + skeleton crate | 3-4h | crate compila, tests vacíos pasan |
| B | Phase 1 PROPOSE (4 proposers, todo local, sin LLM) | 4-5h | `seele consolidate --dry-run` produce plan.json |
| C | Phase 2 REVIEW (Sonnet 4.6 client + batch judge + plan execution within transaction) | 4-5h | `seele consolidate --auto-yes` ejecuta plan completo |
| D | TUI 6ta vista Consolidate con per-kind batch approval + CLI flags | 3-4h | UX completa interactiva |
| E | Evidence (tests + cost-ledger) + devlog + cierre | 2-3h | sprint cerrado |

**Estimado total**: 16-21 horas. 4-5 sesiones de Orlando.

## Comando CLI propuesto
```bash
seele consolidate                              # interactive TUI
seele consolidate --dry-run                    # only Phase 1, no LLM, no execute
seele consolidate --auto-yes                   # skip TUI (peligroso, requiere flag explícito)
seele consolidate --max-llm-cost 5.00          # cap costo
seele consolidate --since 30d                  # only memories since
seele consolidate --local-llm                  # SmolLM2 fallback
seele consolidate --resume <session-id>        # reanudar sesión interrumpida
```

Alias: `seele dream` (familiar to Claude Code users).

## Gates AEGIS
- Gate 1: aprobación del plan táctico + decisión sobre nombre (`consolidate` / `dream` / `sleep` / `mirra`)
- Gate 2: review de Phase 2 execution con sample real (validar UX gate humano + costos)

## Riesgos críticos identificados
1. Memorias zombi (summaries re-summarizables) → tree_depth cap, fold-only
2. Cross-contamination project/scope → GROUP BY (project, scope) en cluster
3. Decay injusto en memorias nuevas → bypass `created_at > 30d`
4. LLM judge alucina conflicts → confidence < 0.9 skip, gate humano obligatorio
5. Re-embedding silencioso → embedding_dirty flag
6. Sleep loop infinito → consolidation_source filter

## Cuándo NO usar este sprint
- Si v0.1.0 release final no está cerrado todavía (pending billing/public flip): cerrar v0.1.0 primero
- Si Sprint LUMEN-01 closure prioritario: terminar primero
- Si la base de memorias de Orlando es <100 observations: consolidate no aporta valor todavía""",
        "metadata": {
            "kind": "architecture",
            "domain": "seele-features",
            "tags": ["plan", "sprint-06", "consolidate", "v0.2.0"],
            "model": "claude-opus-4-7",
            "context_mode": "contextual",
        }
    },
]

def post(obs):
    body = json.dumps({
        "title": obs["title"],
        "content": obs["content"],
        "type": obs["type"],
        "project": "seele",
        "scope": "project",
        "topic_key": obs["topic_key"],
        "metadata": obs.get("metadata", {}),
    }).encode("utf-8")
    req = urllib.request.Request(
        f"{BASE}/memories",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read())
            return True, data.get("id", "?"), None
    except Exception as e:
        return False, None, str(e)

print(f"Persisting sleep-mode research to SEELE...\n")
ok, err = 0, 0
for i, obs in enumerate(OBSERVATIONS, 1):
    success, obs_id, error = post(obs)
    icon = "✓" if success else "✗"
    title_short = obs["title"][:60]
    if success:
        print(f"  {icon} [{i}/{len(OBSERVATIONS)}] {title_short:<62} → {obs_id}")
        ok += 1
    else:
        print(f"  {icon} [{i}/{len(OBSERVATIONS)}] {title_short:<62} → {error}")
        err += 1

print(f"\nDone: {ok} ok · {err} err")
