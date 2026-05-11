//! Export → Import roundtrip tests using a real SQLite DB on each side
//! (source machine + destination machine, both tempdirs).

use seele_core::memory::{ObservationType, Scope};
use seele_core::metadata::Metadata;
use seele_storage::{init_db, ChunkStore, ObservationStore, SaveInput};
use seele_sync::{compute_chunk_id, export_to_dir, import_from_file, ExportFilter, ImportOutcome};
use tempfile::TempDir;

fn save_one(store: &ObservationStore, title: &str, project: &str) {
    store
        .save(SaveInput {
            session_id: None,
            kind: ObservationType::Memory,
            title: title.to_string(),
            content: format!("content for {title}"),
            tool_name: None,
            project: Some(project.to_string()),
            scope: Scope::Project,
            topic_key: None,
            metadata: Metadata::new(),
        })
        .unwrap();
}

#[test]
fn export_writes_a_gzipped_file_with_chunk_id_as_name() {
    let src_td = TempDir::new().unwrap();
    let src_pool = init_db(src_td.path().join("src.db")).unwrap();
    let src_store = ObservationStore::new(src_pool);
    save_one(&src_store, "alpha", "p");
    save_one(&src_store, "beta", "p");

    let out_td = TempDir::new().unwrap();
    let report = export_to_dir(
        &src_store,
        out_td.path(),
        ExportFilter {
            project: Some("p".to_string()),
        },
    )
    .unwrap();

    assert_eq!(report.observation_count, 2);
    assert!(report.path.exists());
    assert!(
        report
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.ends_with(".json.gz"))
            .unwrap_or(false),
        "filename should end in .json.gz"
    );
    assert!(report.bytes_on_disk > 0);
    // gzip magic bytes: 1f 8b.
    let first = std::fs::read(&report.path).unwrap();
    assert_eq!(&first[..2], &[0x1f, 0x8b]);
}

#[test]
fn import_into_clean_db_creates_observations() {
    // 1. Source DB with 3 obs.
    let src_td = TempDir::new().unwrap();
    let src_pool = init_db(src_td.path().join("src.db")).unwrap();
    let src_store = ObservationStore::new(src_pool);
    for t in ["one", "two", "three"] {
        save_one(&src_store, t, "p");
    }

    // 2. Export.
    let chunks_dir = TempDir::new().unwrap();
    let report = export_to_dir(
        &src_store,
        chunks_dir.path(),
        ExportFilter {
            project: Some("p".to_string()),
        },
    )
    .unwrap();

    // 3. Destination DB (empty).
    let dst_td = TempDir::new().unwrap();
    let dst_pool = init_db(dst_td.path().join("dst.db")).unwrap();
    let dst_store = ObservationStore::new(dst_pool.clone());
    let dst_chunks = ChunkStore::new(dst_pool);

    // 4. Import.
    let r = import_from_file(&dst_store, &dst_chunks, "node-A", &report.path).unwrap();
    assert_eq!(r.outcome, ImportOutcome::Imported);
    assert_eq!(r.observation_count_saved, 3);

    // 5. Verify destination has the rows.
    let dst_obs = dst_store
        .list(seele_storage::ObservationQuery::default())
        .unwrap();
    assert_eq!(dst_obs.len(), 3);
    let titles: Vec<&str> = dst_obs.iter().map(|o| o.title.as_str()).collect();
    assert!(titles.contains(&"one"));
    assert!(titles.contains(&"two"));
    assert!(titles.contains(&"three"));
}

#[test]
fn re_importing_same_chunk_is_a_no_op_for_target() {
    let src_td = TempDir::new().unwrap();
    let src_pool = init_db(src_td.path().join("src.db")).unwrap();
    let src_store = ObservationStore::new(src_pool);
    save_one(&src_store, "only", "p");

    let chunks_dir = TempDir::new().unwrap();
    let exp = export_to_dir(
        &src_store,
        chunks_dir.path(),
        ExportFilter {
            project: Some("p".to_string()),
        },
    )
    .unwrap();

    let dst_td = TempDir::new().unwrap();
    let dst_pool = init_db(dst_td.path().join("dst.db")).unwrap();
    let dst_store = ObservationStore::new(dst_pool.clone());
    let dst_chunks = ChunkStore::new(dst_pool);

    let first = import_from_file(&dst_store, &dst_chunks, "node-A", &exp.path).unwrap();
    assert_eq!(first.outcome, ImportOutcome::Imported);
    assert_eq!(first.observation_count_saved, 1);

    let second = import_from_file(&dst_store, &dst_chunks, "node-A", &exp.path).unwrap();
    assert_eq!(second.outcome, ImportOutcome::AlreadyImported);
    assert_eq!(second.observation_count_saved, 0);
}

#[test]
fn same_chunk_imported_under_different_target_keys_is_not_skipped() {
    let src_td = TempDir::new().unwrap();
    let src_pool = init_db(src_td.path().join("src.db")).unwrap();
    let src_store = ObservationStore::new(src_pool);
    save_one(&src_store, "shared", "p");

    let chunks_dir = TempDir::new().unwrap();
    let exp = export_to_dir(
        &src_store,
        chunks_dir.path(),
        ExportFilter {
            project: Some("p".to_string()),
        },
    )
    .unwrap();

    let dst_td = TempDir::new().unwrap();
    let dst_pool = init_db(dst_td.path().join("dst.db")).unwrap();
    let dst_store = ObservationStore::new(dst_pool.clone());
    let dst_chunks = ChunkStore::new(dst_pool);

    let a = import_from_file(&dst_store, &dst_chunks, "node-A", &exp.path).unwrap();
    let b = import_from_file(&dst_store, &dst_chunks, "node-B", &exp.path).unwrap();
    assert_eq!(a.outcome, ImportOutcome::Imported);
    assert_eq!(b.outcome, ImportOutcome::Imported);
    // The observation lands twice in the destination — once per import.
    // dedup_window may merge them as duplicates; the test asserts that
    // both target_keys recorded the import, not the row count.
}

#[test]
fn export_filter_by_project_excludes_others() {
    let src_td = TempDir::new().unwrap();
    let src_pool = init_db(src_td.path().join("src.db")).unwrap();
    let src_store = ObservationStore::new(src_pool);
    save_one(&src_store, "in", "selected");
    save_one(&src_store, "out", "other");

    let chunks_dir = TempDir::new().unwrap();
    let exp = export_to_dir(
        &src_store,
        chunks_dir.path(),
        ExportFilter {
            project: Some("selected".to_string()),
        },
    )
    .unwrap();
    assert_eq!(exp.observation_count, 1);
}

#[test]
fn read_chunk_file_rejects_unknown_future_format_version() {
    use seele_sync::ChunkPayload;
    let payload = ChunkPayload {
        format_version: 999,
        seele_version: "future".to_string(),
        exported_at: chrono::Utc::now(),
        project: None,
        observations: vec![],
    };
    let td = TempDir::new().unwrap();
    let chunk_id = compute_chunk_id(&payload).unwrap();
    let path = td.path().join(format!("{chunk_id}.json.gz"));
    // Write the file manually with the future format.
    let json = serde_json::to_vec(&payload).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    let mut enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    std::io::Write::write_all(&mut enc, &json).unwrap();
    enc.finish().unwrap();

    let err = seele_sync::read_chunk_file(&path).unwrap_err();
    assert!(err.to_string().contains("unsupported format_version"));
}

#[test]
fn read_chunk_file_detects_filename_chunk_id_mismatch() {
    let src_td = TempDir::new().unwrap();
    let src_pool = init_db(src_td.path().join("src.db")).unwrap();
    let src_store = ObservationStore::new(src_pool);
    save_one(&src_store, "x", "p");

    let chunks_dir = TempDir::new().unwrap();
    let exp = export_to_dir(
        &src_store,
        chunks_dir.path(),
        ExportFilter {
            project: Some("p".to_string()),
        },
    )
    .unwrap();
    // Rename file to a different valid-looking 64-hex name.
    let fake = chunks_dir
        .path()
        .join(format!("{}.json.gz", "0".repeat(64)));
    std::fs::rename(&exp.path, &fake).unwrap();

    let err = seele_sync::read_chunk_file(&fake).unwrap_err();
    assert!(err.to_string().contains("chunk_id mismatch"));
}
