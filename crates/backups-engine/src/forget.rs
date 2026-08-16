use anyhow::{bail, Context, Result};
use backups_core::validate_snapshot_id;
use backups_store::Repository;
use std::fs;

#[derive(Debug, Default)]
pub struct ForgetReport {
    pub forgotten: Vec<String>,
    pub latest: Option<String>,
}

/// Remove snapshot manifests by id. Does not delete objects (run `gc` after).
pub fn forget_snapshots(repo: &Repository, ids: &[String]) -> Result<ForgetReport> {
    if ids.is_empty() {
        bail!("no snapshot ids given");
    }
    let mut report = ForgetReport::default();
    let current_latest = repo.latest_id()?;

    for id in ids {
        if id == "latest" {
            bail!("refusing to forget symbolic id 'latest'; pass a concrete snapshot id");
        }
        validate_snapshot_id(id)?;
        let path = repo.root().join("snapshots").join(format!("{id}.json"));
        if !path.exists() {
            bail!("snapshot not found: {id}");
        }
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        report.forgotten.push(id.clone());
    }

    // Fix refs/latest if we deleted it.
    if current_latest
        .as_ref()
        .is_some_and(|l| report.forgotten.iter().any(|f| f == l))
    {
        let remaining = repo.list_snapshots()?;
        let latest_path = repo.root().join("refs").join("latest");
        if let Some(new_latest) = remaining.last() {
            fs::write(&latest_path, format!("{new_latest}\n"))?;
            report.latest = Some(new_latest.clone());
        } else if latest_path.exists() {
            fs::remove_file(&latest_path)?;
            report.latest = None;
        }
    } else {
        report.latest = current_latest;
    }
    Ok(report)
}

/// Keep only the newest `keep_last` snapshots (by id sort = chronological for our id scheme).
pub fn prune_keep_last(repo: &Repository, keep_last: usize) -> Result<ForgetReport> {
    if keep_last == 0 {
        bail!("keep_last must be >= 1");
    }
    let mut ids = repo.list_snapshots()?;
    if ids.len() <= keep_last {
        return Ok(ForgetReport {
            latest: repo.latest_id()?,
            ..Default::default()
        });
    }
    let drop_count = ids.len() - keep_last;
    let to_drop: Vec<String> = ids.drain(..drop_count).collect();
    forget_snapshots(repo, &to_drop)
}
