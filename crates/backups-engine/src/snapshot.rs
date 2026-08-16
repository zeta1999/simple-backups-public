use crate::exclude::ExcludeSet;
use crate::scan::scan_source;
use anyhow::Result;
use backups_core::{FileEntry, FileKind, ObjectId, SnapshotManifest};
use backups_store::Repository;
use chrono::Utc;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct SnapshotOptions {
    pub message: Option<String>,
    pub exclude: Vec<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotStats {
    pub files_total: usize,
    pub files_new: usize,
    pub files_reused: usize,
    pub bytes_stored: u64,
    pub snapshot_id: String,
}

pub fn create_snapshot(
    repo: &Repository,
    source: &Path,
    opts: &SnapshotOptions,
) -> Result<(SnapshotManifest, SnapshotStats)> {
    let excludes = ExcludeSet::new(&opts.exclude)?;
    let scanned = scan_source(source, &excludes)?;

    let parent_id = repo.latest_id()?;
    let parent = match &parent_id {
        Some(id) => Some(repo.load_snapshot(id)?),
        None => None,
    };

    let mut files = BTreeMap::new();
    let mut stats = SnapshotStats::default();
    let mut bytes_stored = 0u64;

    for ent in &scanned {
        match ent.kind {
            FileKind::Dir => {
                files.insert(
                    ent.rel.clone(),
                    FileEntry {
                        path: ent.rel.clone(),
                        kind: FileKind::Dir,
                        object: None,
                        size: None,
                        mode: ent.mode,
                        mtime: ent.mtime,
                        symlink_target: None,
                    },
                );
            }
            FileKind::Symlink => {
                files.insert(
                    ent.rel.clone(),
                    FileEntry {
                        path: ent.rel.clone(),
                        kind: FileKind::Symlink,
                        object: None,
                        size: None,
                        mode: ent.mode,
                        mtime: ent.mtime,
                        symlink_target: ent.symlink_target.clone(),
                    },
                );
            }
            FileKind::File => {
                stats.files_total += 1;
                let (object, reused) = resolve_file_object(repo, parent.as_ref(), ent, opts)?;
                if reused {
                    stats.files_reused += 1;
                } else {
                    stats.files_new += 1;
                    if !opts.dry_run {
                        bytes_stored += ent.size.unwrap_or(0);
                    }
                }
                files.insert(
                    ent.rel.clone(),
                    FileEntry {
                        path: ent.rel.clone(),
                        kind: FileKind::File,
                        object: Some(object),
                        size: ent.size,
                        mode: ent.mode,
                        mtime: ent.mtime,
                        symlink_target: None,
                    },
                );
            }
        }
    }

    let id = format!("{}", Utc::now().format("%Y%m%d-%H%M%S"));
    let id = unique_id(repo, &id)?;
    stats.snapshot_id = id.clone();
    stats.bytes_stored = bytes_stored;

    let manifest = SnapshotManifest::new(
        id,
        source.display().to_string(),
        opts.message.clone(),
        parent_id,
        files,
    )?;

    if !opts.dry_run {
        repo.write_snapshot(&manifest)?;
    }

    Ok((manifest, stats))
}

fn resolve_file_object(
    repo: &Repository,
    parent: Option<&SnapshotManifest>,
    ent: &crate::scan::ScanEntry,
    opts: &SnapshotOptions,
) -> Result<(ObjectId, bool)> {
    // Always content-hash for correctness. mtime/size shortcuts are a future
    // opt-in fast path (see SPECS.md).
    let id = backups_core::hash_file(&ent.abs)?;
    if let Some(prev) = parent.and_then(|p| p.files.get(&ent.rel)) {
        if prev.object.as_ref() == Some(&id) && repo.has_object(&id) {
            return Ok((id, true));
        }
    }
    if !opts.dry_run && !repo.has_object(&id) {
        let stored = repo.put_file(&ent.abs)?;
        debug_assert_eq!(stored, id);
    }
    Ok((id, false))
}

fn unique_id(repo: &Repository, base: &str) -> Result<String> {
    let mut candidate = base.to_string();
    let mut n = 0u32;
    loop {
        let path = repo
            .root()
            .join("snapshots")
            .join(format!("{candidate}.json"));
        if !path.exists() {
            return Ok(candidate);
        }
        n += 1;
        candidate = format!("{base}-{n}");
    }
}
