use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

/// Content-addressed object id (lowercase SHA-256 hex).
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectId(String);

impl ObjectId {
    pub fn from_hex(hex: impl Into<String>) -> Result<Self> {
        let s = hex.into();
        if s.len() != 64 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!("invalid object id (want 64 hex chars)");
        }
        Ok(Self(s.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Shard path: `objects/<ab>/<rest>`.
    pub fn object_path(&self, repo: &Path) -> PathBuf {
        let (a, b) = self.0.split_at(2);
        repo.join("objects").join(a).join(b)
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectId({})", &self.0[..12])
    }
}

pub fn hash_bytes(data: &[u8]) -> ObjectId {
    let mut h = Sha256::new();
    h.update(data);
    ObjectId(hex::encode(h.finalize()))
}

pub fn hash_file(path: &Path) -> Result<ObjectId> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(ObjectId(hex::encode(h.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_sha256() {
        let id = hash_bytes(b"abc");
        assert_eq!(
            id.as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
