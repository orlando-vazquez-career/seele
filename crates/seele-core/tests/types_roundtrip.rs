//! Workspace-level integration tests for `seele-core` types.
//!
//! Verifies serde round-trips and cross-type interactions.

use chrono::Utc;
use proptest::prelude::*;
use seele_core::{
    JudgmentStatus, Link, Metadata, Observation, ObservationType, RelationKind, Scope, SeeleId,
    Session, SessionStatus,
};
use serde_json::Value;

#[test]
fn observation_full_serde_roundtrip() {
    let mut metadata = Metadata::new();
    metadata.set("kind", Value::String("decision".into()));
    metadata.set("axiomatic", Value::Bool(true));
    metadata.set("score", serde_json::json!(1.0));

    let now = Utc::now();
    let obs = Observation {
        id: SeeleId::new(),
        session_id: Some(SeeleId::new()),
        kind: ObservationType::Decision,
        title: "Decisión de stack backend".into(),
        content: "FastAPI async + Postgres + Alembic + Pydantic v2.".into(),
        tool_name: Some("seele_save".into()),
        project: Some("dev-zen".into()),
        scope: Scope::Project,
        topic_key: Some("decision/backend-stack".into()),
        normalized_hash: Some("abc123".into()),
        revision_count: 0,
        duplicate_count: 0,
        last_seen_at: now,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        metadata,
    };

    let json = serde_json::to_string(&obs).unwrap();
    let back: Observation = serde_json::from_str(&json).unwrap();

    assert_eq!(back.id, obs.id);
    assert_eq!(back.title, obs.title);
    assert_eq!(back.scope, Scope::Project);
}

#[test]
fn session_serde_roundtrip() {
    let s = Session {
        id: SeeleId::new(),
        project: "seele".into(),
        directory: Some("/home/u/seele".into()),
        started_at: Utc::now(),
        ended_at: None,
        summary: None,
        status: SessionStatus::Active,
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Session = serde_json::from_str(&json).unwrap();
    assert_eq!(back.status, SessionStatus::Active);
    assert_eq!(back.project, "seele");
}

#[test]
fn link_serde_roundtrip() {
    let l = Link {
        id: SeeleId::new(),
        from_id: SeeleId::new(),
        to_id: SeeleId::new(),
        link_type: seele_core::link_types::DERIVES_FROM.into(),
        metadata: Metadata::new(),
        created_at: Utc::now(),
    };
    let json = serde_json::to_string(&l).unwrap();
    let back: Link = serde_json::from_str(&json).unwrap();
    assert_eq!(back.link_type, "derives_from");
    assert_eq!(back.from_id, l.from_id);
}

#[test]
fn relation_kind_all_serializable() {
    for kind in [
        RelationKind::Supersedes,
        RelationKind::ConflictsWith,
        RelationKind::Scoped,
        RelationKind::Related,
        RelationKind::Compatible,
        RelationKind::NotConflict,
    ] {
        let s = serde_json::to_string(&kind).unwrap();
        let back: RelationKind = serde_json::from_str(&s).unwrap();
        assert_eq!(back, kind);
    }
}

#[test]
fn judgment_status_all_serializable() {
    for status in [
        JudgmentStatus::Pending,
        JudgmentStatus::Judged,
        JudgmentStatus::Orphaned,
        JudgmentStatus::Ignored,
    ] {
        let s = serde_json::to_string(&status).unwrap();
        let back: JudgmentStatus = serde_json::from_str(&s).unwrap();
        assert_eq!(back, status);
    }
}

proptest! {
    #[test]
    fn ulid_string_roundtrip_property(_seed: u64) {
        let id = SeeleId::new();
        let s = id.to_string();
        let parsed: SeeleId = s.parse().unwrap();
        prop_assert_eq!(id, parsed);
    }

    #[test]
    fn ulid_to_i64_is_non_negative(_seed: u64) {
        let id = SeeleId::new();
        prop_assert!(id.as_i64() >= 0);
    }

    #[test]
    fn observation_type_relaxed_known_strings(s in "(decision|architecture|bugfix|pattern|config|discovery|learning|memory|skill|advisor_output|review|verdict)") {
        let parsed = ObservationType::from_str_relaxed(&s);
        prop_assert_eq!(parsed.as_str(), s);
    }

    #[test]
    fn observation_type_relaxed_other_passthrough(s in "[a-z][a-z0-9_]{1,30}") {
        let known = matches!(s.as_str(),
            "decision" | "architecture" | "bugfix" | "pattern" | "config" |
            "discovery" | "learning" | "memory" | "skill" | "advisor_output" |
            "review" | "verdict"
        );
        prop_assume!(!known);
        let parsed = ObservationType::from_str_relaxed(&s);
        match parsed {
            ObservationType::Other(stored) => prop_assert_eq!(stored, s),
            _ => prop_assert!(false, "expected Other variant"),
        }
    }
}
