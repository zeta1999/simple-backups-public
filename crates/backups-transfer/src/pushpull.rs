use anyhow::{bail, Context, Result};
use backups_core::{path_is_safe, validate_snapshot_id, ObjectId, SnapshotManifest};
use backups_store::Repository;
use simple_network::security::pqc::{Identity, SecureConnection};
use simple_network::transport::traits::Listener;
use std::collections::BTreeSet;
use std::path::Path;

use crate::protocol::{SnapshotOffer, WireMsg};
use crate::session::{
    bind_server, client_hello, connect_as_client, connect_with_identity, expect_ack,
    handshake_server, recv_msg, send_msg, server_hello,
};

#[derive(Debug, Default, Clone)]
pub struct TransferStats {
    pub snapshots: usize,
    pub objects_sent: usize,
    pub objects_received: usize,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

pub async fn push_to_peer(
    repo: &Repository,
    vault_path: &Path,
    password: &[u8],
    peer: &str,
    addr: &str,
    latest_only: bool,
) -> Result<TransferStats> {
    let mut conn = connect_as_client(vault_path, password, peer, addr).await?;
    let stats = push_over(&mut conn, repo, latest_only).await?;
    let _ = conn.close().await;
    Ok(stats)
}

pub async fn pull_from_peer(
    repo: &Repository,
    vault_path: &Path,
    password: &[u8],
    peer: &str,
    addr: &str,
    snapshot: &str,
) -> Result<TransferStats> {
    let mut conn = connect_as_client(vault_path, password, peer, addr).await?;
    let stats = pull_over(&mut conn, repo, snapshot).await?;
    let _ = conn.close().await;
    Ok(stats)
}

/// Serve replication sessions (push/pull). If `once`, exit after one session.
pub async fn serve_peer(
    repo: &Repository,
    vault_path: &Path,
    password: &[u8],
    peer: &str,
    addr: &str,
    once: bool,
) -> Result<()> {
    let (mut listener, id0, peer_vk0) = bind_server(vault_path, password, peer, addr).await?;
    let _ = (id0, peer_vk0);
    println!("listening on {addr} for peer '{peer}'");
    loop {
        let conn = listener.accept().await?;
        let mut mgr = crate::vault::VaultManager::new(vault_path);
        mgr.create_or_open(password)?;
        let id = mgr.get_or_create_identity()?;
        let peer_vk = mgr.get_pinned_peer(peer)?;
        let mut secure = handshake_server(conn, id, peer_vk).await?;
        match handle_session(&mut secure, repo).await {
            Ok(stats) => println!(
                "session ok: snapshots={} objects_rx={} objects_tx={} bytes_rx={} bytes_tx={}",
                stats.snapshots,
                stats.objects_received,
                stats.objects_sent,
                stats.bytes_received,
                stats.bytes_sent
            ),
            Err(e) => eprintln!("session error: {e:#}"),
        }
        let _ = secure.close().await;
        if once {
            break;
        }
    }
    Ok(())
}

pub async fn push_with_identity(
    repo: &Repository,
    addr: &str,
    id: Identity,
    peer_vk: Vec<u8>,
) -> Result<TransferStats> {
    push_with_identity_opts(repo, addr, id, peer_vk, false).await
}

pub async fn push_with_identity_opts(
    repo: &Repository,
    addr: &str,
    id: Identity,
    peer_vk: Vec<u8>,
    latest_only: bool,
) -> Result<TransferStats> {
    let mut conn = connect_with_identity(addr, id, peer_vk).await?;
    let stats = push_over(&mut conn, repo, latest_only).await?;
    let _ = conn.close().await;
    Ok(stats)
}

pub async fn pull_with_identity(
    repo: &Repository,
    addr: &str,
    id: Identity,
    peer_vk: Vec<u8>,
    snapshot: &str,
) -> Result<TransferStats> {
    let mut conn = connect_with_identity(addr, id, peer_vk).await?;
    let stats = pull_over(&mut conn, repo, snapshot).await?;
    let _ = conn.close().await;
    Ok(stats)
}

pub async fn serve_with_identity_once(
    repo: &Repository,
    listener: &mut Box<dyn Listener>,
    id: Identity,
    peer_vk: Vec<u8>,
) -> Result<TransferStats> {
    let conn = listener.accept().await?;
    let mut secure = handshake_server(conn, id, peer_vk).await?;
    let stats = handle_session(&mut secure, repo).await?;
    let _ = secure.close().await;
    Ok(stats)
}

async fn push_over(
    conn: &mut SecureConnection,
    repo: &Repository,
    latest_only: bool,
) -> Result<TransferStats> {
    client_hello(conn).await?;
    let mut stats = TransferStats::default();

    let ids = if latest_only {
        match repo.latest_id()? {
            Some(id) => vec![id],
            None => bail!("no local snapshots to push"),
        }
    } else {
        let ids = repo.list_snapshots()?;
        if ids.is_empty() {
            bail!("no local snapshots to push");
        }
        ids
    };

    let mut offers = Vec::new();
    let mut manifests = Vec::new();
    for id in &ids {
        let m = repo.load_snapshot(id)?;
        let object_ids = Repository::objects_in_manifest(&m)
            .into_iter()
            .map(|o| o.to_string())
            .collect();
        offers.push(SnapshotOffer {
            id: m.id.clone(),
            content_hash: m.content_hash.clone(),
            object_ids,
        });
        manifests.push(m);
    }

    send_msg(conn, &WireMsg::PushBegin { snapshots: offers }).await?;

    match recv_msg(conn).await? {
        WireMsg::WantObjects { ids: want } => {
            for id_hex in want {
                let oid = ObjectId::from_hex(&id_hex)?;
                let data = repo.read_object(&oid)?;
                send_object(conn, &oid, &data).await?;
                stats.objects_sent += 1;
                stats.bytes_sent += data.len() as u64;
            }
        }
        other => bail!("expected WantObjects, got {other:?}"),
    }

    for m in &manifests {
        let json = serde_json::to_string(m)?;
        send_msg(
            conn,
            &WireMsg::PushManifest {
                id: m.id.clone(),
                json,
            },
        )
        .await?;
        expect_ack(conn).await?;
        stats.snapshots += 1;
    }

    send_msg(conn, &WireMsg::PushEnd).await?;
    expect_ack(conn).await?;
    Ok(stats)
}

async fn pull_over(
    conn: &mut SecureConnection,
    repo: &Repository,
    snapshot: &str,
) -> Result<TransferStats> {
    client_hello(conn).await?;
    let mut stats = TransferStats::default();

    send_msg(
        conn,
        &WireMsg::PullBegin {
            snapshot: snapshot.to_string(),
        },
    )
    .await?;

    let (id, json, object_ids) = match recv_msg(conn).await? {
        WireMsg::PullManifest {
            id,
            json,
            object_ids,
        } => (id, json, object_ids),
        other => bail!("expected PullManifest, got {other:?}"),
    };

    let want: Vec<String> = object_ids
        .into_iter()
        .filter(|h| {
            ObjectId::from_hex(h)
                .map(|oid| !repo.has_object(&oid))
                .unwrap_or(true)
        })
        .collect();

    send_msg(conn, &WireMsg::WantObjects { ids: want.clone() }).await?;
    for _ in &want {
        let (oid, data) = recv_object(conn).await?;
        let stored = repo.put_bytes(&data)?;
        if stored != oid {
            bail!("object hash mismatch: expected {oid}, got {stored}");
        }
        stats.objects_received += 1;
        stats.bytes_received += data.len() as u64;
    }

    let manifest: SnapshotManifest =
        serde_json::from_str(&json).context("parse pulled manifest")?;
    if manifest.id != id {
        bail!("manifest id mismatch");
    }
    manifest.verify_content_hash()?;
    for oid in Repository::objects_in_manifest(&manifest) {
        if !repo.has_object(&oid) {
            bail!("missing object {oid} after pull");
        }
    }
    repo.write_snapshot(&manifest)?;
    stats.snapshots = 1;

    send_msg(conn, &WireMsg::PullEnd).await?;
    expect_ack(conn).await?;
    Ok(stats)
}

async fn handle_session(conn: &mut SecureConnection, repo: &Repository) -> Result<TransferStats> {
    server_hello(conn).await?;
    match recv_msg(conn).await? {
        WireMsg::PushBegin { snapshots } => handle_push(conn, repo, snapshots).await,
        WireMsg::PullBegin { snapshot } => handle_pull(conn, repo, &snapshot).await,
        other => {
            send_msg(
                conn,
                &WireMsg::Error {
                    message: format!("unexpected {other:?}"),
                },
            )
            .await?;
            bail!("unexpected first request: {other:?}")
        }
    }
}

async fn handle_push(
    conn: &mut SecureConnection,
    repo: &Repository,
    offers: Vec<SnapshotOffer>,
) -> Result<TransferStats> {
    let mut stats = TransferStats::default();

    let mut want: BTreeSet<String> = BTreeSet::new();
    for offer in &offers {
        let already = repo
            .load_snapshot(&offer.id)
            .ok()
            .is_some_and(|m| m.content_hash == offer.content_hash);
        if already {
            continue;
        }
        for oid in &offer.object_ids {
            if ObjectId::from_hex(oid)
                .map(|id| !repo.has_object(&id))
                .unwrap_or(true)
            {
                want.insert(oid.clone());
            }
        }
    }

    send_msg(
        conn,
        &WireMsg::WantObjects {
            ids: want.into_iter().collect(),
        },
    )
    .await?;

    // Receive objects until we get the first PushManifest / or keep reading ObjectMeta.
    loop {
        match recv_msg(conn).await? {
            WireMsg::ObjectMeta { id, len } => {
                let oid = ObjectId::from_hex(&id)?;
                let data = recv_object_body(conn, &oid, len).await?;
                let stored = repo.put_bytes(&data)?;
                if stored != oid {
                    bail!("object hash mismatch: expected {oid}, got {stored}");
                }
                stats.objects_received += 1;
                stats.bytes_received += data.len() as u64;
            }
            WireMsg::PushManifest { id, json } => {
                validate_snapshot_id(&id)?;
                let manifest: SnapshotManifest =
                    serde_json::from_str(&json).context("parse pushed manifest")?;
                if manifest.id != id {
                    bail!("manifest id mismatch");
                }
                validate_snapshot_id(&manifest.id)?;
                for rel in manifest.files.keys() {
                    if !path_is_safe(rel) {
                        bail!("refusing unsafe path in pushed manifest: {rel}");
                    }
                }
                manifest.verify_content_hash()?;
                for oid in Repository::objects_in_manifest(&manifest) {
                    if !repo.has_object(&oid) {
                        bail!("missing object {oid} for snapshot {id}");
                    }
                }
                repo.write_snapshot(&manifest)?;
                send_msg(conn, &WireMsg::Ack).await?;
                stats.snapshots += 1;
            }
            WireMsg::PushEnd => {
                send_msg(conn, &WireMsg::Ack).await?;
                break;
            }
            other => bail!("unexpected during push: {other:?}"),
        }
    }
    Ok(stats)
}

async fn handle_pull(
    conn: &mut SecureConnection,
    repo: &Repository,
    snapshot: &str,
) -> Result<TransferStats> {
    let mut stats = TransferStats::default();
    let manifest = repo.load_snapshot(snapshot)?;
    let object_ids: Vec<String> = Repository::objects_in_manifest(&manifest)
        .into_iter()
        .map(|o| o.to_string())
        .collect();
    let allowed: BTreeSet<String> = object_ids.iter().cloned().collect();
    let json = serde_json::to_string(&manifest)?;
    send_msg(
        conn,
        &WireMsg::PullManifest {
            id: manifest.id.clone(),
            json,
            object_ids,
        },
    )
    .await?;

    let want = match recv_msg(conn).await? {
        WireMsg::WantObjects { ids } => ids,
        other => bail!("expected WantObjects, got {other:?}"),
    };
    for id_hex in want {
        if !allowed.contains(&id_hex) {
            bail!("pull requested object not in advertised snapshot: {id_hex}");
        }
        let oid = ObjectId::from_hex(&id_hex)?;
        let data = repo.read_object(&oid)?;
        send_object(conn, &oid, &data).await?;
        stats.objects_sent += 1;
        stats.bytes_sent += data.len() as u64;
    }

    match recv_msg(conn).await? {
        WireMsg::PullEnd => send_msg(conn, &WireMsg::Ack).await?,
        other => bail!("expected PullEnd, got {other:?}"),
    }
    stats.snapshots = 1;
    Ok(stats)
}

/// Stay under simple-network TCP frame cap (10 MiB).
const MAX_CHUNK: usize = 4 * 1024 * 1024;
/// Reject claimed object sizes that would OOM a receiver.
const MAX_OBJECT_LEN: u64 = 256 * 1024 * 1024;

async fn send_object(conn: &mut SecureConnection, id: &ObjectId, data: &[u8]) -> Result<()> {
    send_msg(
        conn,
        &WireMsg::ObjectMeta {
            id: id.to_string(),
            len: data.len() as u64,
        },
    )
    .await?;
    for chunk in data.chunks(MAX_CHUNK) {
        conn.send(chunk).await?;
    }
    Ok(())
}

async fn recv_object_body(conn: &mut SecureConnection, id: &ObjectId, len: u64) -> Result<Vec<u8>> {
    if len > MAX_OBJECT_LEN {
        bail!("object {id} too large: {len} bytes (max {MAX_OBJECT_LEN})");
    }
    if len == 0 {
        return Ok(Vec::new());
    }
    let mut data = Vec::with_capacity(len as usize);
    while (data.len() as u64) < len {
        let chunk = conn.recv().await?;
        data.extend_from_slice(&chunk);
    }
    if data.len() as u64 != len {
        bail!(
            "object {} length mismatch: header {len}, got {}",
            id,
            data.len()
        );
    }
    let got = backups_core::hash_bytes(&data);
    if &got != id {
        bail!("object content hash mismatch: expected {id}, got {got}");
    }
    Ok(data)
}

async fn recv_object(conn: &mut SecureConnection) -> Result<(ObjectId, Vec<u8>)> {
    let (id, len) = match recv_msg(conn).await? {
        WireMsg::ObjectMeta { id, len } => (ObjectId::from_hex(id)?, len),
        other => bail!("expected ObjectMeta, got {other:?}"),
    };
    let data = recv_object_body(conn, &id, len).await?;
    Ok((id, data))
}
