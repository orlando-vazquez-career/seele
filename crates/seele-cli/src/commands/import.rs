//! `seele import` — one-shot migration from other memory engines.
//!
//! Sub-block D.3 wires `from-engram` to `seele-engram-import` per
//! ADR-13. Re-running over the same source is idempotent (preserved
//! ULIDs short-circuit; cuid-style ids are kept distinct via the
//! `engram_id` breadcrumb).

use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};
use seele_engram_import::EngramImporter;
use seele_http::dto::SaveRequest;
use seele_http::SeeleService;
use seele_storage::{init_db, LinkStore, ObservationStore};
use serde_json::Value;

use crate::app::OutputOpts;
use crate::output;

#[derive(Subcommand, Debug)]
pub enum ImportCmd {
    /// Migrate from an ENGRAM SQLite DB (ADR-13).
    FromEngram(FromEngramArgs),
    /// Batch-import observations from a JSONL file, one per line (T-14).
    FromJsonl(FromJsonlArgs),
}

#[derive(ClapArgs, Debug)]
pub struct FromEngramArgs {
    /// Path to the ENGRAM DB (typically `~/.mnema/mnema.db`).
    pub path: PathBuf,
    /// Re-compute embeddings with the SEELE embedder after importing
    /// (T-08). Imported rows land without vectors (raw insert), so this
    /// runs the same pass as `seele embedder reembed-all` over the
    /// destination DB once the import commits. Skipped on `--dry-run`.
    #[arg(long)]
    pub re_embed: bool,
    /// Show what would be imported without writing.
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(
    cmd: ImportCmd,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    match cmd {
        ImportCmd::FromEngram(args) => run_from_engram(args, db, fake_embedder, out).await,
        ImportCmd::FromJsonl(args) => run_from_jsonl(args, db, fake_embedder, out),
    }
}

#[derive(ClapArgs, Debug)]
pub struct FromJsonlArgs {
    /// Path to the JSONL file: one observation per line. Schema:
    /// `title` + `content` (required); `type`, `project`, `scope`,
    /// `topic_key`, `metadata` (optional). Invalid lines are skipped
    /// and reported at the end — they never abort the batch.
    pub path: PathBuf,
    /// Default project for lines without a `project` field.
    #[arg(long)]
    pub project: Option<String>,
}

/// One JSONL line: the `seele save` schema with `title` + `content`
/// mandatory and everything else optional (`type` defaults to
/// `memory`, like the save DTO).
#[derive(serde::Deserialize)]
struct JsonlObservation {
    title: String,
    content: String,
    #[serde(default = "default_jsonl_type", rename = "type")]
    r#type: String,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    topic_key: Option<String>,
    #[serde(default)]
    metadata: Value,
}

fn default_jsonl_type() -> String {
    "memory".to_string()
}

/// Max detailed per-line errors kept in the report (T-14). `skipped`
/// always counts every invalid line; `errors` holds the first few so a
/// pathological file cannot flood the output.
const MAX_DETAILED_ERRORS: usize = 10;

/// Final `import from-jsonl` report.
#[derive(serde::Serialize)]
struct JsonlImportReport {
    /// Lines whose save succeeded (created, topic-key upsert, or
    /// duplicate-merge — all three mean the observation landed).
    imported: usize,
    /// Lines rejected (bad JSON, missing/empty required fields, or a
    /// failed save). Blank lines are ignored, not skipped.
    skipped: usize,
    /// First [`MAX_DETAILED_ERRORS`] rejections, 1-based line numbers.
    errors: Vec<JsonlLineError>,
}

#[derive(serde::Serialize)]
struct JsonlLineError {
    line: usize,
    reason: String,
}

fn run_from_jsonl(
    args: FromJsonlArgs,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(&args.path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", args.path.display()))?;
    let svc = crate::app::build_service(db, fake_embedder)?;
    let report = import_jsonl_str(&svc, &raw, args.project.as_deref());
    output::emit_split(
        &report,
        || {
            let mut lines = vec![
                format!("imported: {}", report.imported),
                format!("skipped: {}", report.skipped),
            ];
            if !report.errors.is_empty() {
                lines.push(format!(
                    "errors (first {} of {}):",
                    report.errors.len(),
                    report.skipped
                ));
                for e in &report.errors {
                    lines.push(format!("  line {}: {}", e.line, e.reason));
                }
            }
            lines.join("\n")
        },
        out.json,
    )
}

/// Import every line of `raw` through the normal service save (so
/// topic-key upsert and hash dedup apply unchanged). Never fails on a
/// bad line — rejects are counted and detailed in the report.
fn import_jsonl_str(
    svc: &SeeleService,
    raw: &str,
    default_project: Option<&str>,
) -> JsonlImportReport {
    let mut report = JsonlImportReport {
        imported: 0,
        skipped: 0,
        errors: Vec::new(),
    };
    for (idx, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            // Blank lines (a trailing newline is ubiquitous) are not data.
            continue;
        }
        match import_jsonl_line(svc, line, default_project) {
            Ok(()) => report.imported += 1,
            Err(reason) => {
                report.skipped += 1;
                if report.errors.len() < MAX_DETAILED_ERRORS {
                    report.errors.push(JsonlLineError {
                        line: idx + 1,
                        reason,
                    });
                }
            }
        }
    }
    report
}

/// Parse and save a single JSONL line. The line-level `project` wins;
/// `--project` is only the fallback. Errors come back as the report's
/// human-readable `reason`.
fn import_jsonl_line(
    svc: &SeeleService,
    line: &str,
    default_project: Option<&str>,
) -> Result<(), String> {
    let obs: JsonlObservation =
        serde_json::from_str(line).map_err(|e| format!("invalid JSON: {e}"))?;
    if obs.title.trim().is_empty() {
        return Err("empty title".to_string());
    }
    if obs.content.trim().is_empty() {
        return Err("empty content".to_string());
    }
    let req = SaveRequest {
        title: obs.title,
        content: obs.content,
        r#type: obs.r#type,
        project: obs.project.or_else(|| default_project.map(str::to_string)),
        scope: obs.scope,
        topic_key: obs.topic_key,
        session_id: None,
        tool_name: Some("seele-cli/import-jsonl".to_string()),
        metadata: obs.metadata,
    };
    svc.save_observation(req)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Batch size for the post-import re-embed pass (same default as
/// `seele embedder reembed-all`).
const REEMBED_BATCH_SIZE: usize = 64;

/// Import report plus the outcome of the optional `--re-embed` pass.
/// Serde-flattened so JSON consumers see the same fields as before,
/// with `reembed` added when the pass ran.
#[derive(serde::Serialize)]
struct ImportOutcome {
    #[serde(flatten)]
    report: seele_engram_import::ImportReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    reembed: Option<seele_http::service::ReembedReport>,
}

async fn run_from_engram(
    args: FromEngramArgs,
    db: &Option<PathBuf>,
    fake_embedder: bool,
    out: &OutputOpts,
) -> anyhow::Result<()> {
    let path = db.clone().unwrap_or_else(crate::default_db_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pool = init_db(&path)?;
    let observations = ObservationStore::new(pool.clone());
    let links = LinkStore::new(pool);

    let importer = EngramImporter::new(&observations, &links);
    let report = importer.import_from(&args.path, args.dry_run)?;

    // T-08: `--re-embed` is real. Imported rows carry no vectors (raw
    // insert skips the embedder), so right after the import commits we
    // run the same remediation pass as `seele embedder reembed-all`:
    // it re-embeds exactly the rows that are missing a vector (plus any
    // pre-existing stale ones). On a dry-run import nothing was
    // written, so there is nothing to re-embed.
    let reembed = if args.re_embed && !report.dry_run {
        let svc = crate::app::build_service(db, fake_embedder)?;
        Some(svc.reembed_all(None, REEMBED_BATCH_SIZE, false)?)
    } else {
        None
    };

    let outcome = ImportOutcome { report, reembed };
    output::emit_split(
        &outcome,
        || {
            let report = &outcome.report;
            let mut lines = Vec::new();
            lines.push(format!(
                "source: {} ({})",
                report.source_path.display(),
                if report.dry_run { "dry-run" } else { "applied" }
            ));
            lines.push(format!("  rows seen:            {}", report.rows_seen));
            lines.push(format!("  rows inserted:        {}", report.rows_inserted));
            lines.push(format!(
                "  rows skipped (exist): {}",
                report.rows_skipped_existing
            ));
            lines.push(format!("  rows invalid:         {}", report.rows_invalid));
            lines.push(format!("  links created:        {}", report.links_created));
            lines.push(format!("  links dangling:       {}", report.links_dangling));
            if args.re_embed {
                match &outcome.reembed {
                    Some(r) => {
                        lines.push(format!("  re-embed candidates:  {}", r.candidates));
                        lines.push(format!("  re-embedded:          {}", r.reembedded));
                        lines.push(format!("  re-embed skipped:     {}", r.skipped));
                    }
                    None => lines.push("  re-embed:             skipped (dry-run)".to_string()),
                }
            }
            if !report.errors.is_empty() {
                lines.push(String::new());
                lines.push(format!("errors ({}):", report.errors.len()));
                for e in &report.errors {
                    lines.push(format!("  {e}"));
                }
            }
            lines.join("\n")
        },
        out.json,
    )
}

#[cfg(test)]
mod tests {
    //! T-14 — `import from-jsonl`: mixed valid/invalid fixtures, the
    //! `--project` fallback, topic-key idempotency, and the detailed-
    //! error cap. Each test builds the real service over a tempdir DB
    //! with the FakeEmbedder (no ONNX download) and inspects the DB
    //! through `seele_storage::init_db` connections.

    use super::*;
    use std::io::Write as _;
    use std::path::Path;

    fn write_fixture(dir: &tempfile::TempDir, lines: &[&str]) -> PathBuf {
        let path = dir.path().join("fixture.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
        path
    }

    fn build(db: &Path) -> SeeleService {
        crate::app::build_service(&Some(db.to_path_buf()), true).unwrap()
    }

    fn with_db<R>(db: &Path, f: impl FnOnce(&rusqlite::Connection) -> R) -> R {
        let pool = init_db(db).unwrap();
        let conn = pool.get().unwrap();
        f(&conn)
    }

    fn active_count(db: &Path) -> i64 {
        with_db(db, |c| {
            c.query_row(
                "SELECT COUNT(*) FROM observations WHERE deleted_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap()
        })
    }

    #[test]
    fn jsonl_mixed_valid_and_invalid_lines() {
        let td = tempfile::TempDir::new().unwrap();
        let db = td.path().join("s.db");
        let fixture = write_fixture(
            &td,
            &[
                r#"{"title":"t1","content":"c1","type":"decision","topic_key":"decision/x","project":"alpha","metadata":{"source":"jsonl"}}"#,
                r#"{"title":"t2","content":"c2"}"#, // project from --project
                r#"{not json"#,                     // invalid JSON
                r#"{"title":"t3"}"#,                // missing content
                r#"{"title":"   ","content":"c4"}"#, // empty title
                "",                                 // blank: ignored, not skipped
                r#"{"title":"t5","content":"c5","project":"beta"}"#, // beats --project
            ],
        );
        let svc = build(&db);
        let raw = std::fs::read_to_string(&fixture).unwrap();
        let report = import_jsonl_str(&svc, &raw, Some("fallback-proj"));

        assert_eq!(report.imported, 3);
        assert_eq!(report.skipped, 3, "blank line must not count");
        assert_eq!(report.errors.len(), 3);
        assert_eq!(report.errors[0].line, 3);
        assert!(report.errors[0].reason.contains("invalid JSON"));
        assert_eq!(report.errors[1].line, 4);
        assert!(report.errors[1].reason.contains("content"));
        assert_eq!(report.errors[2].line, 5);
        assert!(report.errors[2].reason.contains("title"));

        assert_eq!(active_count(&db), 3);
        // Per-line project wins over the flag; the flag is the fallback;
        // optional fields (type/metadata) pass through the normal save.
        with_db(&db, |c| {
            let (project, ty, meta): (String, String, String) = c
                .query_row(
                    "SELECT project, type, metadata FROM observations WHERE title = 't1'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            assert_eq!(project, "alpha");
            assert_eq!(ty, "decision");
            assert!(meta.contains("jsonl"), "metadata stored: {meta}");

            let fallback: String = c
                .query_row(
                    "SELECT project FROM observations WHERE title = 't2'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(fallback, "fallback-proj");

            let own: String = c
                .query_row(
                    "SELECT project FROM observations WHERE title = 't5'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(own, "beta");
        });
    }

    #[test]
    fn jsonl_reimport_upserts_by_topic_key() {
        let td = tempfile::TempDir::new().unwrap();
        let db = td.path().join("s.db");
        let fixture = write_fixture(
            &td,
            &[
                r#"{"title":"v1","content":"body one","topic_key":"learning/rust","project":"p"}"#,
                r#"{"title":"no-key","content":"body two","project":"p"}"#,
            ],
        );
        let svc = build(&db);
        let raw = std::fs::read_to_string(&fixture).unwrap();

        let first = import_jsonl_str(&svc, &raw, None);
        assert_eq!((first.imported, first.skipped), (2, 0));
        assert_eq!(active_count(&db), 2);

        // Re-importing the same file must not create new rows: the
        // topic-key line upserts (revision_count bumps), the keyless
        // line lands in the hash-dedup window (duplicate merge).
        let second = import_jsonl_str(&svc, &raw, None);
        assert_eq!((second.imported, second.skipped), (2, 0));
        assert_eq!(active_count(&db), 2, "re-import created duplicates");

        with_db(&db, |c| {
            let rev: u32 = c
                .query_row(
                    "SELECT revision_count FROM observations \
                     WHERE topic_key = 'learning/rust'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            // Insert starts at 0; exactly one upsert happened (the
            // second import), so the counter sits at 1.
            assert_eq!(rev, 1, "topic-key row upserted by the re-import");
            let dup: u32 = c
                .query_row(
                    "SELECT duplicate_count FROM observations WHERE title = 'no-key'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(dup, 1, "keyless line merged as duplicate");
        });
    }

    #[test]
    fn jsonl_error_details_capped_at_ten() {
        let td = tempfile::TempDir::new().unwrap();
        let db = td.path().join("s.db");
        let owned: Vec<String> = (0..12).map(|_| "{bad".to_string()).collect();
        let lines: Vec<&str> = owned.iter().map(String::as_str).collect();
        let fixture = write_fixture(&td, &lines);
        let svc = build(&db);
        let raw = std::fs::read_to_string(&fixture).unwrap();

        let report = import_jsonl_str(&svc, &raw, None);
        assert_eq!(report.imported, 0);
        assert_eq!(report.skipped, 12, "every bad line counted");
        assert_eq!(report.errors.len(), MAX_DETAILED_ERRORS);
        assert_eq!(report.errors[0].line, 1);
        assert_eq!(report.errors[9].line, 10);
        assert_eq!(active_count(&db), 0);
    }

    #[test]
    fn jsonl_missing_file_is_a_hard_error() {
        let td = tempfile::TempDir::new().unwrap();
        let db = td.path().join("s.db");
        let out = OutputOpts { json: false };
        let args = FromJsonlArgs {
            path: td.path().join("nope.jsonl"),
            project: None,
        };
        let err = run_from_jsonl(args, &Some(db), true, &out).unwrap_err();
        assert!(err.to_string().contains("cannot read"), "err: {err}");
    }
}
