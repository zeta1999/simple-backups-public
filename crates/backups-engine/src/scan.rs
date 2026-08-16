use crate::exclude::ExcludeSet;
use anyhow::{Context, Result};
use backups_core::{normalize_rel_path, FileKind};
use chrono::{TimeZone, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ScanEntry {
    pub rel: String,
    pub abs: PathBuf,
    pub kind: FileKind,
    pub size: Option<u64>,
    pub mode: Option<u32>,
    pub mtime: Option<chrono::DateTime<Utc>>,
    pub symlink_target: Option<String>,
}

pub fn scan_source(source: &Path, excludes: &ExcludeSet) -> Result<Vec<ScanEntry>> {
    let source = fs::canonicalize(source)
        .with_context(|| format!("canonicalize source {}", source.display()))?;
    let mut out = Vec::new();

    for ent in WalkDir::new(&source).follow_links(false).into_iter() {
        let ent = ent.with_context(|| format!("walk {}", source.display()))?;
        let abs = ent.path().to_path_buf();
        if abs == source {
            continue;
        }
        let rel_path = abs
            .strip_prefix(&source)
            .with_context(|| format!("strip prefix {}", abs.display()))?;
        let rel = normalize_rel_path(rel_path)?;
        if excludes.matches(&rel) {
            continue;
        }

        let meta =
            fs::symlink_metadata(&abs).with_context(|| format!("metadata {}", abs.display()))?;
        let file_type = meta.file_type();
        let mtime = meta.modified().ok().and_then(|t| {
            let secs = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
            Utc.timestamp_opt(secs, 0).single()
        });

        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            Some(meta.permissions().mode())
        };
        #[cfg(not(unix))]
        let mode = None;

        if file_type.is_symlink() {
            let target = fs::read_link(&abs)?.to_string_lossy().into_owned();
            out.push(ScanEntry {
                rel,
                abs,
                kind: FileKind::Symlink,
                size: None,
                mode,
                mtime,
                symlink_target: Some(target),
            });
        } else if file_type.is_dir() {
            out.push(ScanEntry {
                rel,
                abs,
                kind: FileKind::Dir,
                size: None,
                mode,
                mtime,
                symlink_target: None,
            });
        } else if file_type.is_file() {
            out.push(ScanEntry {
                rel,
                abs,
                kind: FileKind::File,
                size: Some(meta.len()),
                mode,
                mtime,
                symlink_target: None,
            });
        }
    }

    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}
