//! Test fixtures shared across the `seele-search` integration tests.
//!
//! Lives under `tests/common/` so cargo treats it as a non-test module — it
//! is `mod common;`-imported by each test binary that needs the fixtures.
//! Each binary that imports this module only uses a subset of the helpers,
//! so unused-warnings would fire spuriously. We silence them here.

#![allow(dead_code)]

use seele_core::id::SeeleId;
use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_core::relation::{JudgmentStatus, RelationKind};
use seele_embedder::{Embedder, FakeEmbedder};
use seele_search::SearchEngine;
use seele_storage::{
    init_db, JudgmentInput, ObservationStore, Pool, RelationInput, RelationStore, SaveInput,
};
use serde_json::json;
use tempfile::TempDir;

pub struct FixtureSet {
    pub td: TempDir,
    pub pool: Pool,
    pub store: ObservationStore,
    pub relations: RelationStore,
    pub embedder: FakeEmbedder,
    pub engine: SearchEngine,
    pub ids: Vec<SeeleId>,
}

impl FixtureSet {
    pub fn new() -> Self {
        let td = TempDir::new().expect("tempdir");
        let pool = init_db(td.path().join("seele.db")).expect("init_db");
        let store = ObservationStore::new(pool.clone());
        let relations = RelationStore::new(pool.clone());
        let embedder = FakeEmbedder;
        let embedder_box: Box<dyn Embedder> = Box::new(FakeEmbedder);
        let engine = SearchEngine::new(pool.clone(), embedder_box);
        Self {
            td,
            pool,
            store,
            relations,
            embedder,
            engine,
            ids: Vec::new(),
        }
    }

    pub fn save(&mut self, input: SaveInput) -> SeeleId {
        let content = input.content.clone();
        let outcome = self.store.save(input).expect("save");
        let id = outcome.id();
        let embedding = self.embedder.embed(&content).expect("embed");
        self.store
            .set_embedding(id, &embedding)
            .expect("set_embedding");
        self.ids.push(id);
        id
    }
}

/// Build a 20-observation realistic fixture covering the variety the search
/// engine should handle in Sprint-02 end-to-end tests.
///
/// Layout:
///   - 2 projects: `dev-zen`, `mnema`.
///   - Types mixed: 6 decision, 4 architecture, 3 bugfix, 3 advisor_output,
///     2 review, 2 verdict.
///   - 4 with `metadata.context_mode='purist'` (advisor_outputs).
///   - 6 with `metadata.axiomatic=true` and `metadata.score=5.0`.
///   - 3 soft-deleted.
///   - 1 supersedes relation (between obs `winner_id` and `loser_id`).
///   - 1 conflicts_with judged relation (`schema_a_id` vs `schema_b_id`).
///
/// Returns the [`FixtureSet`] plus a tuple of useful named handles for tests.
pub struct Populated20 {
    pub fx: FixtureSet,
    pub winner_id: SeeleId,
    pub loser_id: SeeleId,
    pub schema_a_id: SeeleId,
    pub schema_b_id: SeeleId,
}

pub fn populated_20() -> Populated20 {
    let mut fx = FixtureSet::new();

    // Six decisions in dev-zen.
    let winner_id = fx.save(SaveInput {
        session_id: None,
        kind: ObservationType::Decision,
        title: "use connection pooling".into(),
        content: "use pgbouncer for postgres connection pooling".into(),
        tool_name: None,
        project: Some("dev-zen".into()),
        scope: Scope::Project,
        topic_key: Some("decision/connection-pool".into()),
        metadata: Metadata::new(),
    });
    let loser_id = fx.save(SaveInput {
        session_id: None,
        kind: ObservationType::Decision,
        title: "no pooling".into(),
        content: "use pgbouncer for postgres connection pooling".into(),
        tool_name: None,
        project: Some("dev-zen".into()),
        scope: Scope::Project,
        topic_key: Some("decision/connection-pool-v0".into()),
        metadata: Metadata::new(),
    });
    for i in 0..4 {
        fx.save(SaveInput {
            session_id: None,
            kind: ObservationType::Decision,
            title: format!("decision dz {i}"),
            content: format!("dev-zen decision number {i} on rate limit policy"),
            tool_name: None,
            project: Some("dev-zen".into()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::new(),
        });
    }

    // Four architecture observations in mnema. Two with axiomatic + score.
    for i in 0..4 {
        let meta = if i % 2 == 0 {
            Metadata::from_value(json!({"axiomatic": true, "score": 5.0}))
        } else {
            Metadata::new()
        };
        fx.save(SaveInput {
            session_id: None,
            kind: ObservationType::Architecture,
            title: format!("mnema architecture {i}"),
            content: format!("counsel pattern with advisor isolation iteration {i}"),
            tool_name: None,
            project: Some("mnema".into()),
            scope: Scope::Project,
            topic_key: Some(format!("architecture/counsel-pattern-{i}")),
            metadata: meta,
        });
    }

    // Three bugfixes with axiomatic in mnema.
    for i in 0..3 {
        fx.save(SaveInput {
            session_id: None,
            kind: ObservationType::Bugfix,
            title: format!("fix race {i}"),
            content: format!("fix race condition in vec0 install path call {i}"),
            tool_name: None,
            project: Some("mnema".into()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::from_value(json!({"axiomatic": true, "score": 5.0})),
        });
    }

    // Three advisor_outputs purist in mnema.
    for i in 0..3 {
        fx.save(SaveInput {
            session_id: None,
            kind: ObservationType::AdvisorOutput,
            title: format!("primer principios {i}"),
            content: format!("from first principles the answer is iteration {i}"),
            tool_name: None,
            project: Some("mnema".into()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::from_value(json!({"context_mode": "purist"})),
        });
    }
    // One advisor_output contextual.
    fx.save(SaveInput {
        session_id: None,
        kind: ObservationType::AdvisorOutput,
        title: "contextual advisor".into(),
        content: "from prior counsels we know caching layer helps".into(),
        tool_name: None,
        project: Some("mnema".into()),
        scope: Scope::Project,
        topic_key: None,
        metadata: Metadata::from_value(json!({"context_mode": "contextual"})),
    });

    // Two reviews + two verdicts in dev-zen.
    let schema_a_id = fx.save(SaveInput {
        session_id: None,
        kind: ObservationType::Review,
        title: "schema review a".into(),
        content: "review of the schema naming snake_case approach".into(),
        tool_name: None,
        project: Some("dev-zen".into()),
        scope: Scope::Project,
        topic_key: None,
        metadata: Metadata::new(),
    });
    let schema_b_id = fx.save(SaveInput {
        session_id: None,
        kind: ObservationType::Review,
        title: "schema review b".into(),
        content: "review of the schema naming camelCase approach".into(),
        tool_name: None,
        project: Some("dev-zen".into()),
        scope: Scope::Project,
        topic_key: None,
        metadata: Metadata::new(),
    });
    for i in 0..2 {
        fx.save(SaveInput {
            session_id: None,
            kind: ObservationType::Verdict,
            title: format!("verdict {i}"),
            content: format!("the council voted in favor iteration {i}"),
            tool_name: None,
            project: Some("dev-zen".into()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::new(),
        });
    }

    // Three soft-deleted (different topic so they don't collide).
    for i in 0..3 {
        let id = fx.save(SaveInput {
            session_id: None,
            kind: ObservationType::Memory,
            title: format!("tombstone {i}"),
            content: format!("about-to-die observation iteration {i} for cleanup test"),
            tool_name: None,
            project: Some("dev-zen".into()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::new(),
        });
        fx.store.soft_delete(id).expect("soft_delete");
    }

    // Supersedes: winner over loser.
    fx.relations
        .create(RelationInput {
            sync_id: "fixture-sup-1".into(),
            source_id: winner_id,
            target_id: loser_id,
            relation: RelationKind::Supersedes,
            reason: Some("winner has fewer hops".into()),
            evidence: None,
            confidence: None,
            marked_by_actor: None,
            marked_by_kind: None,
            marked_by_model: None,
            session_id: None,
        })
        .expect("create supersedes");

    // Conflicts judged: schema_a vs schema_b.
    let conflict = fx
        .relations
        .create(RelationInput {
            sync_id: "fixture-conflict-1".into(),
            source_id: schema_a_id,
            target_id: schema_b_id,
            relation: RelationKind::ConflictsWith,
            reason: Some("both can't be canon".into()),
            evidence: None,
            confidence: None,
            marked_by_actor: None,
            marked_by_kind: None,
            marked_by_model: None,
            session_id: None,
        })
        .expect("create conflict");
    fx.relations
        .judge(
            conflict.id,
            JudgmentInput {
                status: JudgmentStatus::Judged,
                reason: Some("council picked camelCase".into()),
                evidence: None,
                confidence: None,
            },
        )
        .expect("judge");

    Populated20 {
        fx,
        winner_id,
        loser_id,
        schema_a_id,
        schema_b_id,
    }
}

/// Build a fixture with `n` synthetic observations under project `p`.
/// Content is deterministic and shares the phrase `"common keyword n=<i>"`,
/// so all of them are searchable via `"common keyword"`. Useful for
/// limit / pagination / perf testing.
pub fn populated_n(n: usize) -> FixtureSet {
    let mut fx = FixtureSet::new();
    for i in 0..n {
        fx.save(SaveInput {
            session_id: None,
            kind: ObservationType::Memory,
            title: format!("synthetic {i}"),
            content: format!("common keyword n={i} for bulk search benchmark"),
            tool_name: None,
            project: Some("p".into()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::new(),
        });
    }
    fx
}
