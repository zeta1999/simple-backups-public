use backups_engine::{
    create_snapshot, forget_snapshots, gc_repo, prune_keep_last, restore_snapshot, verify_repo,
    RestoreOptions, SnapshotOptions,
};
use backups_store::Repository;
use std::fs;
use std::io::Write;

#[test]
fn incremental_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    let repo_path = tmp.path().join("repo");
    let restore = tmp.path().join("restore");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.txt"), b"hello").unwrap();
    fs::write(src.join("b.txt"), b"world").unwrap();

    let repo = Repository::init(&repo_path, None).unwrap();
    let (_m1, s1) = create_snapshot(
        &repo,
        &src,
        &SnapshotOptions {
            message: Some("first".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s1.files_total, 2);
    assert_eq!(s1.files_new, 2);

    // Change one file.
    let mut f = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(src.join("a.txt"))
        .unwrap();
    f.write_all(b"HELLO").unwrap();
    drop(f);
    // Touch mtime differently by rewriting; also ensure size change.

    let (_m2, s2) = create_snapshot(
        &repo,
        &src,
        &SnapshotOptions {
            message: Some("second".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s2.files_total, 2);
    // b.txt should reuse; a.txt new.
    assert!(s2.files_reused >= 1, "expected reused files, got {s2:?}");
    assert!(s2.files_new >= 1);

    let latest = repo.load_snapshot("latest").unwrap();
    restore_snapshot(&repo, &latest, &restore, &RestoreOptions::default()).unwrap();
    assert_eq!(fs::read(restore.join("a.txt")).unwrap(), b"HELLO");
    assert_eq!(fs::read(restore.join("b.txt")).unwrap(), b"world");

    let report = verify_repo(&repo, None).unwrap();
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(report.snapshots_checked >= 2);

    // Orphan object should be collected.
    let orphan = repo.put_bytes(b"orphan-data").unwrap();
    assert!(repo.has_object(&orphan));
    let gc = gc_repo(&repo, false).unwrap();
    assert!(gc.objects_deleted >= 1);
    assert!(!repo.has_object(&orphan));

    // Third snapshot, then prune to 1.
    fs::write(src.join("c.txt"), b"c").unwrap();
    create_snapshot(
        &repo,
        &src,
        &SnapshotOptions {
            message: Some("third".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(repo.list_snapshots().unwrap().len() >= 3);
    let pruned = prune_keep_last(&repo, 1).unwrap();
    assert!(!pruned.forgotten.is_empty());
    assert_eq!(repo.list_snapshots().unwrap().len(), 1);
    // Forgetting the last remaining id should clear latest.
    let last = repo.latest_id().unwrap().unwrap();
    forget_snapshots(&repo, &[last]).unwrap();
    assert!(repo.list_snapshots().unwrap().is_empty());
    assert!(repo.latest_id().unwrap().is_none());
}

#[test]
fn restore_rejects_absolute_symlink_escape() {
    use backups_core::{FileEntry, FileKind, SnapshotManifest};
    use std::collections::BTreeMap;

    let tmp = tempfile::tempdir().unwrap();
    let repo = Repository::init(&tmp.path().join("repo"), None).unwrap();
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let restore = tmp.path().join("restore");

    let mut files = BTreeMap::new();
    files.insert(
        "evil".into(),
        FileEntry {
            path: "evil".into(),
            kind: FileKind::Symlink,
            object: None,
            size: None,
            mode: None,
            mtime: None,
            symlink_target: Some(outside.to_string_lossy().into_owned()),
        },
    );
    let manifest =
        SnapshotManifest::new("20260816-000000".into(), "/src".into(), None, None, files).unwrap();
    let err = restore_snapshot(&repo, &manifest, &restore, &RestoreOptions::default()).unwrap_err();
    assert!(
        err.to_string().contains("unsafe symlink"),
        "unexpected error: {err}"
    );
}
