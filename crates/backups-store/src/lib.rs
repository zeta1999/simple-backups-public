//! Local repository: objects, snapshot manifests, refs.

use anyhow::{bail, Context, Result};
use backups_core::{hash_bytes, validate_snapshot_id, ObjectId, SnapshotManifest};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Path, PathBuf};

pub const REPO_MARKER: &str = "simple-backups-repo";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub version: u32,
    #[serde(default)]
    pub description: Option<String>,
}

pub struct Repository {
    root: PathBuf,
}

impl Repository {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn init(root: &Path, description: Option<String>) -> Result<Self> {
        if root.join("config.yaml").exists() {
            bail!("repository already exists at {}", root.display());
        }
        fs::create_dir_all(root.join("objects"))?;
        fs::create_dir_all(root.join("snapshots"))?;
        fs::create_dir_all(root.join("refs"))?;
        fs::create_dir_all(root.join("state"))?;
        let cfg = RepoConfig {
            version: 1,
            description,
        };
        let yaml = serde_yaml::to_string(&cfg)?;
        atomic_write(&root.join("config.yaml"), yaml.as_bytes())?;
        // Marker for humans / tools.
        atomic_write(&root.join("REPO"), format!("{REPO_MARKER}\n").as_bytes())?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn open(root: &Path) -> Result<Self> {
        let cfg = root.join("config.yaml");
        if !cfg.exists() {
            bail!("not a simple-backups repository: {}", root.display());
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn put_bytes(&self, data: &[u8]) -> Result<ObjectId> {
        let id = hash_bytes(data);
        let path = id.object_path(&self.root);
        if path.exists() {
            return Ok(id);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(&path, data)?;
        Ok(id)
    }

    pub fn put_file(&self, src: &Path) -> Result<ObjectId> {
        let id = backups_core::hash_file(src)?;
        let path = id.object_path(&self.root);
        if path.exists() {
            return Ok(id);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Stream copy into temp then rename.
        let dir = path.parent().unwrap();
        let mut tmp = tempfile::NamedTempFile::new_in(dir)
            .with_context(|| format!("temp in {}", dir.display()))?;
        let mut input = File::open(src).with_context(|| format!("open {}", src.display()))?;
        copy(&mut input, tmp.as_file_mut())?;
        tmp.as_file().sync_all()?;
        tmp.persist(&path)
            .with_context(|| format!("persist object {}", path.display()))?;
        Ok(id)
    }

    pub fn has_object(&self, id: &ObjectId) -> bool {
        id.object_path(&self.root).exists()
    }

    pub fn open_object(&self, id: &ObjectId) -> Result<File> {
        File::open(id.object_path(&self.root)).with_context(|| format!("missing object {id}"))
    }

    pub fn read_object(&self, id: &ObjectId) -> Result<Vec<u8>> {
        fs::read(id.object_path(&self.root)).with_context(|| format!("read object {id}"))
    }

    /// Collect unique object ids referenced by a manifest.
    pub fn objects_in_manifest(manifest: &SnapshotManifest) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = manifest
            .files
            .values()
            .filter_map(|e| e.object.clone())
            .collect();
        ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        ids.dedup();
        ids
    }

    pub fn write_snapshot(&self, manifest: &SnapshotManifest) -> Result<PathBuf> {
        validate_snapshot_id(&manifest.id)?;
        manifest.verify_content_hash()?;
        let path = self
            .root
            .join("snapshots")
            .join(format!("{}.json", manifest.id));
        let bytes = serde_json::to_vec_pretty(manifest)?;
        atomic_write(&path, &bytes)?;
        atomic_write(
            &self.root.join("refs").join("latest"),
            format!("{}\n", manifest.id).as_bytes(),
        )?;
        Ok(path)
    }

    pub fn load_snapshot(&self, id: &str) -> Result<SnapshotManifest> {
        validate_snapshot_id(id)?;
        let id = if id == "latest" {
            self.latest_id()?
                .ok_or_else(|| anyhow::anyhow!("no snapshots yet"))?
        } else {
            id.to_string()
        };
        validate_snapshot_id(&id)?;
        let path = self.root.join("snapshots").join(format!("{id}.json"));
        let text = fs::read_to_string(&path)
            .with_context(|| format!("load snapshot {}", path.display()))?;
        let m: SnapshotManifest = serde_json::from_str(&text)?;
        m.verify_content_hash()?;
        Ok(m)
    }

    pub fn latest_id(&self) -> Result<Option<String>> {
        let p = self.root.join("refs").join("latest");
        if !p.exists() {
            return Ok(None);
        }
        let s = fs::read_to_string(p)?;
        Ok(Some(s.trim().to_string()))
    }

    pub fn list_snapshots(&self) -> Result<Vec<String>> {
        let dir = self.root.join("snapshots");
        let mut ids = Vec::new();
        if !dir.exists() {
            return Ok(ids);
        }
        for ent in fs::read_dir(dir)? {
            let ent = ent?;
            let name = ent.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".json") {
                ids.push(id.to_string());
            }
        }
        ids.sort();
        Ok(ids)
    }
}

fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(data)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path)
        .with_context(|| format!("atomic write {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_dedupes() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), None).unwrap();
        let a = repo.put_bytes(b"hello").unwrap();
        let b = repo.put_bytes(b"hello").unwrap();
        assert_eq!(a, b);
        assert!(repo.has_object(&a));
    }

    #[test]
    fn snapshot_id_cannot_escape_snapshots_dir() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path(), None).unwrap();
        assert!(repo.load_snapshot("../../evil").is_err());
        assert!(repo.load_snapshot("a/b").is_err());
    }
}
