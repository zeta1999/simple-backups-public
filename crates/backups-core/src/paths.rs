use anyhow::{bail, Result};
use std::path::{Component, Path};

/// Normalize a relative path to `/`-separated form without `..` or absolute roots.
pub fn normalize_rel_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for c in path.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => bail!("path escapes root: {}", path.display()),
            Component::RootDir | Component::Prefix(_) => {
                bail!("absolute paths are not allowed: {}", path.display())
            }
        }
    }
    if parts.is_empty() {
        bail!("empty relative path");
    }
    Ok(parts.join("/"))
}

/// True if `rel` is safe to restore under `target` (no escape).
pub fn path_is_safe(rel: &str) -> bool {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\0') {
        return false;
    }
    !rel.split('/').any(|p| p == "..")
}

/// Snapshot ids used as filenames under `snapshots/`. Reject path separators
/// and `..` so `join("{id}.json")` cannot escape the snapshots directory.
pub fn validate_snapshot_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 128 {
        bail!("invalid snapshot id");
    }
    if id == "latest" {
        return Ok(());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        bail!("invalid snapshot id: {id}");
    }
    if id.contains("..") {
        bail!("invalid snapshot id: {id}");
    }
    Ok(())
}

/// True if a symlink target is relative and does not escape via `..`.
pub fn symlink_target_is_safe(target: &str) -> bool {
    if target.is_empty() || target.contains('\0') {
        return false;
    }
    let path = Path::new(target);
    if path.is_absolute() {
        return false;
    }
    !path.components().any(|c| matches!(c, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rejects_parent() {
        assert!(normalize_rel_path(Path::new("a/../b")).is_err());
        assert!(!path_is_safe("a/../b"));
    }

    #[test]
    fn normalizes() {
        let p = PathBuf::from("foo").join("bar");
        assert_eq!(normalize_rel_path(&p).unwrap(), "foo/bar");
    }

    #[test]
    fn snapshot_id_rejects_escape() {
        assert!(validate_snapshot_id("../../evil").is_err());
        assert!(validate_snapshot_id("a/b").is_err());
        assert!(validate_snapshot_id("20260816-120000").is_ok());
        assert!(validate_snapshot_id("latest").is_ok());
    }

    #[test]
    fn symlink_target_rejects_absolute_and_parent() {
        assert!(!symlink_target_is_safe("/tmp"));
        assert!(!symlink_target_is_safe("../outside"));
        assert!(symlink_target_is_safe("rel/link"));
    }
}
