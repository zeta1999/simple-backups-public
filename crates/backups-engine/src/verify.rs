use anyhow::Result;
use backups_core::{hash_file, FileKind, SnapshotManifest};
use backups_store::Repository;

#[derive(Debug, Default)]
pub struct VerifyReport {
    pub snapshots_checked: usize,
    pub objects_checked: usize,
    pub errors: Vec<String>,
}

pub fn verify_snapshot(repo: &Repository, manifest: &SnapshotManifest) -> Result<VerifyReport> {
    let mut report = VerifyReport {
        snapshots_checked: 1,
        ..Default::default()
    };
    if let Err(e) = manifest.verify_content_hash() {
        report.errors.push(format!("{}: {e}", manifest.id));
        return Ok(report);
    }
    for (rel, entry) in &manifest.files {
        if entry.kind != FileKind::File {
            continue;
        }
        let Some(oid) = &entry.object else {
            report
                .errors
                .push(format!("{}: {rel}: missing object id", manifest.id));
            continue;
        };
        if !repo.has_object(oid) {
            report
                .errors
                .push(format!("{}: {rel}: missing object {oid}", manifest.id));
            continue;
        }
        let path = oid.object_path(repo.root());
        match hash_file(&path) {
            Ok(got) if &got == oid => report.objects_checked += 1,
            Ok(got) => report.errors.push(format!(
                "{}: {rel}: object hash mismatch (stored as {oid}, hashed {got})",
                manifest.id
            )),
            Err(e) => report
                .errors
                .push(format!("{}: {rel}: read object {oid}: {e}", manifest.id)),
        }
    }
    Ok(report)
}

pub fn verify_repo(repo: &Repository, only: Option<&str>) -> Result<VerifyReport> {
    let mut total = VerifyReport::default();
    let ids = if let Some(id) = only {
        vec![if id == "latest" {
            repo.latest_id()?
                .ok_or_else(|| anyhow::anyhow!("no snapshots"))?
        } else {
            id.to_string()
        }]
    } else {
        repo.list_snapshots()?
    };
    for id in ids {
        let m = repo.load_snapshot(&id)?;
        let r = verify_snapshot(repo, &m)?;
        total.snapshots_checked += r.snapshots_checked;
        total.objects_checked += r.objects_checked;
        total.errors.extend(r.errors);
    }
    Ok(total)
}
