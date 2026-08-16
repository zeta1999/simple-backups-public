//! Blocking PQC helpers used by the C/JNI ABI.

use anyhow::{Context, Result};
use backups_engine::{create_snapshot, SnapshotOptions};
use backups_store::Repository;
use backups_transfer::{generate_pairing_code, pair_peers, push_to_peer, PairRole, VaultManager};
use std::path::Path;

use crate::runtime::runtime;

pub fn identity_ensure(vault: &Path, password: &str) -> Result<String> {
    let mut mgr = VaultManager::new(vault);
    mgr.create_or_open(password.as_bytes())?;
    let id = mgr.get_or_create_identity()?;
    Ok(hex::encode(id.verifying_key()))
}

pub fn gen_code() -> Result<String> {
    generate_pairing_code()
}

pub fn pair_blocking(
    vault: &Path,
    password: &str,
    peer: &str,
    addr: &str,
    code: &str,
    listen: bool,
) -> Result<String> {
    let role = if listen {
        PairRole::Listener
    } else {
        PairRole::Initiator
    };
    let peer_vk = runtime().block_on(pair_peers(
        vault,
        password.as_bytes(),
        peer,
        addr,
        code,
        role,
    ))?;
    Ok(hex::encode(peer_vk))
}

pub fn push_blocking(
    repo: &Path,
    vault: &Path,
    password: &str,
    peer: &str,
    addr: &str,
    latest_only: bool,
) -> Result<String> {
    let repo = Repository::open(repo).with_context(|| format!("open repo {}", repo.display()))?;
    let stats = runtime().block_on(push_to_peer(
        &repo,
        vault,
        password.as_bytes(),
        peer,
        addr,
        latest_only,
    ))?;
    Ok(format!(
        "snapshots={} objects={} bytes={}",
        stats.snapshots, stats.objects_sent, stats.bytes_sent
    ))
}

/// Init repo if needed, snapshot `source` into it, then push to peer.
pub fn snapshot_and_push(
    repo: &Path,
    source: &Path,
    vault: &Path,
    password: &str,
    peer: &str,
    addr: &str,
    message: Option<&str>,
) -> Result<String> {
    if !repo.join("config.yaml").exists() {
        Repository::init(repo, Some("mobile".into()))?;
    }
    let repository = Repository::open(repo)?;
    let (_m, stats) = create_snapshot(
        &repository,
        source,
        &SnapshotOptions {
            message: message.map(|s| s.to_string()),
            ..Default::default()
        },
    )?;
    let push = runtime().block_on(push_to_peer(
        &repository,
        vault,
        password.as_bytes(),
        peer,
        addr,
        true,
    ))?;
    Ok(format!(
        "snap_new={} snap_reused={}; push snapshots={} objects={} bytes={}",
        stats.files_new, stats.files_reused, push.snapshots, push.objects_sent, push.bytes_sent
    ))
}

pub fn has_pinned_peer(vault: &Path, password: &str, peer: &str) -> Result<bool> {
    let mut mgr = VaultManager::new(vault);
    mgr.create_or_open(password.as_bytes())?;
    Ok(mgr.get_pinned_peer(peer).is_ok())
}
