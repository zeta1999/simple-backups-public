use anyhow::Result;
use backups_core::ObjectId;
use backups_store::Repository;
use std::collections::BTreeSet;
use std::fs;

#[derive(Debug, Default)]
pub struct GcReport {
    pub objects_seen: usize,
    pub objects_kept: usize,
    pub objects_deleted: usize,
    pub bytes_freed: u64,
}

/// Delete objects not referenced by any snapshot manifest.
pub fn gc_repo(repo: &Repository, dry_run: bool) -> Result<GcReport> {
    let mut live: BTreeSet<String> = BTreeSet::new();
    for id in repo.list_snapshots()? {
        let m = repo.load_snapshot(&id)?;
        for oid in Repository::objects_in_manifest(&m) {
            live.insert(oid.to_string());
        }
    }

    let mut report = GcReport {
        objects_kept: live.len(),
        ..Default::default()
    };

    let objects_root = repo.root().join("objects");
    if !objects_root.exists() {
        return Ok(report);
    }

    for shard in fs::read_dir(&objects_root)? {
        let shard = shard?;
        if !shard.file_type()?.is_dir() {
            continue;
        }
        let prefix = shard.file_name().to_string_lossy().into_owned();
        for ent in fs::read_dir(shard.path())? {
            let ent = ent?;
            if !ent.file_type()?.is_file() {
                continue;
            }
            let rest = ent.file_name().to_string_lossy().into_owned();
            let hex = format!("{prefix}{rest}");
            report.objects_seen += 1;
            if live.contains(&hex) {
                continue;
            }
            // Validate it looks like an object id before deleting.
            if ObjectId::from_hex(&hex).is_err() {
                continue;
            }
            let len = ent.metadata()?.len();
            if dry_run {
                println!("would delete object {hex} ({len} bytes)");
            } else {
                fs::remove_file(ent.path())?;
            }
            report.objects_deleted += 1;
            report.bytes_freed += len;
        }
    }
    Ok(report)
}
