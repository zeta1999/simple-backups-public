use anyhow::{bail, Context, Result};
use backups_core::{path_is_safe, symlink_target_is_safe, FileKind, SnapshotManifest};
use backups_store::Repository;
use std::fs::{self, File};
use std::io::copy;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct RestoreOptions {
    /// Restore only this relative path (and children if dir).
    pub path_filter: Option<String>,
    pub dry_run: bool,
}

pub fn restore_snapshot(
    repo: &Repository,
    manifest: &SnapshotManifest,
    target: &Path,
    opts: &RestoreOptions,
) -> Result<usize> {
    if !opts.dry_run {
        fs::create_dir_all(target)?;
    }
    let mut restored = 0usize;

    for (rel, entry) in &manifest.files {
        if let Some(filter) = &opts.path_filter {
            if rel != filter && !rel.starts_with(&format!("{filter}/")) {
                continue;
            }
        }
        if !path_is_safe(rel) {
            bail!("refusing unsafe path in manifest: {rel}");
        }
        let dest = join_rel(target, rel)?;

        match entry.kind {
            FileKind::Dir => {
                if opts.dry_run {
                    println!("would mkdir {}", dest.display());
                } else {
                    ensure_real_parent(target, &dest)?;
                    fs::create_dir_all(&dest)?;
                    apply_mode(&dest, entry.mode)?;
                }
            }
            FileKind::Symlink => {
                let link_target = entry
                    .symlink_target
                    .as_deref()
                    .context("symlink missing target")?;
                if !symlink_target_is_safe(link_target) {
                    bail!("refusing unsafe symlink target in manifest: {rel} -> {link_target}");
                }
                if opts.dry_run {
                    println!("would symlink {} -> {link_target}", dest.display());
                } else {
                    ensure_real_parent(target, &dest)?;
                    if dest.symlink_metadata().is_ok() {
                        let _ = fs::remove_file(&dest);
                    }
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(link_target, &dest)?;
                    #[cfg(not(unix))]
                    {
                        // Windows: best-effort file symlink / copy target text.
                        std::os::windows::fs::symlink_file(link_target, &dest)
                            .or_else(|_| fs::write(&dest, link_target.as_bytes()))?;
                    }
                }
                restored += 1;
            }
            FileKind::File => {
                let oid = entry.object.as_ref().context("file missing object id")?;
                if opts.dry_run {
                    println!("would restore {} ({oid})", dest.display());
                } else {
                    ensure_real_parent(target, &dest)?;
                    if dest
                        .symlink_metadata()
                        .ok()
                        .is_some_and(|m| m.file_type().is_symlink())
                    {
                        fs::remove_file(&dest)
                            .with_context(|| format!("remove symlink {}", dest.display()))?;
                    }
                    let mut src = repo.open_object(oid)?;
                    let mut out = File::create(&dest)
                        .with_context(|| format!("create {}", dest.display()))?;
                    copy(&mut src, &mut out)?;
                    out.sync_all()?;
                    apply_mode(&dest, entry.mode)?;
                }
                restored += 1;
            }
        }
    }
    Ok(restored)
}

fn join_rel(root: &Path, rel: &str) -> Result<PathBuf> {
    let mut out = root.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            bail!("path escape");
        }
        out.push(part);
    }
    Ok(out)
}

/// Create parent dirs without following a symlink that escaped `root`.
fn ensure_real_parent(root: &Path, dest: &Path) -> Result<()> {
    let parent = dest.parent().unwrap_or(root);
    fs::create_dir_all(parent)?;
    let root_canon = root
        .canonicalize()
        .with_context(|| format!("canonicalize {}", root.display()))?;
    let parent_canon = parent
        .canonicalize()
        .with_context(|| format!("canonicalize {}", parent.display()))?;
    if !parent_canon.starts_with(&root_canon) {
        bail!(
            "restore path escaped target: {} (via {})",
            dest.display(),
            parent.display()
        );
    }
    Ok(())
}

fn apply_mode(path: &Path, mode: Option<u32>) -> Result<()> {
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(mode & 0o777);
        fs::set_permissions(path, perms)?;
    }
    let _ = (path, mode);
    Ok(())
}
