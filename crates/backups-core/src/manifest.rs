use crate::object_id::{hash_bytes, ObjectId};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT_VERSION: &str = "1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    File,
    Dir,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub kind: FileKind,
    /// Content object for regular files; absent for dirs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub format_version: String,
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Previous snapshot id when this is incremental.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub files: BTreeMap<String, FileEntry>,
    /// SHA-256 of canonical JSON with `content_hash` cleared.
    pub content_hash: String,
}

impl SnapshotManifest {
    pub fn new(
        id: String,
        source: String,
        message: Option<String>,
        parent: Option<String>,
        files: BTreeMap<String, FileEntry>,
    ) -> Result<Self> {
        let mut m = Self {
            format_version: FORMAT_VERSION.to_string(),
            id,
            created_at: Utc::now(),
            source,
            message,
            parent,
            files,
            content_hash: String::new(),
        };
        m.content_hash = m.compute_content_hash()?;
        Ok(m)
    }

    pub fn compute_content_hash(&self) -> Result<String> {
        let mut clone = self.clone();
        clone.content_hash.clear();
        let bytes = serde_json::to_vec(&clone).context("serialize manifest for hash")?;
        Ok(hash_bytes(&bytes).as_str().to_string())
    }

    pub fn verify_content_hash(&self) -> Result<()> {
        let expect = self.compute_content_hash()?;
        if expect != self.content_hash {
            anyhow::bail!(
                "manifest content_hash mismatch: got {}, expected {}",
                self.content_hash,
                expect
            );
        }
        Ok(())
    }

    pub fn file_count(&self) -> usize {
        self.files
            .values()
            .filter(|e| e.kind == FileKind::File)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_stable() {
        let mut files = BTreeMap::new();
        files.insert(
            "a.txt".into(),
            FileEntry {
                path: "a.txt".into(),
                kind: FileKind::File,
                object: Some(
                    ObjectId::from_hex(
                        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                    )
                    .unwrap(),
                ),
                size: Some(3),
                mode: Some(0o644),
                mtime: None,
                symlink_target: None,
            },
        );
        let m = SnapshotManifest::new(
            "test-1".into(),
            "/tmp/src".into(),
            Some("hi".into()),
            None,
            files,
        )
        .unwrap();
        m.verify_content_hash().unwrap();
    }
}
